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
        assert!(!raw.contains("\"pinned\""), "legacy pinned field should be gone: {raw}");
        assert!(raw.contains("pinnedWindowIds"));
    }
}
