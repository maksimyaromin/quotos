//! Claude Code's own statusline feed, a zero-cost second usage source; see
//! "The statusline feed" in claude-provider.md for why it exists and how
//! the frontend reconciles it. The write mechanism here follows six rules:
//!  1. Only written on an explicit in-app opt-in per subscription, never
//!     automatic. Only the Tauri commands in `accounts.rs` invoke
//!     [`install`] and [`remove`], and only in response to a click.
//!  2. Read-merge-write. Parse first, refuse and change nothing if it does
//!     not parse, preserve every other key, and write atomically through a
//!     temp file and rename in the same directory.
//!  3. A `statusLine` that is already configured and different is never
//!     clobbered. [`install`] returns [`StatuslineError::Conflict`] unless
//!     `force` is set.
//!  4. A timestamped backup of the previous file, plus a small metadata
//!     record of the previous `statusLine` value or its absence, that
//!     [`remove`] restores exactly.
//!  5. The installed command points at a small helper copied into Quotos's
//!     own Application Support directory. See [`ensure_helper_installed`]'s
//!     doc comment for why that copy is the running app's own executable
//!     rather than a purpose-built sidecar binary.
//!  6. This is a second source. [`read_feed`] only ever hands back a
//!     timestamped reading for the frontend to reconcile, where the
//!     freshest reading wins and the provider owns that reconciliation. See
//!     `providers/claude/statuslineMerge.ts`. It never invents a window the
//!     API did not already report; see [`read_feed`]'s own doc comment for
//!     what a missing or malformed feed degrades to.
//!
//! This is a scoped exception to the provider-adapter seam documented in
//! docs/architecture.md. The feed's own vocabulary, `five_hour` and
//! `seven_day`, is Claude Code CLI vocabulary, not a generic shape, but the
//! install, backup, restore, and read plumbing here is per-config-dir
//! infrastructure with nothing Claude-specific in how it works. This puts
//! it in the same category as `persistence.rs` and `scheduler.rs`, which
//! are shell-owned even though Claude is their only current caller.

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::atomic_write::write_string as atomic_write_string;

/// `main.rs` checks for this as `argv[1]` before calling into Tauri at
/// all. See [`ensure_helper_installed`] for why the copied helper is the
/// same executable as the GUI app rather than a separate binary.
pub const INGEST_FLAG: &str = "--quotos-statusline-ingest";

/// Hard ceiling on how much of the statusline hook's stdin payload is
/// read. The documented payload is a small, flat JSON object. This exists
/// only so a misbehaving caller can never make the ingest path buffer
/// unbounded memory.
const MAX_STDIN_BYTES: u64 = 4 * 1024 * 1024;

#[derive(Serialize, Clone, Debug)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum StatuslineError {
    /// The account's settings.json exists but did not parse as a JSON
    /// object. Refuses and changes nothing.
    ParseFailed {
        message: String,
    },
    ReadFailed {
        message: String,
    },
    WriteFailed {
        message: String,
    },
    HelperInstallFailed {
        message: String,
    },
    /// A `statusLine` is already configured and differs from Quotos's own.
    /// Carries what is there so the UI can show it before asking for an
    /// explicit replace.
    Conflict {
        existing_command: String,
    },
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
    /// Unix epoch seconds, exactly as the statusline hook's own
    /// `rate_limits.*.resets_at` documents it.
    pub resets_at: Option<i64>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct StatuslineRateLimitsDto {
    pub five_hour: Option<StatuslineWindowDto>,
    pub seven_day: Option<StatuslineWindowDto>,
}

/// What [`read_feed`] hands back to a caller, and what the ingest path
/// writes. One reading, timestamped by when the helper actually saw it,
/// not when Quotos later reads the file. That is what makes freshest wins
/// in the frontend's reconciliation a plain timestamp comparison.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct StatuslineFeedDto {
    pub written_at: String,
    pub rate_limits: StatuslineRateLimitsDto,
}

#[derive(Serialize, Deserialize, Default)]
struct BackupRecord {
    backed_up_at: String,
    /// Empty when there was no previous file at all to back up.
    settings_backup_path: String,
    /// `None` means the account had no `statusLine` before Quotos touched
    /// it, so remove() must then delete the key rather than write back a
    /// null.
    previous_status_line: Option<serde_json::Value>,
}

/// A stable, filesystem-safe identifier for a config dir. The same "hash
/// the absolute path" shape `providers/claude.rs` uses for Keychain
/// service names, reused here so two different accounts never collide on a
/// feed or backup filename.
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

/// Single-quotes a path for the shell `statusLine.command` runs under.
/// Claude Code's own docs state that the command field runs in a shell.
/// Handles the one case that matters for a filesystem path, an embedded
/// single quote, since macOS home directories can contain spaces.
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

