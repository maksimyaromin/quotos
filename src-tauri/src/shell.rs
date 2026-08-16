//! The interactive shell: the status item's repaint pipeline and the panel
//! window's show, hide, dock, detach, and drag lifecycle. One module rather
//! than two because the two sides are mutually recursive: repainting the
//! status item can move the open panel, through `repaint_status_item`,
//! `schedule_resync_after_icon_change`, and `reposition_under_status_item`,
//! and showing or hiding the panel repaints the status item, through
//! `show_panel`, `hide_panel`, and `set_status_item_highlighted`. The pure
//! layers stay out: coordinate math in `geometry`, bitmap composition in
//! `status_item_render`, the NSPanel class swap in `panel_window`.

use std::time::Duration;

use serde::Deserialize;
use tauri::image::Image;
use tauri::{Emitter, Manager};

use crate::geometry::{
    displays_in_points, docked_layout_in_points, drag_target_from_anchor,
    resolve_status_item_point, DockedLayout, DragAnchor,
};
use crate::{panel_window, status_item_render, AppState};

#[tauri::command]
pub(crate) fn hide_panel(app: tauri::AppHandle, window: tauri::WebviewWindow) {
    let _ = window.hide();
    let _ = window.emit("panel-visibility", false);
    set_status_item_highlighted(&app, false);
    clear_docked_target(&app);
}

#[derive(Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StatusItemSegmentDto {
    text: String,
    color: String, // "neutral" | "amber" | "red"
    group_start: bool,
}

