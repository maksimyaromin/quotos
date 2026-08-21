use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Serialize;
use tauri::{Emitter, Manager};

use crate::AppState;
use crate::idle;
use crate::persistence::TrackedAccount;
use crate::providers::{self, AccountDescriptor, FetchError, RawSnapshot};
use crate::statusline;

#[tauri::command(async)]
pub(crate) fn list_accounts() -> Vec<AccountDescriptor> {
    providers::claude::discover_accounts()
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

    let path = PathBuf::from(config_dir);
    let usage = providers::claude::fetch_usage(&state.http, &path).await?;
    let profile = cached_profile(state, account_id, &path).await;

    let statusline = statusline::read_feed(&state.statusline_root, &path);

    let snapshot = RawSnapshot {
        account_id: account_id.to_string(),
        provider: provider.to_string(),
        config_dir: config_dir.to_string(),
        fetched_at: usage.fetched_at,
        usage: usage.body,
        profile,
        statusline,
    };
    state
        .last_ok_snapshot
        .lock()
        .expect("last_ok_snapshot mutex poisoned")
        .insert(account_id.to_string(), snapshot.clone());
    Ok(snapshot)
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
pub(crate) fn statusline_enable(
    state: tauri::State<'_, AppState>,
    config_dir: String,
) -> Result<(), statusline::StatuslineError> {
    statusline::enable(&state.statusline_root, &PathBuf::from(config_dir))
}

#[tauri::command(async)]
pub(crate) fn statusline_disable(
    state: tauri::State<'_, AppState>,
    config_dir: String,
) -> Result<(), statusline::StatuslineError> {
    statusline::disable(&state.statusline_root, &PathBuf::from(config_dir))
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

    let paused = idle::should_pause_automatic_reads(
        idle::system_idle_seconds(),
        state
            .screen_locked
            .load(std::sync::atomic::Ordering::Relaxed),
        idle::AUTO_READ_PAUSE_THRESHOLD,
    );
    if state.scheduler.observe_pause(paused) {
        // Nobody was watching for at least one whole pass; treat every
        // tracked account as due again rather than waiting out whatever
        // was left of its own anchored minute.
        state.scheduler.wake_all(&live_ids);
    }
    if paused {
        return;
    }

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

/// Instant delivery for a Claude Code turn; see "The statusline feed" in
/// docs/claude-provider.md for why this pairs the fresh feed with
/// `last_ok_snapshot` instead of re-emitting a full API read.
pub(crate) fn spawn_statusline_watcher(app: tauri::AppHandle) {
    use notify::{RecursiveMode, Watcher};

    let watch_root = app.state::<AppState>().statusline_root.clone();
    let callback_app = app.clone();
    let callback_root = watch_root.clone();
    let result = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        let Ok(event) = res else { return };
        if !matches!(
            event.kind,
            notify::EventKind::Create(_) | notify::EventKind::Modify(_)
        ) {
            return;
        }
        for path in &event.paths {
            handle_statusline_feed_change(&callback_app, &callback_root, path);
        }
    });
    let Ok(mut watcher) = result else {
        eprintln!("quotos: statusline feed watcher failed to start");
        return;
    };
    if watcher
        .watch(&watch_root, RecursiveMode::Recursive)
        .is_err()
    {
        eprintln!("quotos: statusline feed watcher failed to watch {watch_root:?}");
        return;
    }
    // Leaked on purpose: the watcher must outlive this function to keep
    // observing feed writes for the lifetime of the app.
    std::mem::forget(watcher);
}

fn handle_statusline_feed_change(app: &tauri::AppHandle, app_support_dir: &Path, path: &Path) {
    let Some(slug) = statusline::feed_slug_from_path(path) else {
        return;
    };
    let state = app.state::<AppState>();
    let tracked = state.tracked_store.list();
    let Some(account) = tracked
        .iter()
        .find(|a| statusline::slug_for(&a.config_dir) == slug)
    else {
        return;
    };
    let Some(feed) = statusline::read_feed(app_support_dir, Path::new(&account.config_dir)) else {
        return;
    };
    let cached = state
        .last_ok_snapshot
        .lock()
        .expect("last_ok_snapshot mutex poisoned")
        .get(&account.id)
        .cloned();
    let Some(mut snapshot) = cached else {
        return;
    };
    snapshot.statusline = Some(feed);
    let _ = app.emit("quota-refresh", ScheduledRefreshEvent::Ok { snapshot });
}

#[tauri::command]
pub(crate) async fn kick_scheduler(app: tauri::AppHandle) {
    run_due_pass(&app).await;
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
