pub mod claude;

use serde::Serialize;

/// One configured account Quotos knows how to read. `provider` is a stable
/// slug ("claude") — the frontend keys UI behaviour off entity data, never
/// off this string, so a second provider is a new module plus a new
/// `AccountDescriptor` source, nothing else.
#[derive(Serialize, Clone, Debug)]
pub struct AccountDescriptor {
    pub id: String,
    pub provider: String,
    pub config_dir: String,
}

#[derive(Serialize, Clone, Debug)]
pub struct RawSnapshot {
    pub account_id: String,
    pub provider: String,
    pub config_dir: String,
    pub fetched_at: String,
    pub usage: serde_json::Value,
    pub profile: Option<serde_json::Value>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FetchError {
    /// No credential could be found/read for this account at all.
    NotConnected { message: String },
    /// Credential exists but the provider rejected it, even after the
    /// one-shot refresh-and-retry.
    Unauthorized { message: String },
    /// Provider asked us to slow down. Not an error — resolves on its own.
    RateLimited { retry_after_secs: u64 },
    /// Transport-level failure (offline, DNS, timeout, non-JSON body, etc).
    Network { message: String },
    Other { message: String },
}