/// Sets what is shown beside the status item's glyph, the pinned-
/// subscriptions feature. An empty `segments` clears it back to just the
/// plain, theme-tinted glyph unless the panel is currently open. See
/// `repaint_status_item`. `worst_used_percent` is the bare glyph's own arc
/// fill, sent on every call regardless of `segments` so the arc stays
/// current whether or not anything is pinned.
#[tauri::command]
pub(crate) fn set_tray_status(
    app: tauri::AppHandle,
    segments: Vec<StatusItemSegmentDto>,
    worst_used_percent: u8,
    tooltip: String,
) -> Result<(), String> {
    let Some(status_item) = app.tray_by_id("main-status-item") else {
        return Ok(());
    };
    {
        // This command runs on the main thread and its body is a full
        // bitmap composite plus a set_icon. The frontend fires it on every
        // state change, most of which leave the pinned digits identical,
        // so comparing first turns those into a no-op rather than
        // main-thread work that also reaches
        // schedule_resync_after_icon_change, which moves the open panel.
        let state = app.state::<AppState>();
        let mut last = state
            .last_status_item_segments
            .lock()
            .expect("last_status_item_segments mutex poisoned");
        let mut last_worst = state
            .last_status_item_worst_used_percent
            .lock()
            .expect("last_status_item_worst_used_percent mutex poisoned");
        let mut last_tooltip = state
            .last_status_item_tooltip
            .lock()
            .expect("last_status_item_tooltip mutex poisoned");
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
    repaint_status_item(&app, &status_item)
}

/// Flips the status item's "panel open" highlight on or off and repaints.
/// Called from `show_panel`, `hide_panel`, and the click-away and hide
/// branches, never from the frontend directly, since it is a pure
/// reflection of native window visibility, not app data.
pub(crate) fn set_status_item_highlighted(app: &tauri::AppHandle, highlighted: bool) {
    let Some(status_item) = app.tray_by_id("main-status-item") else {
        return;
    };
    *app.state::<AppState>()
        .status_item_highlighted
        .lock()
        .expect("status_item_highlighted mutex poisoned") = highlighted;
    let _ = repaint_status_item(app, &status_item);
}

/// The one place the status item icon actually gets redrawn, shared by
/// both of its independent inputs: the pinned-subscription digits, from
/// `set_tray_status`, and the "panel open" highlight, from
/// `set_status_item_highlighted`. Neither knows the other's current value,
/// so this always reads both fresh from `AppState` rather than taking
/// either as a parameter.
///
/// `tray-icon` v0.24.2's macOS `set_title` only calls `NSStatusItem`'s
/// `setTitle` when given `Some(..)`; `None` is a silent no-op that leaves
/// the previous title stuck on screen, so this always clears it with
/// `Some("")`, even though digits are drawn into the icon image, not the
/// title. Colored digits have no path through `set_title` at all — see
/// `status_item_render.rs`'s module doc. With any segments present, or the
/// highlight active, this drops `icon_as_template` and paints a composed
/// bitmap instead, since a plain template image cannot carry its own
/// background tint; with neither, it reverts to the plain template glyph.
fn repaint_status_item(
    app: &tauri::AppHandle,
    status_item: &tauri::tray::TrayIcon,
) -> Result<(), String> {
    let state = app.state::<AppState>();
    let segments = state
        .last_status_item_segments
        .lock()
        .expect("last_status_item_segments mutex poisoned")
        .clone();
    let highlighted = *state
        .status_item_highlighted
        .lock()
        .expect("status_item_highlighted mutex poisoned");
    let worst_used_percent = *state
        .last_status_item_worst_used_percent
        .lock()
        .expect("last_status_item_worst_used_percent mutex poisoned");

    status_item.set_title(Some("")).map_err(|e| e.to_string())?;

    // The composited image carries no text a screen reader can read, so
    // the tooltip names every pinned figure instead, composed in full by
    // the frontend and cached here alongside the segments so a
    // native-only repaint, a highlight toggle, keeps it.
    let tooltip = state
        .last_status_item_tooltip
        .lock()
        .expect("last_status_item_tooltip mutex poisoned")
        .clone();
    status_item
        .set_tooltip(Some(&tooltip))
        .map_err(|e| e.to_string())?;

    let icon_width_px = if segments.is_empty() && !highlighted {
        let (rgba, w, h) = status_item_render::plain_glyph_rgba(worst_used_percent);
        status_item
            .set_icon(Some(Image::new_owned(rgba, w, h)))
            .map_err(|e| e.to_string())?;
        status_item
            .set_icon_as_template(true)
            .map_err(|e| e.to_string())?;
        w
    } else {
        let segs: Vec<status_item_render::StatusItemSegment> = segments
            .into_iter()
            .map(|s| status_item_render::StatusItemSegment {
                text: s.text,
                color: match s.color.as_str() {
                    "amber" => status_item_render::StatusItemColor::Amber,
                    "red" => status_item_render::StatusItemColor::Red,
                    _ => status_item_render::StatusItemColor::Neutral,
                },
                group_start: s.group_start,
            })
            .collect();
        let (rgba, w, h) = status_item_render::render(&segs, highlighted, worst_used_percent);
        status_item
            .set_icon(Some(Image::new_owned(rgba, w, h)))
            .map_err(|e| e.to_string())?;
        status_item
            .set_icon_as_template(false)
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

    // Pinning or unpinning a subscription, or the highlight toggling, can
    // change the status item's own width, which on macOS shifts the
    // item's own on-screen x too, since status items lay out
    // right-to-left. Nothing about that resize goes through
    // `TrayIconEvent`, so the last-known item position goes stale the
    // instant this runs and the beak and panel silently drift off the
    // glyph until the next real click or hover, hence re-docking here
    // whenever the panel is open and attached.
    //
    // Gated on the width having actually changed: a same-width repaint
    // cannot have moved the item, and re-docking after one costs three
    // `status_item.rect()` reads and up to three frame-placement calls on
    // the open panel, on the main thread. The highlight toggle never
    // changes the width, by construction, and it fires on every open and
    // close.
    sync_status_item_length(status_item, icon_width_px);

    if width_changed {
        schedule_resync_after_icon_change(app, status_item.clone());
    }
    Ok(())
}

/// `tray-icon` v0.24.2 always creates the status item with
/// `NSVariableStatusItemLength` and never touches its length again. A
/// variable-length item's button is not the same rect as its own image:
/// AppKit reserves a fixed margin on each side of the image, independent
/// of content, which both widens the gap to the neighboring menu bar item
/// and makes a plain click's native highlight, painted across the
/// button's bounds, read wider than the panel-open pill this app draws
/// into the image's own bounds. See `status_item_render::draw_highlight_background`.
///
/// Pinning the item to a fixed length exactly matching the composited
/// image removes that margin, so the button's bounds and the image's
/// bounds become the same rect. Must run on every repaint, not just once:
/// unlike a variable-length item, a fixed-length one never resizes itself
/// when a new, differently sized image is set.
#[cfg(target_os = "macos")]
pub(crate) fn sync_status_item_length(status_item: &tauri::tray::TrayIcon, icon_width_px: u32) {
    // status_item_render's buffer is always 2x an 18pt-tall image, see
    // GLYPH_PX's doc comment, regardless of the display's own backing
    // scale, so dividing by two gives this image's real width in points
    // on any display.
    let width_points = icon_width_px as f64 / 2.0;
    let _ = status_item.with_inner_tray_icon(move |inner| {
        if let Some(ns_status_item) = inner.ns_status_item() {
            ns_status_item.setLength(width_points);
        }
    });
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn sync_status_item_length(_status_item: &tauri::tray::TrayIcon, _icon_width_px: u32) {}

/// Re-reads the status item's current rect and, if the panel is visible
/// and still docked, never while detached, re-applies the docked position
/// and beak offset from it. Pinning or unpinning changes the icon's
/// width, and therefore its on-screen x, with no click to refresh the
/// last-known rect from. Returns whether it actually repositioned
/// anything, so `schedule_resync_after_icon_change` knows whether a retry
/// is still needed.
fn resync_docked_position_after_icon_change(
    app: &tauri::AppHandle,
    status_item: &tauri::tray::TrayIcon,
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
    let rect = status_item.rect().ok().flatten()?;
    let (item_x, item_y) = match rect.position {
        tauri::Position::Physical(p) => (p.x as f64, p.y as f64),
        tauri::Position::Logical(p) => (p.x, p.y),
    };
    *state
        .last_status_item_rect
        .lock()
        .expect("last_status_item_rect mutex poisoned") = Some((item_x, item_y));
    reposition_under_status_item(app, &window, item_x, item_y);
    Some((item_x, item_y))
}

/// A single synchronous call from `resync_docked_position_after_icon_change`
/// reads a stale rect: right after `set_icon()` changes the composited
/// image's width, `status_item.rect()`, called immediately after on the
/// same main-thread dispatch, still reports the item's previous width and
/// position for one to a few runloop turns. `set_icon`'s own dispatch
/// guarantees the image is set, not that `NSStatusItem`'s width-driven
/// layout pass has already run, so this makes one immediate attempt plus
/// a couple of short-delay retries rather than trusting the first read;
/// each retry just re-reads and re-applies, harmless if the previous
/// attempt already landed on the right numbers.
fn schedule_resync_after_icon_change(app: &tauri::AppHandle, status_item: tauri::tray::TrayIcon) {
    if resync_docked_position_after_icon_change(app, &status_item).is_none() {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        for delay_ms in [30, 120] {
            tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
            resync_docked_position_after_icon_change(&app, &status_item);
        }
    });
}

/// Tears the panel off into a real, freestanding window, `detached =
/// true`, or folds it back into a popover, `false`. Detached mode stays
/// out of the hide-on-blur path and shows up in Cmd+Tab, so it can be
/// parked on screen and watched while the user works elsewhere.
///
/// Decorations must stay off in both states: with `titleBarStyle: Overlay`
/// in `tauri.conf.json`, turning decorations on paints real traffic
/// lights over the content and a native title-bar strip regardless of the
/// window's own transparency. Dragging comes from `App.tsx`'s own
/// header-drag calling `drag_window_step` on every `mousemove`, which
/// needs no native title bar at all.
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
        // NSFloatingWindowLevel, which this sets, is right for a
        // free-floating detached window, and specifically wrong for the
        // popover; see the else branch.
        window.set_always_on_top(true).map_err(|e| e.to_string())?;
        // Dragging must never fight the docked-position self-correction;
        // see AppState.docked_target's doc comment.
        clear_docked_target(&app);
        // set_focus() would activate the application, and activating
        // while another app owns a full-screen Space is what makes macOS
        // leave that Space. Tearing the panel off must not move the user.
        if panel_window::make_nonactivating_panel(&window) {
            panel_window::order_front_without_activating(&window);
        } else {
            let _ = window.set_focus();
        }
    } else {
        // Snapping back must restore NSStatusWindowLevel, which
        // set_popover_collection_behavior below does, and must be the
        // only level-setting call in this branch: tao's set_always_on_top,
        // called by the if branch above, ends in an async dispatch to the
        // main queue, and calling it here too would schedule a later
        // runloop turn to drop the level back to floating right after
        // this synchronous restore, undoing it.
        set_popover_collection_behavior(&window);
        // Snapping back must also re-dock the window under the status
        // item, not just restore the chrome. This path has no fresh click
        // to read a rect from, since it is triggered by the panel's own
        // header button, so it uses the last rect seen by any status item
        // event, `None` only before the app's first such event, which
        // cannot happen here since detaching requires the panel to
        // already be open.
        if let Some((item_x, item_y)) = *state
            .last_status_item_rect
            .lock()
            .expect("last_status_item_rect mutex poisoned")
        {
            reposition_under_status_item(&app, &window, item_x, item_y);
        }
    }
    Ok(())
}

