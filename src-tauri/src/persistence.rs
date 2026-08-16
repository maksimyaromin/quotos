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
use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::atomic_write;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct TrackedAccount {
    pub id: String,
    pub provider: String,
    pub config_dir: String,
    /// User's own name for it, or `None` to use the provider-derived label.
    pub label: Option<String>,
    /// v4: pinning moved from the subscription to the limit window — the
    /// persisted set of pinned window ids (see `LimitWindowEntity.id` on the
    /// TS side), replacing the old `pinned: bool`. Wire-renamed to match the
    /// TS `Subscription`/`TrackedAccount` field's own camelCase spelling
    /// (unlike `config_dir` above, this field has no pre-existing snake_case
    /// wire contract to preserve).
    #[serde(rename = "pinnedWindowIds", default)]
    pub pinned_window_ids: Vec<String>,
    /// v4 migration-only: present when loading a pre-v4 file (which wrote
    /// `pinned: true/false` instead of `pinnedWindowIds`) — carried through
    /// as-is so the frontend can detect and migrate it (see
    /// `useSubscriptions.ts`'s `pendingPinMigrationRef`). The frontend never
    /// sends this back on `save_tracked` (only `pinnedWindowIds`), so
    /// `skip_serializing_if` means a record sheds it from disk the moment
    /// it's next saved — the migration signal is present exactly once, on
    /// the one load that still has the old shape to read.
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
    /// Loads from `path` if it exists and parses; a missing or corrupt file
    /// starts empty rather than failing app startup — I6's "nothing tracked
    /// by default" is also the correct fallback for "we couldn't read the
    /// file", not a crash.
    ///
    /// A file that exists but doesn't parse is first moved aside to
    /// `tracked.json.corrupt` (best-effort — starting empty never depends on
    /// it succeeding): `save` atomically overwrites `tracked.json`, so
    /// leaving the unread bytes in place would let the very next save
    /// destroy the only copy of whatever the file held. One slot, latest
    /// corruption wins — the point is that recovery stays possible, not an
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

    /// Overwrites the tracked list and durably persists it (temp file +
    /// `fsync` + atomic rename — see `atomic_write`); by the time this
    /// returns `Ok`, the data is on disk.
    ///
    /// R2: the lock is held across the whole write, not taken after it —
    /// `save_tracked` is an async command the frontend fires on every
    /// membership/label/pin change, so two saves can overlap, and a writer
    /// that renamed last but locked first would leave disk and memory
    /// telling different stories (the scheduler polls from memory, the next
    /// launch loads from disk). A failed write changes neither. The lock is
    /// never held across an `.await` (this is a sync fn on a blocking
    /// thread), so holding it through file I/O blocks only sibling saves.
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
            let n = COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "quotos-persistence-test-{}-{n}",
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

    /// A corrupt file's bytes must survive the load that failed to read them:
    /// moved aside to `tracked.json.corrupt`, out of the path `save`
    /// atomically overwrites — otherwise the very next save destroys the
    /// only copy of whatever the user's file held.
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

    /// R2-5's acceptance test at the storage-layer: what `save` wrote is
    /// exactly what a fresh `load` from the same path returns — the
    /// round-trip a restart depends on.
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

    /// R2: `save` used to run its whole temp+fsync+rename on a *fixed* temp
    /// name outside the mutex (which it only took at the end, to update
    /// memory) — so two overlapping `save_tracked` commands could truncate
    /// each other's temp file mid-write (spliced JSON → `tracked.json.corrupt`
    /// on the next launch → the whole tracked list lost) or land on disk in
    /// the opposite order to memory (the scheduler then polls an account the
    /// disk says is stop-tracked). Whatever the interleaving, one invariant
    /// must hold afterwards: disk and memory agree on one intact list.
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

    // v4 migration: a pre-v4 file on disk carries `pinned: true/false`
    // instead of `pinnedWindowIds` — the Rust side must carry that flag
    // through to the frontend rather than silently dropping it (which would
    // make `useSubscriptions.ts`'s one-shot migration undetectable).
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

    // The legacy flag must not persist forever — once the frontend saves
    // this record back (in the new shape, never sending `pinned`), it drops
    // out of the file on disk.
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
