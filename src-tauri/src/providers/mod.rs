pub mod claude;

use serde::Serialize;

/// One configured account Quotos knows how to read. `provider` is a stable
/// slug, such as "claude". The frontend keys UI behavior off entity data,
/// never off this string, so a second provider is a new module plus a new
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
    /// When the usage HTTP response arrived. Never when this snapshot was
    /// assembled. The statusline merge's freshest-wins comparison runs
    /// against this, and the feed is read after the fetch, so a later
    /// stamp would make the API side always look fresher and silently
    /// disable the second source. See `claude::UsageRead`.
    pub fetched_at: String,
    pub usage: serde_json::Value,
    pub profile: Option<serde_json::Value>,
    /// The zero-cost Claude Code statusline feed's most recent reading for
    /// this config dir, if any. `None` covers "never opted in", "no
    /// interactive session has fed it yet", and "the feed file is stale or
    /// unreadable" identically. See `statusline.rs`'s `read_feed`. The
    /// frontend's provider adapter reconciles this against `usage` itself,
    /// where the freshest reading wins. See
    /// `providers/claude/statuslineMerge.ts`.
    pub statusline: Option<crate::statusline::StatuslineFeedDto>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FetchError {
    /// No credential could be found or read for this account at all.
    NotConnected {
        message: String,
    },
    /// Credential exists but the provider rejected it, even after the
    /// one-shot refresh-and-retry.
    Unauthorized {
        message: String,
    },
    /// The stored credential is present and its refresh half is still
    /// valid, but its access token has expired and Quotos could not get it
    /// renewed on this machine. This is a local problem, not an expired
    /// sign-in. Conflating the two reports a working account as needing to
    /// sign in again. Never say "sign-in expired" for this.
    CredentialStale {
        message: String,
    },
    /// The provider asked Quotos to slow down. Not an error, since it
    /// resolves on its own.
    RateLimited {
        retry_after_secs: u64,
    },
    /// A transport-level failure, such as being offline, a DNS failure, a
    /// timeout, or a non-JSON body.
    Network {
        message: String,
    },
    Other {
        message: String,
    },
}

/// One reservation against the account's shared request budget, per real
/// HTTP request. Passed into the provider rather than taken once by the
/// caller, because only the provider knows how many requests one read
/// actually costs. A single reservation per read attempt would undercount
/// the 401 refresh-and-retry path by a factor of two, leaving Quotos
/// hard-throttled by the provider's own 429 while this limiter still
/// believes it is under budget.
pub trait RequestBudget: Send + Sync {
    /// `Ok(())` reserves one request. `Err(retry_after_secs)` means the
    /// account is at capacity and nothing was reserved.
    fn reserve(&self) -> Result<(), u64>;
}
