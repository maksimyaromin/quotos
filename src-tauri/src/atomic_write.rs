//! A write here never leaves a half-written file at the final path after a
//! crash or a kill, and once it returns `Ok` the bytes are on disk.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// Claude Code spawns one ingest helper process per statusline render, and
/// two live sessions on the same account routinely overlap, so a shared temp
/// name would let one writer's `File::create` truncate another's file
/// between its write and its rename. The pid separates processes; this
/// counter separates concurrent threads within one process.
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
        let _ = fs::remove_file(&temp_path);
    }
    result
}

/// A sibling in the same directory: `rename` is only atomic within one
/// filesystem, and a temp file elsewhere would cross a mount boundary.
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
