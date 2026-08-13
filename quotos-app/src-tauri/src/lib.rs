mod persistence;
mod providers;
mod ratelimit;
mod scheduler;
mod signin;
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
}

#[tauri::command]
fn list_accounts() -> Vec<AccountDescriptor> {
    providers::claude::discover_accounts()
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
    // slot before the network call keeps Quotos from ever being the reason
    // the captain's own /usage view starts 429ing.
    if let Err(retry_after_secs) = state.rate_limiter.try_acquire(account_id) {
        return Err(FetchError::RateLimited { retry_after_secs });
    }

    let path = PathBuf::from(config_dir);
    let usage = providers::claude::fetch_usage(&state.http, &path).await?;

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

    Ok(RawSnapshot {
        account_id: account_id.to_string(),
        provider: provider.to_string(),
        config_dir: config_dir.to_string(),
        fetched_at: chrono::Utc::now().to_rfc3339(),
        usage,
        profile,
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

#[tauri::command]
fn load_tracked(state: tauri::State<'_, AppState>) -> Vec<TrackedAccount> {
    state.tracked_store.list()
}

#[tauri::command]
fn save_tracked(state: tauri::State<'_, AppState>, tracked: Vec<TrackedAccount>) -> Result<(), String> {
    state.tracked_store.save(tracked)
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
}

#[derive(Deserialize, Clone)]
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
    *app.state::<AppState>().last_tray_segments.lock().expect("last_tray_segments mutex poisoned") = segments;
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
    *state.last_icon_width_px.lock().expect("last_icon_width_px mutex poisoned") = icon_width_px;

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
    schedule_resync_after_icon_change(app, tray.clone());
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
/// short-delay retries — the same shape as `schedule_position_correction`'s
/// fix for a different, unrelated AppKit-async-layout race — rather than
/// trusting the first read: each retry just re-reads and re-applies,
/// harmless if the previous attempt already landed on the right numbers.
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
        let _ = window.set_focus();
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
            let layout = reposition_under_tray(&app, &window, tray_x, tray_y);
            schedule_position_correction(&window, layout);
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
fn glyph_center_offset_from_item_left_physical(item_width_physical: Option<f64>, icon_width_px: f64, scale: f64) -> f64 {
    const GLYPH_WIDTH_LOGICAL: f64 = 18.0; // fixed: tray_render's glyph is always drawn at this size first
    let image_width_physical = (icon_width_px / 2.0) * scale;
    let margin_physical = item_width_physical.map(|w| ((w - image_width_physical) / 2.0).max(0.0)).unwrap_or(0.0);
    margin_physical + (GLYPH_WIDTH_LOGICAL / 2.0) * scale
}

/// The window's own fixed logical size, from `tauri.conf.json`'s `width`/
/// `height` — duplicated here (rather than read back from the window, which
/// would just report whatever size it currently, possibly wrongly, is) so
/// the Resized-event guard in `run`'s `setup` has a ground truth to snap
/// back to.
const PANEL_WINDOW_WIDTH_LOGICAL: f64 = 360.0;
const PANEL_WINDOW_HEIGHT_LOGICAL: f64 = 560.0;

fn compute_docked_layout(app: &tauri::AppHandle, window: &tauri::WebviewWindow, tray_x: f64, tray_y: f64) -> Option<(i32, i32, f64)> {
    let monitor = window.monitor_from_point(tray_x, tray_y).ok().flatten().or_else(|| window.current_monitor().ok().flatten())?;

    let scale = monitor.scale_factor();
    let monitor_pos = *monitor.position();
    let monitor_size = *monitor.size();

    let left_offset_px = (6.0 * scale).round() as i32;
    let top_px = (32.0 * scale).round() as i32;
    let right_clamp_px = (340.0 * scale).round() as i32;

    let mut x = tray_x.round() as i32 - left_offset_px;
    x = x.max(monitor_pos.x);
    let max_x = (monitor_pos.x + monitor_size.width as i32 - right_clamp_px).max(monitor_pos.x);
    x = x.min(max_x);

    let y = monitor_pos.y + top_px;

    const PANEL_WIDTH_LOGICAL: f64 = 332.0;
    // Panel.jsx's beak is a 12x12 box rotated in place (default transform
    // origin, so its own visual center never moves off `left + 6`) — the
    // frontend assigns whatever this function returns straight to that
    // box's CSS `left`, i.e. the box's own *left edge*, not its center. And
    // `left` is relative to the panel's own container, which itself sits
    // inset inside the (wider, for shadow-blur margin — see app.css/B9)
    // window: `body`'s `padding-top`-plus-flex-center leaves `(window width
    // − panel width) / 2` of empty margin on each side. Both of those were
    // previously missing from this math — the value computed above only
    // ever matched the *window's* left edge, not the panel's, and was fed
    // in as a center when the frontend treats it as a left edge — so the
    // beak rendered up to ~20pt off from the glyph even when the glyph
    // center itself (`glyph_center_physical`, above) was correct. See
    // `RESULT.md` for the live screenshot measurements that caught this.
    const BEAK_BOX_WIDTH_LOGICAL: f64 = 12.0;
    const BEAK_HALF_WIDTH_LOGICAL: f64 = BEAK_BOX_WIDTH_LOGICAL / 2.0;
    let panel_inset_logical = (PANEL_WINDOW_WIDTH_LOGICAL - PANEL_WIDTH_LOGICAL) / 2.0;

    let item_width_physical = app
        .tray_by_id("main-tray")
        .and_then(|t| t.rect().ok().flatten())
        .map(|r| match r.size {
            tauri::Size::Physical(s) => s.width as f64,
            tauri::Size::Logical(s) => s.width * scale,
        });
    let icon_width_px = *app.state::<AppState>().last_icon_width_px.lock().expect("last_icon_width_px mutex poisoned") as f64;
    let glyph_offset_physical = glyph_center_offset_from_item_left_physical(item_width_physical, icon_width_px, scale);

    let glyph_center_physical = tray_x + glyph_offset_physical;
    let panel_left_physical = x as f64 + panel_inset_logical * scale;
    let beak_center_logical = (glyph_center_physical - panel_left_physical) / scale;
    let beak_left_logical = beak_center_logical - BEAK_HALF_WIDTH_LOGICAL;
    // Docked at `icon_left - 6` (unclamped by the screen edge), the glyph
    // sits close to the panel's own left edge by design — the handoff's own
    // words are "клюв прижат к левому краю" (beak pressed to the left
    // edge) — so this only needs to keep the 12px box's *source* rect
    // within the panel's own width, not enforce some larger cosmetic
    // minimum (an earlier version of this clamp did that, using bounds
    // meant for a *center* value on what was actually a raw, uncorrected
    // left-edge value — see this function's earlier bug, above — which
    // fought the "pressed to the left edge" design on every normal, non-
    // clamped-by-screen-edge open). B5's right-screen-edge case is what
    // actually needs headroom: there, `x` above already clamped the panel's
    // own left edge, so `beak_center_logical` grows well past this range on
    // its own to keep tracking the glyph.
    let beak_left_logical = beak_left_logical.clamp(0.0, PANEL_WIDTH_LOGICAL - BEAK_BOX_WIDTH_LOGICAL);

    Some((x, y, beak_left_logical))
}

/// Applies an already-computed docked position/beak-offset — split out from
/// `reposition_under_tray` so a delayed correction (see `show_panel`) can
/// reapply the exact numbers computed the first time instead of re-deriving
/// them via `compute_docked_layout`'s own `monitor_from_point` call, which
/// was observed giving a *different, still-wrong* answer when queried from
/// a window AppKit had already relocated mid-Space-transition.
fn apply_docked_position(window: &tauri::WebviewWindow, x: i32, y: i32, beak_offset: f64) {
    let _ = window.set_position(tauri::PhysicalPosition::new(x, y));
    let _ = window.emit("panel-beak-offset", beak_offset);
}

/// Moves an already-visible, already-docked-chrome window to sit under the
/// tray icon and tells the frontend where to draw the beak — the shared
/// tail end of both `show_panel` and `set_detached`'s snap-back path (C6).
/// Returns what it applied (or `None` if no monitor could be resolved) so
/// callers can schedule a same-numbers correction — see `show_panel`'s doc
/// comment on `set_popover_collection_behavior` for why that's needed.
fn reposition_under_tray(app: &tauri::AppHandle, window: &tauri::WebviewWindow, tray_x: f64, tray_y: f64) -> Option<(i32, i32, f64)> {
    let layout = compute_docked_layout(app, window, tray_x, tray_y)?;
    apply_docked_position(window, layout.0, layout.1, layout.2);
    Some(layout)
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
/// So: `MoveToActiveSpace`, which is the one that actually works for
/// visibility, plus a short-delay correction in `show_panel` that
/// *reapplies the exact position already computed the first time* rather
/// than recomputing it — recomputing via `monitor_from_point` after the
/// fact was tried too and was itself unreliable (querying it from a window
/// AppKit has already relocated mid-Space-transition returned yet a third,
/// still-wrong position on one run). Reapplying the same literal numbers
/// sidesteps whatever's wrong with the query, not just the transition.
/// Paired with `.Transient` (the standard flag for this kind of ephemeral
/// popover — keeps it out of Mission Control/Exposé's per-Space window
/// list, matching how the Volume/Wi-Fi popovers behave, rather than leaving
/// a phantom entry on every Space it's ever visited).
#[cfg(target_os = "macos")]
fn set_popover_collection_behavior(window: &tauri::WebviewWindow) {
    use objc2_app_kit::{NSWindow, NSWindowCollectionBehavior};
    let Ok(ptr) = window.ns_window() else { return };
    if ptr.is_null() {
        return;
    }
    let ns_window: &NSWindow = unsafe { &*(ptr as *const NSWindow) };
    ns_window.setCollectionBehavior(NSWindowCollectionBehavior::MoveToActiveSpace | NSWindowCollectionBehavior::Transient);
}

#[cfg(not(target_os = "macos"))]
fn set_popover_collection_behavior(_window: &tauri::WebviewWindow) {}

fn show_panel(app: &tauri::AppHandle, window: &tauri::WebviewWindow, tray_x: f64, tray_y: f64) {
    let _ = window.show();
    let _ = window.set_focus();
    let layout = reposition_under_tray(app, window, tray_x, tray_y);
    let _ = window.emit("panel-visibility", true);
    schedule_position_correction(window, layout);
    set_tray_highlighted(app, true);
}

/// `NSWindowCollectionBehaviorMoveToActiveSpace` (see
/// `set_popover_collection_behavior`) is what makes the window actually
/// appear on the user's current Space at all — necessary, confirmed by
/// `kCGWindowIsOnscreen` flipping true for the first time in this whole
/// investigation once it was added. But the Space transition it triggers is
/// itself asynchronous: logging showed the window moving *again*, ~100-150ms
/// after the initial explicit `set_position` call, undoing it (observed
/// final position was nowhere near the target — some AppKit-internal
/// default for "a window landing on a Space it wasn't already positioned
/// on," not anything this code requests). Reapplying the *exact same
/// already-computed* position/beak-offset (not recomputed — see
/// `apply_docked_position`'s doc comment) a little after that settles
/// reliably wins the race; each reapplication is a harmless no-op if
/// nothing moved the window in between. Used by both `show_panel` and
/// `set_detached`'s snap-back path (C6), since re-docking from detached can
/// hit the same Space-transition race.
fn schedule_position_correction(window: &tauri::WebviewWindow, layout: Option<(i32, i32, f64)>) {
    let Some((x, y, beak_offset)) = layout else { return };
    let correction_window = window.clone();
    tauri::async_runtime::spawn(async move {
        for delay_ms in [250, 600] {
            tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
            apply_docked_position(&correction_window, x, y, beak_offset);
        }
    });
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
            let tracked_path = app.path().app_config_dir()?.join("tracked.json");
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
                scheduler: Scheduler::new(),
                sign_in: signin::SignInRegistry::new(),
                last_tray_rect: Mutex::new(None),
                last_icon_width_px: Mutex::new(initial_w),
                tray_highlighted: Mutex::new(false),
                last_tray_segments: Mutex::new(Vec::new()),
            });
            spawn_scheduler(app.handle().clone());

            let window = app
                .get_webview_window("main")
                .expect("the 'main' window must be declared in tauri.conf.json");
            let _ = window.hide();
            set_popover_collection_behavior(&window);

            {
                let blur_window = window.clone();
                let blur_app = app.handle().clone();
                window.on_window_event(move |event| {
                    match event {
                        tauri::WindowEvent::Focused(false) => {
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
                            let scale = blur_window.scale_factor().unwrap_or(1.0);
                            let expected_w = (PANEL_WINDOW_WIDTH_LOGICAL * scale).round() as u32;
                            let expected_h = (PANEL_WINDOW_HEIGHT_LOGICAL * scale).round() as u32;
                            if size.width != expected_w || size.height != expected_h {
                                let _ = blur_window.set_size(tauri::PhysicalSize::new(expected_w, expected_h));
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
                    // Always physical pixels for a real tray event — see
                    // `show_panel`'s doc comment for why this is read
                    // straight off the event rather than through the
                    // positioner plugin's own cached/derived position.
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

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod glyph_offset_tests {
    use super::glyph_center_offset_from_item_left_physical;

    // R3-1 regression guard: these numbers are the real measurements taken
    // against a live tray item (see RESULT.md) — a bare-glyph item measured
    // 36pt wide with a bare 36px-wide (=18pt) composited image, and the
    // item's own visual center (glyph ink centroid, from a pixel-precise
    // screenshot) landed almost exactly on the item's geometric center, i.e.
    // 18pt in from its own left edge, not the old hardcoded 15pt.
    #[test]
    fn bare_glyph_offset_matches_the_measured_item_center() {
        let scale = 2.0;
        let item_width_physical = 36.0 * scale;
        let icon_width_px = 36.0; // GLYPH_PX: bare glyph, no digits
        let offset = glyph_center_offset_from_item_left_physical(Some(item_width_physical), icon_width_px, scale);
        assert!((offset - 18.0 * scale).abs() < 0.01, "expected ~18pt offset, got {}pt", offset / scale);
    }

    // A wider (pinned-digits) image still centers correctly as long as the
    // item's own measured width and the image's own known width are both
    // fresh — this is the whole point of deriving the margin instead of
    // reusing a single constant: it has to hold for any segment count, not
    // just the bare-glyph case above.
    #[test]
    fn pinned_digits_offset_scales_with_the_wider_image_not_a_fixed_constant() {
        let scale = 2.0;
        // Real measurement: item widened from 36pt to 64pt after pinning one
        // segment, while its right edge (and therefore the implied margin)
        // stayed put — see compute_docked_layout's doc comment.
        let item_width_physical = 64.0 * scale;
        let icon_width_px = 46.0 * 2.0; // a wider composited image (glyph + one segment), at the fixed 2x convention
        let offset = glyph_center_offset_from_item_left_physical(Some(item_width_physical), icon_width_px, scale);
        // Same 18pt margin as the bare case (by construction of these two
        // measurements — right edge held fixed), so the glyph's offset from
        // the item's own *current* left edge is unchanged even though the
        // item itself is much wider now.
        assert!((offset - 18.0 * scale).abs() < 0.01, "expected ~18pt offset, got {}pt", offset / scale);
    }

    #[test]
    fn missing_item_rect_falls_back_to_glyph_flush_with_the_left_edge() {
        let scale = 2.0;
        let icon_width_px = 36.0;
        let offset = glyph_center_offset_from_item_left_physical(None, icon_width_px, scale);
        // No margin data available: half the glyph's own width is the best
        // available answer, not a crash or a wildly wrong guess.
        assert!((offset - 9.0 * scale).abs() < 0.01);
    }
}
