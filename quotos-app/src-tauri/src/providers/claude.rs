//! Claude Code provider adapter: Keychain-backed OAuth token, the
//! undocumented-but-first-party `/api/oauth/usage` + `/api/oauth/profile`
//! endpoints. See `data/quotos-source-s1/report.md` in the project for how
//! this was verified.

use std::path::{Path, PathBuf};
use std::process::Command;

use unicode_normalization::UnicodeNormalization;

use super::{AccountDescriptor, FetchError};

const USER_AGENT: &str = "claude-code/2.1.227";
const USAGE_URL: &str = "https://api.anthropic.com/api/oauth/usage";
const PROFILE_URL: &str = "https://api.anthropic.com/api/oauth/profile";

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

/// Scan the home directory for Claude Code config directories: the default
/// `~/.claude` plus any `~/.claude-<name>` sibling (e.g. `~/.claude-team`).
/// This is the "scan the machine" step of the add-subscription flow for the
/// Claude provider.
pub fn discover_accounts() -> Vec<AccountDescriptor> {
    let Some(home) = home_dir() else { return vec![] };
    let mut found = vec![];

    let default_dir = home.join(".claude");
    if default_dir.is_dir() {
        found.push(AccountDescriptor {
            id: "claude:claude".to_string(),
            provider: "claude".to_string(),
            config_dir: default_dir.to_string_lossy().to_string(),
        });
    }

    if let Ok(entries) = std::fs::read_dir(&home) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name == ".claude" || !name.starts_with(".claude-") {
                continue;
            }
            let path = entry.path();
            if !path.is_dir() {
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

/// Keychain service name for a config directory, per the mechanism verified
/// in the research report: the default `~/.claude` uses the bare service
/// name; any other config dir is `Claude Code-credentials-<sha256[:8]>` of
/// the NFC-normalised absolute path.
fn keychain_service_for_config_dir(config_dir: &Path) -> String {
    let is_default = home_dir()
        .map(|h| h.join(".claude") == config_dir)
        .unwrap_or(false);
    if is_default {
        return "Claude Code-credentials".to_string();
    }
    let normalized: String = config_dir.to_string_lossy().nfc().collect();
    let digest = sha2::Sha256::digest_str(&normalized);
    format!("Claude Code-credentials-{}", &digest[..8])
}

// Tiny local helper so we don't need to pull in a whole hex crate for 32 bytes.
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

/// Read the OAuth access token for a config dir from the macOS Keychain.
/// Never logs the credential blob or the token itself.
fn read_access_token(config_dir: &Path) -> Result<String, FetchError> {
    let service = keychain_service_for_config_dir(config_dir);
    let user = std::env::var("USER").map_err(|_| FetchError::NotConnected {
        message: "no $USER in environment".to_string(),
    })?;

    let output = Command::new("security")
        .args(["find-generic-password", "-s", &service, "-a", &user, "-w"])
        .output()
        .map_err(|e| FetchError::NotConnected {
            message: format!("could not invoke security(1): {e}"),
        })?;

    if !output.status.success() {
        return Err(FetchError::NotConnected {
            message: "no Claude Code credentials in the macOS Keychain for this account"
                .to_string(),
        });
    }

    let raw = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let parsed: serde_json::Value = serde_json::from_str(&raw).map_err(|_| FetchError::NotConnected {
        message: "Keychain credential was not the expected JSON shape".to_string(),
    })?;
    let token = parsed
        .pointer("/claudeAiOauth/accessToken")
        .and_then(|v| v.as_str())
        .ok_or_else(|| FetchError::NotConnected {
            message: "Keychain credential had no access token".to_string(),
        })?;
    Ok(token.to_string())
}

/// Zero-cost-to-quota way to force Claude Code to refresh a stale access
/// token: any CLI invocation refreshes-and-writes-back the stored token.
/// `claude mcp list` does no inference. Best-effort — if the `claude`
/// binary is missing this just fails quietly and the retry will still be
/// attempted (and will likely still 401).
fn refresh_token_via_cli(config_dir: &Path) {
    let _ = Command::new("claude")
        .args(["mcp", "list"])
        .env("CLAUDE_CONFIG_DIR", config_dir)
        .output();
}

struct HttpResult {
    status: u16,
    body: serde_json::Value,
    retry_after_secs: Option<u64>,
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

    let status = resp.status().as_u16();
    let retry_after_secs = resp
        .headers()
        .get("retry-after")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok());
    let body = resp.json::<serde_json::Value>().await.unwrap_or(serde_json::Value::Null);
    Ok(HttpResult {
        status,
        body,
        retry_after_secs,
    })
}

/// Fetch `/api/oauth/usage` for a config dir, handling the documented 401
/// refresh-then-retry-once flow. Does not itself rate-limit — the caller
/// (the Tauri command layer) owns the 5-per-300s budget so it can be shared
/// across the usage and profile calls for the same account.
pub async fn fetch_usage(
    client: &reqwest::Client,
    config_dir: &Path,
) -> Result<serde_json::Value, FetchError> {
    let token = read_access_token(config_dir)?;
    let first = get_json(client, USAGE_URL, &token).await?;

    if first.status == 200 {
        return Ok(first.body);
    }
    if first.status == 429 {
        return Err(FetchError::RateLimited {
            retry_after_secs: first.retry_after_secs.unwrap_or(300),
        });
    }
    if first.status == 401 {
        refresh_token_via_cli(config_dir);
        let token = read_access_token(config_dir)?;
        let second = get_json(client, USAGE_URL, &token).await?;
        return match second.status {
            200 => Ok(second.body),
            429 => Err(FetchError::RateLimited {
                retry_after_secs: second.retry_after_secs.unwrap_or(300),
            }),
            401 => Err(FetchError::Unauthorized {
                message: "still unauthorized after refreshing the credential".to_string(),
            }),
            other => Err(FetchError::Network {
                message: format!("unexpected HTTP {other} from /api/oauth/usage"),
            }),
        };
    }
    Err(FetchError::Network {
        message: format!("unexpected HTTP {} from /api/oauth/usage", first.status),
    })
}

/// Fetch `/api/oauth/profile` for account labelling. Best-effort: profile
/// is a "nice to have" for the label, so callers should tolerate `None`.
pub async fn fetch_profile(
    client: &reqwest::Client,
    config_dir: &Path,
) -> Option<serde_json::Value> {
    let token = read_access_token(config_dir).ok()?;
    let result = get_json(client, PROFILE_URL, &token).await.ok()?;
    if result.status == 200 {
        Some(result.body)
    } else {
        None
    }
}
