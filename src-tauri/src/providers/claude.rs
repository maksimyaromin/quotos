//! Claude Code provider adapter. Reads a Keychain-backed OAuth token and
//! calls the undocumented but first-party `/api/oauth/usage` and
//! `/api/oauth/profile` endpoints.

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use unicode_normalization::UnicodeNormalization;

use super::{AccountDescriptor, FetchError, RequestBudget};

const USER_AGENT: &str = "claude-code/2.1.227";
const USAGE_URL: &str = "https://api.anthropic.com/api/oauth/usage";
const PROFILE_URL: &str = "https://api.anthropic.com/api/oauth/profile";

/// How close to its own stated expiry an access token has to be before
/// Quotos renews it before spending a request on it. Claude Code's tokens
/// live roughly 8 hours, so a minute of margin costs nothing and removes
/// the whole "expired between the read and the request" class of 401s.
const EXPIRY_MARGIN_MS: i64 = 60_000;

/// Ceiling on how long Quotos will wait for the Claude Code CLI while it
/// renews a credential. Generous, since the CLI does real work, but
/// bounded, so a wedged child process can never wedge a read.
const CLI_REFRESH_TIMEOUT: Duration = Duration::from_secs(20);

/// Ceiling on the login-shell probe used to find the CLI, defined below. A
/// user's rc files are arbitrary code, and this keeps a slow one from
/// stalling a read.
const CLI_LOOKUP_TIMEOUT: Duration = Duration::from_secs(6);

/// Only ever used when the sign-in really is what is wrong. See
/// `classify_unrenewable`.
const SIGN_IN_EXPIRED: &str = "the stored sign-in is no longer accepted";

/// `security(1)`'s exit status for `errSecItemNotFound`, measured. It is
/// the one failure that genuinely means "not signed in", and every other
/// one must not be reported as such.
const KEYCHAIN_ITEM_NOT_FOUND_EXIT: i32 = 44;

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

/// `QUOTOS_DEBUG_READS=1` traces the credential and read decisions to
/// stderr, the same opt-in shape as the `QUOTOS_DEBUG_*` window flags,
/// silent by default. It deliberately never prints a token, only whether
/// one was found and how long it has left. Debugging a read otherwise
/// means re-deriving facts the app already computed and never surfaced,
/// such as whether the CLI is reachable, whether the token is expired, or
/// whether a renewal did anything.
fn trace_enabled() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("QUOTOS_DEBUG_READS").is_some())
}

macro_rules! read_trace {
    ($($arg:tt)*) => {
        if trace_enabled() {
            eprintln!("[quotos read] {}", format_args!($($arg)*));
        }
    };
}

/// Scans the home directory for Claude Code config directories: the
/// default `~/.claude` plus any `~/.claude-<name>` sibling, such as
/// `~/.claude-team`. This is the "scan the machine" step of the
/// add-subscription flow for the Claude provider.
///
/// A directory that merely matches the naming pattern is not itself a
/// subscription. A plain folder placed alongside the real config
/// directories, for example to share data between them, would otherwise
/// show up as an invented account. A config directory only qualifies when
/// it actually resolves to a usable credential, meaning a Keychain entry
/// exists for its derived service name.
pub fn discover_accounts() -> Vec<AccountDescriptor> {
    let Some(home) = home_dir() else {
        return vec![];
    };
    discover_accounts_in(&home, credential_exists_in_keychain)
}

/// The pure, testable core of discovery. Decides which `~/.claude*`
/// directories under `home` qualify, given a predicate for whether a
/// Keychain credential exists for this config dir. Split out from
/// [`discover_accounts`] so the qualification logic can be unit-tested
/// without touching the real Keychain.
fn discover_accounts_in(
    home: &Path,
    has_credential: impl Fn(&Path) -> bool,
) -> Vec<AccountDescriptor> {
    let mut found = vec![];

    let default_dir = home.join(".claude");
    if default_dir.is_dir() && has_credential(&default_dir) {
        found.push(AccountDescriptor {
            id: "claude:claude".to_string(),
            provider: "claude".to_string(),
            config_dir: default_dir.to_string_lossy().to_string(),
        });
    }

    if let Ok(entries) = std::fs::read_dir(home) {
        let mut candidates: Vec<_> = entries.flatten().collect();
        // Deterministic ordering: directory iteration order is not
        // guaranteed by the OS, and the account list should not reshuffle
        // between runs for no reason.
        candidates.sort_by_key(|e| e.file_name());
        for entry in candidates {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name == ".claude" || !name.starts_with(".claude-") {
                continue;
            }
            let path = entry.path();
            if !path.is_dir() || !has_credential(&path) {
                continue;
            }
            let slug = name.trim_start_matches('.').to_string();
            found.push(AccountDescriptor {
                id: format!("claude:{slug}"),
                provider: "claude".to_string(),
                config_dir: path.to_string_lossy().to_string(),
            });
        }
    }

    found
}

