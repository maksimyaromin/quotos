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
    /// R3-4: the stored credential is present and its *refresh* half is
    /// still valid, but its access token has expired and Quotos could not
    /// get it renewed on this machine. This is a local problem, not an
    /// expired sign-in — conflating the two is what made Quotos tell the
    /// captain his working account needed signing in. Never say "sign-in
    /// expired" for this.
    CredentialStale { message: String },
    /// Provider asked us to slow down. Not an error — resolves on its own.
    RateLimited { retry_after_secs: u64 },
    /// Transport-level failure (offline, DNS, timeout, non-JSON body, etc).
    Network { message: String },
    Other { message: String },
}

/// R3-4: one reservation against the account's shared request budget, per
/// *real* HTTP request. Passed into the provider rather than taken once by
/// the caller, because only the provider knows how many requests one read
/// actually costs — the previous "one reservation per read attempt" under-
/// counted the 401 refresh-and-retry path by a factor of two, which is how
/// Quotos ended up hard-throttled by the provider's own 429 while its own
/// limiter still believed it was under budget.
pub trait RequestBudget: Send + Sync {
    /// `Ok(())` reserves one request; `Err(retry_after_secs)` means the
    /// account is at capacity and nothing was reserved.
    fn reserve(&self) -> Result<(), u64>;
}
