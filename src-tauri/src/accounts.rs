use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Serialize;
use tauri::{Emitter, Manager};

use crate::AppState;
use crate::persistence::TrackedAccount;
use crate::providers::{self, AccountDescriptor, FetchError, RawSnapshot};
use crate::ratelimit::{RateLimitStatus, RateLimiter};
use crate::statusline;

#[tauri::command(async)]
pub(crate) fn list_accounts() -> Vec<AccountDescriptor> {
    providers::claude::discover_accounts()
}

struct AccountBudget<'a> {
    limiter: &'a RateLimiter,
    account_id: &'a str,
}

impl providers::RequestBudget for AccountBudget<'_> {
    fn reserve(&self) -> Result<(), u64> {
        self.limiter.try_acquire(self.account_id)
    }
}

async fn perform_fetch(
    state: &AppState,
    account_id: &str,
    provider: &str,
    config_dir: &str,
) -> Result<RawSnapshot, FetchError> {
    if provider != "claude" {
        return Err(FetchError::Other {
            message: format!("no adapter for provider '{provider}'"),
        });
    }

    let budget = AccountBudget {
        limiter: &state.rate_limiter,
        account_id,
    };

    let path = PathBuf::from(config_dir);
    let usage = providers::claude::fetch_usage(&state.http, &path, &budget).await?;
    let profile = cached_profile(state, account_id, &path).await;

    let statusline = statusline::read_feed(&state.statusline_root, &path);

    Ok(RawSnapshot {
        account_id: account_id.to_string(),
        provider: provider.to_string(),
        config_dir: config_dir.to_string(),
        fetched_at: usage.fetched_at,
        usage: usage.body,
        profile,
        statusline,
    })
}

async fn cached_profile(
    state: &AppState,
    account_id: &str,
    path: &Path,
) -> Option<serde_json::Value> {
    let cached = state
        .profile_cache
        .lock()
        .expect("profile cache poisoned")
        .get(account_id)
        .cloned();
    if cached.is_some() {
        return cached;
    }
    let fetched = providers::claude::fetch_profile(&state.http, path).await;
    if let Some(profile) = &fetched {
        state
            .profile_cache
            .lock()
            .expect("profile cache poisoned")
            .insert(account_id.to_string(), profile.clone());
    }
    fetched
}

fn extract_retry_after(result: &Result<RawSnapshot, FetchError>) -> Option<Duration> {
    match result {
        Err(FetchError::RateLimited { retry_after_secs }) => {
            Some(Duration::from_secs(*retry_after_secs))
        }
        _ => None,
    }
}

#[tauri::command]
pub(crate) async fn fetch_snapshot(
    state: tauri::State<'_, AppState>,
    account_id: String,
    provider: String,
    config_dir: String,
) -> Result<RawSnapshot, FetchError> {
    let result = perform_fetch(&state, &account_id, &provider, &config_dir).await;
    state
        .scheduler
        .mark_attempted(&account_id, extract_retry_after(&result));
    result
}

#[tauri::command(async)]
pub(crate) fn load_tracked(state: tauri::State<'_, AppState>) -> Vec<TrackedAccount> {
    state.tracked_store.list()
}

#[tauri::command(async)]
pub(crate) fn save_tracked(
    state: tauri::State<'_, AppState>,
    tracked: Vec<TrackedAccount>,
) -> Result<(), String> {
    state.tracked_store.save(tracked).inspect_err(|err| {
        eprintln!("quotos: saving the tracked list failed: {err}");
    })
}

#[tauri::command(async)]
pub(crate) fn statusline_status(
    state: tauri::State<'_, AppState>,
    config_dir: String,
) -> Result<statusline::IntegrationStatus, statusline::StatuslineError> {
    statusline::status(&state.statusline_root, &PathBuf::from(config_dir))
}

#[tauri::command(async)]
pub(crate) fn statusline_install(
    state: tauri::State<'_, AppState>,
    config_dir: String,
    force: bool,
) -> Result<statusline::InstallOutcome, statusline::StatuslineError> {
    statusline::install(&state.statusline_root, &PathBuf::from(config_dir), force)
}

#[tauri::command(async)]
pub(crate) fn statusline_remove(
    state: tauri::State<'_, AppState>,
    config_dir: String,
) -> Result<(), statusline::StatuslineError> {
    statusline::remove(&state.statusline_root, &PathBuf::from(config_dir))
}

#[derive(Serialize, Clone)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum ScheduledRefreshEvent {
    Ok {
        snapshot: RawSnapshot,
    },
    Err {
        account_id: String,
        error: FetchError,
    },
}

async fn run_due_pass(app: &tauri::AppHandle) {
    let state = app.state::<AppState>();
    let Some(_pass) = state.scheduler.begin_pass() else {
        return;
    };
    let tracked = state.tracked_store.list();

    let live_ids: std::collections::HashSet<String> =
        tracked.iter().map(|t| t.id.clone()).collect();
    state.scheduler.retain(&live_ids);

    for account in tracked {
        if !state.scheduler.is_due(&account.id) {
            continue;
        }
        let result =
            perform_fetch(&state, &account.id, &account.provider, &account.config_dir).await;
        state
            .scheduler
            .mark_attempted(&account.id, extract_retry_after(&result));
        let event = match result {
            Ok(snapshot) => ScheduledRefreshEvent::Ok { snapshot },
            Err(error) => ScheduledRefreshEvent::Err {
                account_id: account.id.clone(),
                error,
            },
        };
        let _ = app.emit("quota-refresh", event);
    }
}

pub(crate) fn spawn_scheduler(app: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval_at(
            tokio::time::Instant::now() + Duration::from_secs(5),
            Duration::from_secs(5),
        );
        loop {
            interval.tick().await;
            run_due_pass(&app).await;
        }
    });
}

#[tauri::command]
pub(crate) async fn kick_scheduler(app: tauri::AppHandle) {
    run_due_pass(&app).await;
}

#[tauri::command]
pub(crate) fn debug_rate_limit_snapshot(
    state: tauri::State<'_, AppState>,
) -> HashMap<String, RateLimitStatus> {
    state.rate_limiter.snapshot()
}

#[tauri::command]
pub(crate) fn start_sign_in(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    account_id: String,
    config_dir: String,
) -> Result<(), String> {
    state.sign_in.start(app, account_id, config_dir)
}

#[tauri::command]
pub(crate) fn submit_sign_in_code(
    state: tauri::State<'_, AppState>,
    account_id: String,
    code: String,
) -> Result<(), String> {
    state.sign_in.submit_code(&account_id, &code)
}

#[tauri::command]
pub(crate) fn cancel_sign_in(state: tauri::State<'_, AppState>, account_id: String) {
    state.sign_in.cancel(&account_id);
}

#[tauri::command]
pub(crate) fn forget_sign_in(state: tauri::State<'_, AppState>, account_id: String) {
    state.sign_in.forget(&account_id);
}