/// Existence-only Keychain lookup: no secret material is read, just whether
/// an item is present for the service name a config dir would derive.
fn credential_exists_in_keychain(config_dir: &Path) -> bool {
    let service = keychain_service_for_config_dir(config_dir);
    let Ok(user) = std::env::var("USER") else {
        return false;
    };
    Command::new("security")
        .args(["find-generic-password", "-s", &service, "-a", &user])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Whether this config dir is Claude Code's default one, meaning the one
/// it uses when `CLAUDE_CONFIG_DIR` is not set at all. That distinction is
/// load-bearing twice over, in the keychain service name below and in the
/// CLI environment in [`claude_config_dir_env`], so it lives in one place.
fn is_default_config_dir(home: &Path, config_dir: &Path) -> bool {
    home.join(".claude") == config_dir
}

/// Keychain service name for a config directory. The default `~/.claude`
/// uses the bare service name. Any other config dir uses
/// `Claude Code-credentials-<sha256[:8]>` of the NFC-normalized absolute
/// path.
fn keychain_service_for_config_dir(config_dir: &Path) -> String {
    match home_dir() {
        Some(home) => keychain_service_in(&home, config_dir),
        None => keychain_service_hashed(config_dir),
    }
}

fn keychain_service_in(home: &Path, config_dir: &Path) -> String {
    if is_default_config_dir(home, config_dir) {
        return "Claude Code-credentials".to_string();
    }
    keychain_service_hashed(config_dir)
}

fn keychain_service_hashed(config_dir: &Path) -> String {
    let normalized: String = config_dir.to_string_lossy().nfc().collect();
    let digest = sha2::Sha256::digest_str(&normalized);
    format!("Claude Code-credentials-{}", &digest[..8])
}

/// What `CLAUDE_CONFIG_DIR` must be set to when Quotos runs the Claude Code
/// CLI for this account. `None` means it must not be set at all, which is
/// the default account. See "CLAUDE_CONFIG_DIR" in claude-provider.md for
/// why setting it to the default account's own directory is not the same
/// as leaving it unset.
fn claude_config_dir_env(home: &Path, config_dir: &Path) -> Option<PathBuf> {
    if is_default_config_dir(home, config_dir) {
        None
    } else {
        Some(config_dir.to_path_buf())
    }
}

// A local helper avoids pulling in a whole hex crate for 32 bytes.
trait DigestHex {
    fn digest_str(input: &str) -> String;
}
impl DigestHex for sha2::Sha256 {
    fn digest_str(input: &str) -> String {
        use sha2::{Digest, Sha256};
        let hash = Sha256::digest(input.as_bytes());
        hash.iter().map(|b| format!("{b:02x}")).collect()
    }
}

/// What Quotos knows about a stored credential. The token is the only
/// secret here. The two expiry stamps are what make an honest diagnosis
/// possible. Without them a 401 is indistinguishable between "this sign-in
/// is finished" and "this token simply aged out and needs renewing".
#[derive(Clone, Debug, PartialEq, Eq)]
struct StoredCredential {
    access_token: String,
    /// Epoch ms, as stored. `None` when the provider didn't record one.
    expires_at_ms: Option<i64>,
    refresh_expires_at_ms: Option<i64>,
    has_refresh_token: bool,
}

impl StoredCredential {
    /// Past its own stated expiry (with `margin_ms` of look-ahead). A
    /// credential that records no expiry is never assumed expired.
    fn access_expired(&self, now_ms: i64, margin_ms: i64) -> bool {
        self.expires_at_ms
            .is_some_and(|at| at <= now_ms + margin_ms)
    }

    /// Whether the refresh half could still renew this credential without
    /// a new sign-in. A missing refresh expiry is treated as usable. The
    /// provider is the authority, and guessing "expired" here would claim
    /// a working account needs signing in.
    fn refresh_usable(&self, now_ms: i64) -> bool {
        self.has_refresh_token && self.refresh_expires_at_ms.is_none_or(|at| at > now_ms)
    }
}

/// Parse the Keychain blob. Split out from the Keychain call so the shape
/// handling is unit-testable without any credential on the machine.
fn parse_credential(raw: &str) -> Option<StoredCredential> {
    let parsed: serde_json::Value = serde_json::from_str(raw).ok()?;
    let oauth = parsed.get("claudeAiOauth")?;
    let access_token = oauth.get("accessToken")?.as_str()?.to_string();
    if access_token.is_empty() {
        return None;
    }
    Some(StoredCredential {
        access_token,
        expires_at_ms: epoch_ms(oauth.get("expiresAt")),
        refresh_expires_at_ms: epoch_ms(oauth.get("refreshTokenExpiresAt")),
        has_refresh_token: oauth
            .get("refreshToken")
            .and_then(|v| v.as_str())
            .is_some_and(|t| !t.is_empty()),
    })
}

/// Accepts the epoch-milliseconds numbers Claude Code actually stores, and
/// tolerates seconds or an RFC 3339 string rather than silently reading a
/// wrong instant if that ever changes.
fn epoch_ms(value: Option<&serde_json::Value>) -> Option<i64> {
    match value? {
        serde_json::Value::Number(n) => {
            let raw = n.as_f64()?;
            // Anything smaller than this is seconds, not milliseconds:
            // 1e12 ms is 2001, 1e12 s is the year 33658.
            Some(if raw.abs() >= 1e12 {
                raw as i64
            } else {
                (raw * 1000.0) as i64
            })
        }
        serde_json::Value::String(s) => chrono::DateTime::parse_from_rfc3339(s)
            .ok()
            .map(|d| d.timestamp_millis()),
        _ => None,
    }
}

/// Read this account's stored credential from the macOS Keychain. Never
/// logs the blob or the token. Read-only: `security find-generic-password`
/// is the same call Claude Code itself is trusted for on this item (it is
/// the ACL's own trusted application), so this cannot raise a prompt.
fn read_credential(config_dir: &Path) -> Result<StoredCredential, FetchError> {
    let service = keychain_service_for_config_dir(config_dir);
    let user = std::env::var("USER").map_err(|_| FetchError::Other {
        message: "Quotos couldn't tell which macOS user to read the Keychain for.".to_string(),
    })?;

    let output = Command::new("security")
        .args(["find-generic-password", "-s", &service, "-a", &user, "-w"])
        .output()
        .map_err(|e| FetchError::Other {
            message: format!("Quotos couldn't run the macOS Keychain tool ({e})."),
        })?;

    if !output.status.success() {
        return Err(classify_keychain_failure(&output.status));
    }

    let raw = String::from_utf8_lossy(&output.stdout).trim().to_string();
    parse_credential(&raw).ok_or_else(|| FetchError::Other {
        message:
            "Claude Code's stored credential for this account wasn't in a shape Quotos understands."
                .to_string(),
    })
}

/// Only "there is no such item" means the account is not signed in.
/// Anything else, such as a denied ACL, a locked keychain, or a tool that
/// will not run, is a local problem. Saying "sign in" for any of those
/// would misreport a local failure as a missing sign-in.
fn classify_keychain_failure(status: &std::process::ExitStatus) -> FetchError {
    match status.code() {
        Some(KEYCHAIN_ITEM_NOT_FOUND_EXIT) => FetchError::NotConnected {
            message: "Claude Code isn't signed in for this account. Sign in there and Quotos will pick it up."
                .to_string(),
        },
        Some(code) => FetchError::Other {
            message: format!(
                "Quotos couldn't read this account's credential from the Keychain (security exited {code})."
            ),
        },
        None => FetchError::Other {
            message: "Quotos couldn't read this account's credential from the Keychain.".to_string(),
        },
    }
}

/// Where the `claude` CLI actually is. See "Finding the claude CLI" in
/// claude-provider.md for why a menu bar app can't rely on `$PATH`.
///
/// Resolution order, most authoritative first: `$PATH`, then a login-shell
/// probe of the running system, then Claude Code's own documented install
/// locations rebuilt from `$HOME`. Nothing here is specific to one machine.
fn claude_cli_path() -> Option<PathBuf> {
    static CACHE: OnceLock<Mutex<Option<PathBuf>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(None));

    if let Ok(guard) = cache.lock()
        && let Some(cached) = guard.as_ref()
        && is_executable_file(cached)
    {
        return Some(cached.clone());
    }

    // Only successes are cached. A miss must stay re-checkable, so
    // installing Claude Code while Quotos runs does not need a relaunch.
    let found = locate_claude_cli();
    match &found {
        Some(path) => read_trace!("claude CLI resolved to {}", path.display()),
        None => read_trace!(
            "claude CLI NOT FOUND (PATH={:?})",
            std::env::var("PATH").unwrap_or_else(|_| "<unset>".to_string())
        ),
    }
    let found = found?;
    if let Ok(mut guard) = cache.lock() {
        *guard = Some(found.clone());
    }
    Some(found)
}