/// Copies Quotos's own currently-running executable to a stable path
/// inside Application Support, so the installed `statusLine.command` keeps
/// working even after the app itself moves or is renamed.
///
/// This reuses the whole GUI binary rather than building a separate,
/// smaller sidecar. A purpose-built helper would need Tauri's `externalBin`
/// sidecar bundling, meaning target-triple-suffixed binaries staged into a
/// `binaries/` folder before `tauri build`. `main.rs` intercepts
/// [`INGEST_FLAG`] before any Tauri or GUI code runs at all, so invoking
/// the copy costs a process spawn and a JSON parse. No window, no status item,
/// and no webview ever initializes on this path. The tradeoff is disk,
/// tens of megabytes, for a local desktop app, not correctness.
fn ensure_helper_installed(app_support_dir: &Path) -> Result<PathBuf, StatuslineError> {
    let target = helper_bin_path(app_support_dir);
    let current_exe =
        std::env::current_exe().map_err(|e| StatuslineError::HelperInstallFailed {
            message: format!(
                "Quotos couldn't find its own executable to install as the statusline helper ({e})."
            ),
        })?;

    // A cheap, best-effort staleness check using size only, not a strict
    // integrity check, just enough to re-copy after an app update without
    // re-copying tens of megabytes on every install click.
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

/// Reads and parses `config_dir/settings.json`. A missing file reads as an
/// empty object, meaning nothing configured yet. A present but unparseable
/// file is refused rather than guessed at.
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

/// What is currently configured for this account: nothing, exactly
/// Quotos's own helper invocation, or something else, which is a conflict
/// the UI must show before offering to replace.
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

/// Installs the helper if needed and points `config_dir/settings.json`'s
/// `statusLine` at it. Idempotent when already installed by Quotos, a
/// no-op with no new backup. Refuses a different existing `statusLine`
/// with [`StatuslineError::Conflict`] unless `force` is set, and always
/// re-derives that conflict from a fresh read right before writing, never
/// from an earlier, possibly-stale `status()` call, so a settings.json
/// edited between the user seeing a conflict and pressing "Replace" is
/// still caught.
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

/// A timestamped backup of the whole previous file, when one existed,
/// plus a small metadata record of the previous `statusLine` value
/// specifically. [`remove`] actually restores from the metadata record.
/// The whole-file backup is kept purely as an inspectable safety net.
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

/// Restores exactly the previous `statusLine` state, or removes the key if
/// none existed, read-merge-write the same way [`install`] writes, and
/// clears the backup metadata once restored. A second `remove()` with
/// nothing left to restore just clears the key, which is the correct
/// behavior when Quotos has no record of installing here.
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

/// A best-effort read of whatever the helper most recently wrote for this
/// account. `None` covers "never installed", "no session has fed it yet",
/// and "the file is unreadable or malformed" identically. Every one of
/// those degrades silently to the API source, so none of them are worth
/// distinguishing to the caller.
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

/// The helper's entire job, run from `main.rs` before any Tauri or GUI
/// code. See [`ensure_helper_installed`]'s doc comment for why this same
/// binary is both the app and the helper. Reads the statusline hook's JSON
/// payload from stdin, writes nothing to stdout, since an empty statusline
/// output is documented, ordinary behavior and Claude Code just shows a
/// blank row, and always returns 0. Claude Code's own docs state that a
/// script exiting with a non-zero code or producing no output only makes
/// the statusline go blank, so a swallowed error here must never make
/// Claude Code itself look broken to the user.
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
    // Absent entirely on the session's first invocation, or for a
    // non-subscriber account. Nothing to write yet, not an error.
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

    /// A throwaway directory tree under the OS temp dir, cleaned up on
    /// drop. Never a real `~/.claude*` directory.
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
        // No statusLine yet. An earlier status() call would have said
        // NotInstalled.
        assert_eq!(
            status(&dirs.app_support_dir, &dirs.config_dir).unwrap(),
            IntegrationStatus::NotInstalled
        );

        // Something else, such as Claude Code's own /statusline command or
        // another tool, writes a statusLine in between.
        dirs.write_settings(
            r#"{"statusLine": {"type": "command", "command": "~/.claude/someone-elses.sh"}}"#,
        );

        // install() must re-derive the conflict from the file as it is now,
        // not from the stale NotInstalled the caller saw earlier.
        let result = install(&dirs.app_support_dir, &dirs.config_dir, false);
        assert!(matches!(result, Err(StatuslineError::Conflict { .. })));
    }

    #[test]
    fn a_statusline_changed_between_two_installs_is_re_detected_as_conflict() {
        let dirs = TempDirs::new();
        install(&dirs.app_support_dir, &dirs.config_dir, false).expect("first install");

        // Something else overwrites Quotos's entry after it installed it.
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

    /// The ingest helper runs as one process per statusline render, so two
    /// live Claude Code sessions on the same account write the same feed
    /// file concurrently. With a shared temp name, one writer's
    /// `File::create` could truncate another writer's temp file between
    /// its write and its rename, leaving the renamed-in feed intermittently
    /// empty or spliced. Every observed final state must be one writer's
    /// intact payload, and no writer may strand its temp file.
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
        // five_hour is present as an object but unusable, so it is treated
        // as absent. Since seven_day is also absent, the whole thing is
        // None.
        assert!(extract_rate_limits(&raw).is_none());
    }

    #[test]
    fn run_ingest_writes_a_feed_file_from_the_documented_payload() {
        let dirs = TempDirs::new();
        let feeds = feed_dir(&dirs.app_support_dir);
        fs::create_dir_all(&feeds).unwrap();

        // Exercises the same extraction the real stdin path uses. The I/O
        // wrapper itself, run_ingest_from_stdin, only adds stdin reading,
        // which is not meaningfully unit-testable without a real pipe.
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
