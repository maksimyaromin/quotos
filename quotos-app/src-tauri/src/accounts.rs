//! The account-data plane: discovery, the one real fetch path, the shared
//! per-account rate budget, the native refresh scheduler, and the IPC
//! commands for the tracked list, the statusline integration and sign-in.
//! Nothing here knows a window or a tray icon exists — that layer stays in
//! `shell.rs` — which is what keeps "what we know about accounts" and "how the
//! panel shows it" separately readable.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use serde::Serialize;
use tauri::{Emitter, Manager};

use crate::persistence::TrackedAccount;
use crate::providers::{self, AccountDescriptor, FetchError, RawSnapshot};
use crate::ratelimit::{RateLimitStatus, RateLimiter};
use crate::statusline;
use crate::AppState;

/// R4-2: `async` purely for its *threading* effect, not because the body
/// awaits anything. A `#[tauri::command]` without it is `ExecutionContext::
/// Blocking` — Tauri runs it **on the main thread**, inline with the IPC — and
/// this one forks a `security(1)` process per candidate config dir (see
/// `providers/claude.rs`). On the main thread that is a UI stall of however
/// long the Keychain takes, landing exactly when the captain opens the
/// Subscriptions screen. The same reasoning applies to the two `tracked_store`
/// commands below (one of them `fsync`s).
#[tauri::command(async)]
pub(crate) fn list_accounts() -> Vec<AccountDescriptor> {
    providers::claude::discover_accounts()
}

/// R3-4: binds the shared per-account limiter to one account so a provider
/// can reserve a slot per real request without knowing anything about how
/// the budget is stored. See `providers::RequestBudget`.
struct AccountBudget<'a> {
    limiter: &'a RateLimiter,
    account_id: &'a str,
}

impl providers::RequestBudget for AccountBudget<'_> {
    fn reserve(&self) -> Result<(), u64> {
        self.limiter.try_acquire(self.account_id)
    }
}

/// The one real place a network attempt happens. Shared by the
/// `fetch_snapshot` command (manual refresh) and the scheduler loop
/// (automatic refresh) so both go through the same rate-limit reservation
/// and profile cache — see `scheduler.rs`'s module doc for why that sharing
/// is also what makes "a manual refresh resets the minute" fall out for
/// free, with no separate wiring.
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

    // The 5-per-300s budget is shared with Claude Code itself; reserving a
    // slot before each network call keeps Quotos from ever being the reason
    // the captain's own /usage view starts 429ing.
    //
    // R3-4: the reservation moved *into* the provider, one per real request.
    // Taking a single slot here for a read that could quietly make two
    // requests (the 401 refresh-and-retry) is what let Quotos spend the
    // shared allowance twice as fast as its own limiter believed, until the
    // provider itself answered 429 with an hour-long retry-after.
    let budget = AccountBudget {
        limiter: &state.rate_limiter,
        account_id,
    };

    let path = PathBuf::from(config_dir);
    let usage = providers::claude::fetch_usage(&state.http, &path, &budget).await?;

    let profile = {
        let cached = {
            let cache = state.profile_cache.lock().expect("profile cache poisoned");
            cache.get(account_id).cloned()
        };
        match cached {
            Some(p) => Some(p),
            None => {
                let fetched = providers::claude::fetch_profile(&state.http, &path).await;
                if let Some(p) = &fetched {
                    let mut cache = state.profile_cache.lock().expect("profile cache poisoned");
                    cache.insert(account_id.to_string(), p.clone());
                }
                fetched
            }
        }
    };

    // R2-4/S2: opportunistic, never budgeted — a plain file read of
    // whatever the statusline helper last wrote for this config dir, or
    // `None` if it never has (see `statusline::read_feed`'s doc comment).
    // The frontend's provider adapter is what actually reconciles this
    // against `usage` (freshest wins); this is just attaching it to the
    // snapshot both the manual and scheduled read paths already share.
    let statusline = statusline::read_feed(&state.statusline_root, &path);

    Ok(RawSnapshot {
        account_id: account_id.to_string(),
        provider: provider.to_string(),
        config_dir: config_dir.to_string(),
        fetched_at: chrono::Utc::now().to_rfc3339(),
        usage,
        profile,
        statusline,
    })
}

fn retry_after_of(result: &Result<RawSnapshot, FetchError>) -> Option<Duration> {
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
    // R2-4: every attempt — manual or scheduled — resets this account's
    // one-minute clock; see scheduler.rs.
    state
        .scheduler
        .mark_attempted(&account_id, retry_after_of(&result));
    result
}

#[tauri::command(async)]
pub(crate) fn load_tracked(state: tauri::State<'_, AppState>) -> Vec<TrackedAccount> {
    state.tracked_store.list()
}

/// R4-2: off the main thread — see `list_accounts`. This one matters most:
/// `Store::save` is a temp-file write plus an **`fsync`** plus a rename
/// (deliberately, for durability — see persistence.rs), and an `fsync` on the
/// main thread stalls the webview's own rendering for as long as the
/// filesystem takes. The frontend calls this on every membership/label/pin
/// change, which used to include every automatic read's state patch until the
/// caller learned to compare first (`persistence.ts`).
#[tauri::command(async)]
pub(crate) fn save_tracked(
    state: tauri::State<'_, AppState>,
    tracked: Vec<TrackedAccount>,
) -> Result<(), String> {
    state.tracked_store.save(tracked)
}