fn locate_claude_cli() -> Option<PathBuf> {
    if let Some(found) = search_env_path("claude") {
        return Some(found);
    }
    if let Some(found) = ask_login_shell("claude") {
        return Some(found);
    }
    let home = home_dir()?;
    well_known_cli_locations(&home)
        .into_iter()
        .find(|p| is_executable_file(p))
}

/// Claude Code's documented install locations, rebuilt from the running
/// system's `$HOME` rather than hardcoded for one Mac. Only consulted when
/// both `$PATH` and the user's own login shell come up empty.
fn well_known_cli_locations(home: &Path) -> Vec<PathBuf> {
    vec![
        home.join(".local/bin/claude"),
        home.join(".claude/local/claude"),
        home.join(".bun/bin/claude"),
        home.join("Library/pnpm/claude"),
        home.join(".npm-global/bin/claude"),
        PathBuf::from("/opt/homebrew/bin/claude"),
        PathBuf::from("/usr/local/bin/claude"),
    ]
}

fn search_env_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(name))
        .find(|candidate| is_executable_file(candidate))
}

/// Asks the user's own shell where `claude` is. This is the general answer
/// to the no-`PATH` problem above. The shell sources the same rc files
/// that put the CLI on `PATH` in a terminal, so whatever install method
/// was used, this finds it.
///
/// Both `-lc` and `-ilc` are tried, in that order, because a login shell
/// alone is not enough when `PATH` is set in a file such as `.zshrc`,
/// which only an interactive shell reads: a plain `-lc` login shell can
/// find nothing while `-ilc` resolves the same command correctly. The
/// cheaper, quieter form goes first. The interactive form is the fallback,
/// bounded by the same deadline in case an rc file misbehaves.
fn ask_login_shell(name: &str) -> Option<PathBuf> {
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
    for flags in ["-lc", "-ilc"] {
        let mut cmd = Command::new(&shell);
        cmd.arg(flags).arg(format!("command -v {name}"));
        let Some(output) = run_bounded(cmd, CLI_LOOKUP_TIMEOUT, true) else {
            continue;
        };
        // rc files may print banners. The resolved path is whichever line
        // actually names an executable.
        let found = String::from_utf8_lossy(&output.stdout)
            .lines()
            .rev()
            .map(|line| PathBuf::from(line.trim()))
            .find(|candidate| is_executable_file(candidate));
        if found.is_some() {
            return found;
        }
    }
    None
}

