//! Nothing here knows a window or a status item exists. That layer stays in
//! `shell.rs`, which keeps "what we know about accounts" and "how the
//! panel shows it" separately readable.

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

/// This command is `async` purely for its threading effect; it never
/// awaits anything in its body. It forks a `security(1)` process per
/// candidate config dir (see `providers/claude.rs`), and a synchronous
/// command would run that on the main thread inline with the IPC call
/// (see "Tauri commands and the main thread" in platform-constraints.md),
/// stalling the UI for however long the Keychain takes, which lands
/// exactly when the user opens the Subscriptions screen. The same
/// reasoning makes the two `tracked_store` commands below `async` as well,
/// since one of them calls `fsync`.
#[tauri::command(async)]
pub(crate) fn list_accounts() -> Vec<AccountDescriptor> {
    providers::claude::discover_accounts()
}

/// Binds the shared per-account limiter to one account, so a provider can
/// reserve a slot per real request without knowing how the budget is
/// stored. See `providers::RequestBudget`.
struct AccountBudget<'a> {
    limiter: &'a RateLimiter,
    account_id: &'a str,
}

impl providers::RequestBudget for AccountBudget<'_> {
    fn reserve(&self) -> Result<(), u64> {
        self.limiter.try_acquire(self.account_id)
    }
}

/// The one real place a network attempt happens. The `fetch_snapshot`
/// command for a manual refresh and the scheduler loop for an automatic
/// refresh both call this, so both go through the same rate-limit
/// reservation and profile cache. `scheduler.rs`'s module doc explains why
/// that sharing is also what makes a manual refresh reset the account's
/// one-minute clock, with no separate wiring needed.
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

    // See claude-provider.md for the 5-per-300s budget this reserves against.
    let budget = AccountBudget {
        limiter: &state.rate_limiter,
        account_id,
    };

    let path = PathBuf::from(config_dir);
    let usage = providers::claude::fetch_usage(&state.http, &path, &budget).await?;
    let profile = cached_profile(state, account_id, &path).await;

    // Read after the usage fetch, deliberately: a feed line written while
    // that fetch was in flight is newer than usage.fetched_at and can win
    // the frontend's freshest-wins reconciliation. See
    // `statusline::read_feed`'s doc comment for what `None` covers.
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

/// A profile changes rarely enough that one read per account per process
/// lifetime is enough; every later fetch reuses it instead of spending a
/// second Keychain read and HTTP call on data that has not changed.
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
    // Every attempt, manual or scheduled, resets this account's one-minute
    // clock. See scheduler.rs.
    state
        .scheduler
        .mark_attempted(&account_id, extract_retry_after(&result));
    result
}

#[tauri::command(async)]
pub(crate) fn load_tracked(state: tauri::State<'_, AppState>) -> Vec<TrackedAccount> {
    state.tracked_store.list()
}

/// This command runs off the main thread. See `list_accounts` for why.
/// That matters most here: `Store::save` is a temp-file write, an `fsync`,
/// and a rename, deliberate for durability. See `persistence.rs`. An
/// `fsync` on the main thread stalls the webview's own rendering for as
/// long as the filesystem takes. The frontend calls this on every
/// membership, label, or pin change; `persistence.ts` compares against the
/// last saved value first, so an unrelated state patch does not trigger a
/// redundant write.
#[tauri::command(async)]
pub(crate) fn save_tracked(
    state: tauri::State<'_, AppState>,
    tracked: Vec<TrackedAccount>,
) -> Result<(), String> {
    state.tracked_store.save(tracked)
}

/// Reports what is currently configured for this account's statusline.
/// The in-app opt-in offer checks this before it shows anything, so a row
/// never claims "not installed" for an account someone already pointed
/// `statusLine` at some other way. Runs off the main thread because it
/// touches the filesystem, the same reasoning as `list_accounts` and
/// `load_tracked` above.
#[tauri::command(async)]
pub(crate) fn statusline_status(
    state: tauri::State<'_, AppState>,
    config_dir: String,
) -> Result<statusline::IntegrationStatus, statusline::StatuslineError> {
    statusline::status(&state.statusline_root, &PathBuf::from(config_dir))
}

