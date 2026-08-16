//! The status item's repaint pipeline and the panel's show, hide, dock,
//! detach, and drag lifecycle, mutually recursive with each other. See
//! docs/panel-lifecycle.md.

use std::time::Duration;

use serde::Deserialize;
use tauri::image::Image;
use tauri::{Emitter, Manager};

use crate::geometry::{
    DockedLayout, DragAnchor, displays_in_points, docked_layout_in_points, drag_target_from_anchor,
    resolve_status_item_point,
};
use crate::{AppState, panel_window, status_item_render};

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

/// An empty `segments` clears the status item back to the plain glyph.
/// `worst_used_percent` is sent on every call regardless of `segments`
/// so the bare glyph's own arc fill stays current either way.
#[tauri::command]
pub(crate) fn set_status_item_state(
    app: tauri::AppHandle,
    segments: Vec<StatusItemSegmentDto>,
    worst_used_percent: u8,
    tooltip: String,
) -> Result<(), String> {
    let Some(status_item) = app.tray_by_id("main-status-item") else {
        return Ok(());
    };
    if !record_if_changed(&app, segments, worst_used_percent, tooltip) {
        return Ok(());
    }
    repaint_status_item(&app, &status_item)
}

/// Skips the repaint's main-thread bitmap composite when nothing actually
/// changed. See docs/panel-lifecycle.md for why the tooltip is compared too.
fn record_if_changed(
    app: &tauri::AppHandle,
    segments: Vec<StatusItemSegmentDto>,
    worst_used_percent: u8,
    tooltip: String,
) -> bool {
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
    if *last == segments && *last_worst == worst_used_percent && *last_tooltip == tooltip {
        return false;
    }
    *last = segments;
    *last_worst = worst_used_percent;
    *last_tooltip = tooltip;
    true
}

/// Never called from the frontend directly, since it is a pure reflection
/// of native window visibility, not app data.
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

/// The one place the status item icon actually gets redrawn. See
/// docs/panel-lifecycle.md for why this always reads both of its inputs
/// fresh from `AppState` instead of taking either as a parameter.
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
    // the tooltip, composed by the frontend, names every pinned figure.
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
        let dark = status_item_render::is_dark_mode();
        let (rgba, w, h) = status_item_render::render(&segs, highlighted, worst_used_percent, dark);
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

    // A width change shifts the item's on-screen x too, with no event to
    // read it from, so the docked panel needs re-syncing. See docs/panel-lifecycle.md.
    sync_status_item_length(status_item, icon_width_px);

    if width_changed {
        schedule_resync_after_icon_change(app, status_item.clone());
    }
    Ok(())
}

/// Pins the status item to a fixed length exactly matching the composited
/// image; see "tray-icon 0.24.2 on macOS" in platform-constraints.md.
/// Must run on every repaint: a fixed-length item never resizes itself.
#[cfg(target_os = "macos")]
pub(crate) fn sync_status_item_length(status_item: &tauri::tray::TrayIcon, icon_width_px: u32) {
    // status_item_render's buffer is always 2x an 18pt-tall image, so
    // dividing by two gives this image's real width in points.
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
/// and still docked, re-applies the docked position and beak offset from
/// it. Returns whether it repositioned anything, for the caller's retry.
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

/// A single synchronous read right after `set_icon()` reports a stale
/// rect for one to a few runloop turns, so this retries on a short delay
/// rather than trusting the first read. See docs/panel-lifecycle.md.
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
/// true`, or folds it back into a popover, `false`. See docs/panel-lifecycle.md
/// for why decorations must stay off in both states.
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
        enter_detached_mode(&app, &window)
    } else {
        snap_back_to_docked(&app, &window, &state);
        Ok(())
    }
}

