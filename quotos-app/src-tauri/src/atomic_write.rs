//! One durable file write, shared by every store that owns a JSON file on
//! disk (`persistence.rs`'s tracked list, `statusline.rs`'s settings and
//! feed): written to a temp file in the same directory, `fsync`'d, then
//! renamed into place — an atomic same-filesystem replace, so a crash or
//! kill mid-write can never leave a half-written file at the final path,
//! and by the time this returns `Ok` the bytes are already on disk.

use std::fs;
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

/// Distinguishes concurrent writers aiming at the same target: the counter
/// separates threads within one process, the pid in the temp name separates
/// the ingest helper's processes — Claude Code spawns one per statusline
/// render, so two live sessions on the same account overlap routinely. A
/// shared temp name let one writer's `File::create` truncate another's file
/// between its write and its rename (R4).
static TMP_SEQ: AtomicU64 = AtomicU64::new(0);

pub(crate) fn write_string(path: &Path, content: &str) -> Result<(), String> {
    let parent = path.parent().ok_or("path has no parent directory")?;
    let file_name = path
        .file_name()
        .ok_or("path has no file name")?
        .to_string_lossy();
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    // A sibling in the same directory (required for the rename below to be
    // an atomic same-filesystem replace), under a name no other writer can
    // share — see TMP_SEQ above.
    let tmp_path = parent.join(format!(
        "{file_name}.tmp.{}-{}",
        std::process::id(),
        TMP_SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    let written = (|| {
        let mut f = fs::File::create(&tmp_path).map_err(|e| e.to_string())?;
        f.write_all(content.as_bytes()).map_err(|e| e.to_string())?;
        f.sync_all().map_err(|e| e.to_string())?;
        fs::rename(&tmp_path, path).map_err(|e| e.to_string())
    })();
    if written.is_err() {
        // The unique name is this writer's alone, so a failed write's
        // leftover is ours to remove — otherwise every failure strands one.
        let _ = fs::remove_file(&tmp_path);
    }
    written
}
