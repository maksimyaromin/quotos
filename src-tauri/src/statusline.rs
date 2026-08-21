use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::atomic_write::write_string as atomic_write_string;

const LEGACY_HELPER_DIR: &str = "statusline-helper";
const LEGACY_HELPER_BIN: &str = "quotos-statusline-helper";
const LEGACY_FEED_DIR: &str = "statusline-feed";
const LEGACY_BACKUP_DIR: &str = "statusline-backups";

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum StatuslineError {
    ParseFailed { message: String },
    ReadFailed { message: String },
    WriteFailed { message: String },
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

#[derive(Serialize, Deserialize, Default, Clone)]
struct BackupRecord {
    backed_up_at: String,
    settings_backup_path: String,
    previous_status_line: Option<serde_json::Value>,
}

pub(crate) fn slug_for(tag: &str) -> String {
    let digest = Sha256::digest(tag.as_bytes());
    digest.iter().take(8).map(|b| format!("{b:02x}")).collect()
}

pub(crate) fn claude_statusline_dir(app_support_dir: &Path) -> PathBuf {
    app_support_dir.join("claude-statusline")
}

fn script_path(app_support_dir: &Path, slug: &str) -> PathBuf {
    claude_statusline_dir(app_support_dir).join(format!("{slug}.sh"))
}

fn feed_path(app_support_dir: &Path, slug: &str) -> PathBuf {
    claude_statusline_dir(app_support_dir).join(format!("{slug}.json"))
}

fn record_path(app_support_dir: &Path, slug: &str) -> PathBuf {
    claude_statusline_dir(app_support_dir).join(format!("{slug}.record.json"))
}

fn settings_path(config_dir: &Path) -> PathBuf {
    config_dir.join("settings.json")
}

fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

fn our_command(app_support_dir: &Path, slug: &str) -> String {
    shell_quote(&script_path(app_support_dir, slug).to_string_lossy())
}

fn atomic_write_json(path: &Path, value: &serde_json::Value) -> Result<(), String> {
    let json = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
    atomic_write_string(path, &json)
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

/// The POSIX `sh` script installed as the `statusLine` command; see "The
/// statusline feed" in docs/claude-provider.md for what it does and why
/// it takes no arguments.
fn build_script(feed_path: &Path, wrapped_command: Option<&str>) -> String {
    let feed_q = shell_quote(&feed_path.to_string_lossy());
    let emit = match wrapped_command {
        Some(cmd) => format!("printf '%s' \"$payload\" | sh -c {}\n", shell_quote(cmd)),
        None => "echo 'Quotos is listening'\n".to_string(),
    };
    format!(
        "#!/bin/sh\n\
         payload=$(cat)\n\
         tmp={feed_q}.tmp.$$\n\
         printf '%s' \"$payload\" > \"$tmp\" 2>/dev/null && mv -f \"$tmp\" {feed_q}\n\
         {emit}"
    )
}

fn write_script(path: &Path, content: &str) -> Result<(), StatuslineError> {
    atomic_write_string(path, content).map_err(StatuslineError::write)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(path)
            .map_err(|e| StatuslineError::write(e.to_string()))?
            .permissions();
        perms.set_mode(0o700);
        fs::set_permissions(path, perms).map_err(|e| StatuslineError::write(e.to_string()))?;
    }
    Ok(())
}

fn backup(
    app_support_dir: &Path,
    slug: &str,
    raw_existing: Option<&str>,
    previous_status_line: Option<serde_json::Value>,
) -> Result<(), StatuslineError> {
    let dir = claude_statusline_dir(app_support_dir);
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
    atomic_write_json(&record_path(app_support_dir, slug), &value)
        .map_err(StatuslineError::write)?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn write_enabled_state(
    app_support_dir: &Path,
    slug: &str,
    settings_file: &Path,
    settings: &mut serde_json::Value,
    raw_existing_for_backup: Option<&str>,
    record_previous_status_line: Option<serde_json::Value>,
    padding_source: Option<&serde_json::Value>,
    wrapped_command: Option<&str>,
) -> Result<(), StatuslineError> {
    fs::create_dir_all(claude_statusline_dir(app_support_dir))
        .map_err(|e| StatuslineError::write(e.to_string()))?;
    backup(
        app_support_dir,
        slug,
        raw_existing_for_backup,
        record_previous_status_line,
    )?;

    let script_content = build_script(&feed_path(app_support_dir, slug), wrapped_command);
    write_script(&script_path(app_support_dir, slug), &script_content)?;

    let mut entry = serde_json::json!({
        "type": "command",
        "command": our_command(app_support_dir, slug),
    });
    if let Some(src) = padding_source {
        if let Some(p) = src.get("padding") {
            entry["padding"] = p.clone();
        }
        if let Some(r) = src.get("refreshInterval") {
            entry["refreshInterval"] = r.clone();
        }
    }
    settings["statusLine"] = entry;
    atomic_write_json(settings_file, settings).map_err(StatuslineError::write)
}

pub fn status(
    app_support_dir: &Path,
    config_dir: &Path,
) -> Result<IntegrationStatus, StatuslineError> {
    let slug = slug_for(&config_dir.to_string_lossy());
    let our_cmd = our_command(app_support_dir, &slug);
    let settings = read_settings(&settings_path(config_dir))?;
    let installed = settings
        .get("statusLine")
        .and_then(|v| v.get("command"))
        .and_then(|c| c.as_str())
        == Some(our_cmd.as_str());
    Ok(if installed {
        IntegrationStatus::Installed
    } else {
        IntegrationStatus::NotInstalled
    })
}

/// Writes our script as the account's `statusLine`, wrapping whatever was
/// there before. A no-op when we're already installed; never refuses a
/// different existing status line.
pub fn enable(app_support_dir: &Path, config_dir: &Path) -> Result<(), StatuslineError> {
    let slug = slug_for(&config_dir.to_string_lossy());
    let our_cmd = our_command(app_support_dir, &slug);
    let settings_file = settings_path(config_dir);

    let raw_existing = match fs::read_to_string(&settings_file) {
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
        == Some(our_cmd.as_str());
    if already_ours {
        return Ok(());
    }

    let wrapped_command = existing_status_line
        .as_ref()
        .and_then(|v| v.get("command"))
        .and_then(|c| c.as_str())
        .map(str::to_string);

    write_enabled_state(
        app_support_dir,
        &slug,
        &settings_file,
        &mut settings,
        raw_existing.as_deref(),
        existing_status_line.clone(),
        existing_status_line.as_ref(),
        wrapped_command.as_deref(),
    )
}

/// Restores the exact previous `statusLine` (or clears the key when there
/// was none), then deletes every file this account's integration wrote.
pub fn disable(app_support_dir: &Path, config_dir: &Path) -> Result<(), StatuslineError> {
    let slug = slug_for(&config_dir.to_string_lossy());
    let record_file = record_path(app_support_dir, &slug);
    let record: Option<BackupRecord> = fs::read_to_string(&record_file)
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok());
    let previous_status_line = record.as_ref().and_then(|r| r.previous_status_line.clone());

    let settings_file = settings_path(config_dir);
    let mut settings = read_settings(&settings_file)?;
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
    atomic_write_json(&settings_file, &settings).map_err(StatuslineError::write)?;

    let _ = fs::remove_file(script_path(app_support_dir, &slug));
    let _ = fs::remove_file(feed_path(app_support_dir, &slug));
    if let Some(r) = &record
        && !r.settings_backup_path.is_empty()
    {
        let _ = fs::remove_file(&r.settings_backup_path);
    }
    let _ = fs::remove_file(&record_file);

    let dir = claude_statusline_dir(app_support_dir);
    let is_empty = fs::read_dir(&dir)
        .map(|mut entries| entries.next().is_none())
        .unwrap_or(false);
    if is_empty {
        let _ = fs::remove_dir(&dir);
    }
    Ok(())
}

pub(crate) fn read_feed_file(path: &Path) -> Option<StatuslineFeedDto> {
    let metadata = fs::metadata(path).ok()?;
    let modified = metadata.modified().ok()?;
    let written_at: chrono::DateTime<chrono::Utc> = modified.into();
    let raw = fs::read_to_string(path).ok()?;
    let payload: serde_json::Value = serde_json::from_str(&raw).ok()?;
    let rate_limits = extract_rate_limits(payload.get("rate_limits")?)?;
    Some(StatuslineFeedDto {
        written_at: written_at.to_rfc3339(),
        rate_limits,
    })
}

pub fn read_feed(app_support_dir: &Path, config_dir: &Path) -> Option<StatuslineFeedDto> {
    let slug = slug_for(&config_dir.to_string_lossy());
    read_feed_file(&feed_path(app_support_dir, &slug))
}

/// True for a `<16 lowercase hex chars>.json` feed file name — a slug, not
/// a `.record.json` metadata file or a `.settings.json.bak` backup.
pub(crate) fn feed_slug_from_path(path: &Path) -> Option<String> {
    if path.extension().and_then(|e| e.to_str()) != Some("json") {
        return None;
    }
    let stem = path.file_stem()?.to_str()?;
    let is_slug = stem.len() == 16
        && stem
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
    is_slug.then(|| stem.to_string())
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

/// One-time, idempotent migration off the old copied-binary mechanism;
/// see "The statusline feed" in docs/claude-provider.md for the exact
/// matching and cleanup rules.
pub fn migrate_legacy(app_support_dir: &Path, candidate_config_dirs: &[PathBuf]) {
    let helper_dir = app_support_dir.join(LEGACY_HELPER_DIR);
    let feed_dir = app_support_dir.join(LEGACY_FEED_DIR);
    let backup_dir = app_support_dir.join(LEGACY_BACKUP_DIR);
    if !helper_dir.exists() && !feed_dir.exists() && !backup_dir.exists() {
        return;
    }

    let legacy_helper_bin_marker = helper_dir
        .join(LEGACY_HELPER_BIN)
        .to_string_lossy()
        .to_string();

    if let Ok(entries) = fs::read_dir(&backup_dir) {
        for entry in entries.flatten() {
            migrate_one_record(
                app_support_dir,
                candidate_config_dirs,
                &entry.path(),
                &legacy_helper_bin_marker,
            );
        }
    }

    let _ = fs::remove_dir_all(&helper_dir);
    let _ = fs::remove_dir_all(&feed_dir);
    let _ = fs::remove_dir_all(&backup_dir);
}

fn migrate_one_record(
    app_support_dir: &Path,
    candidate_config_dirs: &[PathBuf],
    record_file: &Path,
    legacy_helper_bin_marker: &str,
) {
    if record_file.extension().and_then(|e| e.to_str()) != Some("json") {
        return;
    }
    let Some(slug) = record_file.file_stem().and_then(|s| s.to_str()) else {
        return;
    };
    let Some(config_dir) = candidate_config_dirs
        .iter()
        .find(|c| slug_for(&c.to_string_lossy()) == slug)
    else {
        return;
    };
    let Ok(raw) = fs::read_to_string(record_file) else {
        return;
    };
    let Ok(record) = serde_json::from_str::<BackupRecord>(&raw) else {
        return;
    };

    let settings_file = settings_path(config_dir);
    let Ok(current_raw) = fs::read_to_string(&settings_file) else {
        return;
    };
    let Ok(current) = parse_settings(&current_raw) else {
        return;
    };
    let points_at_old_helper = current
        .get("statusLine")
        .and_then(|v| v.get("command"))
        .and_then(|c| c.as_str())
        .is_some_and(|cmd| cmd.contains(legacy_helper_bin_marker));
    if !points_at_old_helper {
        return;
    }

    let wrapped_command = record
        .previous_status_line
        .as_ref()
        .and_then(|v| v.get("command"))
        .and_then(|c| c.as_str())
        .map(str::to_string);

    let mut settings_to_write = current;
    let _ = write_enabled_state(
        app_support_dir,
        slug,
        &settings_file,
        &mut settings_to_write,
        Some(&current_raw),
        record.previous_status_line,
        None,
        wrapped_command.as_deref(),
    );
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

        fn slug(&self) -> String {
            slug_for(&self.config_dir.to_string_lossy())
        }

        fn dir(&self) -> PathBuf {
            claude_statusline_dir(&self.app_support_dir)
        }
    }

    impl Drop for TempDirs {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(self.config_dir.parent().unwrap());
        }
    }

    #[test]
    fn enable_creates_settings_json_when_none_exists() {
        let dirs = TempDirs::new();
        enable(&dirs.app_support_dir, &dirs.config_dir).expect("enable should succeed");

        let settings: serde_json::Value = serde_json::from_str(&dirs.read_settings_raw()).unwrap();
        let command = settings["statusLine"]["command"].as_str().unwrap();
        assert_eq!(settings["statusLine"]["type"], "command");
        assert!(command.contains(&dirs.slug()));
        assert!(command.contains(".sh"));
    }

    #[test]
    fn enable_preserves_every_other_key() {
        let dirs = TempDirs::new();
        dirs.write_settings(r#"{"otherSetting": true, "nested": {"a": 1}}"#);

        enable(&dirs.app_support_dir, &dirs.config_dir).expect("enable should succeed");

        let settings: serde_json::Value = serde_json::from_str(&dirs.read_settings_raw()).unwrap();
        assert_eq!(settings["otherSetting"], true);
        assert_eq!(settings["nested"]["a"], 1);
        assert!(settings.get("statusLine").is_some());
    }

    #[test]
    fn enable_writes_atomically_leaving_no_tmp_file() {
        let dirs = TempDirs::new();
        enable(&dirs.app_support_dir, &dirs.config_dir).expect("enable should succeed");
        assert!(!dirs.settings_path().with_extension("json.tmp").exists());
    }

    #[test]
    fn enable_refuses_malformed_json_and_changes_nothing() {
        let dirs = TempDirs::new();
        let original = "{ not valid json at all";
        dirs.write_settings(original);

        let result = enable(&dirs.app_support_dir, &dirs.config_dir);
        assert!(matches!(result, Err(StatuslineError::ParseFailed { .. })));
        assert_eq!(
            dirs.read_settings_raw(),
            original,
            "a refused enable must not touch the file"
        );
    }

    #[test]
    fn enable_refuses_a_top_level_json_array() {
        let dirs = TempDirs::new();
        dirs.write_settings("[1, 2, 3]");
        let result = enable(&dirs.app_support_dir, &dirs.config_dir);
        assert!(matches!(result, Err(StatuslineError::ParseFailed { .. })));
    }

    #[test]
    fn enable_wraps_a_different_existing_statusline_instead_of_refusing() {
        let dirs = TempDirs::new();
        dirs.write_settings(
            r#"{"statusLine": {"type": "command", "command": "~/.claude/my-own-script.sh"}, "keepMe": 1}"#,
        );

        enable(&dirs.app_support_dir, &dirs.config_dir).expect("enable should succeed");

        let settings: serde_json::Value = serde_json::from_str(&dirs.read_settings_raw()).unwrap();
        let command = settings["statusLine"]["command"].as_str().unwrap();
        assert!(command.contains(".sh"));
        assert_eq!(settings["keepMe"], 1);

        let script_path = script_path(&dirs.app_support_dir, &dirs.slug());
        let script = fs::read_to_string(&script_path).unwrap();
        assert!(script.contains("~/.claude/my-own-script.sh"));
    }

    #[test]
    fn enable_preserves_padding_and_refresh_interval_from_a_wrapped_entry() {
        let dirs = TempDirs::new();
        dirs.write_settings(
            r#"{"statusLine": {"type": "command", "command": "~/.claude/my-own-script.sh", "padding": 3, "refreshInterval": 5}}"#,
        );

        enable(&dirs.app_support_dir, &dirs.config_dir).expect("enable should succeed");

        let settings: serde_json::Value = serde_json::from_str(&dirs.read_settings_raw()).unwrap();
        assert_eq!(settings["statusLine"]["padding"], 3);
        assert_eq!(settings["statusLine"]["refreshInterval"], 5);
    }

    #[test]
    fn enable_is_idempotent_when_already_installed_by_quotos() {
        let dirs = TempDirs::new();
        enable(&dirs.app_support_dir, &dirs.config_dir).expect("first enable");
        let after_first = dirs.read_settings_raw();

        enable(&dirs.app_support_dir, &dirs.config_dir).expect("second enable should be a no-op");
        assert_eq!(
            dirs.read_settings_raw(),
            after_first,
            "re-enabling our own entry must not rewrite the file"
        );
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
    fn status_reports_installed_after_a_successful_enable() {
        let dirs = TempDirs::new();
        enable(&dirs.app_support_dir, &dirs.config_dir).unwrap();
        assert_eq!(
            status(&dirs.app_support_dir, &dirs.config_dir).unwrap(),
            IntegrationStatus::Installed
        );
    }

    #[test]
    fn status_reports_not_installed_for_someone_elses_statusline() {
        let dirs = TempDirs::new();
        dirs.write_settings(r#"{"statusLine": {"type": "command", "command": "not ours"}}"#);
        assert_eq!(
            status(&dirs.app_support_dir, &dirs.config_dir).unwrap(),
            IntegrationStatus::NotInstalled
        );
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

        enable(&dirs.app_support_dir, &dirs.config_dir).unwrap();
        enable(&dirs.app_support_dir, &other_config_dir).unwrap();

        let a: serde_json::Value = serde_json::from_str(&dirs.read_settings_raw()).unwrap();
        let b: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(settings_path(&other_config_dir)).unwrap())
                .unwrap();
        assert_ne!(a["statusLine"]["command"], b["statusLine"]["command"]);
    }

    #[test]
    fn enable_writes_a_self_contained_no_argument_script() {
        let dirs = TempDirs::new();
        enable(&dirs.app_support_dir, &dirs.config_dir).unwrap();
        let script_path = script_path(&dirs.app_support_dir, &dirs.slug());
        assert!(script_path.is_file());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(&script_path).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o700);
        }
        let settings: serde_json::Value = serde_json::from_str(&dirs.read_settings_raw()).unwrap();
        let command = settings["statusLine"]["command"].as_str().unwrap();
        assert!(!command.contains(' ') || command.starts_with('\''));
    }

    #[test]
    fn disable_restores_the_exact_previous_statusline_value() {
        let dirs = TempDirs::new();
        dirs.write_settings(r#"{"statusLine": {"type": "command", "command": "~/.claude/my-own-script.sh", "padding": 2}, "keepMe": "yes"}"#);

        enable(&dirs.app_support_dir, &dirs.config_dir).expect("enable should succeed");
        disable(&dirs.app_support_dir, &dirs.config_dir).expect("disable should succeed");

        let settings: serde_json::Value = serde_json::from_str(&dirs.read_settings_raw()).unwrap();
        assert_eq!(
            settings["statusLine"]["command"],
            "~/.claude/my-own-script.sh"
        );
        assert_eq!(settings["statusLine"]["padding"], 2);
        assert_eq!(settings["keepMe"], "yes");
    }

    #[test]
    fn disable_deletes_the_key_when_there_was_no_previous_statusline() {
        let dirs = TempDirs::new();
        dirs.write_settings(r#"{"keepMe": "yes"}"#);

        enable(&dirs.app_support_dir, &dirs.config_dir).expect("enable should succeed");
        disable(&dirs.app_support_dir, &dirs.config_dir).expect("disable should succeed");

        let settings: serde_json::Value = serde_json::from_str(&dirs.read_settings_raw()).unwrap();
        assert!(settings.get("statusLine").is_none());
        assert_eq!(settings["keepMe"], "yes");
    }

    #[test]
    fn disable_with_no_enable_history_just_clears_the_key() {
        let dirs = TempDirs::new();
        dirs.write_settings(
            r#"{"statusLine": {"type": "command", "command": "whatever"}, "keepMe": 1}"#,
        );

        disable(&dirs.app_support_dir, &dirs.config_dir).expect("disable should succeed");

        let settings: serde_json::Value = serde_json::from_str(&dirs.read_settings_raw()).unwrap();
        assert!(settings.get("statusLine").is_none());
        assert_eq!(settings["keepMe"], 1);
    }

    #[test]
    fn enable_then_disable_round_trips_to_no_statusline_when_none_existed() {
        let dirs = TempDirs::new();
        enable(&dirs.app_support_dir, &dirs.config_dir).unwrap();
        disable(&dirs.app_support_dir, &dirs.config_dir).unwrap();
        let settings: serde_json::Value = serde_json::from_str(&dirs.read_settings_raw()).unwrap();
        assert!(settings.get("statusLine").is_none());
    }

    #[test]
    fn disable_removes_the_claude_statusline_directory_when_it_becomes_empty() {
        let dirs = TempDirs::new();
        enable(&dirs.app_support_dir, &dirs.config_dir).unwrap();
        assert!(dirs.dir().exists());
        disable(&dirs.app_support_dir, &dirs.config_dir).unwrap();
        assert!(
            !dirs.dir().exists(),
            "the claude-statusline dir must be gone"
        );
    }

    #[test]
    fn disable_leaves_other_accounts_directory_entries_alone() {
        let dirs = TempDirs::new();
        let other_config_dir = dirs.app_support_dir.parent().unwrap().join("other-config");
        fs::create_dir_all(&other_config_dir).unwrap();

        enable(&dirs.app_support_dir, &dirs.config_dir).unwrap();
        enable(&dirs.app_support_dir, &other_config_dir).unwrap();
        disable(&dirs.app_support_dir, &dirs.config_dir).unwrap();

        assert!(dirs.dir().exists(), "the other account's files must remain");
        assert_eq!(
            status(&dirs.app_support_dir, &other_config_dir).unwrap(),
            IntegrationStatus::Installed
        );
    }

    #[test]
    fn read_feed_is_none_when_nothing_was_ever_written() {
        let dirs = TempDirs::new();
        assert!(read_feed(&dirs.app_support_dir, &dirs.config_dir).is_none());
    }

    #[test]
    fn read_feed_is_none_for_a_malformed_file_rather_than_panicking() {
        let dirs = TempDirs::new();
        fs::create_dir_all(dirs.dir()).unwrap();
        fs::write(dirs.dir().join(format!("{}.json", dirs.slug())), "not json").unwrap();
        assert!(read_feed(&dirs.app_support_dir, &dirs.config_dir).is_none());
    }

    #[test]
    fn read_feed_extracts_rate_limits_from_the_raw_claude_code_payload() {
        let dirs = TempDirs::new();
        fs::create_dir_all(dirs.dir()).unwrap();
        let payload = r#"{"cwd":"/x","model":{"id":"a","display_name":"A"},"rate_limits":{"five_hour":{"used_percentage":23.5,"resets_at":1738425600},"seven_day":{"used_percentage":41.2,"resets_at":1738857600}}}"#;
        fs::write(dirs.dir().join(format!("{}.json", dirs.slug())), payload).unwrap();

        let feed = read_feed(&dirs.app_support_dir, &dirs.config_dir).unwrap();
        assert_eq!(feed.rate_limits.five_hour.unwrap().used_percentage, 23.5);
        assert_eq!(
            feed.rate_limits.seven_day.unwrap().resets_at,
            Some(1738857600)
        );
        assert!(!feed.written_at.is_empty());
    }

    #[test]
    fn read_feed_written_at_reflects_the_file_mtime_not_a_payload_field() {
        let dirs = TempDirs::new();
        fs::create_dir_all(dirs.dir()).unwrap();
        let payload = r#"{"rate_limits":{"five_hour":{"used_percentage":1,"resets_at":null}}}"#;
        let feed_file = dirs.dir().join(format!("{}.json", dirs.slug()));
        fs::write(&feed_file, payload).unwrap();

        let before = fs::metadata(&feed_file).unwrap().modified().unwrap();
        let feed = read_feed(&dirs.app_support_dir, &dirs.config_dir).unwrap();
        let written_at: chrono::DateTime<chrono::Utc> = before.into();
        assert_eq!(feed.written_at, written_at.to_rfc3339());
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
    fn shell_quote_handles_spaces_and_embedded_quotes() {
        assert_eq!(shell_quote("/Users/a b/x"), "'/Users/a b/x'");
        assert_eq!(shell_quote("it's"), "'it'\\''s'");
    }

    #[test]
    fn feed_slug_from_path_matches_only_bare_slug_json_files() {
        assert_eq!(
            feed_slug_from_path(Path::new("/x/0123456789abcdef.json")),
            Some("0123456789abcdef".to_string())
        );
        assert_eq!(
            feed_slug_from_path(Path::new("/x/0123456789abcdef.record.json")),
            None
        );
        assert_eq!(
            feed_slug_from_path(Path::new(
                "/x/0123456789abcdef-20260101T000000.000Z.settings.json.bak"
            )),
            None
        );
        assert_eq!(
            feed_slug_from_path(Path::new("/x/0123456789abcdef.sh")),
            None
        );
    }

    fn write_and_run_script(script: &str, stdin_payload: &str) -> (String, bool) {
        let root = std::env::temp_dir().join(format!(
            "quotos-statusline-script-test-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).unwrap();
        let script_file = root.join("statusline.sh");
        write_script(&script_file, script).unwrap();

        use std::io::Write;
        use std::process::{Command, Stdio};
        let mut child = Command::new(&script_file)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("failed to spawn generated script");
        child
            .stdin
            .take()
            .unwrap()
            .write_all(stdin_payload.as_bytes())
            .unwrap();
        let output = child.wait_with_output().unwrap();
        let _ = fs::remove_dir_all(&root);
        (
            String::from_utf8_lossy(&output.stdout).to_string(),
            output.status.success(),
        )
    }

    const SAMPLE_PAYLOAD: &str = r#"{"model":{"display_name":"Opus"},"workspace":{"current_dir":"/home/user/project"},"rate_limits":{"five_hour":{"used_percentage":23.5,"resets_at":1738425600},"seven_day":{"used_percentage":41.2,"resets_at":1738857600}}}"#;

    #[test]
    fn generated_script_without_a_wrap_writes_the_feed_and_prints_listening_text() {
        let dirs = TempDirs::new();
        fs::create_dir_all(dirs.dir()).unwrap();
        let feed_file = feed_path(&dirs.app_support_dir, &dirs.slug());
        let script = build_script(&feed_file, None);

        let (stdout, ok) = write_and_run_script(&script, SAMPLE_PAYLOAD);
        assert!(ok);
        assert_eq!(stdout.trim(), "Quotos is listening");

        let feed_written = fs::read_to_string(&feed_file).unwrap();
        assert_eq!(feed_written, SAMPLE_PAYLOAD);
        assert!(
            fs::read_dir(feed_file.parent().unwrap())
                .unwrap()
                .flatten()
                .all(|e| !e.file_name().to_string_lossy().contains(".tmp.")),
            "no stranded temp file"
        );
    }

    #[test]
    fn generated_script_with_a_wrap_passes_stdin_through_and_writes_the_feed() {
        let dirs = TempDirs::new();
        fs::create_dir_all(dirs.dir()).unwrap();
        let feed_file = feed_path(&dirs.app_support_dir, &dirs.slug());
        let script = build_script(&feed_file, Some("echo wrapped"));

        let (stdout, ok) = write_and_run_script(&script, SAMPLE_PAYLOAD);
        assert!(ok);
        assert_eq!(stdout.trim(), "wrapped");
        assert_eq!(fs::read_to_string(&feed_file).unwrap(), SAMPLE_PAYLOAD);
    }

    #[test]
    fn generated_script_wraps_a_previous_command_containing_single_quotes() {
        let dirs = TempDirs::new();
        fs::create_dir_all(dirs.dir()).unwrap();
        let feed_file = feed_path(&dirs.app_support_dir, &dirs.slug());
        // A realistic previous command whose own syntax uses single quotes,
        // e.g. a jq filter — not merely a raw apostrophe in the output text.
        let script = build_script(&feed_file, Some("echo 'wrapped output'"));

        let (stdout, ok) = write_and_run_script(&script, SAMPLE_PAYLOAD);
        assert!(ok);
        assert_eq!(stdout.trim(), "wrapped output");
    }

    #[test]
    fn migrate_legacy_regenerates_the_script_and_removes_the_legacy_directories() {
        let dirs = TempDirs::new();
        dirs.write_settings(r#"{"otherSetting": true}"#);

        let helper_dir = dirs.app_support_dir.join(LEGACY_HELPER_DIR);
        fs::create_dir_all(&helper_dir).unwrap();
        let helper_bin = helper_dir.join(LEGACY_HELPER_BIN);
        fs::write(&helper_bin, "fake binary").unwrap();

        let feed_dir = dirs.app_support_dir.join(LEGACY_FEED_DIR);
        fs::create_dir_all(&feed_dir).unwrap();

        let backup_dir = dirs.app_support_dir.join(LEGACY_BACKUP_DIR);
        fs::create_dir_all(&backup_dir).unwrap();
        let record = serde_json::json!({
            "backed_up_at": "2026-08-15T12:00:00Z",
            "settings_backup_path": "",
            "previous_status_line": {
                "type": "command",
                "command": "~/.claude/pre-existing.sh",
            },
        });
        fs::write(
            backup_dir.join(format!("{}.json", dirs.slug())),
            serde_json::to_string(&record).unwrap(),
        )
        .unwrap();

        // Point settings.json at the old helper, as the pre-migration installer would have.
        let old_command = shell_quote(&helper_bin.to_string_lossy());
        dirs.write_settings(&format!(
            r#"{{"otherSetting": true, "statusLine": {{"type": "command", "command": "{old_command} x y"}}}}"#
        ));

        migrate_legacy(
            &dirs.app_support_dir,
            std::slice::from_ref(&dirs.config_dir),
        );

        assert!(!helper_dir.exists());
        assert!(!feed_dir.exists());
        assert!(!backup_dir.exists());

        let settings: serde_json::Value = serde_json::from_str(&dirs.read_settings_raw()).unwrap();
        let command = settings["statusLine"]["command"].as_str().unwrap();
        assert!(command.contains(".sh"));
        assert!(!command.contains(LEGACY_HELPER_BIN));
        assert_eq!(settings["otherSetting"], true);

        let script = fs::read_to_string(script_path(&dirs.app_support_dir, &dirs.slug())).unwrap();
        assert!(script.contains("~/.claude/pre-existing.sh"));

        assert_eq!(
            status(&dirs.app_support_dir, &dirs.config_dir).unwrap(),
            IntegrationStatus::Installed
        );
    }

    #[test]
    fn migrate_legacy_is_idempotent_on_a_second_run() {
        let dirs = TempDirs::new();
        let helper_dir = dirs.app_support_dir.join(LEGACY_HELPER_DIR);
        fs::create_dir_all(&helper_dir).unwrap();

        migrate_legacy(
            &dirs.app_support_dir,
            std::slice::from_ref(&dirs.config_dir),
        );
        assert!(!helper_dir.exists());

        // Nothing left to migrate; a second run must be a silent no-op.
        migrate_legacy(
            &dirs.app_support_dir,
            std::slice::from_ref(&dirs.config_dir),
        );
    }

    #[test]
    fn migrate_legacy_leaves_settings_untouched_when_no_longer_pointing_at_the_old_helper() {
        let dirs = TempDirs::new();
        dirs.write_settings(
            r#"{"statusLine": {"type": "command", "command": "~/.claude/something-else.sh"}}"#,
        );

        let backup_dir = dirs.app_support_dir.join(LEGACY_BACKUP_DIR);
        fs::create_dir_all(&backup_dir).unwrap();
        let record = serde_json::json!({
            "backed_up_at": "2026-08-15T12:00:00Z",
            "settings_backup_path": "",
            "previous_status_line": serde_json::Value::Null,
        });
        fs::write(
            backup_dir.join(format!("{}.json", dirs.slug())),
            serde_json::to_string(&record).unwrap(),
        )
        .unwrap();

        let before = dirs.read_settings_raw();
        migrate_legacy(
            &dirs.app_support_dir,
            std::slice::from_ref(&dirs.config_dir),
        );
        assert_eq!(dirs.read_settings_raw(), before);
        assert!(!backup_dir.exists());
    }
}