/// The explicit in-app opt-in write. Only a user's own click in the panel
/// calls this. See the write-mechanism contract in `statusline.rs`.
#[tauri::command(async)]
pub(crate) fn statusline_install(
    state: tauri::State<'_, AppState>,
    config_dir: String,
    force: bool,
) -> Result<statusline::InstallOutcome, statusline::StatuslineError> {
    statusline::install(&state.statusline_root, &PathBuf::from(config_dir), force)
}

/// Removes the integration. Restores exactly the previous `statusLine`
/// state, or clears the key if there was none, per the write-mechanism
/// contract in `statusline.rs`.
#[tauri::command(async)]
pub(crate) fn statusline_remove(
    state: tauri::State<'_, AppState>,
    config_dir: String,
) -> Result<(), statusline::StatuslineError> {
    statusline::remove(&state.statusline_root, &PathBuf::from(config_dir))
}

/// What the scheduler's automatic reads report back to the frontend. The
/// same shape a manual refresh's `fetch_snapshot` result already carries,
/// pushed as an event instead of returned from an `invoke` call.
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

/// One pass over every tracked account: fetches whichever are currently
/// due and emits a `quota-refresh` event per attempt. Shared by
/// `spawn_scheduler`'s periodic loop and by `kick_scheduler`.
async fn run_due_pass(app: &tauri::AppHandle) {
    let state = app.state::<AppState>();
    // See Scheduler::begin_pass for why skipping here, rather than
    // waiting, is the correct behavior when a pass is already running.
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

/// The single native scheduler. It ticks every 5 seconds, which is cheap
/// because each tick is just a due-time comparison per tracked account
/// with no network call unless something is actually due. It runs for the
/// app's lifetime regardless of panel visibility, because a native
/// OS-level timer has no notion of a hidden webview to be throttled by.
///
/// The first periodic tick is deliberately delayed by 5 seconds instead of
/// firing immediately: every tracked account is due the moment the app
/// starts, and an immediate tick could fire and emit before the frontend
/// has mounted and subscribed to `quota-refresh`, silently losing that
/// first read because events are not queued for late subscribers.
/// `kick_scheduler` covers the real read-at-launch case; this loop's own
/// first tick is only a safety net for if that kick is somehow skipped.
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

/// Called once, right after the frontend has subscribed to
/// `quota-refresh`, so the initial read at launch is still near-instant
/// rather than waiting on `spawn_scheduler`'s delayed first tick. This
/// goes through the exact same `is_due` and `mark_attempted` bookkeeping
/// as the periodic loop, so it is a nudge to run the one scheduler sooner
/// rather than a second scheduler: whichever of this call or the loop's
/// own tick reaches a given account first makes the other a no-op.
#[tauri::command]
pub(crate) async fn kick_scheduler(app: tauri::AppHandle) {
    run_due_pass(&app).await;
}

/// Read-only introspection of the shared rate budget, for the frontend's
/// developer-only state dump. app.tsx gates the dump on
/// `import.meta.env.DEV`, so it never ships in a production build's UI.
/// The command itself is harmless either way, since it is read-only and
/// touches no credentials.
#[tauri::command]
pub(crate) fn debug_rate_limit_snapshot(
    state: tauri::State<'_, AppState>,
) -> HashMap<String, RateLimitStatus> {
    state.rate_limiter.snapshot()
}

/// Starts Claude Code's own sign-in for a broken row's account. See
/// `signin.rs`'s module doc for exactly what this does and does not do.
#[tauri::command]
pub(crate) fn start_sign_in(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    account_id: String,
    config_dir: String,
) -> Result<(), String> {
    state.sign_in.start(app, account_id, config_dir)
}

/// Relays a code pasted into the panel's own field to the waiting `claude
/// setup-token` process, exactly as if it had been typed into a real
/// terminal.
#[tauri::command]
pub(crate) fn submit_sign_in_code(
    state: tauri::State<'_, AppState>,
    account_id: String,
    code: String,
) -> Result<(), String> {
    state.sign_in.submit_code(&account_id, &code)
}

/// Cancels an in-progress sign-in. Called either as a panel action or as
/// cleanup when the row is removed mid-flow.
#[tauri::command]
pub(crate) fn cancel_sign_in(state: tauri::State<'_, AppState>, account_id: String) {
    state.sign_in.cancel(&account_id);
}

/// Called after the frontend has handled `sign-in-finished`, so a retry
/// starts clean.
#[tauri::command]
pub(crate) fn forget_sign_in(state: tauri::State<'_, AppState>, account_id: String) {
    state.sign_in.forget(&account_id);
}