/// Sets `NSFloatingWindowLevel`, right for a free-floating detached
/// window and wrong for the popover; see `snap_back_to_docked`.
fn enter_detached_mode(
    app: &tauri::AppHandle,
    window: &tauri::WebviewWindow,
) -> Result<(), String> {
    window.set_always_on_top(true).map_err(|e| e.to_string())?;
    clear_docked_target(app);
    if panel_window::make_nonactivating_panel(window) {
        panel_window::order_front_without_activating(window);
    } else {
        let _ = window.set_focus();
    }
    Ok(())
}

/// Restores `NSStatusWindowLevel` and must be the only level-setting call
/// on this path, and re-docks from the last seen status item rect. See
/// docs/panel-lifecycle.md for why a second `set_always_on_top` would race it.
fn snap_back_to_docked(app: &tauri::AppHandle, window: &tauri::WebviewWindow, state: &AppState) {
    set_popover_collection_behavior(window);
    if let Some((item_x, item_y)) = *state
        .last_status_item_rect
        .lock()
        .expect("last_status_item_rect mutex poisoned")
    {
        reposition_under_status_item(app, window, item_x, item_y);
    }
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

/// Moves the window's top-left synchronously on the main thread, closing
/// a one-frame flash `tao`'s own async `set_outer_position` leaves. See
/// docs/panel-lifecycle.md. Returns whether the synchronous path was taken.
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
    // See DisplayPoints for the y-up/y-down flip this undoes.
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
/// AppKit's `performWindowDragWithEvent:`, which measurably reactivates
/// the app while the mouse stays down. See docs/panel-lifecycle.md.
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
/// gesture, so the next drag re-anchors instead of jumping on its first move.
#[tauri::command]
pub(crate) fn end_window_drag(state: tauri::State<'_, AppState>) {
    *state
        .manual_drag_anchor
        .lock()
        .expect("manual_drag_anchor mutex poisoned") = None;
}

/// Reads the live global mouse location and the window's own top-left,
/// both in the same global points `place_window_top_left_sync` writes in.
/// Both are natively in points, so no per-display scale factor applies.
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

/// Applies a docked position and beak offset, and records it as
/// `AppState.docked_target`, which `WindowEvent::Moved` reapplies
/// whenever something relocates the window. See docs/architecture.md.
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

/// Moves an already-visible, docked window under the status item and
/// tells the frontend where to draw the beak, the shared tail end of
/// both `show_panel` and `set_detached`'s snap-back path.
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

/// Clears `AppState.docked_target` so the `WindowEvent::Moved`
/// self-correction stops reasserting a position that no longer applies.
pub(crate) fn clear_docked_target(app: &tauri::AppHandle) {
    *app.state::<AppState>()
        .docked_target
        .lock()
        .expect("docked_target mutex poisoned") = None;
}

/// Grants this window a place on every full-screen Space at once, the
/// only option with no asynchronous Space transition to race. Reapplied
/// on every show, idempotent. See docs/panel-lifecycle.md.
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

/// `alwaysOnTop` gets `NSFloatingWindowLevel`, not enough to be seen over
/// a full-screen Space; a popover belongs at `NSStatusWindowLevel`.
/// Overridable through `QUOTOS_DEBUG_WINDOW_LEVEL` for diagnosis.
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

/// Whether this window is on the Space the user is looking at, through
/// the public `isOnActiveSpace` rather than a private route.
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

/// Shows the panel and gives it focus without activating the app. The
/// fallback branch activates normally, taken only if the panel class
/// swap did not happen; see docs/platform-constraints.md.
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

/// Positions the window before showing it, then reapplies the position
/// once more after `show()`, a no-op unless ordering-front moved the
/// frame. See docs/panel-lifecycle.md.
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

/// Opt-in placement trace, `QUOTOS_DEBUG_POS=1`, off by default. Every
/// number the docked-position arithmetic consumes and produces.
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
/// through `outer_position()`, which is scale-factor-relative; see
/// `DisplayPoints`.
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
/// instead of hiding it, since closing a deliberately parked window must
/// be an explicit action.
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
