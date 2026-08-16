//! The interactive shell: the tray icon's repaint pipeline and the panel
//! window's show/hide/dock/detach/drag lifecycle. One module rather than
//! two because the two sides are mutually recursive — repainting the tray
//! can move the open panel (`repaint_tray_icon` →
//! `schedule_resync_after_icon_change` → `reposition_under_tray`), and
//! showing or hiding the panel repaints the tray (`show_panel`/`hide_panel`
//! → `set_tray_highlighted`). The pure layers stay out: coordinate math in
//! `geometry`, bitmap composition in `tray_render`, the NSPanel class swap
//! in `panel_window`.

use std::time::Duration;

use serde::Deserialize;
use tauri::image::Image;
use tauri::{Emitter, Manager};

use crate::geometry::{
    displays_in_points, docked_layout_in_points, drag_target_from_anchor, resolve_tray_point,
    DockedLayout, DragAnchor,
};
use crate::{panel_window, tray_render, AppState};

#[tauri::command]
pub(crate) fn hide_panel(app: tauri::AppHandle, window: tauri::WebviewWindow) {
    let _ = window.hide();
    let _ = window.emit("panel-visibility", false);
    set_tray_highlighted(&app, false);
    clear_docked_target(&app);
}

#[derive(Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TraySegmentDto {
    text: String,
    color: String, // "neutral" | "amber" | "red"
    /// v4: see `tray_render::TraySegment::group_start`.
    group_start: bool,
}

/// Sets what's shown beside the tray glyph — the pinned-subscriptions
/// feature (R2-2). Empty `segments` clears it back to just the plain,
/// theme-tinted glyph (unless the panel is currently open — see
/// `repaint_tray_icon`, A11). v4: `worst_used_percent` (0-100) is the bare
/// glyph's own arc fill — docs/design/NOTES.md §1's "worst active limit across
/// everything tracked", sent on every call regardless of `segments` so the
/// arc stays current whether or not anything is pinned.
#[tauri::command]
pub(crate) fn set_tray_status(
    app: tauri::AppHandle,
    segments: Vec<TraySegmentDto>,
    worst_used_percent: u8,
    tooltip: String,
) -> Result<(), String> {
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
        let mut last = state
            .last_tray_segments
            .lock()
            .expect("last_tray_segments mutex poisoned");
        let mut last_worst = state
            .last_tray_worst_used_percent
            .lock()
            .expect("last_tray_worst_used_percent mutex poisoned");
        let mut last_tooltip = state
            .last_tray_tooltip
            .lock()
            .expect("last_tray_tooltip mutex poisoned");
        // The tooltip participates in the skip guard because it can change
        // alone: renaming a subscription rewrites its tooltip line while
        // leaving every digit byte-identical.
        if *last == segments && *last_worst == worst_used_percent && *last_tooltip == tooltip {
            return Ok(());
        }
        *last = segments;
        *last_worst = worst_used_percent;
        *last_tooltip = tooltip;
    }
    repaint_tray_icon(&app, &tray)
}

