//! R2-5: the tracked-subscriptions list (membership, custom names, pins),
//! now owned natively instead of living in the WKWebView's `localStorage`.
//!
//! Reproduced first, on a real packaged build: seeding `localStorage` with a
//! renamed subscription and then quitting via `app.exit(0)` — exactly what
//! the tray's "Quit Quotos" already does — left the *key*
//! (`quotos.tracked.v1`) on disk in WebKit's own SQLite-backed store, but
//! not its *value*. `localStorage.setItem` returns as soon as the in-memory
//! page state is updated; WebKit flushes the backing store to disk on its
//! own schedule, and an abrupt process exit right after a write can beat
//! that flush. A plain file, written synchronously and `fsync`'d before the
//! write call returns, doesn't have that race — by the time `save` returns
//! `Ok`, the data is already durable, so even an immediate `app.exit(0)`
//! right after is safe.
//!
//! A file over SQLite for the same reason CLAUDE.md already gives for other
//! choices in this app: the tracked list is a handful of accounts, so a
//! plain JSON file is simpler to read, review, and hand-edit than a
//! database, and it survives everything SQLite would here — there's no
//! query pattern this data needs that a full-file read/write doesn't
//! already serve.

use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct TrackedAccount {
    pub id: String,
    pub provider: String,
    pub config_dir: String,
    /// User's own name for it, or `None` to use the provider-derived label.
    pub label: Option<String>,
    pub pinned: bool,
}

#[derive(Serialize, Deserialize, Default)]
struct PersistedShape {
    version: u32,
    tracked: Vec<TrackedAccount>,
}

pub struct Store {
    path: PathBuf,
    tracked: Mutex<Vec<TrackedAccount>>,
}

impl Store {
    /// Loads from `path` if it exists and parses; a missing or corrupt file
    /// starts empty rather than failing app startup — I6's "nothing tracked
    /// by default" is also the correct fallback for "we couldn't read the
    /// file", not a crash.
    pub fn load(path: PathBuf) -> Self {
        let tracked = fs::read_to_string(&path)
            .ok()
            .and_then(|raw| serde_json::from_str::<PersistedShape>(&raw).ok())
            .map(|shape| shape.tracked)
            .unwrap_or_default();
        Self { path, tracked: Mutex::new(tracked) }
    }

    pub fn list(&self) -> Vec<TrackedAccount> {
        self.tracked.lock().expect("tracked store mutex poisoned").clone()
    }

    /// Overwrites the tracked list and durably persists it: written to a
    /// temp file in the same directory, `fsync`'d, then renamed into place
    /// (an atomic replace on the same filesystem) — a crash or kill mid-write
    /// can never leave a half-written, unparseable file behind, and nothing
    /// observes a partial write via the final path.
    pub fn save(&self, tracked: Vec<TrackedAccount>) -> Result<(), String> {
        let shape = PersistedShape { version: 1, tracked: tracked.clone() };
        let json = serde_json::to_string_pretty(&shape).map_err(|e| e.to_string())?;

        let parent = self.path.parent().ok_or("tracked store path has no parent directory")?;
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;

        let tmp_path = self.path.with_extension("json.tmp");
        {
            let mut f = fs::File::create(&tmp_path).map_err(|e| e.to_string())?;
            f.write_all(json.as_bytes()).map_err(|e| e.to_string())?;
            f.sync_all().map_err(|e| e.to_string())?;
        }
        fs::rename(&tmp_path, &self.path).map_err(|e| e.to_string())?;

        *self.tracked.lock().expect("tracked store mutex poisoned") = tracked;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    struct TempDir {
        path: PathBuf,
    }

    impl TempDir {
        fn new() -> Self {
            let n = COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!("quotos-persistence-test-{}-{n}", std::process::id()));
            fs::create_dir_all(&path).expect("create temp dir");
            Self { path }
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn sample(label: &str) -> TrackedAccount {
        TrackedAccount {
            id: "claude:claude".to_string(),
            provider: "claude".to_string(),
            config_dir: "~/.claude".to_string(),
            label: Some(label.to_string()),
            pinned: true,
        }
    }

    #[test]
    fn missing_file_starts_empty() {
        let dir = TempDir::new();
        let store = Store::load(dir.path.join("tracked.json"));
        assert!(store.list().is_empty());
    }

    #[test]
    fn corrupt_file_starts_empty_instead_of_panicking() {
        let dir = TempDir::new();
        let path = dir.path.join("tracked.json");
        fs::write(&path, "{ not valid json").unwrap();
        let store = Store::load(path);
        assert!(store.list().is_empty());
    }

    /// R2-5's acceptance test at the storage-layer: what `save` wrote is
    /// exactly what a fresh `load` from the same path returns — the
    /// round-trip a restart depends on.
    #[test]
    fn save_then_reload_from_a_fresh_store_survives() {
        let dir = TempDir::new();
        let path = dir.path.join("tracked.json");

        let store = Store::load(path.clone());
        store.save(vec![sample("Renamed Personal")]).expect("save should succeed");

        let reloaded = Store::load(path);
        assert_eq!(reloaded.list(), vec![sample("Renamed Personal")]);
    }

    #[test]
    fn save_creates_missing_parent_directories() {
        let dir = TempDir::new();
        let path = dir.path.join("nested").join("deeper").join("tracked.json");
        let store = Store::load(path.clone());
        store.save(vec![sample("X")]).expect("save should create parent dirs");
        assert!(path.exists());
    }

    #[test]
    fn save_overwrites_rather_than_appends() {
        let dir = TempDir::new();
        let path = dir.path.join("tracked.json");
        let store = Store::load(path.clone());
        store.save(vec![sample("First")]).unwrap();
        store.save(vec![sample("Second")]).unwrap();

        let reloaded = Store::load(path);
        assert_eq!(reloaded.list(), vec![sample("Second")]);
    }

    #[test]
    fn no_leftover_tmp_file_after_a_successful_save() {
        let dir = TempDir::new();
        let path = dir.path.join("tracked.json");
        let store = Store::load(path.clone());
        store.save(vec![sample("X")]).unwrap();
        assert!(!path.with_extension("json.tmp").exists());
    }
}