fn compute_docked_layout(
    app: &tauri::AppHandle,
    window: &tauri::WebviewWindow,
    item_x: f64,
    item_y: f64,
) -> Option<DockedLayout> {
    let displays = displays_in_points(window);
    let (index, item_left, item_top) = resolve_status_item_point(&displays, item_x, item_y)?;
    let display = displays[index];

    // Same tray-icon conversion as the position, see DisplayPoints, so
    // the same display's scale factor undoes it.
    let item_size = app
        .tray_by_id("main-status-item")
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

    let item_bottom = item_top + item_size.map(|(_, h)| h).unwrap_or(0.0);
    Some(docked_layout_in_points(
        display,
        item_left,
        item_top,
        item_bottom,
        item_size.map(|(w, _)| w),
        icon_width_px,
    ))
}

/// Moves the window's top-left to a global-point coordinate, synchronously
/// on the main thread wherever that is possible.
///
/// The synchronicity matters: `tao`'s own `set_outer_position` ends in an
/// async dispatch of `setFrameTopLeftPoint:` onto the main queue, while
/// `show()` runs inline. Called in either order from the click handler,
/// already on the main thread, the window becomes visible at its stale
/// position first and only moves a runloop turn later, a guaranteed
/// one-frame flash at wherever it last was. Placing it directly through
/// `NSWindow` closes that gap: by the time `show()` runs the frame is
/// already right.
///
/// Returns whether the synchronous path was taken. Callers fall back to
/// Tauri's own async `set_position` on non-macOS, or when the call
/// arrives off the main thread, where `setFrameTopLeftPoint:` is not safe.
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
    // AppKit's global space is y-up from the primary screen's bottom-left.
    // x and y here are CG-style, y-down from its top-left. NSScreen.screens'
    // first element is by definition the screen whose origin is (0,0), so
    // its own height is the flip constant.
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