fn is_executable_file(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

/// Run a child with a hard deadline, killing it if it overruns. Quotos runs
/// the Claude Code CLI on the read path, so "it hung" must degrade into "no
/// refresh happened" rather than into a stuck read.
fn run_bounded(mut cmd: Command, timeout: Duration, capture: bool) -> Option<Output> {
    cmd.stdin(Stdio::null());
    cmd.stderr(Stdio::null());
    cmd.stdout(if capture {
        Stdio::piped()
    } else {
        Stdio::null()
    });

    let mut child = cmd.spawn().ok()?;
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return None;
                }
                std::thread::sleep(Duration::from_millis(25));
            }
            Err(_) => return None,
        }
    }
    child.wait_with_output().ok()
}

/// A zero-cost-to-quota way to have Claude Code renew a stale access
/// token. Any CLI invocation refreshes and writes back the stored
/// credential, and `claude mcp list` does no inference beyond that: an
/// expired token's `expiresAt` advances across exactly this call.
///
/// Returns whether the CLI actually ran. This is not a claim that anything
/// was renewed. The caller proves that by re-reading the credential, which
/// is the only thing that can distinguish an actual renewal from the CLI
/// running without changing anything.
fn run_cli_credential_refresh(config_dir: &Path) -> bool {
    let Some(invocation) = cli_invocation(config_dir) else {
        return false;
    };

    let mut cmd = Command::new(&invocation.program);
    cmd.args(["mcp", "list"]);
    match &invocation.config_dir_env {
        Some(dir) => {
            cmd.env("CLAUDE_CONFIG_DIR", dir);
        }
        None => {
            cmd.env_remove("CLAUDE_CONFIG_DIR");
        }
    }
    // The CLI shells out to other tools. A Finder-launched app hands it no
    // PATH at all, so this gives it at least its own directory plus the
    // system defaults.
    cmd.env("PATH", &invocation.path_env);

    run_bounded(cmd, CLI_REFRESH_TIMEOUT, false).is_some()
}

/// A `PATH` for child processes that works even when this process
/// inherited none, such as after a Finder or Dock launch.
fn child_path_including(extra: Option<&Path>) -> std::ffi::OsString {
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Some(dir) = extra {
        dirs.push(dir.to_path_buf());
    }
    if let Some(existing) = std::env::var_os("PATH") {
        dirs.extend(std::env::split_paths(&existing));
    }
    for fallback in ["/usr/bin", "/bin", "/usr/sbin", "/sbin"] {
        let p = PathBuf::from(fallback);
        if !dirs.contains(&p) {
            dirs.push(p);
        }
    }
    std::env::join_paths(dirs).unwrap_or_else(|_| std::ffi::OsString::from("/usr/bin:/bin"))
}

/// Everything a caller needs to spawn the Claude Code CLI for one account.
/// Shared with the sign-in flow in `signin.rs`, which spawns the CLI
/// through a pty rather than `std::process::Command`, but needs the same
/// binary resolution, the same `CLAUDE_CONFIG_DIR` decision, and the same
/// `PATH` this module produces.
pub struct CliInvocation {
    pub program: PathBuf,
    /// `Some(dir)` sets `CLAUDE_CONFIG_DIR` to it. `None` means it must not
    /// be set at all, which is the default account. See
    /// [`claude_config_dir_env`].
    pub config_dir_env: Option<PathBuf>,
    pub path_env: std::ffi::OsString,
}

/// `None` when the Claude Code CLI can't be found on this machine at all.
pub fn cli_invocation(config_dir: &Path) -> Option<CliInvocation> {
    let program = claude_cli_path()?;
    let home = home_dir()?;
    let path_env = child_path_including(program.parent());
    Some(CliInvocation {
        config_dir_env: claude_config_dir_env(&home, config_dir),
        program,
        path_env,
    })
}

/// Has the CLI renew the credential, then re-reads it. Returns `Some` only
/// when the stored credential genuinely changed. That proof is what keeps
/// a no-op refresh from costing a second request and from being reported
/// as an expired sign-in.
fn renew_credential(config_dir: &Path, previous: &StoredCredential) -> Option<StoredCredential> {
    if !run_cli_credential_refresh(config_dir) {
        return None;
    }
    let renewed = read_credential(config_dir).ok()?;
    (renewed.access_token != previous.access_token).then_some(renewed)
}

/// Same, off the async runtime's worker threads. The CLI call is real work
/// with a real, bounded wall-clock cost.
async fn renew_credential_async(
    config_dir: &Path,
    previous: &StoredCredential,
) -> Option<StoredCredential> {
    let dir = config_dir.to_path_buf();
    let previous = previous.clone();
    tauri::async_runtime::spawn_blocking(move || renew_credential(&dir, &previous))
        .await
        .ok()
        .flatten()
}

/// What a 401 means once the credential could not be renewed here. Three
/// genuinely different situations produce three different truths, and
/// only the first two are a sign-in problem.
fn classify_unrenewable(credential: &StoredCredential, now_ms: i64) -> FetchError {
    if !credential.access_expired(now_ms, 0) {
        // Current by its own metadata and still refused. The sign-in
        // behind it is finished, whether revoked or signed out elsewhere.
        return FetchError::Unauthorized {
            message: format!("{SIGN_IN_EXPIRED} (the stored token has not expired)"),
        };
    }
    if !credential.refresh_usable(now_ms) {
        // Expired, and nothing left that could renew it without a new
        // sign-in. This is the real "sign-in expired".
        return FetchError::Unauthorized {
            message: format!("{SIGN_IN_EXPIRED} (its renewal window has closed)"),
        };
    }
    // Expired, but perfectly renewable. Quotos just could not do it here.
    // Saying "sign in again" would be a lie, since the account is signed
    // in.
    FetchError::CredentialStale {
        message: "This account's access token has expired and Quotos couldn't renew it here. Use Claude Code for this account once and Quotos will pick it up."
            .to_string(),
    }
}