/// A11: flips the tray icon's "panel open" highlight on or off and repaints.
/// Called from `show_panel`/`hide_panel`/the click-away and hide branches —
/// never from the frontend directly, since it's a pure reflection of native
/// window visibility, not app data.
pub(crate) fn set_tray_highlighted(app: &tauri::AppHandle, highlighted: bool) {
    let Some(tray) = app.tray_by_id("main-tray") else {
        return;
    };
    *app.state::<AppState>()
        .tray_highlighted
        .lock()
        .expect("tray_highlighted mutex poisoned") = highlighted;
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
    let segments = state
        .last_tray_segments
        .lock()
        .expect("last_tray_segments mutex poisoned")
        .clone();
    let highlighted = *state
        .tray_highlighted
        .lock()
        .expect("tray_highlighted mutex poisoned");
    let worst_used_percent = *state
        .last_tray_worst_used_percent
        .lock()
        .expect("last_tray_worst_used_percent mutex poisoned");

    tray.set_title(Some("")).map_err(|e| e.to_string())?;

    // I7: the composited image carries no text a screen reader can read —
    // the tooltip names every pinned figure, with the product name. Composed
    // in full by the frontend (`lib/traySegments.ts`'s `buildTrayTooltip`,
    // where the labels live) and cached in `AppState` alongside the
    // segments, so a native-only repaint (highlight toggle) keeps it.
    let tooltip = state
        .last_tray_tooltip
        .lock()
        .expect("last_tray_tooltip mutex poisoned")
        .clone();
    tray.set_tooltip(Some(&tooltip))
        .map_err(|e| e.to_string())?;

    let icon_width_px = if segments.is_empty() && !highlighted {
        let (rgba, w, h) = tray_render::plain_glyph_rgba(worst_used_percent);
        tray.set_icon(Some(Image::new_owned(rgba, w, h)))
            .map_err(|e| e.to_string())?;
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
        tray.set_icon(Some(Image::new_owned(rgba, w, h)))
            .map_err(|e| e.to_string())?;
        tray.set_icon_as_template(false)
            .map_err(|e| e.to_string())?;
        w
    };
    let width_changed = {
        let mut last = state
            .last_icon_width_px
            .lock()
            .expect("last_icon_width_px mutex poisoned");
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
    sync_status_item_length(tray, icon_width_px);

    if width_changed {
        schedule_resync_after_icon_change(app, tray.clone());
    }
    Ok(())
}

/// Tray frame unification (quotos-tray-frame-t1): `tray-icon` v0.24.2
/// always creates the status item with `NSVariableStatusItemLength` (see
/// `TrayIcon::create` in the crate source) and never touches its length
/// again after that. A variable-length item's *button* is not the same
/// rect as its own image: measured live, with nothing pinned the item
/// reported 46pt wide for a 28pt image, and with five digit segments
/// pinned it reported 208pt for a 190pt image — 18pt of extra width both
/// times, i.e. a fixed ~9pt AppKit margin on each side, independent of
/// content. That extra margin is both defects the captain reported: it is
/// the oversized gap to the next menu bar extra (nothing else reserves
/// that space), and it is why a plain click's native highlight — which
/// AppKit paints across the *button's* bounds — reads wider than the
/// panel-open pill this app draws itself, into the *image's* bounds (see
/// `tray_render::draw_highlight_background`).
///
/// Pinning the item to a fixed length exactly matching the composited
/// image removes the margin outright, so the button's bounds and the
/// image's bounds become the same rect (measured live post-fix: 192pt
/// reported for a 190pt image, the ~1pt/side left over matching AppKit's
/// own minimal button content inset, not a reintroduced margin) — both
/// draws then share one frame, which is the whole fix for defect 2.
///
/// This alone does **not** fully close defect 1's gap to the next menu bar
/// extra, and the reason is a real, resolved finding, not a loose end: this
/// function shrinks the button *symmetrically* about its own centre (AppKit
/// re-centres on `setLength`, confirmed live — the item's left edge moved
/// right by the same ~8pt its right edge moved left), so only about half of
/// the removed margin ever reaches the trailing edge. Measured live before
/// and after this function alone, with the same five pinned segments, the
/// visual gap from the last digit's own ink to the neighbouring extra's ink
/// held at 32pt either way — the ~8pt freed on the trailing side just
/// became a wider no-man's-land between Quotos's new (narrower) right edge
/// and the neighbour, which doesn't move: nothing here compacts sibling
/// status items together, and no evidence of a macOS-enforced minimum
/// inter-item spacing was found either (that "no-man's-land" component
/// measured 17.5-18.5pt across this investigation, already at or under the
/// 19-21pt baseline gap between two *unrelated* neighbours on the same
/// bar). The quotos-tray-frame-t1 followup traced the rest of the 32pt to
/// `tray_render`'s own trailing `CELL_WIDTH_PX` reserve — genuinely
/// content layout, but the one piece of it the followup authorized trimming
/// (see that constant's own doc comment) — and closing that got the
/// measured gap from 32pt down to 25pt, within `SIDE_PAD_PX` plus a couple
/// of points of the 19-21pt baseline. `sync_status_item_length` itself
/// didn't change for that; it just now reflects a narrower image. Must run
/// on every repaint, not just once: unlike a variable-length item, a
/// fixed-length one never resizes itself when a new, differently-sized
/// image is set — leaving it stale would clip or under-fill the button the
/// next time the digit count changes.
#[cfg(target_os = "macos")]
pub(crate) fn sync_status_item_length(tray: &tauri::tray::TrayIcon, icon_width_px: u32) {
    // `tray_render`'s buffer is always 2x an 18pt-tall image (see
    // `GLYPH_PX`'s doc comment), regardless of the display's own backing
    // scale — so `/2.0` is this image's real width in points on any
    // display, the same convention `geometry.rs`'s
    // `glyph_center_offset_from_item_left_points` already relies on.
    let width_points = icon_width_px as f64 / 2.0;
    let _ = tray.with_inner_tray_icon(move |inner| {
        if let Some(status_item) = inner.ns_status_item() {
            status_item.setLength(width_points);
        }
    });
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn sync_status_item_length(_tray: &tauri::tray::TrayIcon, _icon_width_px: u32) {}

/// Re-reads the tray item's *current* rect and, if the panel is visible and
/// still docked (never while detached — the window isn't under the icon at
/// all then), re-applies the docked position/beak-offset from it. See
/// `set_tray_status`'s call site for why this needs to exist at all: pin/
/// unpin changes the icon's width (and therefore its on-screen x) with no
/// tray click to refresh `last_tray_rect` from. Returns whether it actually
/// repositioned anything, so `schedule_resync_after_icon_change` knows
/// whether a retry is still needed.
fn resync_docked_position_after_icon_change(
    app: &tauri::AppHandle,
    tray: &tauri::tray::TrayIcon,
) -> Option<(f64, f64)> {
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
    *state
        .last_tray_rect
        .lock()
        .expect("last_tray_rect mutex poisoned") = Some((tray_x, tray_y));
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
pub(crate) fn set_detached(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    state: tauri::State<'_, AppState>,
    detached: bool,
) -> Result<(), String> {
    *state.detached.lock().expect("detached mutex poisoned") = detached;
    window
        .set_skip_taskbar(!detached)
        .map_err(|e| e.to_string())?;
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
        if let Some((tray_x, tray_y)) = *state
            .last_tray_rect
            .lock()
            .expect("last_tray_rect mutex poisoned")
        {
            reposition_under_tray(&app, &window, tray_x, tray_y);
        }
    }
    Ok(())
}

fn compute_docked_layout(
    app: &tauri::AppHandle,
    window: &tauri::WebviewWindow,
    tray_x: f64,
    tray_y: f64,
) -> Option<DockedLayout> {
    let displays = displays_in_points(window);
    let (index, tray_left, tray_top) = resolve_tray_point(&displays, tray_x, tray_y)?;
    let display = displays[index];

    // Same `tray-icon` conversion as the position (see `DisplayPoints`), so
    // the same display's scale factor undoes it.
    let item_size = app
        .tray_by_id("main-tray")
        .and_then(|t| t.rect().ok().flatten())
        .map(|r| match r.size {
            tauri::Size::Physical(s) => (
                s.width as f64 / display.scale,
                s.height as f64 / display.scale,
            ),
            tauri::Size::Logical(s) => (s.width, s.height),
        });
    let icon_width_px = *app
        .state::<AppState>()
        .last_icon_width_px
        .lock()
        .expect("last_icon_width_px mutex poisoned") as f64;

    let tray_bottom = tray_top + item_size.map(|(_, h)| h).unwrap_or(0.0);
    Some(docked_layout_in_points(
        display,
        tray_left,
        tray_top,
        tray_bottom,
        item_size.map(|(w, _)| w),
        icon_width_px,
    ))
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

    let Some(mtm) = MainThreadMarker::new() else {
        return false;
    };
    let Ok(ptr) = window.ns_window() else {
        return false;
    };
    if ptr.is_null() {
        return false;
    }
    // AppKit's global space is y-up from the *primary* screen's bottom-left;
    // `x`/`y` here are CG-style, y-down from its top-left. `NSScreen.screens`'
    // first element is by definition the screen whose origin is (0,0), so its
    // own height is the flip constant — the same conversion `tao`'s
    // `util::window_position` makes with `CGDisplay::main().pixels_high()`.
    let screens = NSScreen::screens(mtm);
    let Some(primary) = screens.iter().next() else {
        return false;
    };
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
pub(crate) fn drag_window_step(window: tauri::WebviewWindow, state: tauri::State<'_, AppState>) {
    let Some((mouse, window_origin)) = current_mouse_and_window_points(&window) else {
        return;
    };
    let mut anchor = state
        .manual_drag_anchor
        .lock()
        .expect("manual_drag_anchor mutex poisoned");
    match *anchor {
        None => {
            *anchor = Some(DragAnchor {
                mouse,
                window_top_left: window_origin,
            })
        }
        Some(start) => {
            let (x, y) = drag_target_from_anchor(start, mouse);
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
pub(crate) fn end_window_drag(state: tauri::State<'_, AppState>) {
    *state
        .manual_drag_anchor
        .lock()
        .expect("manual_drag_anchor mutex poisoned") = None;
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
fn current_mouse_and_window_points(
    window: &tauri::WebviewWindow,
) -> Option<((f64, f64), (f64, f64))> {
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

    Some((
        (mouse.x, flip - mouse.y),
        (frame.origin.x, flip - window_top),
    ))
}

#[cfg(not(target_os = "macos"))]
fn current_mouse_and_window_points(
    _window: &tauri::WebviewWindow,
) -> Option<((f64, f64), (f64, f64))> {
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
pub(crate) fn apply_docked_position(
    app: &tauri::AppHandle,
    window: &tauri::WebviewWindow,
    layout: DockedLayout,
) {
    *app.state::<AppState>()
        .docked_target
        .lock()
        .expect("docked_target mutex poisoned") = Some(layout);
    if !place_window_top_left_sync(window, layout.x, layout.y) {
        let _ = window.set_position(tauri::LogicalPosition::new(layout.x, layout.y));
    }
    let _ = window.emit("panel-beak-offset", layout.beak_left);
}

/// Moves an already-visible, already-docked-chrome window to sit under the
/// tray icon and tells the frontend where to draw the beak — the shared
/// tail end of both `show_panel` and `set_detached`'s snap-back path (C6).
/// Returns what it applied (or `None` if no display could be resolved).
fn reposition_under_tray(
    app: &tauri::AppHandle,
    window: &tauri::WebviewWindow,
    tray_x: f64,
    tray_y: f64,
) -> Option<DockedLayout> {
    let layout = compute_docked_layout(app, window, tray_x, tray_y)?;
    apply_docked_position(app, window, layout);
    Some(layout)
}

/// Clears the docked position target (see `AppState.docked_target`) so the
/// `WindowEvent::Moved` self-correction stops reasserting a position that
/// no longer applies — called whenever the window stops being "docked and
/// should stay exactly here": hiding, and detaching (dragging must never
/// fight the drag).
pub(crate) fn clear_docked_target(app: &tauri::AppHandle) {
    *app.state::<AppState>()
        .docked_target
        .lock()
        .expect("docked_target mutex poisoned") = None;
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
pub(crate) fn set_popover_collection_behavior(window: &tauri::WebviewWindow) {
    use objc2_app_kit::{NSWindow, NSWindowCollectionBehavior};
    let Ok(ptr) = window.ns_window() else { return };
    if ptr.is_null() {
        return;
    }
    let ns_window: &NSWindow = unsafe { &*(ptr as *const NSWindow) };
    let behavior = match std::env::var("QUOTOS_DEBUG_SPACE_BEHAVIOR").ok().as_deref() {
        Some("join") => {
            NSWindowCollectionBehavior::CanJoinAllSpaces
                | NSWindowCollectionBehavior::FullScreenAuxiliary
        }
        Some("move") => {
            NSWindowCollectionBehavior::MoveToActiveSpace
                | NSWindowCollectionBehavior::FullScreenAuxiliary
        }
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
    let level = std::env::var("QUOTOS_DEBUG_WINDOW_LEVEL")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(NS_STATUS_WINDOW_LEVEL);
    ns_window.setLevel(level);
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn set_popover_collection_behavior(_window: &tauri::WebviewWindow) {}

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
    let class = unsafe { (*(ptr as *const objc2::runtime::AnyObject)).class() }
        .name()
        .to_string_lossy()
        .into_owned();
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

    let (Some(_mtm), Ok(ptr)) = (MainThreadMarker::new(), window.ns_window()) else {
        return;
    };
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
pub(crate) fn show_panel(
    app: &tauri::AppHandle,
    window: &tauri::WebviewWindow,
    tray_x: f64,
    tray_y: f64,
) {
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
    let item = app
        .tray_by_id("main-tray")
        .and_then(|t| t.rect().ok().flatten())
        .map(|r| (r.position, r.size));
    let icon_width_px = *app
        .state::<AppState>()
        .last_icon_width_px
        .lock()
        .expect("last_icon_width_px mutex poisoned");
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
    Some((
        frame.origin.x,
        flip - (frame.origin.y + frame.size.height),
        frame.size.width,
        frame.size.height,
    ))
}

#[cfg(not(target_os = "macos"))]
fn window_frame_points(_window: &tauri::WebviewWindow) -> Option<(f64, f64, f64, f64)> {
    None
}

/// Left-clicking the tray icon while detached brings the window forward
/// instead of hiding it — closing a window the captain deliberately parked
/// on screen must be an explicit action, not an accidental side effect of
/// clicking the tray glyph again.
pub(crate) fn toggle_panel(
    app: &tauri::AppHandle,
    window: &tauri::WebviewWindow,
    detached: bool,
    tray_x: f64,
    tray_y: f64,
) {
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