/// Moves a detached window by hand, one `mousemove` at a time, instead of
/// calling AppKit's `-[NSWindow performWindowDragWithEvent:]`, which
/// `tao`'s `startDragging()` bottoms out in. `performWindowDragWithEvent:`
/// does move the window, but doing so while the mouse stays down
/// measurably reactivates the application, the same Space-losing failure
/// `panel_window.rs` exists to prevent — and the same problem reproduces
/// for this function's own replacement mechanism,
/// `place_window_top_left_sync`, so relocating a window while the mouse
/// button is held over it appears to carry an implicit activation outside
/// `NSWindowStyleMaskNonactivatingPanel`'s own promise, which is scoped to
/// key and main status, not window-server-level drag handling.
/// Reactivation is therefore scoped to the physical gesture's duration
/// only, never on show and never on an external window-manager move.
///
/// Called on every `mousemove` while a header drag is in progress. The
/// first call of a gesture only records where the cursor and the window
/// each started, in `AppState.manual_drag_anchor`. Every later call sets
/// the window's frame directly from the live delta, through the same
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

/// Clears `AppState.manual_drag_anchor` at the end of a header-drag
/// gesture. Without this, the next drag's first `drag_window_step` call
/// would see the previous gesture's stale anchor instead of re-anchoring
/// to where this new one actually started, and jump the window on its
/// first move.
#[tauri::command]
pub(crate) fn end_window_drag(state: tauri::State<'_, AppState>) {
    *state
        .manual_drag_anchor
        .lock()
        .expect("manual_drag_anchor mutex poisoned") = None;
}

