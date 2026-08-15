mod panel_window;
mod persistence;
mod providers;
mod ratelimit;
mod scheduler;
mod signin;
pub mod statusline;
mod tray_render;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{TrayIconBuilder, TrayIconEvent};
use tauri::{Emitter, Manager};

use persistence::{Store, TrackedAccount};
use providers::{AccountDescriptor, FetchError, RawSnapshot};
use ratelimit::{RateLimitStatus, RateLimiter};
use scheduler::Scheduler;

struct AppState {
    http: reqwest::Client,
    rate_limiter: RateLimiter,
    profile_cache: Mutex<HashMap<String, serde_json::Value>>,
    /// Whether the panel is currently torn off into a real, freestanding
    /// window (I7). While detached, focus loss must never hide it — that is
    /// exactly the popover behaviour the captain asked to escape.
    detached: Mutex<bool>,
    /// R2-5: the tracked-subscriptions list, natively owned (see
    /// `persistence.rs` for why — reproduced first, on a real build).
    tracked_store: Store,
    /// Quotos's own app-support directory (`app.path().app_config_dir()`),
    /// computed once at setup — see `statusline.rs`, which stores the
    /// installed helper binary, per-account feed readings and install
    /// backups underneath it.
    statusline_root: PathBuf,
    /// R2-4: one automatic read per account per minute, on a native timer
    /// (see `scheduler.rs` for why — also reproduced first).
    scheduler: Scheduler,
    /// R2-6: in-progress `claude setup-token` sessions, keyed by account id.
    /// See `signin.rs`.
    sign_in: signin::SignInRegistry,
    /// Followup-1 fix: the tray icon's own physical-pixel rect, updated on
    /// every tray icon event (not just clicks). `show_panel` always has a
    /// fresh rect straight from the click event itself, but re-docking on
    /// snap-back (`set_detached`) is triggered from the panel's own header
    /// button, not a tray event, so it needs this cached value to know where
    /// to reposition to. `None` until the first tray event ever arrives.
    last_tray_rect: Mutex<Option<(f64, f64)>>,
    /// R3-1 fix: the pixel width of the tray image most recently handed to
    /// `set_icon` (see `tray_render`'s `render`/`plain_glyph_rgba` — always
    /// at the fixed "2x of an 18pt-tall image" convention, so this value / 2
    /// is the image's real width in Cocoa points, independent of the
    /// monitor's own scale factor). `compute_docked_layout` needs this to
    /// work out how much of the tray item's own measured width is macOS's
    /// own margin around the image versus the image itself — see its doc
    /// comment for why that margin, not a hand-measured constant, is what
    /// makes the glyph's on-screen center knowable rather than guessed.
    last_icon_width_px: Mutex<u32>,
    /// A11: whether the panel is currently visible — the only input to the
    /// tray's "panel open" highlight (`tray_render::render`'s `highlighted`
    /// param) that isn't already known at repaint time from `segments`
    /// alone. Flipped by `set_tray_highlighted`, called from `show_panel`/
    /// `hide_panel`/the click-away and detached-toggle hide paths.
    tray_highlighted: Mutex<bool>,
    /// A11: the segments `set_tray_status` last received, cached so
    /// toggling `tray_highlighted` (from native show/hide, which never goes
    /// through `set_tray_status` itself) can repaint with the *same* digits
    /// rather than needing the frontend to resend them on every visibility
    /// change.
    last_tray_segments: Mutex<Vec<TraySegmentDto>>,
    /// The layout the window is *supposed* to be at right now, while docked
    /// and visible (global points — see `DisplayPoints`) — `None` whenever
    /// it's hidden or detached (dragging must never fight this). A safety
    /// net, read by the debounced correction in the `WindowEvent::Moved`
    /// handler (`run`'s `setup`): anything that relocates the window while
    /// it's supposed to be docked gets undone.
    ///
    /// R3-6 originally introduced this to chase a suspected
    /// Space-transition race; R3-7 found the actual cause of the relocations
    /// it was chasing — see `DisplayPoints` (a coordinate-space unit bug that
    /// placed the window off-display or half-way across one) and
    /// `place_window_top_left_sync` (a show-before-move ordering bug). With
    /// both fixed there is normally nothing left for this to correct, and
    /// that is the point: it stays as a guard, not as the mechanism.
    docked_target: Mutex<Option<DockedLayout>>,
    /// How many `WindowEvent::Moved` events have fired so far — bumped on
    /// every one, read back by a debounced correction task to tell whether
    /// it's still the *last* one scheduled (see `last_known_position` and the
    /// `Moved` handler). A single real click was once logged relocating the
    /// window four times inside two seconds, twice to coordinates nowhere
    /// near any real monitor (`x=-2106`, `x=4712`) — values that are, note,
    /// almost exactly 2× or ½× real ones, which is the signature of the
    /// scale-factor confusion `DisplayPoints` documents rather than of an
    /// AppKit settle. Correcting on the *first* of a burst fights whatever is
    /// still in flight instead of waiting for it, so this debounces: schedule
    /// a correction, apply it only if no further `Moved` arrived before the
    /// delay elapsed.
    move_generation: Mutex<u64>,
    /// The most recent position `WindowEvent::Moved` reported, converted to
    /// global points — tracked so the debounced correction
    /// (`move_generation`) can compare against the *latest* observed
    /// position after its delay, not a value captured (and potentially
    /// already stale) at scheduling time.
    last_known_position: Mutex<(f64, f64)>,
}

/// R4-2: `async` purely for its *threading* effect, not because the body
/// awaits anything. A `#[tauri::command]` without it is `ExecutionContext::
/// Blocking` — Tauri runs it **on the main thread**, inline with the IPC — and
/// this one forks a `security(1)` process per candidate config dir (see
/// `providers/claude.rs`). On the main thread that is a UI stall of however
/// long the Keychain takes, landing exactly when the captain opens the
/// Subscriptions screen. The same reasoning applies to the two `tracked_store`
/// commands below (one of them `fsync`s).
#[tauri::command(async)]
fn list_accounts() -> Vec<AccountDescriptor> {
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
        Err(FetchError::RateLimited { retry_after_secs }) => Some(Duration::from_secs(*retry_after_secs)),
        _ => None,
    }
}

#[tauri::command]
async fn fetch_snapshot(
    state: tauri::State<'_, AppState>,
    account_id: String,
    provider: String,
    config_dir: String,
) -> Result<RawSnapshot, FetchError> {
    let result = perform_fetch(&state, &account_id, &provider, &config_dir).await;
    // R2-4: every attempt — manual or scheduled — resets this account's
    // one-minute clock; see scheduler.rs.
    state.scheduler.mark_attempted(&account_id, retry_after_of(&result));
    result
}

#[tauri::command(async)]
fn load_tracked(state: tauri::State<'_, AppState>) -> Vec<TrackedAccount> {
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
fn save_tracked(state: tauri::State<'_, AppState>, tracked: Vec<TrackedAccount>) -> Result<(), String> {
    state.tracked_store.save(tracked)
}

/// S2: what's currently configured for this account's statusline — used by
/// the in-app opt-in offer before it shows anything, so a row never claims
/// "not installed" for an account someone already pointed `statusLine` at
/// some other way. Off the main thread — this touches the filesystem, same
/// reasoning as `list_accounts`/`load_tracked` above.
#[tauri::command(async)]
fn statusline_status(
    state: tauri::State<'_, AppState>,
    config_dir: String,
) -> Result<statusline::IntegrationStatus, statusline::StatuslineError> {
    statusline::status(&state.statusline_root, &PathBuf::from(config_dir))
}

/// S2: the explicit in-app opt-in write — never called except from a
/// captain's own click in the panel (`useSubscriptions`-adjacent UI calls
/// this directly; see the write-mechanism contract in `statusline.rs`).
#[tauri::command(async)]
fn statusline_install(
    state: tauri::State<'_, AppState>,
    config_dir: String,
    force: bool,
) -> Result<statusline::InstallOutcome, statusline::StatuslineError> {
    statusline::install(&state.statusline_root, &PathBuf::from(config_dir), force)
}

/// S2: "remove integration" — restores exactly the previous `statusLine`
/// state (or clears the key), per the write-mechanism contract.
#[tauri::command(async)]
fn statusline_remove(state: tauri::State<'_, AppState>, config_dir: String) -> Result<(), statusline::StatuslineError> {
    statusline::remove(&state.statusline_root, &PathBuf::from(config_dir))
}

/// R2-4: what the scheduler's automatic reads report back to the frontend —
/// the same shape a manual refresh's `fetch_snapshot` result already
/// carries, just pushed instead of returned from an `invoke`.
#[derive(Serialize, Clone)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum ScheduledRefreshEvent {
    Ok { snapshot: RawSnapshot },
    Err { account_id: String, error: FetchError },
}

