mod accounts;
mod geometry;
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

use serde::Deserialize;
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{TrayIconBuilder, TrayIconEvent};
use tauri::{Emitter, Manager};

use persistence::Store;
use ratelimit::RateLimiter;
use scheduler::Scheduler;

use geometry::{
    displays_in_points, docked_layout_in_points, drag_target_from_anchor, resolve_tray_point, DockedLayout,
    PANEL_WINDOW_HEIGHT_LOGICAL, PANEL_WINDOW_WIDTH_LOGICAL,
};

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
    /// v4: the glyph's own arc fill last set by `set_tray_status` (0-100) —
    /// the worst active limit across everything tracked, design/NOTES.md
    /// §1. Cached the same way `last_tray_segments` is, for the same reason:
    /// `repaint_tray_icon` is the one place that actually redraws, and it's
    /// called from both `set_tray_status` (frontend data) and
    /// `set_tray_highlighted` (native visibility), neither of which knows
    /// the other's current value.
    last_tray_worst_used_percent: Mutex<u8>,
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
    /// Anchor for `drag_window_step`'s manual, frame-based detached-window
    /// drag (see that function's doc comment for why native
    /// `performWindowDragWithEvent:` dragging isn't used) — the cursor and
    /// the window's own top-left, both in global points, captured on the
    /// first `mousemove` of a header-drag gesture. `None` whenever no manual
    /// drag is in progress.
    manual_drag_anchor: Mutex<Option<((f64, f64), (f64, f64))>>,
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
    /// v4: see `tray_render::TraySegment::group_start`.
    group_start: bool,
}

