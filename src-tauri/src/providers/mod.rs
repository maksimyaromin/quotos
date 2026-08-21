pub mod claude;

use serde::Serialize;

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
    pub statusline: Option<crate::statusline::StatuslineFeedDto>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FetchError {
    NotConnected { message: String },
    Unauthorized { message: String },
    CredentialStale { message: String },
    RateLimited { retry_after_secs: u64 },
    Network { message: String },
    Other { message: String },
}
