pub mod claude;

use serde::Serialize;

/// `provider` is a stable slug, such as "claude". See "The provider seam"
/// in architecture.md for what adding a second provider touches.
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
    /// When the usage HTTP response arrived, never when this snapshot was
    /// assembled. See "The statusline feed" in claude-provider.md for why
    /// a later stamp silently disables the freshest-wins comparison.
    pub fetched_at: String,
    pub usage: serde_json::Value,
    pub profile: Option<serde_json::Value>,
    /// The zero-cost Claude Code statusline feed's most recent reading for
    /// this config dir, if any. `None` covers "never opted in", "no
    /// interactive session has fed it yet", and "the feed file is stale or
    /// unreadable" identically. See `statusline.rs`'s `read_feed`. The
    /// frontend's provider adapter reconciles this against `usage` itself,
    /// where the freshest reading wins. See
    /// `providers/claude/statusline-merge.ts`.
    pub statusline: Option<crate::statusline::StatuslineFeedDto>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FetchError {
    /// No credential could be found or read for this account at all.
    NotConnected {
        message: String,
    },
    /// The sign-in itself has ended. See "Sign-in recovery" in
    /// claude-provider.md.
    Unauthorized {
        message: String,
    },
    /// The access token merely aged out and Quotos could not renew it on
    /// this machine, a local problem rather than an expired sign-in. See
    /// "Sign-in recovery" in claude-provider.md.
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
