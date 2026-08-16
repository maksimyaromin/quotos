//! Writes a file durably: to a temp file in the same directory, fsync'd,
//! then renamed into place as an atomic same-filesystem replace. A crash or
//! kill mid-write can never leave a half-written file at the final path, and
//! once this returns `Ok` the bytes are already on disk. `persistence.rs`'s
//! tracked list and `statusline.rs`'s settings and feed both write through
//! here.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// The pid in the temp file name separates concurrent processes. Claude
/// Code spawns one ingest helper process per statusline render, and two
/// live sessions on the same account routinely overlap. This counter
/// further separates concurrent threads within one process. Without both, a
/// shared temp name lets one writer's `File::create` truncate another
/// writer's file between its write and its rename.
static TEMP_FILE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub(crate) fn write_string(path: &Path, content: &str) -> Result<(), String> {
    let parent = path.parent().ok_or("path has no parent directory")?;
    let file_name = path
        .file_name()
        .ok_or("path has no file name")?
        .to_string_lossy();
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;

    let temp_path = unique_temp_path(parent, &file_name);
    let result = write_then_rename(&temp_path, path, content);
    if result.is_err() {
        // The unique name belongs to this writer alone, so any leftover
        // from a failed write is safe to remove without racing another
        // writer.
        let _ = fs::remove_file(&temp_path);
    }
    result
}

/// A same-directory sibling name, required for the rename in
/// `write_then_rename` to be an atomic same-filesystem replace, made unique
/// against every other concurrent writer by `TEMP_FILE_SEQUENCE`.
fn unique_temp_path(parent: &Path, file_name: &str) -> PathBuf {
    parent.join(format!(
        "{file_name}.tmp.{}-{}",
        std::process::id(),
        TEMP_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ))
}

fn write_then_rename(temp_path: &Path, path: &Path, content: &str) -> Result<(), String> {
    let mut file = fs::File::create(temp_path).map_err(|e| e.to_string())?;
    file.write_all(content.as_bytes())
        .map_err(|e| e.to_string())?;
    file.sync_all().map_err(|e| e.to_string())?;
    fs::rename(temp_path, path).map_err(|e| e.to_string())
}