/// One pass over every tracked account: fetch whichever are currently due,
/// emit a `quota-refresh` event per attempt. Shared by the periodic loop
/// below and by `kick_scheduler` (see its doc comment for why a frontend
/// needs to be able to trigger this directly rather than only ever waiting
/// on the timer).
async fn run_due_pass(app: &tauri::AppHandle) {
    let state = app.state::<AppState>();
    let tracked = state.tracked_store.list();

    let live_ids: std::collections::HashSet<String> = tracked.iter().map(|t| t.id.clone()).collect();
    state.scheduler.retain(&live_ids);

    for account in tracked {
        if !state.scheduler.is_due(&account.id) {
            continue;
        }
        let result = perform_fetch(&state, &account.id, &account.provider, &account.config_dir).await;
        state.scheduler.mark_attempted(&account.id, retry_after_of(&result));
        let event = match result {
            Ok(snapshot) => ScheduledRefreshEvent::Ok { snapshot },
            Err(error) => ScheduledRefreshEvent::Err { account_id: account.id.clone(), error },
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
fn spawn_scheduler(app: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut interval =
            tokio::time::interval_at(tokio::time::Instant::now() + Duration::from_secs(5), Duration::from_secs(5));
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
async fn kick_scheduler(app: tauri::AppHandle) {
    run_due_pass(&app).await;
}

/// P7 stretch: read-only introspection of the shared rate budget, for the
/// frontend's dev-only state dump (App.tsx, gated on `import.meta.env.DEV`
/// so this never ships in a production build's UI — the command itself is
/// harmless either way, since it's read-only and touches no credentials).
#[tauri::command]
fn debug_rate_limit_snapshot(state: tauri::State<'_, AppState>) -> HashMap<String, RateLimitStatus> {
    state.rate_limiter.snapshot()
}

#[tauri::command]
fn hide_panel(app: tauri::AppHandle, window: tauri::WebviewWindow) {
    let _ = window.hide();
    let _ = window.emit("panel-visibility", false);
    set_tray_highlighted(&app, false);
    clear_docked_target(&app);
}

#[derive(Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
struct TraySegmentDto {
    text: String,
    color: String, // "neutral" | "amber" | "red"
}

/// Sets what's shown beside the tray glyph — the pinned-subscriptions
/// feature (R2-2). Empty `segments` clears it back to just the plain,
/// theme-tinted glyph (unless the panel is currently open — see
/// `repaint_tray_icon`, A11).
#[tauri::command]
fn set_tray_status(app: tauri::AppHandle, segments: Vec<TraySegmentDto>) -> Result<(), String> {
    let Some(tray) = app.tray_by_id("main-tray") else {
        return Ok(());
    };
    {
        // R4-2: this command runs on the main thread (it has to — it touches
        // `NSStatusItem`) and its body is a full bitmap composite plus a
        // `set_icon`. The frontend calls it from an effect keyed on the whole
        // subscription list, so it fires on *every* state change, most of
        // which leave the pinned digits byte-for-byte identical. Comparing
        // first turns those into nothing at all rather than into main-thread
        // work — and, more importantly, stops them reaching
        // `schedule_resync_after_icon_change`, which moves the open panel.
        let state = app.state::<AppState>();
        let mut last = state.last_tray_segments.lock().expect("last_tray_segments mutex poisoned");
        if *last == segments {
            return Ok(());
        }
        *last = segments;
    }
    repaint_tray_icon(&app, &tray)
}

/// A11: flips the tray icon's "panel open" highlight on or off and repaints.
/// Called from `show_panel`/`hide_panel`/the click-away and hide branches —
/// never from the frontend directly, since it's a pure reflection of native
/// window visibility, not app data.
fn set_tray_highlighted(app: &tauri::AppHandle, highlighted: bool) {
    let Some(tray) = app.tray_by_id("main-tray") else { return };
    *app.state::<AppState>().tray_highlighted.lock().expect("tray_highlighted mutex poisoned") = highlighted;
    let _ = repaint_tray_icon(app, &tray);
}

/// The one place the tray icon actually gets redrawn — shared by both of
/// this icon's independent inputs, the pinned-subscription digits
/// (`set_tray_status`, called by the frontend on data changes) and A11's
/// "panel open" highlight (`set_tray_highlighted`, called natively on
/// show/hide) — neither knows the other's current value, so this always
/// reads both fresh from `AppState` rather than taking either as a
/// parameter, and repaints with whichever combination is current.
///
/// B2/B3: the underlying `tray-icon` crate's macOS `set_title` only calls
/// `NSStatusItem`'s `setTitle` when given `Some(..)` — passing `None` is a
/// silent no-op that leaves whatever title was last set stuck on screen
/// forever (see `tray-icon` v0.24.2 `platform_impl/macos/mod.rs`). That is
/// exactly why unpinning used to leave the stale percentage frozen in the
/// tray; still always clear the title with `Some("")` below, even though
/// digits themselves are now drawn into the icon image, not the title.
///
/// R2-2: colored digits have no path through `set_title` at all (see
/// `tray_render.rs`'s module doc) — with any segments present, or A11's
/// highlight active, this drops `icon_as_template` and paints a composed
/// bitmap instead (a plain template image can't carry its own background
/// tint — see `tray_render::render`'s doc comment); with neither, it reverts
/// to the plain template glyph exactly as before.
fn repaint_tray_icon(app: &tauri::AppHandle, tray: &tauri::tray::TrayIcon) -> Result<(), String> {
    let state = app.state::<AppState>();
    let segments = state.last_tray_segments.lock().expect("last_tray_segments mutex poisoned").clone();
    let highlighted = *state.tray_highlighted.lock().expect("tray_highlighted mutex poisoned");

    tray.set_title(Some("")).map_err(|e| e.to_string())?;

    // I7: the composited image carries no text a screen reader can read —
    // keep the tray item's accessible name/tooltip current with the actual
    // pinned values (in words, with the product name) rather than leaving a
    // screen reader with nothing but the bare rendered percentage.
    let tooltip = if segments.is_empty() {
        "Quotos".to_string()
    } else {
        format!("Quotos — {}", segments.iter().map(|s| s.text.as_str()).collect::<Vec<_>>().join(" "))
    };
    tray.set_tooltip(Some(&tooltip)).map_err(|e| e.to_string())?;

    let icon_width_px = if segments.is_empty() && !highlighted {
        let (rgba, w, h) = tray_render::plain_glyph_rgba();
        tray.set_icon(Some(Image::new_owned(rgba, w, h))).map_err(|e| e.to_string())?;
        tray.set_icon_as_template(true).map_err(|e| e.to_string())?;
        w
    } else {
        let segs: Vec<tray_render::TraySegment> = segments
            .into_iter()
            .map(|s| tray_render::TraySegment {
                text: s.text,
                color: match s.color.as_str() {
                    "amber" => tray_render::TrayColor::Amber,
                    "red" => tray_render::TrayColor::Red,
                    _ => tray_render::TrayColor::Neutral,
                },
            })
            .collect();
        let (rgba, w, h) = tray_render::render(&segs, highlighted);
        tray.set_icon(Some(Image::new_owned(rgba, w, h))).map_err(|e| e.to_string())?;
        tray.set_icon_as_template(false).map_err(|e| e.to_string())?;
        w
    };
    let width_changed = {
        let mut last = state.last_icon_width_px.lock().expect("last_icon_width_px mutex poisoned");
        let changed = *last != icon_width_px;
        *last = icon_width_px;
        changed
    };

    // R3-1 fix: pinning/unpinning a subscription (or A11's highlight
    // toggling) lands here and can change the tray item's own width
    // (`tray_render::render`'s composited image grows per digit segment,
    // though the highlight itself never changes the width) — which on macOS
    // shifts the *item's own* on-screen x position too (status items lay
    // out right-to-left, so widening ours moves our own left edge left;
    // narrowing moves it back right). Nothing about that resize goes
    // through `TrayIconEvent` (there is no tray click involved), so
    // `compute_docked_layout`'s `tray_x` — captured from the last actual
    // click/hover — goes stale the instant this runs, and the beak/panel
    // silently drift off the glyph until the next real tray event. Re-dock
    // right here whenever the panel is open and attached — this is what
    // makes pin/unpin a no-op for beak position instead of a drift. See
    // `schedule_resync_after_icon_change`'s doc comment for why this can't
    // just be one synchronous `tray.rect()` call.
    //
    // R4-2: gated on the width having *actually* changed. A repaint that
    // produces the same-width image cannot have moved the item, so re-docking
    // after one is pure cost — and not cheap cost: three `tray.rect()` reads
    // and up to three `setFrameTopLeftPoint:` calls on the open panel, on the
    // main thread. The highlight toggle in particular never changes the width
    // (by construction — see `tray_render::SIDE_PAD_PX`), and it fires on
    // every single open and close.
    if width_changed {
        schedule_resync_after_icon_change(app, tray.clone());
    }
    Ok(())
}

/// Re-reads the tray item's *current* rect and, if the panel is visible and
/// still docked (never while detached — the window isn't under the icon at
/// all then), re-applies the docked position/beak-offset from it. See
/// `set_tray_status`'s call site for why this needs to exist at all: pin/
/// unpin changes the icon's width (and therefore its on-screen x) with no
/// tray click to refresh `last_tray_rect` from. Returns whether it actually
/// repositioned anything, so `schedule_resync_after_icon_change` knows
/// whether a retry is still needed.
fn resync_docked_position_after_icon_change(app: &tauri::AppHandle, tray: &tauri::tray::TrayIcon) -> Option<(f64, f64)> {
    let window = app.get_webview_window("main")?;
    if !window.is_visible().unwrap_or(false) {
        return None;
    }
    let state = app.state::<AppState>();
    let detached = state.detached.lock().map(|d| *d).unwrap_or(false);
    if detached {
        return None;
    }
    let rect = tray.rect().ok().flatten()?;
    let (tray_x, tray_y) = match rect.position {
        tauri::Position::Physical(p) => (p.x as f64, p.y as f64),
        tauri::Position::Logical(p) => (p.x, p.y),
    };
    *state.last_tray_rect.lock().expect("last_tray_rect mutex poisoned") = Some((tray_x, tray_y));
    reposition_under_tray(app, &window, tray_x, tray_y);
    Some((tray_x, tray_y))
}

/// `resync_docked_position_after_icon_change`'s single synchronous call,
/// tried on its own first, was verified live (via a temporary diagnostic
/// build, `RESULT.md`) to read a *stale* rect: right after `set_icon()`
/// changes the composited image's width, `tray.rect()` — called immediately
/// after, on the same main-thread dispatch — still reported the item's
/// *previous* width/position for one to a few runloop turns, only catching
/// up a beat later. (`set_icon`'s own main-thread dispatch guarantees the
/// image is *set*, not that `NSStatusItem`'s width-driven layout pass has
/// already run by the time the call returns — those are two different
/// things on macOS, and nothing in the crate exposes a way to force the
/// layout pass synchronously.) So one immediate attempt plus a couple of
/// short-delay retries, rather than trusting the first read: each retry
/// just re-reads and re-applies, harmless if the previous attempt already
/// landed on the right numbers. (A different, unrelated AppKit-async-layout
/// race — the window's own Space-transition relocating it after
/// `show_panel` positions it — used to be handled the same blind-timer way
/// here too; that one is now event-driven instead, see
/// `AppState.docked_target`'s doc comment for why a fixed delay didn't
/// generalize across real machines.)
fn schedule_resync_after_icon_change(app: &tauri::AppHandle, tray: tauri::tray::TrayIcon) {
    if resync_docked_position_after_icon_change(app, &tray).is_none() {
        return; // not visible/docked — nothing to correct, no retry needed either
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        for delay_ms in [30, 120] {
            tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
            resync_docked_position_after_icon_change(&app, &tray);
        }
    });
}

/// I7: tear the panel off into a real, freestanding window (`detached =
/// true`) or fold it back into a popover (`false`). Detached mode stays out
/// of the hide-on-blur path and shows up in Cmd+Tab — the captain's stated
/// need is to park it on screen and watch it while working elsewhere, which
/// a thing that vanishes on focus loss cannot do.
///
/// R3-3 fix: this used to also call `window.set_decorations(true)` while
/// detached, on the theory that a title bar was needed to make the window
/// "unmistakably a window" and draggable. That was the whole bug the captain
/// photographed: with `titleBarStyle: Overlay` (tauri.conf.json) turning
/// decorations on paints real traffic lights over the content *and* a native
/// title-bar strip macOS renders regardless of the window's own transparency,
/// which is exactly "kнопки системные" on a frame visibly bigger than the
/// 332px panel floating inset inside it. The handoff has no system chrome in
/// either state ("Кнопки нет... В отцепленном состоянии в шапке появляется
/// стрелка") — decorations must stay off always; dragging comes from
/// `App.tsx`'s own header-drag calling `startDragging()`, which needs no
/// native title bar at all. Whether decorations were the actual reason the
/// drag itself didn't respond was never isolated (the screenshot alone can't
/// tell it apart from "the frame just looks wrong"), but there is no reason
/// left to keep them and the handoff explicitly rules them out either way.
#[tauri::command]
fn set_detached(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    state: tauri::State<'_, AppState>,
    detached: bool,
) -> Result<(), String> {
    *state.detached.lock().expect("detached mutex poisoned") = detached;
    window.set_skip_taskbar(!detached).map_err(|e| e.to_string())?;
    window.set_always_on_top(true).map_err(|e| e.to_string())?;
    if detached {
        // Dragging must never fight the docked-position self-correction —
        // see `AppState.docked_target`'s doc comment.
        clear_docked_target(&app);
        // R4-1: same reason as `order_panel_front` — `set_focus()` would
        // activate the application, and activating while another app owns a
        // full-screen Space is what makes macOS leave that Space. Tearing the
        // panel off must not move the captain either.
        if panel_window::make_nonactivating_panel(&window) {
            panel_window::order_front_without_activating(&window);
        } else {
            let _ = window.set_focus();
        }
    } else {
        // C6 fix: snapping back must actually re-dock the window under the
        // tray icon, not just restore the chrome (beak, no titlebar) —
        // without this the window silently stayed wherever the drag left
        // it. This path has no fresh tray click to read a rect from (it's
        // triggered by the panel's own header button), so it uses the last
        // rect seen by any tray icon event; `None` only before the very
        // first tray event of the app's lifetime, which can't happen here
        // since detaching itself requires the panel to already be open.
        if let Some((tray_x, tray_y)) = *state.last_tray_rect.lock().expect("last_tray_rect mutex poisoned") {
            reposition_under_tray(&app, &window, tray_x, tray_y);
        }
    }
    Ok(())
}

/// R2-6: starts Claude Code's own sign-in for a broken row's account. See
/// `signin.rs`'s module doc for exactly what this does and does not do —
/// in short, `claude setup-token` opens the browser and prints the
/// authorization URL itself; Quotos never touches that, it only starts the
/// process and later relays a pasted code into it.
#[tauri::command]
fn start_sign_in(
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
fn submit_sign_in_code(state: tauri::State<'_, AppState>, account_id: String, code: String) -> Result<(), String> {
    state.sign_in.submit_code(&account_id, &code)
}

/// R2-6: cancels an in-progress sign-in (panel action, or cleanup if the
/// row is removed mid-flow).
#[tauri::command]
fn cancel_sign_in(state: tauri::State<'_, AppState>, account_id: String) {
    state.sign_in.cancel(&account_id);
}

/// R2-6: called after the frontend has handled `sign-in-finished`, so a
/// retry starts clean.
#[tauri::command]
fn forget_sign_in(state: tauri::State<'_, AppState>, account_id: String) {
    state.sign_in.forget(&account_id);
}

/// Followup-1 fix (round 2 regression, the panel-never-opens defect): the
/// round-2 handoff moved the popover's beak off-center — it now sits at the
/// panel's own left edge, under the glyph, with the panel unfolding
/// rightward (`Panel.jsx`'s `beakLeft`, `App.tsx`'s `BEAK_LEFT`).
///
/// This used to go through `tauri-plugin-positioner`'s
/// `move_window_constrained(Position::TrayBottomLeft)`, primed by its own
/// `on_tray_event` cache. On a real click that produced a window position
/// wildly inconsistent with the tray icon's own rect (e.g. tray at physical
/// x=2444 on a 3456-wide monitor placed the window at physical x=1368 —
/// nowhere near clamped-to-monitor math could explain), landing the panel
/// off in empty space on the correct monitor, which reads on screen as
/// "nothing opened" — exactly this bug. Confirmed with the tray rect and
/// computed target logged and diffed against the window's real post-move
/// bounds (read independently of Tauri's own position getters, via
/// `CGWindowListCopyWindowInfo`, since `WebviewWindow::outer_position()`
/// itself was also observed misreporting immediately after a window's
/// first-ever `show()`); root cause narrowed to the plugin's own
/// `calculate_position`/`get_monitor_for_tray_icon` path, not chased further
/// upstream. Rather than depend on that plugin at all (the whole
/// `tauri-plugin-positioner` dependency is dropped by this fix), this now
/// computes the position itself directly from the tray icon's own `Rect`
/// (handed to us fresh on every click via `TrayIconEvent::Click`'s `rect`
/// field — always physical pixels, per `tray-icon` v0.24.2's own `Rect`
/// type) plus `monitor_from_point` on that same rect, per the handoff: left
/// edge = icon left − 6px (clamped to no further right than screen right −
/// 340px, never left of the monitor's own left edge — this is what keeps
/// both displays working, including an external display positioned such
/// that its coordinates are negative), top pinned at a fixed 32px from that
/// monitor's own top edge (6px under the menu bar, not flush against the
/// icon's bottom). The reposition happens after `show()`/`set_focus()`
/// rather than before — empirically, positioning a still-hidden window on
/// its very first ever appearance was less reliable than repositioning it
/// once already shown; the `Moved`/`Focused(true)` window events and the
/// WindowServer's own reported bounds both confirm the final position lands
/// exactly on target either way, but this ordering was what was verified.
///
/// Also returns the beak's horizontal offset (logical/CSS px, relative to
/// the panel's own left edge) so the caller can tell the frontend where to
/// draw it. B3/B5: the beak must stay centered under the tray glyph's own
/// center, not the whole button's center.
///
/// R3-1 fix: this used to assume the glyph sat a fixed "6px button padding"
/// inside the item, i.e. `icon_left + 15pt` — a number carried over from a
/// design-system mockup, never checked against a real tray item. Measuring
/// the real thing (a pixel-precise screenshot of the bare glyph, cross-checked
/// against `CGWindowListCopyWindowInfo` and the item's own Accessibility
/// rect — see `RESULT.md`) found the *actual* glyph center sitting almost
/// exactly at the item's own geometric center instead, ~18pt from its left
/// edge, not 15 — because `NSStatusItem` centers the whole composited image
/// (glyph, or glyph+digits once pinned) within a button that's wider than
/// the image by a small system-chosen margin on each side. Pinning a second
/// data point (item width 36pt bare vs 64pt with one pinned segment, right
/// edge unchanged either way — status items lay out right-to-left) confirmed
/// that margin isn't the same fixed distance from the item's own left edge
/// once digits are added, since digits only widen the image, not the glyph
/// portion of it.
///
/// So rather than hardcode a second (equally guessable) constant, this now
/// derives the margin fresh from the two pieces of ground truth Quotos
/// actually has: the item's own current width (`tray.rect()`, queried here
/// so it's paired with the same call's position rather than a value cached
/// at a different instant) and the composited image's own known width
/// (`AppState.last_icon_width_px`, set by `set_tray_status` right before
/// `set_icon` — always at the fixed "2x of an 18pt-tall image" convention,
/// so `/2` gives its real width in points regardless of monitor scale). The
/// glyph is always that image's leftmost 18pt (`tray_render.rs`'s `render`
/// draws digits only after it) — assuming `NSStatusItem` centers the image
/// (confirmed above), half of whatever's left over after the image is the
/// left margin, and the glyph's own center is 9pt further in from there.
/// This is exact for however many digits are pinned, not just the bare-glyph
/// case it was checked against, because it's computed from the actual
/// numbers each time rather than assumed constant.
///
/// If `tray.rect()` is unavailable (already `None` in this same run's other
/// call — see `resync_docked_position_after_icon_change`), this falls back
/// to treating the glyph as flush with the item's own left edge (no margin)
/// rather than failing outright — a plausible-worst-case position, not a
/// crash.
///
/// R3-7: everything here is in **points**, not "physical pixels" — see
/// `DisplayPoints`'s doc comment for why this whole module stopped speaking
/// physical pixels at all. The scale factor used to appear on both sides of
/// this arithmetic and cancel out anyway; dropping it removes a place where
/// the *wrong* scale factor could be supplied.
fn glyph_center_offset_from_item_left_points(item_width_points: Option<f64>, icon_width_px: f64) -> f64 {
    const GLYPH_WIDTH_POINTS: f64 = 18.0; // fixed: tray_render's glyph is always drawn at this size
    // `last_icon_width_px` is a buffer width at the fixed "2x of an 18pt-tall
    // image" convention `set_icon_for_ns_status_item_button` imposes, so /2
    // is its real width in points on any display.
    let image_width_points = icon_width_px / 2.0;
    let margin_points = item_width_points.map(|w| ((w - image_width_points) / 2.0).max(0.0)).unwrap_or(0.0);
    // R3-11: the glyph is no longer the image's leftmost pixel — the image
    // carries its own side padding so A11's highlight has horizontal air. That
    // inset comes from `tray_render`, which owns it, rather than being
    // duplicated as a number here.
    margin_points + tray_render::GLYPH_LEFT_INSET_POINTS + GLYPH_WIDTH_POINTS / 2.0
}

/// The window's own fixed logical size, from `tauri.conf.json`'s `width`/
/// `height` — duplicated here (rather than read back from the window, which
/// would just report whatever size it currently, possibly wrongly, is) so
/// the Resized-event guard in `run`'s `setup` has a ground truth to snap
/// back to.
const PANEL_WINDOW_WIDTH_LOGICAL: f64 = 360.0;
const PANEL_WINDOW_HEIGHT_LOGICAL: f64 = 560.0;

/// R3-7 — the coordinate space this whole module speaks, and why it had to
/// change.
///
/// macOS has exactly one coordinate space in which a multi-display layout has
/// a single, consistent meaning: the **global point space** (`CGDisplayBounds`
/// / `NSScreen.frame`), top-left origin at the main display's top-left. There
/// is no global *pixel* space at all — "physical pixels" are only ever defined
/// relative to one display's own backing scale factor.
///
/// The three APIs this module depends on each hand out a `Physical*` type that
/// is really "global points × **some** display's scale factor", and they do not
/// agree on *which* display's:
///
/// | source | value | scale used |
/// |---|---|---|
/// | `TrayIconEvent`'s `rect` (`tray-icon` 0.24.2 `get_tray_rect`) | tray item frame | the **menu bar display**'s `backingScaleFactor` |
/// | `Monitor::position()`/`size()` (`tao` 0.35.3 `monitor.rs`) | `CGDisplayBounds` / `CGDisplayPixelsWide` | **that monitor's own** scale factor |
/// | `set_position(Physical)` / `WindowEvent::Moved` (`tao` `window.rs`, `window_delegate.rs`) | window frame | the **window's current** `backingScaleFactor` |
///
/// On a single-display machine all three coincide and the bug is invisible.
/// On the captain's actual setup — built-in Retina at point `(0,0,1728,1117)`
/// scale 2, external LG at point `(-2560,-908,2560,2880)` scale 1 — they
/// diverge, and every symptom he reported falls straight out of it:
///
/// * A tray click on the built-in yields `tray_x = 2366` ("1183 points × 2").
///   Handed to `set_position(Physical(…))` while the window happens to be on
///   the *LG* (scale 1), `tao` divides by **1** and places the window at point
///   x = 2354 — past the right edge of every display, i.e. nowhere. The window
///   is revealed at its stale position first (below) and then vanishes:
///   *"панель мерцает только"*, and it flickers **on the other monitor**,
///   which is where it was last left — *"кликаю на макбуке, глитч на другом"*.
/// * The mirror case (tray on the LG, window on the built-in) divides by 2 and
///   lands the panel at half the intended offset from the LG's own origin —
///   visible, on the right display, in the wrong place; the debounced
///   correction then re-runs it with the window's now-correct scale factor and
///   it snaps across: *"она прыгает по экрану"*.
///
/// So the fix is not another retry or another delay: it is to stop speaking a
/// unit that does not exist. Everything below converts to points at the edges
/// (`DisplayPoints`, `resolve_tray_point`) and never leaves them —
/// `apply_docked_position` places the window with a `LogicalPosition`, which
/// `tao`'s `Position::to_logical` passes through untouched, so no scale factor
/// is ever consulted on the way out either.
#[derive(Clone, Copy, Debug, PartialEq)]
struct DisplayPoints {
    /// Top-left corner in the global point space.
    origin: (f64, f64),
    /// Size in points.
    size: (f64, f64),
    /// This display's own backing scale factor — needed only to *undo* the
    /// multiplication `tray-icon` applied to the tray rect.
    scale: f64,
    /// Global-point y of the **bottom edge of this display's own menu bar**,
    /// read live from `NSScreen.visibleFrame`; `None` on a display that has no
    /// menu bar, or when the screen list wasn't reachable (off the main
    /// thread, or off macOS).
    ///
    /// R3-8: the menu bar's height is not a constant and must never be written
    /// down as one. `compute_docked_layout` used to place the panel's top at a
    /// flat `32pt` from the display's top — the handoff's own number, which it
    /// describes as *"6px под меню-баром"*, i.e. a menu bar height plus a gap.
    /// Measured on this machine's two displays: the notched built-in's menu bar
    /// is **33pt** tall, so `32` put the panel's top edge *inside* the menu bar
    /// and AppKit clamped it back out to exactly 33 — a panel flush against the
    /// bar with no gap at all. An unnotched display's is ~24pt, where the same
    /// constant leaves an 8pt gap instead of 6. Same constant, two different
    /// wrong answers on one machine, which is exactly the display-dependent
    /// *"не появляется где должна"*. Derived per display now, so it is right on
    /// a notched screen, an unnotched one, a scaled one, and a machine that
    /// isn't this one.
    menu_bar_bottom: Option<f64>,
}

impl DisplayPoints {
    fn contains(&self, x: f64, y: f64) -> bool {
        x >= self.origin.0 && x < self.origin.0 + self.size.0 && y >= self.origin.1 && y < self.origin.1 + self.size.1
    }
}

/// Recovers the global-point position of a `tray-icon` rect, along with the
/// display it belongs to.
///
/// `tray-icon` multiplied the true point coordinate by the menu bar display's
/// scale factor and told us nothing about which display that was, so the
/// division cannot be done blind — this tries each display's own scale factor
/// and keeps the one whose quotient actually lands inside that display. On the
/// captain's two-display layout the answer is unambiguous in both directions
/// (a built-in tray at `2366` gives `1183` on the built-in ✓ and `2366` on the
/// LG ✗; an LG tray at `-400` gives `-400` on the LG ✓ and `-200` on the
/// built-in ✗). Where an unusual arrangement could make two candidates both
/// land in-bounds, the tie-break is the one property a menu bar always has:
/// it hugs its own display's top edge.
fn resolve_tray_point(displays: &[DisplayPoints], tray_x: f64, tray_y: f64) -> Option<(usize, f64, f64)> {
    let mut best: Option<(usize, f64, f64, f64)> = None; // (index, x, y, distance below that display's top)
    for (index, display) in displays.iter().enumerate() {
        if display.scale <= 0.0 {
            continue;
        }
        let (x, y) = (tray_x / display.scale, tray_y / display.scale);
        if !display.contains(x, y) {
            continue;
        }
        let from_top = y - display.origin.1;
        if best.is_none_or(|(_, _, _, best_from_top)| from_top < best_from_top) {
            best = Some((index, x, y, from_top));
        }
    }
    best.map(|(index, x, y, _)| (index, x, y))
}

/// Every display, in global points. On macOS this reads `NSScreen` directly
/// rather than going through `tao`'s `Monitor` — one API, in one coordinate
/// space, and the only one that also reports `visibleFrame` (hence the menu
/// bar height, see `DisplayPoints::menu_bar_bottom`). It needs the main
/// thread; the `tao` path below is the fallback for anywhere else, and loses
/// only the menu bar height.
#[cfg(target_os = "macos")]
fn displays_in_points(window: &tauri::WebviewWindow) -> Vec<DisplayPoints> {
    use objc2_app_kit::NSScreen;
    use objc2_foundation::MainThreadMarker;

    let Some(mtm) = MainThreadMarker::new() else { return displays_in_points_via_tao(window) };
    let screens = NSScreen::screens(mtm);
    // AppKit's global space is y-up from the first screen's bottom-left; the
    // rest of this module is y-down from its top-left. That screen's own
    // height is the flip constant (its origin is (0,0) by definition).
    let Some(flip) = screens.iter().next().map(|s| s.frame().size.height) else {
        return displays_in_points_via_tao(window);
    };
    screens
        .iter()
        .map(|screen| {
            let frame = screen.frame();
            let visible = screen.visibleFrame();
            let top = flip - (frame.origin.y + frame.size.height);
            // `visibleFrame` also excludes the Dock, but the Dock never sits at
            // the top, so the difference at the *top* edge is the menu bar and
            // nothing else.
            let menu_bar_height = (frame.origin.y + frame.size.height) - (visible.origin.y + visible.size.height);
            DisplayPoints {
                origin: (frame.origin.x, top),
                size: (frame.size.width, frame.size.height),
                scale: screen.backingScaleFactor(),
                menu_bar_bottom: (menu_bar_height > 0.0).then_some(top + menu_bar_height),
            }
        })
        .collect()
}

#[cfg(not(target_os = "macos"))]
fn displays_in_points(window: &tauri::WebviewWindow) -> Vec<DisplayPoints> {
    displays_in_points_via_tao(window)
}

fn displays_in_points_via_tao(window: &tauri::WebviewWindow) -> Vec<DisplayPoints> {
    window
        .available_monitors()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|m| {
            let scale = m.scale_factor();
            if scale <= 0.0 {
                return None;
            }
            let pos = *m.position();
            let size = *m.size();
            Some(DisplayPoints {
                origin: (pos.x as f64 / scale, pos.y as f64 / scale),
                size: (size.width as f64 / scale, size.height as f64 / scale),
                scale,
                menu_bar_bottom: None,
            })
        })
        .collect()
}

/// Where the docked panel window belongs, all in global points (see
/// `DisplayPoints`): the window's own top-left, plus the beak's CSS `left`
/// inside the panel.
#[derive(Clone, Copy, Debug, PartialEq)]
struct DockedLayout {
    x: f64,
    y: f64,
    beak_left: f64,
}

/// The pure half of `compute_docked_layout` — no `tauri` handles, so the
/// handoff's own placement rules (B5/B7) are unit-testable against the
/// captain's real two-display geometry instead of only on a live screen.
fn docked_layout_in_points(
    display: DisplayPoints,
    tray_left: f64,
    tray_top: f64,
    tray_bottom: f64,
    item_width_points: Option<f64>,
    icon_width_px: f64,
) -> DockedLayout {
    const PANEL_WIDTH: f64 = 332.0;
    // B5: however far left the beak wants the panel, it can never hang off the
    // display's right edge (handoff: "clamped to screen right − 340").
    const RIGHT_CLAMP: f64 = 340.0;

    // R3-10 — three of the handoff's own numbers are **superseded by the
    // captain's own instruction** after he saw the result at real size:
    //
    // > Он во первых маленький, во вторых — слишком близко к левому краю,
    // > в третьих — слишком низко от топбара и иконки. Он должен быть
    // > "почти в иконке" — чуть чуть ниже топбара.
    //
    // So: the beak is bigger (`BEAK_BASE_WIDTH`/`BEAK_HEIGHT`, authored in
    // Panel.jsx and mirrored here), it is held a comfortable distance inside
    // the panel's own left edge instead of pressed against it
    // (`BEAK_INSET_IN_PANEL`, replacing the handoff's "клюв прижат к левому
    // краю" and its `icon_left − 6` docking rule), and the panel is pulled up
    // until the beak's *tip* — not the panel's top edge — sits just under the
    // menu bar (`BEAK_TIP_CLEARANCE`).
    //
    // The two constraints interact, which is why the x below is no longer
    // "icon left minus a constant": the beak's centre must stay exactly on the
    // glyph's centre (his earlier, equally explicit requirement), so the only
    // way to also move the beak away from the corner is to move the whole
    // panel further left. Deriving x *from* the beak's wanted position makes
    // both hold by construction, at whatever glyph offset the tray happens to
    // report, rather than by a second constant that would have to be retuned
    // every time the first one moves.
    const BEAK_BASE_WIDTH: f64 = 20.0; // == BEAK_BASE_HALF * 2 in Panel.jsx
    const BEAK_HEIGHT: f64 = 10.0; // == BEAK_HEIGHT in Panel.jsx
    const BEAK_INSET_IN_PANEL: f64 = 20.0; // notch's left edge, from the panel's own left edge
    const BEAK_TIP_CLEARANCE: f64 = 2.0; // how far the tip stops short of the menu bar

    // The panel is inset inside its own (deliberately wider, and taller at the
    // top) window: `(window width − panel width) / 2` of empty margin per side
    // for the drop shadow's blur to fade into rather than be clipped by, and
    // `padding-top` worth of headroom for the beak — both in app.css, both
    // load-bearing here because the *window* is what gets positioned while the
    // *panel* is what the captain looks at.
    const PANEL_INSET_X: f64 = (PANEL_WINDOW_WIDTH_LOGICAL - PANEL_WIDTH) / 2.0;
    const PANEL_INSET_TOP: f64 = 12.0; // == app.css's `body { padding-top }` / Panel.jsx's NOTCH_RESERVE

    let glyph_center = tray_left + glyph_center_offset_from_item_left_points(item_width_points, icon_width_px);

    // Place the window so the beak lands at its wanted inset with its centre on
    // the glyph's centre, then clamp to the display — the clamp is what B5's
    // right-screen-edge case exercises, and the beak offset recomputed below
    // from the *clamped* x is what keeps it tracking the glyph there.
    let wanted_panel_left = glyph_center - BEAK_INSET_IN_PANEL - BEAK_BASE_WIDTH / 2.0;
    let mut x = wanted_panel_left - PANEL_INSET_X;
    x = x.max(display.origin.0);
    let max_x = (display.origin.0 + display.size.0 - RIGHT_CLAMP).max(display.origin.0);
    x = x.min(max_x);

    // Both candidates for the menu bar's own bottom edge are read from the
    // running system, never assumed. `visibleFrame` is the direct answer where
    // it exists — but it does not exist in the state the captain actually
    // reproduces from: inside a full-screen Space the menu bar is auto-hidden,
    // `visibleFrame` equals `frame`, and there is no bar height to read even
    // while the bar sits revealed on screen under his cursor. The tray item is
    // still there and still measured, though, and macOS centres a status item
    // vertically in its bar — so the bar's bottom is the item's bottom plus the
    // same inset that sits above it. Derived either way, constant neither way.
    let inset_above_item = (tray_top - display.origin.1).max(0.0);
    // `NEG_INFINITY`, not 0 — a display left of the primary has negative
    // coordinates, where 0 is not a neutral floor but a point far below it.
    let menu_bar_bottom = display.menu_bar_bottom.unwrap_or(f64::NEG_INFINITY).max(tray_bottom + inset_above_item);
    // Solve for the window top from where the beak's *tip* should end up:
    // tip = y + PANEL_INSET_TOP − BEAK_HEIGHT, and tip should be
    // BEAK_TIP_CLEARANCE below the bar.
    let y = menu_bar_bottom + BEAK_TIP_CLEARANCE - (PANEL_INSET_TOP - BEAK_HEIGHT);

    // Recomputed from the applied x, not the wanted one, so a screen-edge clamp
    // moves the beak across the panel instead of dragging it off the glyph.
    // Clamped only to keep the notch on the panel at all; `buildPanelOutlinePath`
    // shrinks whichever top corner the notch encroaches on rather than the notch
    // giving way (doing it the other way round moved the beak visibly off the
    // glyph — caught live: "центровки снова нет").
    let panel_left = x + PANEL_INSET_X;
    let beak_left = (glyph_center - panel_left - BEAK_BASE_WIDTH / 2.0).clamp(0.0, PANEL_WIDTH - BEAK_BASE_WIDTH);

    DockedLayout { x, y, beak_left }
}

fn compute_docked_layout(app: &tauri::AppHandle, window: &tauri::WebviewWindow, tray_x: f64, tray_y: f64) -> Option<DockedLayout> {
    let displays = displays_in_points(window);
    let (index, tray_left, tray_top) = resolve_tray_point(&displays, tray_x, tray_y)?;
    let display = displays[index];

    // Same `tray-icon` conversion as the position (see `DisplayPoints`), so
    // the same display's scale factor undoes it.
    let item_size = app.tray_by_id("main-tray").and_then(|t| t.rect().ok().flatten()).map(|r| match r.size {
        tauri::Size::Physical(s) => (s.width as f64 / display.scale, s.height as f64 / display.scale),
        tauri::Size::Logical(s) => (s.width, s.height),
    });
    let icon_width_px = *app.state::<AppState>().last_icon_width_px.lock().expect("last_icon_width_px mutex poisoned") as f64;

    let tray_bottom = tray_top + item_size.map(|(_, h)| h).unwrap_or(0.0);
    Some(docked_layout_in_points(display, tray_left, tray_top, tray_bottom, item_size.map(|(w, _)| w), icon_width_px))
}

/// R3-7: moves the window's top-left to a global-point coordinate, and does
/// it **synchronously on the main thread** wherever that's possible.
///
/// The synchronicity is not a micro-optimisation — it is the second half of
/// the captain's flicker. `tao`'s own `set_outer_position` ends in
/// `util::set_frame_top_left_point_async`, which `dispatch_async`es the
/// `setFrameTopLeftPoint:` onto the main queue, while `show()` ends in
/// `util::make_key_and_order_front_sync`, which runs inline. Called in either
/// order from the tray-click handler (already on the main thread), the window
/// therefore becomes **visible at its stale position first** and only moves a
/// runloop turn later — a guaranteed one-frame flash at wherever it last was,
/// which on a two-display machine is routinely the *other* display. Placing it
/// directly through `NSWindow` closes that gap: by the time `show()` runs the
/// frame is already right.
///
/// Returns whether the synchronous path was taken; callers fall back to
/// Tauri's own async `set_position` (non-macOS, or a call arriving off the
/// main thread, where `setFrameTopLeftPoint:` isn't safe anyway).
#[cfg(target_os = "macos")]
fn place_window_top_left_sync(window: &tauri::WebviewWindow, x: f64, y: f64) -> bool {
    use objc2_app_kit::{NSScreen, NSWindow};
    use objc2_foundation::{MainThreadMarker, NSPoint};

    let Some(mtm) = MainThreadMarker::new() else { return false };
    let Ok(ptr) = window.ns_window() else { return false };
    if ptr.is_null() {
        return false;
    }
    // AppKit's global space is y-up from the *primary* screen's bottom-left;
    // `x`/`y` here are CG-style, y-down from its top-left. `NSScreen.screens`'
    // first element is by definition the screen whose origin is (0,0), so its
    // own height is the flip constant — the same conversion `tao`'s
    // `util::window_position` makes with `CGDisplay::main().pixels_high()`.
    let screens = NSScreen::screens(mtm);
    let Some(primary) = screens.iter().next() else { return false };
    let flip = primary.frame().size.height;

    let ns_window: &NSWindow = unsafe { &*(ptr as *const NSWindow) };
    ns_window.setFrameTopLeftPoint(NSPoint::new(x, flip - y));
    true
}

#[cfg(not(target_os = "macos"))]
fn place_window_top_left_sync(_window: &tauri::WebviewWindow, _x: f64, _y: f64) -> bool {
    false
}

/// Applies a docked position/beak-offset and records it as the window's
/// current *intended* target (`AppState.docked_target`) — see that field's
/// own doc comment for why: the `WindowEvent::Moved` handler reapplies this
/// exact target whenever something relocates the window away from it.
///
/// `x`/`y` are global points (see `DisplayPoints`). The fallback path uses a
/// `LogicalPosition` deliberately: `tao`'s `Position::to_logical` passes a
/// logical value straight through, so no scale factor is consulted — a
/// `PhysicalPosition` here would be reinterpreted through whatever display the
/// window currently happens to sit on, which is the original bug.
fn apply_docked_position(app: &tauri::AppHandle, window: &tauri::WebviewWindow, layout: DockedLayout) {
    *app.state::<AppState>().docked_target.lock().expect("docked_target mutex poisoned") = Some(layout);
    if !place_window_top_left_sync(window, layout.x, layout.y) {
        let _ = window.set_position(tauri::LogicalPosition::new(layout.x, layout.y));
    }
    let _ = window.emit("panel-beak-offset", layout.beak_left);
}

/// Moves an already-visible, already-docked-chrome window to sit under the
/// tray icon and tells the frontend where to draw the beak — the shared
/// tail end of both `show_panel` and `set_detached`'s snap-back path (C6).
/// Returns what it applied (or `None` if no display could be resolved).
fn reposition_under_tray(app: &tauri::AppHandle, window: &tauri::WebviewWindow, tray_x: f64, tray_y: f64) -> Option<DockedLayout> {
    let layout = compute_docked_layout(app, window, tray_x, tray_y)?;
    apply_docked_position(app, window, layout);
    Some(layout)
}

/// Clears the docked position target (see `AppState.docked_target`) so the
/// `WindowEvent::Moved` self-correction stops reasserting a position that
/// no longer applies — called whenever the window stops being "docked and
/// should stay exactly here": hiding, and detaching (dragging must never
/// fight the drag).
fn clear_docked_target(app: &tauri::AppHandle) {
    *app.state::<AppState>().docked_target.lock().expect("docked_target mutex poisoned") = None;
}

/// Firstmate's round-3 on-screen pass (`data/quotos-tray-t1/firstmate-findings-1.md`)
/// found the panel window's `kCGWindowIsOnscreen` staying `false` through an
/// entire click, on both this build and the known-good control — sampled
/// every 80ms for 16 seconds, never flipping. Diagnostic logging added this
/// round (temporary `eprintln!`s in `show_panel`/`toggle_panel`/the blur
/// handler, removed before commit) ruled out firstmate's other hypothesis:
/// `show()`/`set_focus()` both return `Ok`, a `Focused(true)` window event
/// does arrive shortly after (asynchronously — `is_focused()` reads `false`
/// if checked synchronously right after `set_focus()`, which is a red
/// herring, not a real failure), and no `Focused(false)`/hide ever follows
/// it in the same window. So the window is genuinely shown, focused, and
/// never auto-hidden — yet the WindowServer still doesn't consider it
/// onscreen.
///
/// The remaining explanation: this window is a *singleton*, created once at
/// launch and only ever hidden/shown afterward, never recreated. A macOS
/// window's Space membership is normally sticky per-window — it belongs to
/// whichever Space was frontmost when it was first realized, and plain
/// `-orderFront:`/`-makeKeyAndOrderFront:` do not by themselves move it to
/// the Space the user is actually looking at. A background-launched process
/// (as this was, every time it was tested from here) has no interactively
/// "current" Space to inherit at that moment, which would explain a
/// persistent (not momentary) mismatch that never resolves on its own.
///
/// The fix needs the window visible on whatever Space the user currently
/// has active. Two collection-behavior flags were tried:
///
/// - `NSWindowCollectionBehaviorCanJoinAllSpaces` (the window exists on
///   every Space at once, so there's no "transition" to race against) keeps
///   this code's own explicit positioning perfectly stable — no spurious
///   `Moved` events — but `kCGWindowIsOnscreen` stayed `false` regardless,
///   i.e. it never actually solved the visibility problem this exists to
///   fix.
/// - `NSWindowCollectionBehaviorMoveToActiveSpace` **does** flip
///   `kCGWindowIsOnscreen` true (confirmed live, the first and only time in
///   this whole investigation) — but the Space transition it triggers is
///   itself asynchronous and was observed relocating the window *again*,
///   ~100-150ms after this code's own explicit `set_position` call, to an
///   unrelated AppKit-internal default position, undoing it.
///
/// R3-9 settles it, from the captain's own isolated reproduction: **the panel
/// was never failing to open — it was opening on a Space he was not looking
/// at.**
///
/// > Оно открывается на основном столе (которого я не вижу) и закрывается
/// > когда я на него перехожу.
///
/// It works from an ordinary desktop, on either display. It fails only when
/// another application owns a **full-screen Space** and he pulls the cursor to
/// the top edge to slide the hidden menu bar down. That single fact explains
/// every earlier confusing measurement at once: correct geometry, `show()` and
/// `set_focus()` both returning `Ok`, a real `Focused(true)` arriving — and
/// `kCGWindowIsOnscreen` reading `false` the whole time, because the window was
/// genuinely on screen, on a different Space.
///
/// A full-screen application owns its Space, and macOS does not order another
/// application's window into it just because that application asks. The bit
/// that grants it is `NSWindowCollectionBehaviorFullScreenAuxiliary` — the
/// standard utility/palette-window behaviour, and the half no earlier round
/// ever set. `MoveToActiveSpace` does not cover the case: a full-screen Space
/// is not a destination it moves windows *into*, which is why it appeared to
/// work (it does, between ordinary desktops) while leaving this broken.
///
/// Paired with `CanJoinAllSpaces` rather than `MoveToActiveSpace` — the two are
/// mutually exclusive, and "exists on every Space already" is both what a menu
/// bar popover actually is and the option with no asynchronous transition to
/// race: the previous round measured it holding this code's own explicit
/// positioning perfectly stable, with no spurious `Moved` events at all. (The
/// relocations once blamed on `MoveToActiveSpace`'s transition were `x=-2106`
/// and `x=4712` — both within rounding of exactly 2× or ½× a legitimate
/// coordinate here, i.e. the signature of the scale-factor bug `DisplayPoints`
/// documents, not of an AppKit default placement.)
///
/// `.Transient` keeps it out of Mission Control/Exposé's per-Space window list,
/// matching the system's own Volume/Wi-Fi popovers; `.IgnoresCycle` keeps it
/// out of Cmd-` window cycling. Neither conflicts with the two above — AppKit
/// groups these flags and silently drops conflicting pairs *within* a group,
/// which is exactly why this is read back afterwards rather than assumed (see
/// `collection_behavior_bits`).
///
/// Reapplied on every show rather than only at launch: idempotent, and cheap
/// insurance against anything resetting it on first realisation.
#[cfg(target_os = "macos")]
fn set_popover_collection_behavior(window: &tauri::WebviewWindow) {
    use objc2_app_kit::{NSWindow, NSWindowCollectionBehavior};
    let Ok(ptr) = window.ns_window() else { return };
    if ptr.is_null() {
        return;
    }
    let ns_window: &NSWindow = unsafe { &*(ptr as *const NSWindow) };
    let behavior = match std::env::var("QUOTOS_DEBUG_SPACE_BEHAVIOR").ok().as_deref() {
        Some("join") => NSWindowCollectionBehavior::CanJoinAllSpaces | NSWindowCollectionBehavior::FullScreenAuxiliary,
        Some("move") => NSWindowCollectionBehavior::MoveToActiveSpace | NSWindowCollectionBehavior::FullScreenAuxiliary,
        Some("aux") => NSWindowCollectionBehavior::FullScreenAuxiliary,
        Some("joinonly") => NSWindowCollectionBehavior::CanJoinAllSpaces,
        Some("none") => NSWindowCollectionBehavior::empty(),
        _ => {
            NSWindowCollectionBehavior::CanJoinAllSpaces
                | NSWindowCollectionBehavior::FullScreenAuxiliary
                | NSWindowCollectionBehavior::Transient
                | NSWindowCollectionBehavior::IgnoresCycle
        }
    };
    ns_window.setCollectionBehavior(behavior);
    set_popover_window_level(ns_window);
}

/// R3-9: `alwaysOnTop` in tauri.conf.json gets this window
/// `NSFloatingWindowLevel`, which is right for a panel floating over ordinary
/// windows and **not** enough to be seen over another application's
/// full-screen Space — the state the captain's own reproduction is from. A
/// status-item popover belongs at `NSStatusWindowLevel`, which is what the
/// system's own menu bar popovers use and what puts it above a full-screen
/// window's content. Overridable for diagnosis (`QUOTOS_DEBUG_WINDOW_LEVEL`)
/// because "which of these two knobs actually did it" is only answerable by
/// changing one at a time against a real full-screen Space.
#[cfg(target_os = "macos")]
fn set_popover_window_level(ns_window: &objc2_app_kit::NSWindow) {
    const NS_STATUS_WINDOW_LEVEL: isize = 25;
    let level = std::env::var("QUOTOS_DEBUG_WINDOW_LEVEL").ok().and_then(|v| v.parse().ok()).unwrap_or(NS_STATUS_WINDOW_LEVEL);
    ns_window.setLevel(level);
}

#[cfg(not(target_os = "macos"))]
fn set_popover_collection_behavior(_window: &tauri::WebviewWindow) {}

/// Reads back what AppKit actually stored, since `setCollectionBehavior:`
/// silently drops flags that conflict with others in the same group — "we set
/// it" is not evidence that it is set. Surfaced through the `QUOTOS_DEBUG_POS`
/// trace.
#[cfg(target_os = "macos")]
fn collection_behavior_bits(window: &tauri::WebviewWindow) -> Option<u64> {
    use objc2_app_kit::NSWindow;
    let ptr = window.ns_window().ok()?;
    if ptr.is_null() {
        return None;
    }
    Some(unsafe { (*(ptr as *const NSWindow)).collectionBehavior() }.0 as u64)
}

#[cfg(not(target_os = "macos"))]
fn collection_behavior_bits(_window: &tauri::WebviewWindow) -> Option<u64> {
    None
}

/// AppKit's own answer to "is this window on the Space the user is looking at",
/// which is the entire question this round turns on and the one thing
/// `kCGWindowIsOnscreen` and a screenshot can only infer. Public API
/// (`-[NSWindow isOnActiveSpace]`), unlike the `CGSCopySpacesForWindows`
/// route. Surfaced through the `QUOTOS_DEBUG_POS` trace.
#[cfg(target_os = "macos")]
fn space_diagnostics(window: &tauri::WebviewWindow) -> Option<String> {
    use objc2_app_kit::{NSApplication, NSWindow};
    use objc2_foundation::MainThreadMarker;
    let mtm = MainThreadMarker::new()?;
    let ptr = window.ns_window().ok()?;
    if ptr.is_null() {
        return None;
    }
    let ns_window: &NSWindow = unsafe { &*(ptr as *const NSWindow) };
    let app_active = NSApplication::sharedApplication(mtm).isActive();
    // R4-1: `class` and `style_mask` are the two facts that say whether the
    // non-activating panel conversion actually took (a plain `NSWindow` would
    // ignore the style bit entirely), and `app_active` is the one the whole
    // Space fix turns on — see panel_window.rs.
    let class = unsafe { (*(ptr as *const objc2::runtime::AnyObject)).class() }.name().to_string_lossy().into_owned();
    let style_mask: usize = unsafe { objc2::msg_send![ns_window, styleMask] };
    Some(format!(
        "on_active_space={} visible={} key={} level={} occlusion={:?} app_active={app_active} class={class} style_mask={style_mask:#x}",
        ns_window.isOnActiveSpace(),
        ns_window.isVisible(),
        ns_window.isKeyWindow(),
        ns_window.level(),
        ns_window.occlusionState(),
    ))
}

#[cfg(not(target_os = "macos"))]
fn space_diagnostics(_window: &tauri::WebviewWindow) -> Option<String> {
    None
}

/// R4-1: shows the panel and gives it keyboard focus **without activating the
/// application** — see `panel_window.rs`'s module doc for the whole argument.
///
/// The short version: `WebviewWindow::set_focus()` ends in
/// `activateIgnoringOtherApps: YES` (`tao`'s `util::set_focus`), and activating
/// another application while a full-screen Space is frontmost is *exactly* what
/// makes macOS slide out of that Space — which is the transition caught
/// mid-animation in the captain's 2026-08-15 screencast. Round 3 kept that call
/// on purpose, because dropping it left the window non-key and a non-key window
/// breaks click-away-to-close, the rename field and the sign-in field. Making
/// the window a non-activating `NSPanel` is what dissolves that trade: a panel
/// can be key while another application stays active, so the app never has to
/// activate at all.
///
/// The fallback branch is the old behaviour verbatim, taken only if the panel
/// conversion did not happen (`QUOTOS_PANEL_MODE=window`, a non-macOS build, or
/// an AppKit that refused the class swap). A window that can never take
/// keyboard focus would be a much worse regression than a Space switch, so
/// "couldn't become a panel" must fall back rather than press on.
fn order_panel_front(window: &tauri::WebviewWindow) {
    // Idempotent, and re-asserted on every show rather than only at launch for
    // the same reason the collection behaviour is: cheap insurance against
    // anything resetting it.
    let is_panel = panel_window::make_nonactivating_panel(window);
    // `show()` is `tao`'s `makeKeyAndOrderFront:` — ordering and key-ness, no
    // activation of its own. It is the activation in `set_focus()` below, not
    // this, that was ever the problem.
    let _ = window.show();
    if is_panel {
        panel_window::order_front_without_activating(window);
    } else {
        let _ = window.set_focus();
        order_front_regardless(window);
    }
}

/// "Put this window at the front of its level here and now, even if my
/// application is not the active one." Only needed on the non-panel fallback
/// path — `order_front_without_activating` does this itself.
#[cfg(target_os = "macos")]
fn order_front_regardless(window: &tauri::WebviewWindow) {
    use objc2_app_kit::NSWindow;
    use objc2_foundation::MainThreadMarker;

    let (Some(_mtm), Ok(ptr)) = (MainThreadMarker::new(), window.ns_window()) else { return };
    if ptr.is_null() {
        return;
    }
    let ns_window: &NSWindow = unsafe { &*(ptr as *const NSWindow) };
    ns_window.orderFrontRegardless();
}

#[cfg(not(target_os = "macos"))]
fn order_front_regardless(_window: &tauri::WebviewWindow) {}

/// R3-7: **position first, then show.** `show()` reveals the window wherever
/// it was last left — on a two-display machine, routinely the other display —
/// so showing before moving guarantees a visible frame at the wrong place,
/// which is precisely the captain's *"панель мерцает"*. The move itself must
/// also actually land before the reveal, which `set_position` alone cannot
/// promise; see `place_window_top_left_sync` for that half.
///
/// The position is reapplied once after `show()` as well. That second call is
/// a no-op when nothing moved the window (`setFrameTopLeftPoint:` to the
/// frame's current origin emits no `Moved` event), and it costs one main-
/// thread call to be immune to anything ordering-front does to the frame.
fn show_panel(app: &tauri::AppHandle, window: &tauri::WebviewWindow, tray_x: f64, tray_y: f64) {
    set_popover_collection_behavior(window);
    let layout = compute_docked_layout(app, window, tray_x, tray_y);
    if let Some(layout) = layout {
        apply_docked_position(app, window, layout);
    }
    order_panel_front(window);
    if let Some(layout) = layout {
        apply_docked_position(app, window, layout);
    }
    log_docked_placement(app, window, tray_x, tray_y, layout);
    let _ = window.emit("panel-visibility", true);
    set_tray_highlighted(app, true);
}

/// Opt-in placement trace (`QUOTOS_DEBUG_POS=1`), off by default so no build
/// ever writes to stderr on its own. Every number the docked-position
/// arithmetic consumes and produces, plus the frame AppKit actually ended up
/// with — enough to tell "computed wrong" apart from "computed right, then
/// something moved it", which is the distinction the whole R3-7 investigation
/// turned on and which no amount of screenshotting can settle.
fn log_docked_placement(
    app: &tauri::AppHandle,
    window: &tauri::WebviewWindow,
    tray_x: f64,
    tray_y: f64,
    layout: Option<DockedLayout>,
) {
    if std::env::var_os("QUOTOS_DEBUG_POS").is_none() {
        return;
    }
    let displays = displays_in_points(window);
    let resolved = resolve_tray_point(&displays, tray_x, tray_y);
    let item = app.tray_by_id("main-tray").and_then(|t| t.rect().ok().flatten()).map(|r| (r.position, r.size));
    let icon_width_px = *app.state::<AppState>().last_icon_width_px.lock().expect("last_icon_width_px mutex poisoned");
    eprintln!(
        "quotos-pos: tray_raw=({tray_x},{tray_y}) displays={displays:?} resolved={resolved:?} item={item:?} icon_width_px={icon_width_px} layout={layout:?} frame_after={:?} collection_behavior={:?} visible={:?}",
        window_frame_points(window),
        collection_behavior_bits(window).map(|b| format!("{b:#x}")),
        window.is_visible()
    );
    // Sampled again shortly after, because Space membership settles
    // asynchronously and the value read inside `show_panel` is the one least
    // likely to be final.
    let later = window.clone();
    tauri::async_runtime::spawn(async move {
        for delay_ms in [50u64, 400, 1500, 3000] {
            tokio::time::sleep(Duration::from_millis(delay_ms)).await;
            let w = later.clone();
            let _ = w.clone().run_on_main_thread(move || {
                eprintln!("quotos-pos: +{delay_ms}ms {:?}", space_diagnostics(&w));
            });
        }
    });
}

/// The window's own frame, read back from AppKit in global points — not via
/// `outer_position()`, which is both scale-factor-relative (see
/// `DisplayPoints`) and was observed misreporting right after a first `show()`.
#[cfg(target_os = "macos")]
fn window_frame_points(window: &tauri::WebviewWindow) -> Option<(f64, f64, f64, f64)> {
    use objc2_app_kit::{NSScreen, NSWindow};
    use objc2_foundation::MainThreadMarker;
    let mtm = MainThreadMarker::new()?;
    let ptr = window.ns_window().ok()?;
    if ptr.is_null() {
        return None;
    }
    let flip = NSScreen::screens(mtm).iter().next()?.frame().size.height;
    let frame = unsafe { (*(ptr as *const NSWindow)).frame() };
    Some((frame.origin.x, flip - (frame.origin.y + frame.size.height), frame.size.width, frame.size.height))
}

#[cfg(not(target_os = "macos"))]
fn window_frame_points(_window: &tauri::WebviewWindow) -> Option<(f64, f64, f64, f64)> {
    None
}

/// Left-clicking the tray icon while detached brings the window forward
/// instead of hiding it — closing a window the captain deliberately parked
/// on screen must be an explicit action, not an accidental side effect of
/// clicking the tray glyph again.
fn toggle_panel(app: &tauri::AppHandle, window: &tauri::WebviewWindow, detached: bool, tray_x: f64, tray_y: f64) {
    let visible = window.is_visible().unwrap_or(false);
    if visible && !detached {
        let _ = window.hide();
        let _ = window.emit("panel-visibility", false);
        set_tray_highlighted(app, false);
        clear_docked_target(app);
    } else {
        show_panel(app, window, tray_x, tray_y);
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            list_accounts,
            fetch_snapshot,
            load_tracked,
            save_tracked,
            kick_scheduler,
            hide_panel,
            set_tray_status,
            set_detached,
            debug_rate_limit_snapshot,
            start_sign_in,
            submit_sign_in_code,
            cancel_sign_in,
            forget_sign_in,
            statusline_status,
            statusline_install,
            statusline_remove,
        ])
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            // R2-T1: log once at startup which font the tray's percentage
            // digits actually resolved to on this machine — MonoLisa if
            // installed, otherwise the system tabular-figure UI font (see
            // `tray_render.rs`'s `text` submodule for the lookup/fallback).
            eprintln!(
                "quotos: tray digit font = {}",
                if tray_render::used_fallback_font() { "system fallback (MonoLisa not found)" } else { "MonoLisa" }
            );

            // Built here rather than via the builder's own `.manage()`
            // because the tracked-list store needs `app.path()`, which
            // isn't available until setup — see persistence.rs for why a
            // plain, `fsync`'d file is what R2-5's reproduction called for.
            let app_support_dir = app.path().app_config_dir()?;
            let tracked_path = app_support_dir.join("tracked.json");
            // Computed here (rather than down by the tray builder, where it
            // used to live) so `AppState.last_icon_width_px` can start at the
            // exact same width as the icon the builder below actually sets —
            // both reuse this one `(rgba, w, h)` rather than each calling
            // `plain_glyph_rgba()` separately and risking the two drifting.
            let (initial_rgba, initial_w, initial_h) = tray_render::plain_glyph_rgba();
            app.manage(AppState {
                http: reqwest::Client::builder()
                    .timeout(Duration::from_secs(10))
                    .build()
                    .expect("failed to build HTTP client"),
                rate_limiter: RateLimiter::new(5, Duration::from_secs(300)),
                profile_cache: Mutex::new(HashMap::new()),
                detached: Mutex::new(false),
                tracked_store: Store::load(tracked_path),
                statusline_root: app_support_dir,
                scheduler: Scheduler::new(),
                sign_in: signin::SignInRegistry::new(),
                last_tray_rect: Mutex::new(None),
                last_icon_width_px: Mutex::new(initial_w),
                tray_highlighted: Mutex::new(false),
                last_tray_segments: Mutex::new(Vec::new()),
                docked_target: Mutex::new(None),
                move_generation: Mutex::new(0),
                last_known_position: Mutex::new((0.0, 0.0)),
            });
            spawn_scheduler(app.handle().clone());

            let window = app
                .get_webview_window("main")
                .expect("the 'main' window must be declared in tauri.conf.json");
            let _ = window.hide();
            // R4-1: before anything else touches the window — the class swap
            // is what lets every later show avoid activating the application
            // (and therefore avoid dragging the captain off a full-screen
            // Space). See panel_window.rs.
            panel_window::make_nonactivating_panel(&window);
            set_popover_collection_behavior(&window);

            {
                let blur_window = window.clone();
                let blur_app = app.handle().clone();
                window.on_window_event(move |event| {
                    match event {
                        tauri::WindowEvent::Focused(focused) => {
                            if std::env::var_os("QUOTOS_DEBUG_POS").is_some() {
                                eprintln!("quotos-pos: Focused({focused}) visible={:?}", blur_window.is_visible());
                            }
                            if *focused {
                                return;
                            }
                            // Diagnostic escape hatch, off by default: the panel
                            // hides the instant anything else takes focus, which
                            // on a machine being driven by an agent is
                            // immediately — leaving no way to photograph the
                            // docked panel beside the real menu bar and judge it
                            // by eye, which is how this round's acceptance bar is
                            // actually written. Never set in a shipped run.
                            if std::env::var_os("QUOTOS_DEBUG_KEEP_OPEN").is_some() {
                                return;
                            }
                            let detached = blur_app
                                .state::<AppState>()
                                .detached
                                .lock()
                                .map(|d| *d)
                                .unwrap_or(false);
                            if !detached {
                                let _ = blur_window.hide();
                                let _ = blur_window.emit("panel-visibility", false);
                                set_tray_highlighted(&blur_app, false);
                                clear_docked_target(&blur_app);
                            }
                        }
                        // R3-3 (Magnet question): `resizable: false` in
                        // tauri.conf.json only disables the *native*
                        // resize-handle drag — it does not stop a third-party
                        // window manager like Magnet, which resizes windows
                        // by calling the Accessibility API's `kAXSizeAttribute`
                        // setter directly (that goes straight to `-setFrame:`,
                        // bypassing the style-mask resizable bit entirely).
                        // Quotos never asked Magnet's snap actions to be
                        // driven here — this only reasons about what the
                        // window's own configuration permits — but a snap
                        // action landing on this window would otherwise
                        // silently resize a fixed-size popover whose layout
                        // math (panel width, shadow margin, beak offset) all
                        // assumes exactly 360x560 logical, which cannot
                        // reflow. Snapping the size straight back is what
                        // "resist being resized into nonsense" means for a
                        // window with no resizable layout to fall back to;
                        // it does not fight a *move* (a Magnet action that
                        // only repositions is left alone), only a resize.
                        tauri::WindowEvent::Resized(size) => {
                            // Compared and reasserted in *logical* units for
                            // the same reason positions are (see
                            // `DisplayPoints`): the incoming `PhysicalSize` is
                            // the window's own scale factor applied to its
                            // point size, so converting with that same factor
                            // is the only comparison that means anything on a
                            // mixed-DPI setup.
                            let scale = blur_window.scale_factor().unwrap_or(1.0);
                            let (w, h) = (size.width as f64 / scale, size.height as f64 / scale);
                            if (w - PANEL_WINDOW_WIDTH_LOGICAL).abs() > 0.5 || (h - PANEL_WINDOW_HEIGHT_LOGICAL).abs() > 0.5 {
                                let _ = blur_window
                                    .set_size(tauri::LogicalSize::new(PANEL_WINDOW_WIDTH_LOGICAL, PANEL_WINDOW_HEIGHT_LOGICAL));
                            }
                        }
                        // R3-6: the self-correcting half of `AppState.docked_target`
                        // — whenever AppKit relocates the window away from
                        // where it's supposed to be docked (the
                        // Space-transition race `set_popover_collection_behavior`'s
                        // doc comment describes), nudges it back — but only
                        // once the relocating has gone *quiet* for
                        // `MOVE_SETTLE_MS`, not on every single `Moved`
                        // event (see `move_generation`'s doc comment for why
                        // that immediate-reapply version, this fix's first
                        // draft, plausibly made things worse). Tracks its
                        // own "am I still the most recently scheduled
                        // correction" via the generation counter — a classic
                        // debounce — so a burst of AppKit-internal
                        // relocations gets to finish before this reasserts
                        // anything, and every scheduled-but-superseded
                        // attempt is a cheap no-op.
                        tauri::WindowEvent::Moved(pos) => {
                            let state = blur_app.state::<AppState>();
                            // `WindowEvent::Moved` reports the frame origin in
                            // points multiplied by the window's *current*
                            // backing scale factor (`tao`'s
                            // `window_delegate.rs::emit_move_event`) — undone
                            // here so everything downstream compares in the one
                            // coordinate space that exists (see `DisplayPoints`).
                            let scale = blur_window.scale_factor().unwrap_or(1.0);
                            let observed = (pos.x as f64 / scale, pos.y as f64 / scale);
                            *state.last_known_position.lock().expect("last_known_position mutex poisoned") = observed;
                            let this_generation = {
                                let mut generation = state.move_generation.lock().expect("move_generation mutex poisoned");
                                *generation += 1;
                                *generation
                            };
                            let has_target = state.docked_target.lock().expect("docked_target mutex poisoned").is_some();
                            if has_target {
                                let app2 = blur_app.clone();
                                let window2 = blur_window.clone();
                                tauri::async_runtime::spawn(async move {
                                    const MOVE_SETTLE_MS: u64 = 180;
                                    tokio::time::sleep(std::time::Duration::from_millis(MOVE_SETTLE_MS)).await;
                                    let state2 = app2.state::<AppState>();
                                    let is_still_latest =
                                        *state2.move_generation.lock().expect("move_generation mutex poisoned") == this_generation;
                                    if !is_still_latest {
                                        return; // a newer Moved event superseded this one — let it debounce instead
                                    }
                                    let Some(target) = *state2.docked_target.lock().expect("docked_target mutex poisoned") else {
                                        return; // hidden or detached by the time this fired
                                    };
                                    let current = *state2.last_known_position.lock().expect("last_known_position mutex poisoned");
                                    // A tolerance, not exact equality: AppKit was logged settling
                                    // the window a point or so off whatever was requested and
                                    // never reporting that final nudge distinguishably from our
                                    // own resulting `Moved`. Correcting a sub-point gap is
                                    // imperceptible and produced an endless correct→drift→correct
                                    // loop — itself a contributor to "flickers" — while a
                                    // genuinely wrong placement (the wrong display, or off every
                                    // display) is orders of magnitude larger than this.
                                    const SETTLE_TOLERANCE_POINTS: f64 = 2.0;
                                    if (current.0 - target.x).abs() > SETTLE_TOLERANCE_POINTS
                                        || (current.1 - target.y).abs() > SETTLE_TOLERANCE_POINTS
                                    {
                                        apply_docked_position(&app2, &window2, target);
                                    }
                                });
                            }
                        }
                        _ => {}
                    }
                });
            }

            let quit_item = MenuItem::with_id(app, "quit", "Quit Quotos", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&quit_item])?;

            // Built from the same procedural glyph `set_tray_status` uses
            // (see `tray_render::plain_glyph_rgba`'s doc comment) rather than
            // a static bundled asset, so there is no window between launch
            // and the first `set_tray_status` call where a stale/blurry
            // fixed-size icon could show. `initial_rgba`/`initial_w`/
            // `initial_h` were computed up above, alongside `AppState`.
            let tray = TrayIconBuilder::with_id("main-tray")
                .icon(Image::new_owned(initial_rgba, initial_w, initial_h))
                .icon_as_template(true)
                .menu(&menu)
                .show_menu_on_left_click(false)
                // I7: the composited glyph+digits image (see `tray_render.rs`)
                // carries no text VoiceOver can read at all — without this,
                // a screen reader has nothing to announce for the item beyond
                // maybe the bare rendered percentage with no product name.
                // `set_tray_status` keeps this current as pinned digits
                // change; this is just the pre-any-data baseline.
                .tooltip("Quotos")
                .on_menu_event(|app, event| {
                    if event.id.as_ref() == "quit" {
                        app.exit(0);
                    }
                })
                .on_tray_icon_event(|tray, event| {
                    // Carried through raw, exactly as `tray-icon` reports it
                    // — the conversion into a coordinate space that actually
                    // means something happens once, in `compute_docked_layout`
                    // via `resolve_tray_point`. See `DisplayPoints` for what
                    // this number really is (it is not physical pixels, and it
                    // is not points either).
                    let rect_position = match &event {
                        TrayIconEvent::Click { rect, .. }
                        | TrayIconEvent::DoubleClick { rect, .. }
                        | TrayIconEvent::Enter { rect, .. }
                        | TrayIconEvent::Leave { rect, .. }
                        | TrayIconEvent::Move { rect, .. } => Some(rect.position),
                        _ => None,
                    };
                    let tray_xy = rect_position.map(|p| match p {
                        tauri::Position::Physical(p) => (p.x as f64, p.y as f64),
                        tauri::Position::Logical(p) => (p.x, p.y),
                    });
                    let app = tray.app_handle();
                    if let Some(xy) = tray_xy {
                        // C6: cached so `set_detached`'s snap-back path (not
                        // itself a tray event) still knows where to re-dock.
                        *app.state::<AppState>().last_tray_rect.lock().expect("last_tray_rect mutex poisoned") = Some(xy);
                    }

                    if let (
                        TrayIconEvent::Click {
                            button: tauri::tray::MouseButton::Left,
                            button_state: tauri::tray::MouseButtonState::Up,
                            ..
                        },
                        Some((tray_x, tray_y)),
                    ) = (&event, tray_xy)
                    {
                        if let Some(window) = app.get_webview_window("main") {
                            let detached = app
                                .state::<AppState>()
                                .detached
                                .lock()
                                .map(|d| *d)
                                .unwrap_or(false);
                            toggle_panel(app, &window, detached, tray_x, tray_y);
                        }
                    }
                })
                .build(app)?;
            app.manage(tray);

            // Diagnostic only, off by default: opens the panel from the tray
            // item's own rect a few seconds after launch, with no click at
            // all. The captain watches this menu bar while he works and a
            // previous round's repeated test-clicking was itself visible to
            // him; this makes the whole Space/geometry question iterable
            // without ever touching his menu bar. `tray.rect()` is available
            // without a click — only the *event* needs one.
            if std::env::var_os("QUOTOS_DEBUG_AUTO_OPEN").is_some() {
                let auto_app = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(Duration::from_secs(3)).await;
                    let _ = auto_app.clone().run_on_main_thread(move || {
                        let app = &auto_app;
                        let (Some(window), Some(tray)) = (app.get_webview_window("main"), app.tray_by_id("main-tray")) else {
                            return;
                        };
                        let Some(rect) = tray.rect().ok().flatten() else { return };
                        let (x, y) = match rect.position {
                            tauri::Position::Physical(p) => (p.x as f64, p.y as f64),
                            tauri::Position::Logical(p) => (p.x, p.y),
                        };
                        show_panel(app, &window, x, y);

                    });
                });
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod glyph_offset_tests {
    use super::glyph_center_offset_from_item_left_points;

    // R3-1 regression guard: these numbers are the real measurements taken
    // against a live tray item (see RESULT.md) — a bare-glyph item measured
    // 36pt wide with a bare 36px-wide (=18pt) composited image, and the
    // item's own visual center (glyph ink centroid, from a pixel-precise
    // screenshot) landed almost exactly on the item's geometric center, i.e.
    // 18pt in from its own left edge, not the old hardcoded 15pt.
    #[test]
    fn bare_glyph_offset_matches_the_measured_item_center() {
        // R3-11: the composited image is the 18pt glyph plus 6pt of padding
        // per side (see tray_render's SIDE_PAD_PX) = 30pt, inside an item
        // NSStatusItem makes 8pt wider still on each side. The glyph stays
        // centred in that image, so its centre remains the item's own centre.
        let offset = glyph_center_offset_from_item_left_points(Some(46.0), 60.0);
        assert!((offset - 23.0).abs() < 0.01, "expected ~23pt offset, got {offset}pt");
    }

    // A wider (pinned-digits) image still centers correctly as long as the
    // item's own measured width and the image's own known width are both
    // fresh — this is the whole point of deriving the margin instead of
    // reusing a single constant: it has to hold for any segment count, not
    // just the bare-glyph case above.
    #[test]
    fn pinned_digits_offset_scales_with_the_wider_image_not_a_fixed_constant() {
        // Real measurement: item widened from 36pt to 64pt after pinning one
        // segment, while its right edge (and therefore the implied margin)
        // stayed put — see `glyph_center_offset_from_item_left_points`.
        let offset = glyph_center_offset_from_item_left_points(Some(74.0), 58.0 * 2.0);
        // Same system margin as the bare case (by construction of these two
        // measurements — right edge held fixed), so the glyph's offset from
        // the item's own *current* left edge is unchanged even though the
        // item itself is much wider now.
        assert!((offset - 23.0).abs() < 0.01, "expected ~23pt offset, got {offset}pt");
    }

    // R3-7: the offset is a property of the tray item and its own composited
    // image, both of which are already in points — no display scale factor
    // may enter into it. Two displays of different scale must give the same
    // answer for the same item.
    #[test]
    fn the_offset_is_independent_of_any_display_scale_factor() {
        let retina = glyph_center_offset_from_item_left_points(Some(46.0), 60.0);
        let non_retina = glyph_center_offset_from_item_left_points(Some(46.0), 60.0);
        assert_eq!(retina, non_retina);
    }

    #[test]
    fn missing_item_rect_falls_back_to_glyph_flush_with_the_left_edge() {
        let offset = glyph_center_offset_from_item_left_points(None, 60.0);
        // No margin data available: the image's own known padding plus half
        // the glyph's width is the best available answer, not a crash or a
        // wildly wrong guess.
        assert!((offset - 15.0).abs() < 0.01, "got {offset}");
    }
}

#[cfg(test)]
mod docked_layout_tests {
    use super::{docked_layout_in_points, resolve_tray_point, DisplayPoints};

    // The captain's own two-display arrangement, in the global point space
    // macOS actually uses (`NSScreen.frame` / `CGDisplayBounds`): the built-in
    // Retina MacBook display at (0,0) 1728x1117 scale 2, and the external LG
    // at (-2560,-908) 2560x2880 scale 1. Every number below is expressed the
    // way one of the three real APIs would hand it over — see `DisplayPoints`.
    // Menu bar heights are the real measured ones: 33pt on the notched
    // built-in (`NSScreen` frame 1117 vs visibleFrame 1084), and a standard
    // 24pt on an unnotched external — deliberately different, because a single
    // constant being wrong on one of them is the R3-8 bug.
    const BUILT_IN: DisplayPoints =
        DisplayPoints { origin: (0.0, 0.0), size: (1728.0, 1117.0), scale: 2.0, menu_bar_bottom: Some(33.0) };
    const EXTERNAL: DisplayPoints =
        DisplayPoints { origin: (-2560.0, -855.0), size: (2560.0, 2880.0), scale: 1.0, menu_bar_bottom: Some(-831.0) };

    // A tray item as macOS lays one out: a 24pt-tall button centred in
    // whatever menu bar it is in, so it can never extend below that bar.
    // Checked against the real thing on the notched built-in, whose AX rect
    // measured (1183, 4, 36, 24) against a 33pt bar — this gives 4.5.
    // A bare tray item as it really measures: the 18pt glyph padded to a 30pt
    // image (tray_render's SIDE_PAD_PX), inside an NSStatusItem 8pt wider on
    // each side again.
    const ITEM_W: f64 = 46.0;
    const ICON_PX: f64 = 60.0; // that 30pt image at the fixed 2x convention

    fn glyph_centre(tray_left: f64, item_w: f64, icon_px: f64) -> f64 {
        tray_left + super::glyph_center_offset_from_item_left_points(Some(item_w), icon_px)
    }

    fn item_top(display: DisplayPoints) -> f64 {
        let bar_height = display.menu_bar_bottom.map(|b| b - display.origin.1).unwrap_or(33.0);
        display.origin.1 + (bar_height - 24.0).max(0.0) / 2.0
    }

    fn item_bottom(display: DisplayPoints) -> f64 {
        item_top(display) + 24.0
    }

    fn displays() -> Vec<DisplayPoints> {
        vec![BUILT_IN, EXTERNAL]
    }

    // R3-7 regression guard, built-in half. `tray-icon` multiplies the tray
    // item's true point position (1183, 0) by the *menu bar display's* scale
    // factor, so the value arriving here is 2366 — which is not a coordinate
    // in any real space. Dividing by the built-in's own scale recovers 1183;
    // dividing by the external's leaves 2366, which is off every display.
    #[test]
    fn a_tray_rect_from_the_retina_built_in_resolves_to_that_display_in_points() {
        let (index, x, y) = resolve_tray_point(&displays(), 2366.0, 0.0).expect("built-in tray point must resolve");
        assert_eq!(index, 0);
        assert!((x - 1183.0).abs() < 0.01, "got {x}");
        assert!(y.abs() < 0.01, "got {y}");
    }

    // The mirror case: the menu bar living on the 1x external display, whose
    // point coordinates are negative. Here `tray-icon`'s multiplication is by
    // 1 and the raw value is already the answer — but only if the *external*
    // display's scale is the one used to undo it. Halving it (the built-in's
    // scale) lands somewhere else entirely, which is the second half of the
    // captain's "прыгает по экрану".
    #[test]
    fn a_tray_rect_from_the_1x_external_display_resolves_to_that_display_in_points() {
        let (index, x, y) = resolve_tray_point(&displays(), -400.0, -855.0).expect("external tray point must resolve");
        assert_eq!(index, 1);
        assert!((x + 400.0).abs() < 0.01, "got {x}");
        assert!((y + 855.0).abs() < 0.01, "got {y}");
    }

    #[test]
    fn a_tray_rect_matching_no_display_resolves_to_nothing_rather_than_a_guess() {
        assert!(resolve_tray_point(&displays(), 90_000.0, 90_000.0).is_none());
    }

    // R3-10: the beak's *tip* sits `BEAK_TIP_CLEARANCE` below the menu bar of
    // the display the icon is actually on — the captain's "почти в иконке".
    // Tip = y + (panel top inset 12 − beak height 10), so the window's own top
    // lands flush with the bar: 33 on the notched built-in, −825 on the
    // external (bar bottom −831, i.e. a standard 24pt bar), whose coordinates
    // are negative and whose bar is a different height again.
    #[test]
    fn the_beaks_tip_sits_just_under_the_menu_bar_of_its_own_display() {
        let tip = |l: super::DockedLayout| l.y + (12.0 - 10.0);

        let built_in = docked_layout_in_points(BUILT_IN, 1183.0, item_top(BUILT_IN), item_bottom(BUILT_IN), Some(ITEM_W), ICON_PX);
        assert!((tip(built_in) - (33.0 + 2.0)).abs() < 0.01, "got {}", tip(built_in));

        let external = docked_layout_in_points(EXTERNAL, -400.0, item_top(EXTERNAL), item_bottom(EXTERNAL), Some(ITEM_W), ICON_PX);
        assert!((tip(external) - (-831.0 + 2.0)).abs() < 0.01, "got {}", tip(external));
    }

    // R3-8 regression guard: two displays whose menu bars differ in height must
    // get *different* tops relative to their own origin. A constant would give
    // the same number for both, which is the bug.
    #[test]
    fn the_top_follows_each_displays_own_menu_bar_height_not_one_constant() {
        let built_in = docked_layout_in_points(BUILT_IN, 1183.0, item_top(BUILT_IN), item_bottom(BUILT_IN), Some(ITEM_W), ICON_PX);
        let external = docked_layout_in_points(EXTERNAL, -400.0, item_top(EXTERNAL), item_bottom(EXTERNAL), Some(ITEM_W), ICON_PX);
        let below_own_top = |l: super::DockedLayout, d: DisplayPoints| l.y - d.origin.1;
        assert!((below_own_top(built_in, BUILT_IN) - 33.0).abs() < 0.01, "got {}", below_own_top(built_in, BUILT_IN));
        assert!((below_own_top(external, EXTERNAL) - 24.0).abs() < 0.01, "got {}", below_own_top(external, EXTERNAL));
        assert_ne!(below_own_top(built_in, BUILT_IN), below_own_top(external, EXTERNAL));
    }

    // The state the captain actually reproduces from — inside a full-screen
    // Space, where the menu bar is auto-hidden and `visibleFrame` reports no
    // bar at all even while the bar is sitting revealed under his cursor. The
    // tray item is still measured, and reconstructing the bar from it
    // (a status item is centred in its bar) must land on the same answer as
    // reading the bar directly, not on some degraded approximation.
    #[test]
    fn a_hidden_menu_bar_is_reconstructed_from_the_tray_item_to_the_same_answer() {
        let hidden_bar = DisplayPoints { menu_bar_bottom: None, ..BUILT_IN };
        let with_bar = docked_layout_in_points(BUILT_IN, 1183.0, item_top(BUILT_IN), item_bottom(BUILT_IN), Some(ITEM_W), ICON_PX);
        let without = docked_layout_in_points(hidden_bar, 1183.0, item_top(hidden_bar), item_bottom(hidden_bar), Some(ITEM_W), ICON_PX);
        assert!((without.y - with_bar.y).abs() < 0.01, "{} vs {}", without.y, with_bar.y);
    }

    // A display left of the primary has negative coordinates throughout — a
    // "no menu bar reported" floor of 0 would read as far *below* such a
    // display's real bar rather than as neutral.
    #[test]
    fn the_fallback_floor_works_on_a_negative_coordinate_display() {
        let hidden_bar = DisplayPoints { menu_bar_bottom: None, ..EXTERNAL };
        let layout = docked_layout_in_points(hidden_bar, -400.0, item_top(hidden_bar), item_bottom(hidden_bar), Some(ITEM_W), ICON_PX);
        assert!(layout.y < 0.0 && layout.y > EXTERNAL.origin.1, "got {}", layout.y);
    }

    // R3-10: the beak is held a comfortable distance inside the panel's own
    // left edge rather than pressed against it — his "слишком близко к левому
    // краю" — which, since its centre must still be the glyph's centre, is
    // achieved by moving the whole panel left rather than by moving the beak.
    #[test]
    fn the_beak_sits_clear_of_the_panels_left_corner_by_moving_the_panel_not_the_beak() {
        let layout = docked_layout_in_points(BUILT_IN, 1183.0, item_top(BUILT_IN), item_bottom(BUILT_IN), Some(ITEM_W), ICON_PX);
        assert!((layout.beak_left - 20.0).abs() < 0.01, "got {}", layout.beak_left);
        // Well clear of the 12pt corner radius, unlike the handoff's own
        // "pressed to the left edge" rule this replaces.
        assert!(layout.beak_left > 12.0);
        // ...and still exactly on the glyph.
        let beak_centre = layout.x + 14.0 + layout.beak_left + 10.0;
        assert!((beak_centre - glyph_centre(1183.0, ITEM_W, ICON_PX)).abs() < 0.01, "got {beak_centre}");
    }

    // R3-1/the captain's own acceptance bar: pinning digits widens the tray
    // item, and macOS lays status items out right-to-left, so the item's own
    // left edge moves left — carrying the glyph with it, since `NSStatusItem`
    // centres the whole composited image and the glyph is that image's
    // leftmost 18pt (measured live: glyph centre 1238.75 bare → 1209.75
    // pinned, see RESULT.md). Nothing can hold the beak still *and* under the
    // glyph, because the glyph itself moved; what must hold is that the beak
    // lands on the glyph's real centre in **both** states, with the panel
    // tracking the item the same way in both. Getting that wrong is exactly
    // the drift the captain photographed across pin/unpin.
    #[test]
    fn the_beak_lands_on_the_glyph_centre_both_before_and_after_pinning() {
        // The absolute on-screen centre of the notch = window x + the 14pt
        // shadow-blur inset + beak_left + half the 12pt notch.
        let beak_centre = |l: super::DockedLayout| l.x + 14.0 + l.beak_left + 10.0;

        // Bare: 36pt item, 18pt image; right edge at 1183 + 36 = 1219.
        let bare = docked_layout_in_points(BUILT_IN, 1183.0, item_top(BUILT_IN), item_bottom(BUILT_IN), Some(ITEM_W), ICON_PX);
        let bare_glyph_centre: f64 = glyph_centre(1183.0, ITEM_W, ICON_PX);
        assert!((beak_centre(bare) - bare_glyph_centre).abs() < 0.01, "bare: {}", beak_centre(bare));
        assert!((bare.x - (bare_glyph_centre - 44.0)).abs() < 0.01, "bare panel: {}", bare.x);

        // Pinned "29%": the item grows to 64pt with the same right edge, so
        // its left edge moves to 1219 − 64 = 1155, and the composited image
        // is 46pt wide (glyph + one segment).
        let pinned = docked_layout_in_points(BUILT_IN, 1155.0, item_top(BUILT_IN), item_bottom(BUILT_IN), Some(74.0), 58.0 * 2.0);
        let pinned_glyph_centre = glyph_centre(1155.0, 74.0, 58.0 * 2.0);
        assert!((beak_centre(pinned) - pinned_glyph_centre).abs() < 0.01, "pinned: {}", beak_centre(pinned));
        assert!((pinned.x - (pinned_glyph_centre - 44.0)).abs() < 0.01, "pinned panel: {}", pinned.x);

        // Unpinning restores the bare geometry exactly — no hysteresis, since
        // every input is re-derived rather than accumulated.
        let unpinned = docked_layout_in_points(BUILT_IN, 1183.0, item_top(BUILT_IN), item_bottom(BUILT_IN), Some(ITEM_W), ICON_PX);
        assert_eq!(bare, unpinned);
    }

    // B5: at the right screen edge the panel stops (clamped to screen right −
    // 340) and the beak keeps tracking the glyph past the panel's own centre.
    #[test]
    fn at_the_right_screen_edge_the_panel_stops_and_the_beak_keeps_tracking() {
        // Icon hard against the built-in's right edge.
        let tray_left = 1728.0 - 40.0;
        let layout = docked_layout_in_points(BUILT_IN, tray_left, item_top(BUILT_IN), item_bottom(BUILT_IN), Some(ITEM_W), ICON_PX);
        assert!((layout.x - (1728.0 - 340.0)).abs() < 0.01, "panel must clamp, got {}", layout.x);
        let beak_centre = layout.x + 14.0 + layout.beak_left + 10.0;
        assert!((beak_centre - glyph_centre(tray_left, ITEM_W, ICON_PX)).abs() < 0.01, "beak must still track the glyph, got {beak_centre}");
    }

    // The notch can never leave the panel, however extreme the geometry.
    #[test]
    fn the_beak_stays_within_the_panel() {
        for tray_left in [-3000.0f64, -2560.0, 0.0, 900.0, 1727.0, 5000.0] {
            let layout = docked_layout_in_points(BUILT_IN, tray_left, item_top(BUILT_IN), item_bottom(BUILT_IN), Some(ITEM_W), ICON_PX);
            assert!(layout.beak_left >= 0.0 && layout.beak_left <= 332.0 - 20.0, "tray_left={tray_left} gave {}", layout.beak_left);
        }
    }
}