/// Reads the live global mouse location and the window's own current
/// top-left, both in the same CG-style, y-down, top-left-of-primary-screen
/// global points `place_window_top_left_sync` writes in; see
/// `DisplayPoints`'s doc comment for why points, not physical pixels, are
/// the only safe coordinate space for this arithmetic. Both
/// `NSEvent.mouseLocation` and `NSWindow.frame` are natively in points
/// already, so there is no per-display scale factor to resolve here.
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

/// Applies a docked position and beak offset, and records it as the
/// window's current intended target, `AppState.docked_target`. See that
/// field's own doc comment: the `WindowEvent::Moved` handler reapplies
/// this exact target whenever something relocates the window away from
/// it.
///
/// `x` and `y` are global points, see `DisplayPoints`. The fallback path
/// uses a `LogicalPosition` deliberately: `tao`'s `Position::to_logical`
/// passes a logical value straight through, so no scale factor is
/// consulted. A `PhysicalPosition` here would be reinterpreted through
/// whatever display the window currently happens to sit on.
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

/// Moves an already-visible, already-docked-chrome window to sit under
/// the status item and tells the frontend where to draw the beak, the
/// shared tail end of both `show_panel` and `set_detached`'s snap-back
/// path. Returns what it applied, or `None` if no display could be
/// resolved.
fn reposition_under_status_item(
    app: &tauri::AppHandle,
    window: &tauri::WebviewWindow,
    item_x: f64,
    item_y: f64,
) -> Option<DockedLayout> {
    let layout = compute_docked_layout(app, window, item_x, item_y)?;
    apply_docked_position(app, window, layout);
    Some(layout)
}

/// Clears the docked position target, see `AppState.docked_target`, so
/// the `WindowEvent::Moved` self-correction stops reasserting a position
/// that no longer applies. Called whenever the window stops being docked
/// and expected to stay exactly here: hiding, and detaching, since
/// dragging must never fight the drag.
pub(crate) fn clear_docked_target(app: &tauri::AppHandle) {
    *app.state::<AppState>()
        .docked_target
        .lock()
        .expect("docked_target mutex poisoned") = None;
}

/// A full-screen application owns its Space, and macOS does not order
/// another application's window into it just because that application
/// asks. The flag that grants it is
/// `NSWindowCollectionBehaviorFullScreenAuxiliary`, the standard utility
/// and palette window behavior. Paired with `CanJoinAllSpaces`: the window
/// then exists on every Space at once, which is both what a menu bar
/// popover actually is and the option with no asynchronous Space
/// transition to race, unlike `MoveToActiveSpace`.
///
/// `.Transient` keeps it out of Mission Control and Exposé's per-Space
/// window list, matching the system's own Volume and Wi-Fi popovers.
/// `.IgnoresCycle` keeps it out of Cmd-` window cycling.
///
/// Reapplied on every show rather than only at launch: idempotent, and
/// cheap insurance against anything resetting it after first realization.
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

/// `alwaysOnTop` in `tauri.conf.json` gets this window
/// `NSFloatingWindowLevel`, which is right for a panel floating over
/// ordinary windows and not enough to be seen over another application's
/// full-screen Space. A status-item popover belongs at
/// `NSStatusWindowLevel`, the level the system's own menu bar popovers
/// use. Overridable through `QUOTOS_DEBUG_WINDOW_LEVEL` for diagnosis.
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
/// silently drops flags that conflict with others in the same group.
/// Surfaced through the `QUOTOS_DEBUG_POS` trace.
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

/// AppKit's own answer to whether this window is on the Space the user is
/// looking at, through the public `-[NSWindow isOnActiveSpace]` rather
/// than the private `CGSCopySpacesForWindows` route. Surfaced through the
/// `QUOTOS_DEBUG_POS` trace.
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
    // class and style_mask say whether the non-activating panel
    // conversion actually took; see panel_window.rs.
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