struct HttpResult {
    status: u16,
    body: serde_json::Value,
    retry_after_secs: Option<u64>,
    /// Stamped the moment the response arrived, before the body was read.
    /// See [`UsageRead::fetched_at`] for why the placement is load-bearing.
    fetched_at: String,
}

async fn get_json(
    client: &reqwest::Client,
    url: &str,
    token: &str,
) -> Result<HttpResult, FetchError> {
    let resp = client
        .get(url)
        .bearer_auth(token)
        .header("anthropic-beta", "oauth-2025-04-20")
        .header("User-Agent", USER_AGENT)
        .header("Content-Type", "application/json")
        .send()
        .await
        .map_err(|e| FetchError::Network {
            message: e.to_string(),
        })?;

    // The server produced its answer no later than this. Anything written
    // after it, such as a statusline feed line, is genuinely fresher.
    let fetched_at = chrono::Utc::now().to_rfc3339();
    let status = resp.status().as_u16();
    let retry_after_secs = resp
        .headers()
        .get("retry-after")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok());
    let body = match resp.json::<serde_json::Value>().await {
        Ok(body) => body,
        // A 200 whose body cannot be read, whether from a mid-body reset,
        // a timeout, or a proxy's HTML error page, is a failed read, full
        // stop. Swallowing it as Null would be indistinguishable from a
        // genuine "no limits to report" answer, a healthy-looking read
        // that silently wipes every window.
        Err(e) if status == 200 => {
            return Err(FetchError::Network {
                message: format!("The provider's answer couldn't be read: {e}"),
            });
        }
        // A non-200 answer is classified by its status alone. Its body is
        // never consumed, so an unreadable one changes nothing.
        Err(_) => serde_json::Value::Null,
    };
    Ok(HttpResult {
        status,
        body,
        retry_after_secs,
        fetched_at,
    })
}

/// Non-2xx statuses that are not 401 or 429. Kept separate so a 403 can
/// never be quietly folded into "needs sign-in". A refusal is not a
/// missing authentication.
fn map_unexpected_status(status: u16) -> FetchError {
    match status {
        403 => FetchError::Other {
            message: "The provider refused this request (HTTP 403).".to_string(),
        },
        other => FetchError::Network {
            message: format!("The provider answered with HTTP {other}."),
        },
    }
}

fn rate_limited(retry_after_secs: Option<u64>) -> FetchError {
    FetchError::RateLimited {
        retry_after_secs: retry_after_secs.unwrap_or(300),
    }
}

/// A completed usage read: the payload plus the moment its HTTP response
/// arrived, `fetched_at`. See "The statusline feed" in claude-provider.md
/// for why that moment, not snapshot-assembly time, is load-bearing.
pub struct UsageRead {
    pub body: serde_json::Value,
    pub fetched_at: String,
}

/// Fetches `/api/oauth/usage` for a config dir.
///
/// The read follows three rules:
///  1. Renew before spending a request whenever the stored token is at or
///     past its own expiry. Claude Code's tokens live roughly 8 hours, so
///     this is the ordinary case, and handling it locally means an
///     aged-out token never reaches the user as a 401 at all.
///  2. On an unexpected 401, renew once, retrying only if the credential
///     actually changed. See "Sign-in recovery" in claude-provider.md.
///  3. Reserve one budget slot per real request, not per read attempt.
pub async fn fetch_usage(
    client: &reqwest::Client,
    config_dir: &Path,
    budget: &dyn RequestBudget,
) -> Result<UsageRead, FetchError> {
    let mut credential = read_credential(config_dir)?;
    let mut renewed_this_read = false;
    read_trace!(
        "{}: credential found, access token {}, refresh {}",
        config_dir.display(),
        match credential.expires_at_ms {
            Some(at) => format!("expires in {}s", (at - now_ms()) / 1000),
            None => "has no recorded expiry".to_string(),
        },
        if credential.refresh_usable(now_ms()) {
            "usable"
        } else {
            "unusable"
        },
    );

    if credential.access_expired(now_ms(), EXPIRY_MARGIN_MS) {
        match renew_credential_async(config_dir, &credential).await {
            Some(renewed) => {
                read_trace!(
                    "{}: renewed the credential before spending a request",
                    config_dir.display()
                );
                credential = renewed;
                renewed_this_read = true;
            }
            None => read_trace!("{}: renewal changed nothing", config_dir.display()),
        }
    }

    budget.reserve().map_err(|secs| rate_limited(Some(secs)))?;
    let first = get_json(client, USAGE_URL, &credential.access_token).await?;
    read_trace!(
        "{}: /usage answered HTTP {}",
        config_dir.display(),
        first.status
    );
    match first.status {
        200 => {
            return Ok(UsageRead {
                body: first.body,
                fetched_at: first.fetched_at,
            });
        }
        429 => return Err(rate_limited(first.retry_after_secs)),
        401 => {}
        other => return Err(map_unexpected_status(other)),
    }

    if renewed_this_read {
        // The credential was just renewed, and the provider still refuses
        // it. That is a genuine expired sign-in, and retrying would only
        // spend another request to be told the same thing.
        return Err(FetchError::Unauthorized {
            message: format!("{SIGN_IN_EXPIRED} (refused immediately after a successful renewal)"),
        });
    }

    let Some(renewed) = renew_credential_async(config_dir, &credential).await else {
        let verdict = classify_unrenewable(&credential, now_ms());
        read_trace!(
            "{}: 401 and nothing renewed it, {:?}",
            config_dir.display(),
            verdict
        );
        return Err(verdict);
    };

    budget.reserve().map_err(|secs| rate_limited(Some(secs)))?;
    let second = get_json(client, USAGE_URL, &renewed.access_token).await?;
    read_trace!(
        "{}: /usage retry answered HTTP {}",
        config_dir.display(),
        second.status
    );
    match second.status {
        200 => Ok(UsageRead {
            body: second.body,
            fetched_at: second.fetched_at,
        }),
        429 => Err(rate_limited(second.retry_after_secs)),
        401 => Err(FetchError::Unauthorized {
            message: format!("{SIGN_IN_EXPIRED} (refused after renewing it)"),
        }),
        other => Err(map_unexpected_status(other)),
    }
}