/// Sets what's shown beside the tray glyph — the pinned-subscriptions
/// feature (R2-2). Empty `segments` clears it back to just the plain,
/// theme-tinted glyph (unless the panel is currently open — see
/// `repaint_tray_icon`, A11). v4: `worst_used_percent` (0-100) is the bare
/// glyph's own arc fill — design/NOTES.md §1's "worst active limit across
/// everything tracked", sent on every call regardless of `segments` so the
/// arc stays current whether or not anything is pinned.
#[tauri::command]
fn set_tray_status(app: tauri::AppHandle, segments: Vec<TraySegmentDto>, worst_used_percent: u8) -> Result<(), String> {
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
        let mut last_worst = state.last_tray_worst_used_percent.lock().expect("last_tray_worst_used_percent mutex poisoned");
        if *last == segments && *last_worst == worst_used_percent {
            return Ok(());
        }
        *last = segments;
        *last_worst = worst_used_percent;
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
    let worst_used_percent = *state.last_tray_worst_used_percent.lock().expect("last_tray_worst_used_percent mutex poisoned");

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
        let (rgba, w, h) = tray_render::plain_glyph_rgba(worst_used_percent);
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
                group_start: s.group_start,
            })
            .collect();
        let (rgba, w, h) = tray_render::render(&segs, highlighted, worst_used_percent);
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
/// `App.tsx`'s own header-drag calling `drag_window_step` on every
/// `mousemove` (see that function's doc comment), which needs no native
/// title bar at all. Whether decorations were the actual reason the
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
    if detached {
        // R3-9's own doc comment: `NSFloatingWindowLevel` (what this sets) is
        // right for a free-floating detached window, and specifically wrong
        // for the popover — see the `else` branch below for why this can't
        // just be called unconditionally for both.
        window.set_always_on_top(true).map_err(|e| e.to_string())?;
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
        // Snapping back must restore `NSStatusWindowLevel` — needed to stay
        // above a full-screen Space the same way the tray-click-driven open
        // does — which is what `set_popover_collection_behavior` below does.
        // It must be the *only* level-setting call in this branch: `tao`'s
        // `set_always_on_top` (what the `if` branch above calls) ends in
        // `util::set_level_async`, a `DispatchQueue.main.async` — calling it
        // here too would schedule a *later* runloop turn to drop the level
        // back to floating right after this synchronous restore set it to
        // status, undoing it. Confirmed live: with both calls present, a
        // temporary trace showed this function's own `setLevel(25)` running
        // and returning, and the window still read back at the floating
        // level moments later — the async callback from a stale
        // `set_always_on_top(true)` (still queued from the detach that
        // preceded this snap-back) won the race by construction, since it
        // can only run after this whole synchronous command returns. Without
        // this fix a single detach+reattach permanently stuck the popover at
        // the lower level for the rest of the app's run — unreachable, and so
        // unseen, until dragging itself was fixed (see `drag_window_step`'s
        // doc comment for that story).
        set_popover_collection_behavior(&window);
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

/// I7's detached-window drag moves the window by hand, one `mousemove` at a
/// time, instead of calling AppKit's `-[NSWindow performWindowDragWithEvent:]`
/// (what `tao`'s `startDragging()`/`drag_window()` bottoms out in — and what
/// the header actually called before this). Two independent things were found
/// wrong with that native path, in order:
///
/// 1. `startDragging()`'s IPC call (`plugin:window|start_dragging`) was
///    silently denied the whole time — `capabilities/default.json` never
///    granted `core:window:allow-start-dragging` (it isn't part of
///    `core:window:default`/`core:default`, unlike `allow-show`/`allow-hide`/
///    etc., which are explicitly listed there for the same reason). The
///    denial rejects the JS promise, and since the call site never awaited or
///    caught it, the rejection was invisible — this alone fully explains "the
///    window doesn't drag at all," independent of anything below.
/// 2. Once that permission is granted, `performWindowDragWithEvent:` *does*
///    move the window — but doing so while the mouse stays down measurably
///    reactivates the app, which is exactly the Space-losing bug
///    `panel_window.rs` (R4-1) exists to prevent. This function's own
///    replacement mechanism (see below) turned out to have the **same**
///    problem, which is the more important finding: it is not specific to
///    `performWindowDragWithEvent:`.
///
/// The measurement (`NSWorkspace.frontmostApplication`, read from a separate
/// process — the same ground truth `panel_window.rs`'s own A/B uses, since
/// `-[NSApplication isActive]` read from inside is useless here too): a
/// synthetic click-and-hold on the header that never moves the cursor never
/// activates the app, and neither does a full click-drag-release gesture
/// anywhere else in the panel that never touches the window's frame — so
/// WKWebView gaining first responder, or a live mouse-tracking gesture by
/// itself, isn't the trigger. What *does* trigger it, reproduced cleanly and
/// repeatedly: **any call that changes the window's frame while the mouse
/// button is still down over it** — and that held not just for
/// `performWindowDragWithEvent:`, but for this function's own replacement,
/// the plain, otherwise-inert `place_window_top_left_sync` (`-[NSWindow
/// setFrameTopLeftPoint:]`, the same call the tray-docking path already uses
/// with no activation side effect *outside* a live mouse-down gesture). A
/// single frame-set mid-gesture was enough — it does not need repetition.
/// Reproduced identically whether the window is the non-activating
/// `QuotosNonActivatingPanel` or the old activating path
/// (`QUOTOS_PANEL_MODE=window`), so this is not something the R4-1 panel swap
/// caused or can be asked to fix by itself, and not something a different
/// repositioning API sidesteps either. Read as a plain fact about this OS:
/// relocating a window while the user is actively holding the mouse down on
/// it appears to carry an implicit "bring this app forward" outside
/// `NSWindowStyleMaskNonactivatingPanel`'s own promise, which is scoped to
/// key/main status, not to window-server-level drag handling.
///
/// So this is a **partial** fix, not a closed one: `drag_window_step` below
/// does make the window move — which it never did before point 1 above — but
/// live-following the cursor while the button is held still reactivates the
/// app for that gesture's duration, same as the native path did. Escalated
/// rather than picked silently; the captain's call (2026-08-15): ship it —
/// live-follow dragging, reactivation and all, beats a live "hand focus back"
/// correction (rejected, unverifiable from this sandbox against a real
/// full-screen Space) and a commit-only-on-mouseup shape (rejected, feels
/// like repositioning rather than dragging). Reactivation is scoped to the
/// physical gesture only — never on show, never on a Magnet/AX move.
///
/// `drag_window_step` is called on every `mousemove` while a header drag is
/// in progress. The first call of a gesture only records where the cursor and
/// the window each started (`AppState.manual_drag_anchor`); every call after
/// that sets the window's frame directly from the live delta, via the same
/// synchronous `place_window_top_left_sync` the docking path already uses.
#[tauri::command]
fn drag_window_step(window: tauri::WebviewWindow, state: tauri::State<'_, AppState>) {
    let Some((mouse, window_origin)) = current_mouse_and_window_points(&window) else { return };
    let mut anchor = state.manual_drag_anchor.lock().expect("manual_drag_anchor mutex poisoned");
    match *anchor {
        None => *anchor = Some((mouse, window_origin)),
        Some((anchor_mouse, anchor_window)) => {
            let (x, y) = drag_target_from_anchor(anchor_mouse, anchor_window, mouse);
            drop(anchor);
            place_window_top_left_sync(&window, x, y);
        }
    }
}

/// Clears `AppState.manual_drag_anchor` at the end of a header-drag gesture
/// (mouseup) — without this, the *next* drag's first `drag_window_step` call
/// would see the previous gesture's stale anchor instead of re-anchoring to
/// where this new one actually started, and jump the window on its first
/// move.
#[tauri::command]
fn end_window_drag(state: tauri::State<'_, AppState>) {
    *state.manual_drag_anchor.lock().expect("manual_drag_anchor mutex poisoned") = None;
}

/// Reads the live global mouse location and the window's own current
/// top-left, both in the same CG-style (y-down, top-left-of-primary-screen)
/// global points `place_window_top_left_sync` writes in — see
/// `DisplayPoints`'s doc comment for why points, not physical pixels, are the
/// only coordinate space safe for this kind of arithmetic. Both `NSEvent
/// .mouseLocation` and `NSWindow.frame` are natively in points already, so
/// unlike `TrayIconEvent.rect`/`Monitor.position()` there is no per-display
/// scale factor to resolve here at all.
#[cfg(target_os = "macos")]
fn current_mouse_and_window_points(window: &tauri::WebviewWindow) -> Option<((f64, f64), (f64, f64))> {
    use objc2_app_kit::{NSEvent, NSScreen, NSWindow};
    use objc2_foundation::MainThreadMarker;

    let mtm = MainThreadMarker::new()?;
    let ptr = window.ns_window().ok()?;
    if ptr.is_null() {
        return None;
    }
    let screens = NSScreen::screens(mtm);
    let primary = screens.iter().next()?;
    let flip = primary.frame().size.height;

    let mouse = NSEvent::mouseLocation();
    let ns_window: &NSWindow = unsafe { &*(ptr as *const NSWindow) };
    let frame = ns_window.frame();
    let window_top = frame.origin.y + frame.size.height;

    Some(((mouse.x, flip - mouse.y), (frame.origin.x, flip - window_top)))
}

#[cfg(not(target_os = "macos"))]
fn current_mouse_and_window_points(_window: &tauri::WebviewWindow) -> Option<((f64, f64), (f64, f64))> {
    None
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
            accounts::list_accounts,
            accounts::fetch_snapshot,
            accounts::load_tracked,
            accounts::save_tracked,
            accounts::kick_scheduler,
            hide_panel,
            set_tray_status,
            set_detached,
            drag_window_step,
            end_window_drag,
            accounts::debug_rate_limit_snapshot,
            accounts::start_sign_in,
            accounts::submit_sign_in_code,
            accounts::cancel_sign_in,
            accounts::forget_sign_in,
            accounts::statusline_status,
            accounts::statusline_install,
            accounts::statusline_remove,
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
            let (initial_rgba, initial_w, initial_h) = tray_render::plain_glyph_rgba(0);
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
                last_tray_worst_used_percent: Mutex::new(0),
                docked_target: Mutex::new(None),
                move_generation: Mutex::new(0),
                last_known_position: Mutex::new((0.0, 0.0)),
                manual_drag_anchor: Mutex::new(None),
            });
            accounts::spawn_scheduler(app.handle().clone());

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