/// Shows the panel and gives it keyboard focus without activating the
/// application. See `panel_window.rs`'s module doc for the whole argument.
/// The fallback branch activates the application in the ordinary way,
/// taken only if the panel conversion did not happen: a non-macOS build,
/// `QUOTOS_PANEL_MODE=window`, or an AppKit that refused the class swap. A
/// window that can never take keyboard focus would be a worse regression
/// than a Space switch.
fn order_panel_front(window: &tauri::WebviewWindow) {
    let is_panel = panel_window::make_nonactivating_panel(window);
    let _ = window.show();
    if is_panel {
        panel_window::order_front_without_activating(window);
    } else {
        let _ = window.set_focus();
        order_front_regardless(window);
    }
}

/// Puts this window at the front of its level here and now, even if the
/// application is not the active one. Only needed on the non-panel
/// fallback path; `order_front_without_activating` does this itself.
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

/// Positions the window before showing it. `show()` reveals the window
/// wherever it was last left, so showing before moving guarantees a
/// visible frame at the wrong place on a multi-display machine. The move
/// itself must also actually land before the reveal, which `set_position`
/// alone cannot promise; see `place_window_top_left_sync` for that half.
///
/// The position is reapplied once more after `show()`. That second call
/// is a no-op when nothing moved the window, since setting the frame to
/// its current origin emits no `Moved` event, and it costs one
/// main-thread call to be immune to anything ordering-front does to the
/// frame.
pub(crate) fn show_panel(
    app: &tauri::AppHandle,
    window: &tauri::WebviewWindow,
    item_x: f64,
    item_y: f64,
) {
    set_popover_collection_behavior(window);
    let layout = compute_docked_layout(app, window, item_x, item_y);
    if let Some(layout) = layout {
        apply_docked_position(app, window, layout);
    }
    order_panel_front(window);
    if let Some(layout) = layout {
        apply_docked_position(app, window, layout);
    }
    log_docked_placement(app, window, item_x, item_y, layout);
    let _ = window.emit("panel-visibility", true);
    set_status_item_highlighted(app, true);
}

/// Opt-in placement trace, `QUOTOS_DEBUG_POS=1`, off by default so no
/// build ever writes to stderr on its own. Every number the docked-
/// position arithmetic consumes and produces, plus the frame AppKit
/// actually ended up with.
fn log_docked_placement(
    app: &tauri::AppHandle,
    window: &tauri::WebviewWindow,
    item_x: f64,
    item_y: f64,
    layout: Option<DockedLayout>,
) {
    if std::env::var_os("QUOTOS_DEBUG_POS").is_none() {
        return;
    }
    let displays = displays_in_points(window);
    let resolved = resolve_status_item_point(&displays, item_x, item_y);
    let item = app
        .tray_by_id("main-status-item")
        .and_then(|t| t.rect().ok().flatten())
        .map(|r| (r.position, r.size));
    let icon_width_px = *app
        .state::<AppState>()
        .last_icon_width_px
        .lock()
        .expect("last_icon_width_px mutex poisoned");
    eprintln!(
        "quotos-pos: item_raw=({item_x},{item_y}) displays={displays:?} resolved={resolved:?} item={item:?} icon_width_px={icon_width_px} layout={layout:?} frame_after={:?} collection_behavior={:?} visible={:?}",
        window_frame_points(window),
        collection_behavior_bits(window).map(|b| format!("{b:#x}")),
        window.is_visible()
    );
    // Sampled again shortly after, since Space membership settles
    // asynchronously and the value read inside show_panel is the one
    // least likely to be final.
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

/// The window's own frame, read back from AppKit in global points, not
/// through `outer_position()`, which is both scale-factor-relative, see
/// `DisplayPoints`, and was observed misreporting right after a first
/// `show()`.
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

/// Left-clicking the status item while detached brings the window forward
/// instead of hiding it. Closing a window the user deliberately parked on
/// screen must be an explicit action, not an accidental side effect of
/// clicking the glyph again.
pub(crate) fn toggle_panel(
    app: &tauri::AppHandle,
    window: &tauri::WebviewWindow,
    detached: bool,
    item_x: f64,
    item_y: f64,
) {
    let visible = window.is_visible().unwrap_or(false);
    if visible && !detached {
        let _ = window.hide();
        let _ = window.emit("panel-visibility", false);
        set_status_item_highlighted(app, false);
        clear_docked_target(app);
    } else {
        show_panel(app, window, item_x, item_y);
    }
}