/// Fetches `/api/oauth/profile` for account labeling. Best effort: the
/// profile is a nice-to-have for the label, so callers should tolerate
/// `None`.
///
/// Deliberately outside the request budget, and deliberately fetched at
/// most once per account per app run, since the caller caches it. The
/// fixed one-read-per-minute cadence already consumes the whole
/// 5-per-300s allowance, so charging this call too would make the limiter
/// refuse a scheduled read every launch. One un-budgeted request per
/// account per run is a bounded, documented overshoot.
pub async fn fetch_profile(
    client: &reqwest::Client,
    config_dir: &Path,
) -> Option<serde_json::Value> {
    let credential = read_credential(config_dir).ok()?;
    let result = get_json(client, PROFILE_URL, &credential.access_token)
        .await
        .ok()?;
    if result.status == 200 {
        Some(result.body)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    /// A throwaway home directory under the OS temp dir, cleaned up on drop.
    /// Avoids pulling in a `tempfile` dependency for one test module.
    struct TempHome {
        path: PathBuf,
    }

    impl TempHome {
        fn new() -> Self {
            let count = COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "quotos-discover-test-{}-{count}",
                std::process::id()
            ));
            std::fs::create_dir_all(&path).expect("create temp home");
            Self { path }
        }

        fn mkdir(&self, name: &str) -> PathBuf {
            let p = self.path.join(name);
            std::fs::create_dir_all(&p).expect("create temp subdir");
            p
        }
    }

    impl Drop for TempHome {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }

    /// Exactly two Keychain-backed config dirs must yield exactly two
    /// subscriptions. A plain folder that merely matches the `.claude-*`
    /// naming pattern must never appear, because it resolves no
    /// credential.
    #[test]
    fn only_credentialed_config_dirs_become_subscriptions() {
        let home = TempHome::new();
        let claude = home.mkdir(".claude");
        let team = home.mkdir(".claude-team");
        home.mkdir(".claude-shared"); // no credential, must be excluded
        home.mkdir(".not-claude-at-all"); // wrong naming pattern entirely

        let credentialed: HashSet<PathBuf> = [claude.clone(), team.clone()].into_iter().collect();
        let found = discover_accounts_in(&home.path, |dir| credentialed.contains(dir));

        let mut ids: Vec<String> = found.iter().map(|a| a.id.clone()).collect();
        ids.sort();
        assert_eq!(
            ids,
            vec![
                "claude:claude".to_string(),
                "claude:claude-team".to_string()
            ]
        );
    }

    /// A directory in the right place with no credential behind it must
    /// not become a subscription, full stop.
    #[test]
    fn plain_folder_without_credential_is_not_a_subscription() {
        let home = TempHome::new();
        home.mkdir(".claude-shared");

        let found = discover_accounts_in(&home.path, |_| false);

        assert!(
            found.is_empty(),
            "a folder in a certain place is not a subscription"
        );
    }

    /// A credentialed default `~/.claude` alone is still discovered.
    #[test]
    fn default_dir_alone_is_discovered_when_credentialed() {
        let home = TempHome::new();
        home.mkdir(".claude");

        let found = discover_accounts_in(&home.path, |_| true);

        assert_eq!(found.len(), 1);
        assert_eq!(found[0].id, "claude:claude");
    }

    /// No `.claude*` directories at all: no subscriptions, no panic.
    #[test]
    fn empty_home_yields_nothing() {
        let home = TempHome::new();
        let found = discover_accounts_in(&home.path, |_| true);
        assert!(found.is_empty());
    }

    /// See `claude_config_dir_env`'s doc comment for why this split matters.
    #[test]
    fn the_default_config_dir_must_not_set_claude_config_dir() {
        let home = PathBuf::from("/Users/someone");
        assert_eq!(claude_config_dir_env(&home, &home.join(".claude")), None);
    }

    /// Any other config dir is a genuinely separate account and does need
    /// the variable.
    #[test]
    fn a_sibling_config_dir_still_sets_claude_config_dir() {
        let home = PathBuf::from("/Users/someone");
        let team = home.join(".claude-team");
        assert_eq!(claude_config_dir_env(&home, &team), Some(team));
    }

    /// The same default and sibling split the keychain lookup already
    /// relies on, kept in one place so the two can never disagree.
    #[test]
    fn keychain_service_matches_the_default_config_dir_split() {
        let home = PathBuf::from("/Users/someone");
        assert_eq!(
            keychain_service_in(&home, &home.join(".claude")),
            "Claude Code-credentials"
        );
        let sibling = keychain_service_in(&home, &home.join(".claude-team"));
        assert!(
            sibling.starts_with("Claude Code-credentials-"),
            "got {sibling}"
        );
        assert_ne!(sibling, "Claude Code-credentials");
    }

    fn credential(
        expires_at_ms: Option<i64>,
        refresh_expires_at_ms: Option<i64>,
        has_refresh_token: bool,
    ) -> StoredCredential {
        StoredCredential {
            access_token: "token".to_string(),
            expires_at_ms,
            refresh_expires_at_ms,
            has_refresh_token,
        }
    }

    /// An access token that simply aged out, with a refresh token that is
    /// still perfectly valid, is not an expired sign-in. The account is
    /// signed in, and Claude Code renews it on its own next use.
    #[test]
    fn an_aged_out_token_with_a_valid_refresh_token_is_never_an_expired_sign_in() {
        let now = 1_000_000_000_000;
        let stale = credential(Some(now - 60_000), Some(now + 30 * 86_400_000), true);
        match classify_unrenewable(&stale, now) {
            FetchError::CredentialStale { message } => {
                assert!(
                    !message.to_lowercase().contains("sign in again"),
                    "got {message}"
                );
            }
            other => panic!("expected CredentialStale, got {other:?}"),
        }
    }

    /// The genuinely signed-out case still reports as such: nothing left
    /// that could renew the credential without a new sign-in.
    #[test]
    fn an_expired_token_with_no_usable_refresh_is_an_expired_sign_in() {
        let now = 1_000_000_000_000;
        let dead = credential(Some(now - 60_000), Some(now - 10_000), true);
        assert!(matches!(
            classify_unrenewable(&dead, now),
            FetchError::Unauthorized { .. }
        ));

        let no_refresh = credential(Some(now - 60_000), None, false);
        assert!(matches!(
            classify_unrenewable(&no_refresh, now),
            FetchError::Unauthorized { .. }
        ));
    }

    /// A token the provider refuses while it is still current by its own
    /// metadata really is a dead sign-in (revoked, or signed out elsewhere).
    #[test]
    fn a_current_token_the_provider_refuses_is_an_expired_sign_in() {
        let now = 1_000_000_000_000;
        let current = credential(Some(now + 3_600_000), Some(now + 86_400_000), true);
        assert!(matches!(
            classify_unrenewable(&current, now),
            FetchError::Unauthorized { .. }
        ));
    }

    /// A credential with no recorded expiry must never be assumed expired.
    /// Guessing here is what produces false sign-in warnings.
    #[test]
    fn a_credential_without_an_expiry_is_not_treated_as_expired() {
        let now = 1_000_000_000_000;
        assert!(!credential(None, None, true).access_expired(now, EXPIRY_MARGIN_MS));
    }

    /// The margin exists so a token that expires seconds from now is renewed
    /// before a request is spent on it, not after it 401s.
    #[test]
    fn a_token_expiring_within_the_margin_counts_as_expired() {
        let now = 1_000_000_000_000;
        let nearly = credential(Some(now + EXPIRY_MARGIN_MS / 2), None, true);
        assert!(nearly.access_expired(now, EXPIRY_MARGIN_MS));
        assert!(!nearly.access_expired(now, 0));
    }

    /// The shape Claude Code actually stores: epoch **milliseconds**.
    #[test]
    fn parses_the_stored_credential_shape() {
        let raw = r#"{"claudeAiOauth":{"accessToken":"a","refreshToken":"r",
            "expiresAt":1786853721000,"refreshTokenExpiresAt":1789000000000,
            "subscriptionType":"max"}}"#;
        let parsed = parse_credential(raw).expect("parses");
        assert_eq!(parsed.access_token, "a");
        assert_eq!(parsed.expires_at_ms, Some(1786853721000));
        assert_eq!(parsed.refresh_expires_at_ms, Some(1789000000000));
        assert!(parsed.has_refresh_token);
    }

    /// Seconds and RFC 3339 are tolerated rather than silently read as a
    /// wrong instant, which would misclassify every read.
    #[test]
    fn tolerates_other_timestamp_encodings() {
        assert_eq!(
            epoch_ms(Some(&serde_json::json!(1786831321u64))),
            Some(1786831321000)
        );
        assert_eq!(
            epoch_ms(Some(&serde_json::json!("2026-08-15T22:02:01Z"))),
            Some(1786831321000)
        );
        assert_eq!(epoch_ms(Some(&serde_json::json!(null))), None);
        assert_eq!(epoch_ms(None), None);
    }

    #[test]
    fn a_blob_without_an_access_token_is_not_a_credential() {
        assert!(parse_credential(r#"{"claudeAiOauth":{"refreshToken":"r"}}"#).is_none());
        assert!(parse_credential(r#"{"claudeAiOauth":{"accessToken":""}}"#).is_none());
        assert!(parse_credential("not json at all").is_none());
    }

    /// A 403 is a refusal, not a missing authentication. It must never
    /// travel down the sign-in path.
    #[test]
    fn a_403_is_not_an_authentication_problem() {
        assert!(matches!(
            map_unexpected_status(403),
            FetchError::Other { .. }
        ));
        assert!(matches!(
            map_unexpected_status(500),
            FetchError::Network { .. }
        ));
        assert!(matches!(
            map_unexpected_status(418),
            FetchError::Network { .. }
        ));
    }

    /// The CLI is looked up, never assumed to be on `PATH`: a Finder- or
    /// Dock-launched app inherits no `PATH` at all, so children must be
    /// handed a usable one.
    #[test]
    fn child_path_always_contains_the_system_directories() {
        let path = child_path_including(Some(Path::new("/somewhere/bin")));
        let dirs: Vec<PathBuf> = std::env::split_paths(&path).collect();
        assert_eq!(
            dirs.first().map(PathBuf::as_path),
            Some(Path::new("/somewhere/bin"))
        );
        for required in ["/usr/bin", "/bin"] {
            assert!(
                dirs.iter().any(|d| d == Path::new(required)),
                "missing {required} in {dirs:?}"
            );
        }
    }

    #[test]
    fn well_known_cli_locations_are_derived_from_the_running_home() {
        let locations = well_known_cli_locations(Path::new("/Users/someone"));
        assert!(locations.contains(&PathBuf::from("/Users/someone/.local/bin/claude")));
        assert!(locations.iter().all(|p| p.file_name().unwrap() == "claude"));
    }

    /// See `UsageRead`'s doc comment for why this timing is load-bearing.
    /// The server here delays its body by 400ms, so the stamp must land
    /// well inside that window rather than after it.
    #[test]
    fn fetched_at_marks_response_arrival_not_body_completion() {
        use std::io::{Read as _, Write as _};

        const BODY_DELAY: Duration = Duration::from_millis(400);
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind test server");
        let addr = listener.local_addr().expect("test server addr");

        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut request = [0u8; 4096];
            let _ = stream.read(&mut request);
            let body = br#"{"ok":true}"#;
            let head = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n",
                body.len()
            );
            stream.write_all(head.as_bytes()).expect("write head");
            stream.flush().expect("flush head");
            let headers_sent = chrono::Utc::now();
            std::thread::sleep(BODY_DELAY);
            stream.write_all(body).expect("write body");
            headers_sent
        });

        let client = reqwest::Client::new();
        let result = tauri::async_runtime::block_on(get_json(
            &client,
            &format!("http://{addr}/"),
            "test-token",
        ))
        .expect("get_json against the local server");
        let headers_sent = server.join().expect("server thread");

        assert_eq!(result.status, 200);
        assert_eq!(result.body, serde_json::json!({"ok": true}));
        let fetched_at = chrono::DateTime::parse_from_rfc3339(&result.fetched_at)
            .expect("fetched_at parses as RFC 3339")
            .with_timezone(&chrono::Utc);
        let lag = fetched_at - headers_sent;
        assert!(
            lag < chrono::Duration::milliseconds(300),
            "fetched_at lags response arrival by {lag}, stamped after the body was read?"
        );
    }

    /// Serve one canned HTTP response on a local socket and run `get_json`
    /// against it.
    fn get_json_against(status_line: &str, body: &[u8]) -> Result<HttpResult, FetchError> {
        use std::io::{Read as _, Write as _};

        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind test server");
        let addr = listener.local_addr().expect("test server addr");
        let response = format!(
            "HTTP/1.1 {status_line}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n",
            body.len()
        );
        let body = body.to_vec();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut request = [0u8; 4096];
            let _ = stream.read(&mut request);
            stream.write_all(response.as_bytes()).expect("write head");
            stream.write_all(&body).expect("write body");
        });
        let client = reqwest::Client::new();
        let result = tauri::async_runtime::block_on(get_json(
            &client,
            &format!("http://{addr}/"),
            "test-token",
        ));
        server.join().expect("server thread");
        result
    }

    /// A 200 whose body is not JSON, such as a proxy's HTML error page or
    /// a truncated stream, must be a failed read. Treating it as `Ok(Null)`
    /// would read as a healthy "no limits reported yet" to the whole
    /// pipeline, wiping every window and status item digit while still bumping
    /// lastReadAt. A failed read instead lets the frontend's
    /// prior-good-data logic say "behind" honestly.
    #[test]
    fn a_200_with_an_unreadable_body_is_a_failed_read_not_an_empty_one() {
        let result = get_json_against("200 OK", b"<html>gateway error</html>");
        match result {
            Err(FetchError::Network { message }) => {
                assert!(
                    message.contains("couldn't be read"),
                    "unexpected message: {message}"
                );
            }
            Err(other) => panic!("expected a Network error, got {other:?}"),
            Ok(ok) => panic!("expected a Network error, got an HTTP {} answer", ok.status),
        }
    }

    /// The deliberate asymmetry of the test above. A non-200 answer is
    /// classified by its status alone, and its body is never consumed, so
    /// an HTML-bodied 401 must still reach the 401 handling rather than
    /// become a network error that hides the real verdict.
    #[test]
    fn a_non_200_with_an_unreadable_body_still_classifies_by_status() {
        let result = get_json_against("401 Unauthorized", b"<html>denied</html>")
            .expect("a 401 is a classified answer, not a transport failure");
        assert_eq!(result.status, 401);
        assert_eq!(result.body, serde_json::Value::Null);
    }
}
