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
    pub label: Option<String>,
    #[serde(rename = "pinnedWindowIds", default)]
    pub pinned_window_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pinned: Option<bool>,
}

/// One named collection of pinned limit windows, spanning any number of
/// tracked accounts. Purely additive: a file written before pin groups
/// shipped simply loads with none.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PinGroup {
    pub id: String,
    pub name: String,
    pub collapsed: bool,
    pub order: u32,
    #[serde(default)]
    pub member_keys: Vec<String>,
}

#[derive(Serialize, Deserialize, Default)]
struct PersistedShape {
    version: u32,
    tracked: Vec<TrackedAccount>,
    #[serde(default)]
    groups: Vec<PinGroup>,
}

#[derive(Default)]
struct StoreContents {
    tracked: Vec<TrackedAccount>,
    groups: Vec<PinGroup>,
}

pub struct Store {
    path: PathBuf,
    contents: Mutex<StoreContents>,
}

impl Store {
    pub fn load(path: PathBuf) -> Self {
        let contents = match fs::read_to_string(&path) {
            Err(_) => StoreContents::default(),
            Ok(raw) => match serde_json::from_str::<PersistedShape>(&raw) {
                Ok(shape) => StoreContents {
                    tracked: shape.tracked,
                    groups: shape.groups,
                },
                Err(_) => {
                    let _ = fs::rename(&path, path.with_extension("json.corrupt"));
                    StoreContents::default()
                }
            },
        };
        Self {
            path,
            contents: Mutex::new(contents),
        }
    }

    pub fn list(&self) -> Vec<TrackedAccount> {
        self.contents
            .lock()
            .expect("tracked store mutex poisoned")
            .tracked
            .clone()
    }

    pub fn list_groups(&self) -> Vec<PinGroup> {
        self.contents
            .lock()
            .expect("tracked store mutex poisoned")
            .groups
            .clone()
    }

    pub fn save(&self, tracked: Vec<TrackedAccount>) -> Result<(), String> {
        self.write(|contents| contents.tracked = tracked)
    }

    pub fn save_groups(&self, groups: Vec<PinGroup>) -> Result<(), String> {
        self.write(|contents| contents.groups = groups)
    }

    /// Both halves of the file live under one lock, so writing either
    /// one rewrites the whole shape without losing the other.
    fn write(&self, apply: impl FnOnce(&mut StoreContents)) -> Result<(), String> {
        let mut in_memory = self.contents.lock().expect("tracked store mutex poisoned");
        let mut next = StoreContents {
            tracked: in_memory.tracked.clone(),
            groups: in_memory.groups.clone(),
        };
        apply(&mut next);
        let shape = PersistedShape {
            version: 1,
            tracked: next.tracked.clone(),
            groups: next.groups.clone(),
        };
        let json = serde_json::to_string_pretty(&shape).map_err(|e| e.to_string())?;
        atomic_write::write_string(&self.path, &json)?;
        *in_memory = next;
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
    fn a_failed_write_returns_err_and_leaves_memory_untouched() {
        let dir = TempDir::new();
        let obstruction = dir.path.join("blocked");
        fs::write(&obstruction, "not a directory").unwrap();
        let store = Store::load(obstruction.join("tracked.json"));

        let result = store.save(vec![sample("Should not persist")]);

        assert!(
            result.is_err(),
            "a parent path blocked by a file must fail to save"
        );
        assert!(
            store.list().is_empty(),
            "a failed save must not be remembered as if it had succeeded"
        );
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

    fn sample_group(name: &str) -> PinGroup {
        PinGroup {
            id: "g1".to_string(),
            name: name.to_string(),
            collapsed: true,
            order: 0,
            member_keys: vec!["claude%3Aclaude::weekly_all".to_string()],
        }
    }

    #[test]
    fn groups_round_trip_through_a_save_and_a_fresh_load() {
        let dir = TempDir::new();
        let path = dir.path.join("tracked.json");

        let store = Store::load(path.clone());
        store.save(vec![sample("Personal")]).unwrap();
        store.save_groups(vec![sample_group("Money")]).unwrap();

        let reloaded = Store::load(path);
        assert_eq!(reloaded.list_groups(), vec![sample_group("Money")]);
        assert_eq!(reloaded.list(), vec![sample("Personal")]);
    }

    #[test]
    fn saving_one_half_of_the_file_never_drops_the_other() {
        let dir = TempDir::new();
        let path = dir.path.join("tracked.json");

        let store = Store::load(path.clone());
        store.save_groups(vec![sample_group("Money")]).unwrap();
        store.save(vec![sample("Personal")]).unwrap();

        let reloaded = Store::load(path);
        assert_eq!(
            reloaded.list_groups(),
            vec![sample_group("Money")],
            "saving the tracked list must not erase the groups beside it"
        );
    }

    #[test]
    fn a_file_written_before_pin_groups_shipped_loads_with_none() {
        let dir = TempDir::new();
        let path = dir.path.join("tracked.json");
        fs::write(
            &path,
            r#"{"version":1,"tracked":[{"id":"claude:claude","provider":"claude","config_dir":"~/.claude","label":null,"pinnedWindowIds":["weekly_all"]}]}"#,
        )
        .unwrap();

        let store = Store::load(path);
        assert!(store.list_groups().is_empty());
        assert_eq!(store.list().len(), 1);
    }

    #[test]
    fn a_group_serializes_its_member_keys_in_the_frontend_spelling() {
        let dir = TempDir::new();
        let path = dir.path.join("tracked.json");
        let store = Store::load(path.clone());
        store.save_groups(vec![sample_group("Money")]).unwrap();

        let raw = fs::read_to_string(&path).unwrap();
        assert!(raw.contains("memberKeys"), "unexpected shape: {raw}");
        assert!(!raw.contains("member_keys"));
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
