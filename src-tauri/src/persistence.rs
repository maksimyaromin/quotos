//! The tracked-subscriptions list, meaning membership, custom names, and
//! pins, lives in a plain JSON file rather than the WKWebView's
//! `localStorage`. A `localStorage` write returns as soon as the in-memory
//! page state updates, and WebKit flushes its backing store to disk on its
//! own schedule. Whether an abrupt process exit, such as choosing "Quit
//! Quotos", can race that flush was never conclusively pinned down, but a
//! file written synchronously and `fsync`'d before `save` returns has no
//! dependency on that flush timing at all. Once `save` returns `Ok`, the
//! data is durable regardless of what happens immediately after.
//!
//! A JSON file instead of SQLite for the same reason as every other data
//! store this small in this app: the tracked list is a handful of
//! accounts, so a plain file is simpler to read, review, and hand-edit than
//! a database, and it serves every query pattern this data needs.

use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::atomic_write;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct TrackedAccount {
    pub id: String,
    pub provider: String,
    pub config_dir: String,
    /// The user chooses this name for the account. `None` falls back to the
    /// provider-derived label.
    pub label: Option<String>,
    /// The persisted set of pinned window ids, matching
    /// `LimitWindowEntity.id` on the TypeScript side. Pinning is a property
    /// of the limit window, not the subscription as a whole, so an account
    /// can pin one window and leave its others unpinned. Serialized as
    /// `pinnedWindowIds` to match the TypeScript `Subscription` and
    /// `TrackedAccount` fields' own camelCase spelling.
    #[serde(rename = "pinnedWindowIds", default)]
    pub pinned_window_ids: Vec<String>,
    /// Present only when this record was loaded from a file that predates
    /// `pinnedWindowIds` and wrote `pinned: true` or `pinned: false`
    /// instead. The frontend reads this field to detect and migrate such a
    /// record, through `useSubscriptions.ts`'s `pendingPinMigrationRef`, and
    /// never sends it back on `save_tracked`. `skip_serializing_if` means a
    /// record sheds this field from disk the moment it is next saved, so it
    /// appears exactly once, on the one load that still has the old shape
    /// to read.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pinned: Option<bool>,
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
    /// Loads the tracked list from `path`. A missing or corrupt file starts
    /// empty instead of failing app startup.
    ///
    /// A file that exists but fails to parse is moved aside to
    /// `tracked.json.corrupt` first. This is best effort: starting empty
    /// never depends on the move succeeding. `save` atomically overwrites
    /// `tracked.json`, so leaving the unread bytes in place would let the
    /// very next save destroy the only copy of whatever the file held. The
    /// backup keeps exactly one slot, so a second corruption overwrites the
    /// first. The goal is to keep recovery possible, not to keep an
    /// archive.
    pub fn load(path: PathBuf) -> Self {
        let tracked = match fs::read_to_string(&path) {
            Err(_) => Vec::new(),
            Ok(raw) => match serde_json::from_str::<PersistedShape>(&raw) {
                Ok(shape) => shape.tracked,
                Err(_) => {
                    let _ = fs::rename(&path, path.with_extension("json.corrupt"));
                    Vec::new()
                }
            },
        };
        Self {
            path,
            tracked: Mutex::new(tracked),
        }
    }

    pub fn list(&self) -> Vec<TrackedAccount> {
        self.tracked
            .lock()
            .expect("tracked store mutex poisoned")
            .clone()
    }

    /// Overwrites the tracked list and durably persists it through
    /// `atomic_write`, so once this returns `Ok` the data is on disk.
    ///
    /// The lock is held across the whole write, not taken only to update
    /// memory afterward. `save_tracked` is an async command the frontend
    /// fires on every membership, label, or pin change, so two saves can
    /// overlap. A writer that renamed its file last but locked the mutex
    /// first would leave disk and memory telling different stories, since
    /// the scheduler polls from memory while the next launch loads from
    /// disk. A failed write changes neither. This is a synchronous function
    /// running on a blocking thread, so the lock is never held across an
    /// await, and holding it through file I/O blocks only sibling saves.
    pub fn save(&self, tracked: Vec<TrackedAccount>) -> Result<(), String> {
        let shape = PersistedShape {
            version: 1,
            tracked: tracked.clone(),
        };
        let json = serde_json::to_string_pretty(&shape).map_err(|e| e.to_string())?;

        let mut in_memory = self.tracked.lock().expect("tracked store mutex poisoned");
        atomic_write::write_string(&self.path, &json)?;
        *in_memory = tracked;
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
            let count = COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "quotos-persistence-test-{}-{count}",
                std::process::id()
            ));
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
            pinned_window_ids: vec!["weekly_all".to_string()],
            pinned: None,
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

    #[test]
    fn a_corrupt_file_is_preserved_where_no_save_can_overwrite_it() {
        let dir = TempDir::new();
        let path = dir.path.join("tracked.json");
        let corrupt_bytes = "{ not valid json";
        fs::write(&path, corrupt_bytes).unwrap();

        let store = Store::load(path.clone());
        let backup = path.with_extension("json.corrupt");
        assert!(!path.exists(), "the unparseable file should be moved aside");
        assert_eq!(fs::read_to_string(&backup).unwrap(), corrupt_bytes);

        store.save(vec![sample("Fresh start")]).unwrap();
        assert_eq!(
            fs::read_to_string(&backup).unwrap(),
            corrupt_bytes,
            "saving must not touch the preserved backup"
        );
        assert_eq!(
            Store::load(path).list(),
            vec![sample("Fresh start")],
            "the store itself should carry on normally"
        );
    }

    #[test]
    fn a_valid_file_never_leaves_a_corrupt_backup() {
        let dir = TempDir::new();
        let path = dir.path.join("tracked.json");
        Store::load(path.clone()).save(vec![sample("X")]).unwrap();

        let reloaded = Store::load(path.clone());
        assert_eq!(reloaded.list(), vec![sample("X")]);
        assert!(!path.with_extension("json.corrupt").exists());
    }

    #[test]
    fn save_then_reload_from_a_fresh_store_survives() {
        let dir = TempDir::new();
        let path = dir.path.join("tracked.json");

        let store = Store::load(path.clone());
        store
            .save(vec![sample("Renamed Personal")])
            .expect("save should succeed");

        let reloaded = Store::load(path);
        assert_eq!(reloaded.list(), vec![sample("Renamed Personal")]);
    }

    #[test]
    fn save_creates_missing_parent_directories() {
        let dir = TempDir::new();
        let path = dir.path.join("nested").join("deeper").join("tracked.json");
        let store = Store::load(path.clone());
        store
            .save(vec![sample("X")])
            .expect("save should create parent dirs");
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
        let leftovers: Vec<String> = fs::read_dir(&dir.path)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|name| name != "tracked.json")
            .collect();
        assert!(leftovers.is_empty(), "unexpected files: {leftovers:?}");
    }

    /// Without a lock held across the whole write, two overlapping saves
    /// can truncate each other's temp file mid-write, corrupting the file
    /// on the next launch, or land on disk in the opposite order from
    /// memory, leaving the scheduler polling an account the file no longer
    /// lists. Whatever the interleaving, disk and memory must agree on one
    /// intact list afterward.
    #[test]
    fn concurrent_saves_leave_disk_and_memory_agreeing_on_one_intact_list() {
        let dir = TempDir::new();
        let path = dir.path.join("tracked.json");
        let store = std::sync::Arc::new(Store::load(path.clone()));

        let writers: Vec<_> = (0..4)
            .map(|writer| {
                let store = std::sync::Arc::clone(&store);
                std::thread::spawn(move || {
                    for round in 0..25 {
                        store
                            .save(vec![sample(&format!("writer-{writer}-round-{round}"))])
                            .expect("a concurrent save must not fail");
                    }
                })
            })
            .collect();
        for writer in writers {
            writer.join().expect("writer thread panicked");
        }

        let on_disk = Store::load(path).list();
        assert_eq!(on_disk, store.list(), "disk and memory must be one truth");
        let leftovers: Vec<String> = fs::read_dir(&dir.path)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|name| name != "tracked.json")
            .collect();
        assert!(leftovers.is_empty(), "unexpected files: {leftovers:?}");
    }

    // A file written before pinnedWindowIds existed carries pinned: true or
    // pinned: false. The Rust side must carry that field through to the
    // frontend rather than drop it silently, or useSubscriptions.ts's
    // one-shot migration has nothing to detect.
    #[test]
    fn a_legacy_pinned_file_loads_with_the_flag_intact_and_no_pinned_window_ids() {
        let dir = TempDir::new();
        let path = dir.path.join("tracked.json");
        fs::write(
            &path,
            r#"{"version":1,"tracked":[{"id":"claude:claude","provider":"claude","config_dir":"~/.claude","label":null,"pinned":true}]}"#,
        )
        .unwrap();

        let store = Store::load(path);
        let loaded = store.list();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].pinned, Some(true));
        assert!(loaded[0].pinned_window_ids.is_empty());
    }

    // The legacy field must not persist forever. Once the frontend saves
    // this record back in the new shape, which never sends pinned, it
    // drops out of the file on disk.
    #[test]
    fn saving_a_migrated_record_drops_the_legacy_pinned_field_from_disk() {
        let dir = TempDir::new();
        let path = dir.path.join("tracked.json");
        fs::write(
            &path,
            r#"{"version":1,"tracked":[{"id":"claude:claude","provider":"claude","config_dir":"~/.claude","label":null,"pinned":true}]}"#,
        )
        .unwrap();

        let store = Store::load(path.clone());
        store.save(vec![sample("Migrated")]).unwrap();

        let raw = fs::read_to_string(&path).unwrap();
        assert!(
            !raw.contains("\"pinned\""),
            "legacy pinned field should be gone: {raw}"
        );
        assert!(raw.contains("pinnedWindowIds"));
    }
}
