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
    /// `None` falls back to the provider-derived label.
    pub label: Option<String>,
    /// Pinning is per limit window, matching `LimitWindowEntity.id`, not
    /// per subscription, so an account can pin one window and leave others.
    #[serde(rename = "pinnedWindowIds", default)]
    pub pinned_window_ids: Vec<String>,
    /// A legacy field, migrated on load and shed on the next save. See
    /// "Persistence" in docs/architecture.md.
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
    /// A file that fails to parse is moved aside to `tracked.json.corrupt`
    /// first, best effort. See "Persistence" in docs/architecture.md.
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

    /// Once this returns `Ok`, disk and memory have updated together under
    /// one lock. See "Persistence" in docs/architecture.md for why two
    /// overlapping saves need that lock held across the whole write.
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
