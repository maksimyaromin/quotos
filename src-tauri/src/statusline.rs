use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::atomic_write::write_string as atomic_write_string;

pub const INGEST_FLAG: &str = "--quotos-statusline-ingest";

const MAX_STDIN_BYTES: u64 = 4 * 1024 * 1024;

#[derive(Serialize, Clone, Debug)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum StatuslineError {
    ParseFailed { message: String },
    ReadFailed { message: String },
    WriteFailed { message: String },
    HelperInstallFailed { message: String },
    Conflict { existing_command: String },
}

impl StatuslineError {
    fn write(message: impl Into<String>) -> Self {
        Self::WriteFailed {
            message: message.into(),
        }
    }
    fn read(message: impl Into<String>) -> Self {
        Self::ReadFailed {
            message: message.into(),
        }
    }
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum IntegrationStatus {
    NotInstalled,
    Installed,
    Conflict { existing_command: String },
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct InstallOutcome {
    pub replaced_existing: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct StatuslineWindowDto {
    pub used_percentage: f64,
    pub resets_at: Option<i64>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct StatuslineRateLimitsDto {
    pub five_hour: Option<StatuslineWindowDto>,
    pub seven_day: Option<StatuslineWindowDto>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct StatuslineFeedDto {
    pub written_at: String,
    pub rate_limits: StatuslineRateLimitsDto,
}

#[derive(Serialize, Deserialize, Default)]
struct BackupRecord {
    backed_up_at: String,
    settings_backup_path: String,
    previous_status_line: Option<serde_json::Value>,
}

fn slug_for(tag: &str) -> String {
    let digest = Sha256::digest(tag.as_bytes());
    digest.iter().take(8).map(|b| format!("{b:02x}")).collect()
}

fn helper_dir(app_support_dir: &Path) -> PathBuf {
    app_support_dir.join("statusline-helper")
}

pub fn helper_bin_path(app_support_dir: &Path) -> PathBuf {
    helper_dir(app_support_dir).join("quotos-statusline-helper")
}

fn feed_dir(app_support_dir: &Path) -> PathBuf {
    app_support_dir.join("statusline-feed")
}

fn backup_dir(app_support_dir: &Path) -> PathBuf {
    app_support_dir.join("statusline-backups")
}

fn meta_path(app_support_dir: &Path, slug: &str) -> PathBuf {
    backup_dir(app_support_dir).join(format!("{slug}.json"))
}

fn settings_path(config_dir: &Path) -> PathBuf {
    config_dir.join("settings.json")
}

fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

fn build_command(helper: &Path, config_dir: &Path, feed_dir: &Path) -> String {
    format!(
        "{} {} {} {}",
        shell_quote(&helper.to_string_lossy()),
        INGEST_FLAG,
        shell_quote(&config_dir.to_string_lossy()),
        shell_quote(&feed_dir.to_string_lossy()),
    )
}

fn atomic_write_json(path: &Path, value: &serde_json::Value) -> Result<(), String> {
    let json = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
    atomic_write_string(path, &json)
}

fn ensure_helper_installed(app_support_dir: &Path) -> Result<PathBuf, StatuslineError> {
    let target = helper_bin_path(app_support_dir);
    let current_exe =
        std::env::current_exe().map_err(|e| StatuslineError::HelperInstallFailed {
            message: format!(
                "Quotos couldn't find its own executable to install as the statusline helper ({e})."
            ),
        })?;

    let needs_copy = match (fs::metadata(&target), fs::metadata(&current_exe)) {
        (Ok(target_meta), Ok(current_meta)) => target_meta.len() != current_meta.len(),
        _ => true,
    };
    if needs_copy {
        let dir = helper_dir(app_support_dir);
        fs::create_dir_all(&dir).map_err(|e| StatuslineError::HelperInstallFailed {
            message: e.to_string(),
        })?;
        let tmp = dir.join("quotos-statusline-helper.tmp");
        fs::copy(&current_exe, &tmp).map_err(|e| StatuslineError::HelperInstallFailed {
            message: format!("Quotos couldn't copy its own helper binary ({e})."),
        })?;
        #[cfg(unix)]
        mark_executable(&tmp)?;
        fs::rename(&tmp, &target).map_err(|e| StatuslineError::HelperInstallFailed {
            message: e.to_string(),
        })?;
    }
    Ok(target)
}

#[cfg(unix)]
fn mark_executable(path: &Path) -> Result<(), StatuslineError> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = fs::metadata(path)
        .map_err(|e| StatuslineError::HelperInstallFailed {
            message: e.to_string(),
        })?
        .permissions();
    perms.set_mode(0o755);
    fs::set_permissions(path, perms).map_err(|e| StatuslineError::HelperInstallFailed {
        message: e.to_string(),
    })
}

fn read_settings(path: &Path) -> Result<serde_json::Value, StatuslineError> {
    match fs::read_to_string(path) {
        Ok(raw) => parse_settings(&raw),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(serde_json::json!({})),
        Err(e) => Err(StatuslineError::read(format!(
            "Quotos couldn't read this account's settings.json ({e})."
        ))),
    }
}

fn parse_settings(raw: &str) -> Result<serde_json::Value, StatuslineError> {
    let parsed: serde_json::Value =
        serde_json::from_str(raw).map_err(|e| StatuslineError::ParseFailed {
            message: format!(
                "Quotos couldn't parse this account's settings.json ({e}). Nothing was changed."
            ),
        })?;
    if !parsed.is_object() {
        return Err(StatuslineError::ParseFailed {
            message: "This account's settings.json isn't a JSON object at the top level. Nothing was changed.".to_string(),
        });
    }
    Ok(parsed)
}

pub fn status(
    app_support_dir: &Path,
    config_dir: &Path,
) -> Result<IntegrationStatus, StatuslineError> {
    let our_command = build_command(
        &helper_bin_path(app_support_dir),
        config_dir,
        &feed_dir(app_support_dir),
    );
    let settings = read_settings(&settings_path(config_dir))?;
    let Some(existing) = settings.get("statusLine") else {
        return Ok(IntegrationStatus::NotInstalled);
    };
    let command = existing.get("command").and_then(|c| c.as_str());
    if command == Some(our_command.as_str()) {
        Ok(IntegrationStatus::Installed)
    } else {
        Ok(IntegrationStatus::Conflict {
            existing_command: command
                .map(str::to_string)
                .unwrap_or_else(|| existing.to_string()),
        })
    }
}

pub fn install(
    app_support_dir: &Path,
    config_dir: &Path,
    force: bool,
) -> Result<InstallOutcome, StatuslineError> {
    let helper = ensure_helper_installed(app_support_dir)?;
    let feeds = feed_dir(app_support_dir);
    fs::create_dir_all(&feeds).map_err(|e| StatuslineError::write(e.to_string()))?;
    let our_command = build_command(&helper, config_dir, &feeds);

    let path = settings_path(config_dir);
    let raw_existing = match fs::read_to_string(&path) {
        Ok(s) => Some(s),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => {
            return Err(StatuslineError::read(format!(
                "Quotos couldn't read this account's settings.json ({e})."
            )));
        }
    };
    let mut settings = match &raw_existing {
        Some(raw) => parse_settings(raw)?,
        None => serde_json::json!({}),
    };

    let existing_status_line = settings.get("statusLine").cloned();
    let already_ours = existing_status_line
        .as_ref()
        .and_then(|v| v.get("command"))
        .and_then(|c| c.as_str())
        == Some(our_command.as_str());

    if already_ours {
        return Ok(InstallOutcome {
            replaced_existing: false,
        });
    }
    if !force && let Some(existing) = &existing_status_line {
        let existing_command = existing
            .get("command")
            .and_then(|c| c.as_str())
            .map(str::to_string)
            .unwrap_or_else(|| existing.to_string());
        return Err(StatuslineError::Conflict { existing_command });
    }

    backup(
        app_support_dir,
        config_dir,
        raw_existing.as_deref(),
        existing_status_line.clone(),
    )?;

    settings["statusLine"] = serde_json::json!({ "type": "command", "command": our_command });
    atomic_write_json(&path, &settings).map_err(StatuslineError::write)?;

    Ok(InstallOutcome {
        replaced_existing: existing_status_line.is_some(),
    })
}

fn backup(
    app_support_dir: &Path,
    config_dir: &Path,
    raw_existing: Option<&str>,
    previous_status_line: Option<serde_json::Value>,
) -> Result<(), StatuslineError> {
    let slug = slug_for(&config_dir.to_string_lossy());
    let dir = backup_dir(app_support_dir);
    fs::create_dir_all(&dir).map_err(|e| StatuslineError::write(e.to_string()))?;

    let settings_backup_path = match raw_existing {
        Some(raw) => {
            let ts = chrono::Utc::now().format("%Y%m%dT%H%M%S%.3fZ").to_string();
            let bak_path = dir.join(format!("{slug}-{ts}.settings.json.bak"));
            atomic_write_string(&bak_path, raw).map_err(StatuslineError::write)?;
            bak_path.to_string_lossy().to_string()
        }
        None => String::new(),
    };

    let record = BackupRecord {
        backed_up_at: chrono::Utc::now().to_rfc3339(),
        settings_backup_path,
        previous_status_line,
    };
    let value = serde_json::to_value(&record).map_err(|e| StatuslineError::write(e.to_string()))?;
    atomic_write_json(&meta_path(app_support_dir, &slug), &value)
        .map_err(StatuslineError::write)?;
    Ok(())
}

pub fn remove(app_support_dir: &Path, config_dir: &Path) -> Result<(), StatuslineError> {
    let slug = slug_for(&config_dir.to_string_lossy());
    let meta_file = meta_path(app_support_dir, &slug);
    let previous_status_line = fs::read_to_string(&meta_file)
        .ok()
        .and_then(|raw| serde_json::from_str::<BackupRecord>(&raw).ok())
        .and_then(|r| r.previous_status_line);

    let path = settings_path(config_dir);
    let mut settings = read_settings(&path)?;

    match previous_status_line {
        Some(prev) => {
            settings["statusLine"] = prev;
        }
        None => {
            if let Some(obj) = settings.as_object_mut() {
                obj.remove("statusLine");
            }
        }
    }
    atomic_write_json(&path, &settings).map_err(StatuslineError::write)?;
    let _ = fs::remove_file(&meta_file);
    Ok(())
}

pub fn read_feed(app_support_dir: &Path, config_dir: &Path) -> Option<StatuslineFeedDto> {
    let slug = slug_for(&config_dir.to_string_lossy());
    let path = feed_dir(app_support_dir).join(format!("{slug}.json"));
    let raw = fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}

fn extract_window(v: &serde_json::Value) -> Option<StatuslineWindowDto> {
    let used_percentage = v.get("used_percentage")?.as_f64()?;
    let resets_at = v.get("resets_at").and_then(|r| r.as_i64());
    Some(StatuslineWindowDto {
        used_percentage,
        resets_at,
    })
}

fn extract_rate_limits(v: &serde_json::Value) -> Option<StatuslineRateLimitsDto> {
    let five_hour = v.get("five_hour").and_then(extract_window);
    let seven_day = v.get("seven_day").and_then(extract_window);
    if five_hour.is_none() && seven_day.is_none() {
        return None;
    }
    Some(StatuslineRateLimitsDto {
        five_hour,
        seven_day,
    })
}

pub fn run_ingest_from_stdin(config_dir_tag: &str, feed_dir_arg: &str) -> i32 {
    let mut buf = String::new();
    if std::io::stdin()
        .take(MAX_STDIN_BYTES)
        .read_to_string(&mut buf)
        .is_err()
    {
        return 0;
    }
    let Ok(payload) = serde_json::from_str::<serde_json::Value>(&buf) else {
        return 0;
    };
    let Some(rate_limits_raw) = payload.get("rate_limits") else {
        return 0;
    };
    let Some(rate_limits) = extract_rate_limits(rate_limits_raw) else {
        return 0;
    };

    let slug = slug_for(config_dir_tag);
    let out_path = Path::new(feed_dir_arg).join(format!("{slug}.json"));
    let record = StatuslineFeedDto {
        written_at: chrono::Utc::now().to_rfc3339(),
        rate_limits,
    };
    if let Ok(value) = serde_json::to_value(&record) {
        let _ = atomic_write_json(&out_path, &value);
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    struct TempDirs {
        config_dir: PathBuf,
        app_support_dir: PathBuf,
    }

    impl TempDirs {
        fn new() -> Self {
            let count = COUNTER.fetch_add(1, Ordering::Relaxed);
            let root = std::env::temp_dir().join(format!(
                "quotos-statusline-test-{}-{count}",
                std::process::id()
            ));
            let config_dir = root.join("config");
            let app_support_dir = root.join("app-support");
            fs::create_dir_all(&config_dir).unwrap();
            fs::create_dir_all(&app_support_dir).unwrap();
            Self {
                config_dir,
                app_support_dir,
            }
        }

        fn settings_path(&self) -> PathBuf {
            settings_path(&self.config_dir)
        }

        fn write_settings(&self, json: &str) {
            fs::write(self.settings_path(), json).unwrap();
        }

        fn read_settings_raw(&self) -> String {
            fs::read_to_string(self.settings_path()).unwrap()
        }
    }

    impl Drop for TempDirs {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(self.config_dir.parent().unwrap());
        }
    }

    #[test]
    fn install_creates_settings_json_when_none_exists() {
        let dirs = TempDirs::new();
        let outcome = install(&dirs.app_support_dir, &dirs.config_dir, false)
            .expect("install should succeed");
        assert!(!outcome.replaced_existing);

        let settings: serde_json::Value = serde_json::from_str(&dirs.read_settings_raw()).unwrap();
        let command = settings["statusLine"]["command"].as_str().unwrap();
        assert_eq!(settings["statusLine"]["type"], "command");
        assert!(command.contains(INGEST_FLAG));
        assert!(command.contains(&dirs.config_dir.to_string_lossy().to_string()));
    }

    #[test]
    fn install_preserves_every_other_key() {
        let dirs = TempDirs::new();
        dirs.write_settings(r#"{"otherSetting": true, "nested": {"a": 1}}"#);

        install(&dirs.app_support_dir, &dirs.config_dir, false).expect("install should succeed");

        let settings: serde_json::Value = serde_json::from_str(&dirs.read_settings_raw()).unwrap();
        assert_eq!(settings["otherSetting"], true);
        assert_eq!(settings["nested"]["a"], 1);
        assert!(settings.get("statusLine").is_some());
    }

    #[test]
    fn install_writes_atomically_leaving_no_tmp_file() {
        let dirs = TempDirs::new();
        install(&dirs.app_support_dir, &dirs.config_dir, false).expect("install should succeed");
        assert!(!dirs.settings_path().with_extension("json.tmp").exists());
    }

    #[test]
    fn install_refuses_malformed_json_and_changes_nothing() {
        let dirs = TempDirs::new();
        let original = "{ not valid json at all";
        dirs.write_settings(original);

        let result = install(&dirs.app_support_dir, &dirs.config_dir, false);
        assert!(matches!(result, Err(StatuslineError::ParseFailed { .. })));
        assert_eq!(
            dirs.read_settings_raw(),
            original,
            "a refused install must not touch the file"
        );
    }

    #[test]
    fn install_refuses_a_top_level_json_array() {
        let dirs = TempDirs::new();
        dirs.write_settings("[1, 2, 3]");
        let result = install(&dirs.app_support_dir, &dirs.config_dir, false);
        assert!(matches!(result, Err(StatuslineError::ParseFailed { .. })));
    }

    #[test]
    fn install_reports_conflict_for_a_different_existing_statusline_without_force() {
        let dirs = TempDirs::new();
        dirs.write_settings(
            r#"{"statusLine": {"type": "command", "command": "~/.claude/my-own-script.sh"}}"#,
        );

        let result = install(&dirs.app_support_dir, &dirs.config_dir, false);
        match result {
            Err(StatuslineError::Conflict { existing_command }) => {
                assert_eq!(existing_command, "~/.claude/my-own-script.sh");
            }
            other => panic!("expected Conflict, got {other:?}"),
        }
        let settings: serde_json::Value = serde_json::from_str(&dirs.read_settings_raw()).unwrap();
        assert_eq!(
            settings["statusLine"]["command"],
            "~/.claude/my-own-script.sh"
        );
    }

    #[test]
    fn install_with_force_replaces_a_different_existing_statusline() {
        let dirs = TempDirs::new();
        dirs.write_settings(r#"{"statusLine": {"type": "command", "command": "~/.claude/my-own-script.sh"}, "keepMe": 1}"#);

        let outcome = install(&dirs.app_support_dir, &dirs.config_dir, true)
            .expect("forced install should succeed");
        assert!(outcome.replaced_existing);

        let settings: serde_json::Value = serde_json::from_str(&dirs.read_settings_raw()).unwrap();
        assert!(
            settings["statusLine"]["command"]
                .as_str()
                .unwrap()
                .contains(INGEST_FLAG)
        );
        assert_eq!(
            settings["keepMe"], 1,
            "unrelated keys must survive a forced replace too"
        );
    }

    #[test]
    fn install_is_idempotent_when_already_installed_by_quotos() {
        let dirs = TempDirs::new();
        install(&dirs.app_support_dir, &dirs.config_dir, false).expect("first install");
        let after_first = dirs.read_settings_raw();

        let outcome = install(&dirs.app_support_dir, &dirs.config_dir, false)
            .expect("second install should be a no-op success");
        assert!(!outcome.replaced_existing);
        assert_eq!(
            dirs.read_settings_raw(),
            after_first,
            "re-installing our own entry must not rewrite the file"
        );
    }

    #[test]
    fn a_statusline_that_appears_between_a_status_check_and_install_is_still_caught() {
        let dirs = TempDirs::new();
        assert_eq!(
            status(&dirs.app_support_dir, &dirs.config_dir).unwrap(),
            IntegrationStatus::NotInstalled
        );

        dirs.write_settings(
            r#"{"statusLine": {"type": "command", "command": "~/.claude/someone-elses.sh"}}"#,
        );

        let result = install(&dirs.app_support_dir, &dirs.config_dir, false);
        assert!(matches!(result, Err(StatuslineError::Conflict { .. })));
    }

    #[test]
    fn a_statusline_changed_between_two_installs_is_re_detected_as_conflict() {
        let dirs = TempDirs::new();
        install(&dirs.app_support_dir, &dirs.config_dir, false).expect("first install");

        dirs.write_settings(
            r#"{"statusLine": {"type": "command", "command": "~/.claude/someone-elses.sh"}}"#,
        );

        let result = install(&dirs.app_support_dir, &dirs.config_dir, false);
        match result {
            Err(StatuslineError::Conflict { existing_command }) => {
                assert_eq!(existing_command, "~/.claude/someone-elses.sh");
            }
            other => panic!("expected a fresh Conflict, got {other:?}"),
        }
    }

    #[test]
    fn remove_restores_the_exact_previous_statusline_value() {
        let dirs = TempDirs::new();
        dirs.write_settings(r#"{"statusLine": {"type": "command", "command": "~/.claude/my-own-script.sh", "padding": 2}, "keepMe": "yes"}"#);

        install(&dirs.app_support_dir, &dirs.config_dir, true)
            .expect("forced install should succeed");
        remove(&dirs.app_support_dir, &dirs.config_dir).expect("remove should succeed");

        let settings: serde_json::Value = serde_json::from_str(&dirs.read_settings_raw()).unwrap();
        assert_eq!(
            settings["statusLine"]["command"],
            "~/.claude/my-own-script.sh"
        );
        assert_eq!(settings["statusLine"]["padding"], 2);
        assert_eq!(settings["keepMe"], "yes");
    }

    #[test]
    fn remove_deletes_the_key_when_there_was_no_previous_statusline() {
        let dirs = TempDirs::new();
        dirs.write_settings(r#"{"keepMe": "yes"}"#);

        install(&dirs.app_support_dir, &dirs.config_dir, false).expect("install should succeed");
        remove(&dirs.app_support_dir, &dirs.config_dir).expect("remove should succeed");

        let settings: serde_json::Value = serde_json::from_str(&dirs.read_settings_raw()).unwrap();
        assert!(settings.get("statusLine").is_none());
        assert_eq!(settings["keepMe"], "yes");
    }

    #[test]
    fn remove_with_no_install_history_just_clears_the_key() {
        let dirs = TempDirs::new();
        dirs.write_settings(
            r#"{"statusLine": {"type": "command", "command": "whatever"}, "keepMe": 1}"#,
        );

        remove(&dirs.app_support_dir, &dirs.config_dir).expect("remove should succeed");

        let settings: serde_json::Value = serde_json::from_str(&dirs.read_settings_raw()).unwrap();
        assert!(settings.get("statusLine").is_none());
        assert_eq!(settings["keepMe"], 1);
    }

    #[test]
    fn remove_leaves_a_timestamped_backup_file_of_the_previous_settings() {
        let dirs = TempDirs::new();
        dirs.write_settings(
            r#"{"statusLine": {"type": "command", "command": "~/.claude/my-own-script.sh"}}"#,
        );
        install(&dirs.app_support_dir, &dirs.config_dir, true)
            .expect("forced install should succeed");

        let backups = fs::read_dir(backup_dir(&dirs.app_support_dir))
            .unwrap()
            .flatten()
            .collect::<Vec<_>>();
        let bak = backups.iter().find(|e| {
            e.file_name()
                .to_string_lossy()
                .ends_with(".settings.json.bak")
        });
        assert!(bak.is_some(), "expected a timestamped raw backup file");
        let content = fs::read_to_string(bak.unwrap().path()).unwrap();
        assert!(content.contains("my-own-script.sh"));
    }

    #[test]
    fn install_then_remove_round_trips_to_no_statusline_when_none_existed() {
        let dirs = TempDirs::new();
        install(&dirs.app_support_dir, &dirs.config_dir, false).unwrap();
        remove(&dirs.app_support_dir, &dirs.config_dir).unwrap();
        let settings: serde_json::Value = serde_json::from_str(&dirs.read_settings_raw()).unwrap();
        assert!(settings.get("statusLine").is_none());
    }

    #[test]
    fn status_reports_not_installed_for_a_missing_settings_file() {
        let dirs = TempDirs::new();
        assert_eq!(
            status(&dirs.app_support_dir, &dirs.config_dir).unwrap(),
            IntegrationStatus::NotInstalled
        );
    }

    #[test]
    fn status_reports_installed_after_a_successful_install() {
        let dirs = TempDirs::new();
        install(&dirs.app_support_dir, &dirs.config_dir, false).unwrap();
        assert_eq!(
            status(&dirs.app_support_dir, &dirs.config_dir).unwrap(),
            IntegrationStatus::Installed
        );
    }

    #[test]
    fn status_reports_conflict_for_someone_elses_statusline() {
        let dirs = TempDirs::new();
        dirs.write_settings(r#"{"statusLine": {"type": "command", "command": "not ours"}}"#);
        match status(&dirs.app_support_dir, &dirs.config_dir).unwrap() {
            IntegrationStatus::Conflict { existing_command } => {
                assert_eq!(existing_command, "not ours")
            }
            other => panic!("expected Conflict, got {other:?}"),
        }
    }

    #[test]
    fn status_refuses_malformed_json_rather_than_guessing() {
        let dirs = TempDirs::new();
        dirs.write_settings("{ broken");
        assert!(matches!(
            status(&dirs.app_support_dir, &dirs.config_dir),
            Err(StatuslineError::ParseFailed { .. })
        ));
    }

    #[test]
    fn two_config_dirs_get_distinct_commands_and_feed_files() {
        let dirs = TempDirs::new();
        let other_config_dir = dirs.app_support_dir.parent().unwrap().join("other-config");
        fs::create_dir_all(&other_config_dir).unwrap();

        install(&dirs.app_support_dir, &dirs.config_dir, false).unwrap();
        install(&dirs.app_support_dir, &other_config_dir, false).unwrap();

        let a: serde_json::Value = serde_json::from_str(&dirs.read_settings_raw()).unwrap();
        let b: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(settings_path(&other_config_dir)).unwrap())
                .unwrap();
        assert_ne!(a["statusLine"]["command"], b["statusLine"]["command"]);
    }

    #[test]
    fn install_copies_a_helper_binary_into_app_support() {
        let dirs = TempDirs::new();
        install(&dirs.app_support_dir, &dirs.config_dir, false).unwrap();
        let helper = helper_bin_path(&dirs.app_support_dir);
        assert!(helper.is_file());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(&helper).unwrap().permissions().mode();
            assert!(mode & 0o111 != 0, "helper must be executable");
        }
    }

    #[test]
    fn read_feed_is_none_when_nothing_was_ever_written() {
        let dirs = TempDirs::new();
        assert!(read_feed(&dirs.app_support_dir, &dirs.config_dir).is_none());
    }

    #[test]
    fn read_feed_is_none_for_a_malformed_file_rather_than_panicking() {
        let dirs = TempDirs::new();
        let slug = slug_for(&dirs.config_dir.to_string_lossy());
        fs::create_dir_all(feed_dir(&dirs.app_support_dir)).unwrap();
        fs::write(
            feed_dir(&dirs.app_support_dir).join(format!("{slug}.json")),
            "not json",
        )
        .unwrap();
        assert!(read_feed(&dirs.app_support_dir, &dirs.config_dir).is_none());
    }

    #[test]
    fn read_feed_returns_what_was_written_by_the_ingest_path() {
        let dirs = TempDirs::new();
        let feeds = feed_dir(&dirs.app_support_dir);
        fs::create_dir_all(&feeds).unwrap();

        let stdin_payload = r#"{"rate_limits": {"five_hour": {"used_percentage": 23.5, "resets_at": 1738425600}, "seven_day": {"used_percentage": 41.2, "resets_at": 1738857600}}}"#;
        let record = extract_rate_limits(
            &serde_json::from_str::<serde_json::Value>(stdin_payload).unwrap()["rate_limits"],
        )
        .unwrap();
        let feed = StatuslineFeedDto {
            written_at: "2026-08-15T12:00:00Z".to_string(),
            rate_limits: record,
        };
        let slug = slug_for(&dirs.config_dir.to_string_lossy());
        atomic_write_json(
            &feeds.join(format!("{slug}.json")),
            &serde_json::to_value(&feed).unwrap(),
        )
        .unwrap();

        let read_back = read_feed(&dirs.app_support_dir, &dirs.config_dir).unwrap();
        assert_eq!(read_back, feed);
    }

    #[test]
    fn overlapping_writers_leave_one_intact_payload_and_no_temp_files() {
        let dirs = TempDirs::new();
        let target = feed_dir(&dirs.app_support_dir).join("acct.json");

        let payloads: Vec<String> = (0..4).map(|i| format!(r#"{{"writer":{i}}}"#)).collect();
        let writers: Vec<_> = payloads
            .iter()
            .map(|payload| {
                let target = target.clone();
                let payload = payload.clone();
                std::thread::spawn(move || {
                    for _ in 0..25 {
                        atomic_write_string(&target, &payload).expect("write must succeed");
                    }
                })
            })
            .collect();
        for writer in writers {
            writer.join().expect("writer thread");
        }

        let survivor = fs::read_to_string(&target).expect("target exists");
        assert!(
            payloads.contains(&survivor),
            "target must be one writer's intact payload, got: {survivor:?}"
        );
        let leftovers: Vec<String> = fs::read_dir(target.parent().unwrap())
            .expect("feed dir")
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|name| name.contains(".tmp"))
            .collect();
        assert!(leftovers.is_empty(), "stranded temp files: {leftovers:?}");
    }

    #[test]
    fn extracts_both_windows_from_the_documented_shape() {
        let raw = serde_json::json!({
            "five_hour": { "used_percentage": 23.5, "resets_at": 1738425600 },
            "seven_day": { "used_percentage": 41.2, "resets_at": 1738857600 },
        });
        let feed = extract_rate_limits(&raw).unwrap();
        assert_eq!(feed.five_hour.unwrap().used_percentage, 23.5);
        assert_eq!(feed.seven_day.unwrap().resets_at, Some(1738857600));
    }

    #[test]
    fn extracts_a_single_present_window_leaving_the_other_none() {
        let raw =
            serde_json::json!({ "five_hour": { "used_percentage": 5.0, "resets_at": 1738425600 } });
        let feed = extract_rate_limits(&raw).unwrap();
        assert!(feed.five_hour.is_some());
        assert!(feed.seven_day.is_none());
    }

    #[test]
    fn absent_rate_limits_extracts_to_none() {
        let empty = serde_json::json!({});
        assert!(extract_rate_limits(&empty).is_none());
    }

    #[test]
    fn a_window_missing_used_percentage_is_skipped_not_zeroed() {
        let raw = serde_json::json!({ "five_hour": { "resets_at": 1738425600 } });
        assert!(extract_rate_limits(&raw).is_none());
    }

    #[test]
    fn run_ingest_writes_a_feed_file_from_the_documented_payload() {
        let dirs = TempDirs::new();
        let feeds = feed_dir(&dirs.app_support_dir);
        fs::create_dir_all(&feeds).unwrap();

        let payload: serde_json::Value = serde_json::from_str(
            r#"{"rate_limits": {"five_hour": {"used_percentage": 2, "resets_at": 100}, "seven_day": null}}"#,
        )
        .unwrap();
        let rate_limits = extract_rate_limits(&payload["rate_limits"]).unwrap();
        assert!(rate_limits.five_hour.is_some());
        assert!(rate_limits.seven_day.is_none());
    }

    #[test]
    fn shell_quote_handles_spaces_and_embedded_quotes() {
        assert_eq!(shell_quote("/Users/a b/x"), "'/Users/a b/x'");
        assert_eq!(shell_quote("it's"), "'it'\\''s'");
    }

    #[test]
    fn built_command_contains_the_ingest_flag_and_both_paths_quoted() {
        let cmd = build_command(
            Path::new("/a b/helper"),
            Path::new("/c d/.claude"),
            Path::new("/e f/feed"),
        );
        assert!(cmd.contains(INGEST_FLAG));
        assert!(cmd.contains("'/a b/helper'"));
        assert!(cmd.contains("'/c d/.claude'"));
        assert!(cmd.contains("'/e f/feed'"));
    }
}