/// S2: what's currently configured for this account's statusline — used by
/// the in-app opt-in offer before it shows anything, so a row never claims
/// "not installed" for an account someone already pointed `statusLine` at
/// some other way. Off the main thread — this touches the filesystem, same
/// reasoning as `list_accounts`/`load_tracked` above.
#[tauri::command(async)]
pub(crate) fn statusline_status(
    state: tauri::State<'_, AppState>,
    config_dir: String,
) -> Result<statusline::IntegrationStatus, statusline::StatuslineError> {
    statusline::status(&state.statusline_root, &PathBuf::from(config_dir))
}

/// S2: the explicit in-app opt-in write — never called except from a
/// captain's own click in the panel (`useSubscriptions`-adjacent UI calls
/// this directly; see the write-mechanism contract in `statusline.rs`).
#[tauri::command(async)]
pub(crate) fn statusline_install(
    state: tauri::State<'_, AppState>,
    config_dir: String,
    force: bool,
) -> Result<statusline::InstallOutcome, statusline::StatuslineError> {
    statusline::install(&state.statusline_root, &PathBuf::from(config_dir), force)
}

/// S2: "remove integration" — restores exactly the previous `statusLine`
/// state (or clears the key), per the write-mechanism contract.
#[tauri::command(async)]
pub(crate) fn statusline_remove(
    state: tauri::State<'_, AppState>,
    config_dir: String,
) -> Result<(), statusline::StatuslineError> {
    statusline::remove(&state.statusline_root, &PathBuf::from(config_dir))
}

/// R2-4: what the scheduler's automatic reads report back to the frontend —
/// the same shape a manual refresh's `fetch_snapshot` result already
/// carries, just pushed instead of returned from an `invoke`.
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

/// One pass over every tracked account: fetch whichever are currently due,
/// emit a `quota-refresh` event per attempt. Shared by the periodic loop
/// below and by `kick_scheduler` (see its doc comment for why a frontend
/// needs to be able to trigger this directly rather than only ever waiting
/// on the timer).
async fn run_due_pass(app: &tauri::AppHandle) {
    let state = app.state::<AppState>();
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
            .mark_attempted(&account.id, retry_after_of(&result));
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

/// R2-4: the single native scheduler. Ticks every 5s (cheap: just a
/// due-time comparison per tracked account, no network unless something is
/// actually due), reads whichever tracked accounts are due for their
/// once-a-minute automatic read, and pushes each result to the frontend as
/// a `quota-refresh` event. Runs for the app's lifetime regardless of panel
/// visibility — unlike the JS `setInterval` it replaces, a native tokio
/// timer has no notion of "hidden webview" to be throttled by at all.
///
/// The first periodic tick is deliberately delayed by 5s rather than firing
/// immediately: every tracked account is "due" the moment the app starts
/// (nothing has attempted it yet), and an immediate tick could fire — and
/// emit — before the frontend has even mounted and subscribed to
/// `quota-refresh`, silently losing that first read (events aren't queued
/// for late subscribers). `kick_scheduler` below covers the real "read at
/// launch" case instead, called once the frontend is actually listening;
/// this loop's own first tick is just the safety net if that kick is ever
/// somehow skipped.
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

/// R2-4: lets the frontend trigger one due-check pass immediately instead of
/// waiting out the periodic loop's first tick — called once, right after
/// the frontend has subscribed to `quota-refresh`, so the initial read at
/// launch is still near-instant. Goes through the exact same
/// `is_due`/`mark_attempted` bookkeeping as the periodic loop, so this is a
/// nudge to run the one scheduler sooner, not a second one: whichever of
/// this or the loop's own tick gets there first for a given account is a
/// no-op for the other.
#[tauri::command]
pub(crate) async fn kick_scheduler(app: tauri::AppHandle) {
    run_due_pass(&app).await;
}

/// P7 stretch: read-only introspection of the shared rate budget, for the
/// frontend's dev-only state dump (App.tsx, gated on `import.meta.env.DEV`
/// so this never ships in a production build's UI — the command itself is
/// harmless either way, since it's read-only and touches no credentials).
#[tauri::command]
pub(crate) fn debug_rate_limit_snapshot(
    state: tauri::State<'_, AppState>,
) -> HashMap<String, RateLimitStatus> {
    state.rate_limiter.snapshot()
}

/// R2-6: starts Claude Code's own sign-in for a broken row's account. See
/// `signin.rs`'s module doc for exactly what this does and does not do —
/// in short, `claude setup-token` opens the browser and prints the
/// authorization URL itself; Quotos never touches that, it only starts the
/// process and later relays a pasted code into it.
#[tauri::command]
pub(crate) fn start_sign_in(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    account_id: String,
    config_dir: String,
) -> Result<(), String> {
    state.sign_in.start(app, account_id, config_dir)
}

/// R2-6: relays a code pasted into the panel's own field to the waiting
/// `claude setup-token` process, exactly as if it had been typed into a
/// real terminal.
#[tauri::command]
pub(crate) fn submit_sign_in_code(
    state: tauri::State<'_, AppState>,
    account_id: String,
    code: String,
) -> Result<(), String> {
    state.sign_in.submit_code(&account_id, &code)
}

/// R2-6: cancels an in-progress sign-in (panel action, or cleanup if the
/// row is removed mid-flow).
#[tauri::command]
pub(crate) fn cancel_sign_in(state: tauri::State<'_, AppState>, account_id: String) {
    state.sign_in.cancel(&account_id);
}

/// R2-6: called after the frontend has handled `sign-in-finished`, so a
/// retry starts clean.
#[tauri::command]
pub(crate) fn forget_sign_in(state: tauri::State<'_, AppState>, account_id: String) {
    state.sign_in.forget(&account_id);
}
