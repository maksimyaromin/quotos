//! Writes a file durably: to a temp file in the same directory, fsync'd,
//! then renamed into place as an atomic same-filesystem replace. A crash or
//! kill mid-write can never leave a half-written file at the final path, and
//! once this returns `Ok` the bytes are already on disk. `persistence.rs`'s
//! tracked list and `statusline.rs`'s settings and feed both write through
//! here.

use std::fs;
use std::io::Write;
use std::path::Path;
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
    // The temp file must be a sibling in the same directory for the rename
    // below to be an atomic same-filesystem replace. TEMP_FILE_SEQUENCE
    // keeps its name unique to this writer.
    let temp_path = parent.join(format!(
        "{file_name}.tmp.{}-{}",
        std::process::id(),
        TEMP_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    let written = (|| {
        let mut file = fs::File::create(&temp_path).map_err(|e| e.to_string())?;
        file.write_all(content.as_bytes())
            .map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
        fs::rename(&temp_path, path).map_err(|e| e.to_string())
    })();
    if written.is_err() {
        // The unique name belongs to this writer alone, so any leftover
        // from a failed write is safe to remove without racing another
        // writer.
        let _ = fs::remove_file(&temp_path);
    }
    written
}
