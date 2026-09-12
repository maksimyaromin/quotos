use std::time::Duration;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use serde::Deserialize;
use tauri::image::Image;
use tauri::{Emitter, Manager};

use crate::geometry::{
    DockedLayout, DragAnchor, click_x_in_icon_px, displays_in_points, docked_layout_in_points,
    drag_target_from_anchor, resolve_status_item_point,
};
use crate::{AppState, panel_window, status_item_render};

#[tauri::command]
pub(crate) fn hide_panel(app: tauri::AppHandle, window: tauri::WebviewWindow) {
    let _ = window.hide();
    let _ = window.emit("panel-visibility", false);
    set_status_item_highlighted(&app, false);
    clear_docked_target(&app);
}

/// One item in the menu bar, as the frontend describes it. Mirrors
/// `StatusItemSegment` in `types/entities.ts`.
#[derive(Deserialize, Clone, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub(crate) enum StatusItemSegmentDto {
    /// The group's slug, derived by `lib/pin-groups.ts` from its name.
    /// The one thing a click can fold a group by, which `group_id`
    /// names.
    Chip {
        slug: String,
        color: String,
        group_id: String,
    },
    Figure {
        text: String,
        color: String,
    },
}

impl StatusItemSegmentDto {
    fn to_segment(&self) -> status_item_render::StatusItemSegment {
        match self {
            StatusItemSegmentDto::Chip { slug, color, .. } => {
                status_item_render::StatusItemSegment::Chip {
                    slug: slug.clone(),
                    color: status_item_render::GroupColor::parse(color),
                }
            }
            StatusItemSegmentDto::Figure { text, color } => {
                status_item_render::StatusItemSegment::Figure {
                    text: text.clone(),
                    color: match color.as_str() {
                        "amber" => status_item_render::StatusItemColor::Amber,
                        "red" => status_item_render::StatusItemColor::Red,
                        _ => status_item_render::StatusItemColor::Neutral,
                    },
                }
            }
        }
    }

    fn group_id(&self) -> Option<&str> {
        match self {
            StatusItemSegmentDto::Chip { group_id, .. } => Some(group_id),
            StatusItemSegmentDto::Figure { .. } => None,
        }
    }
}

pub(crate) fn to_segments(
    dtos: &[StatusItemSegmentDto],
) -> Vec<status_item_render::StatusItemSegment> {
    dtos.iter().map(StatusItemSegmentDto::to_segment).collect()
}

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

/// The Customize display screen's preview, drawn by the compositor the
/// live tray is drawn by; see "One renderer, two surfaces" in
/// docs/status-item-rendering.md.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StatusItemImage {
    width: u32,
    height: u32,
    /// The raw RGBA bytes, base64 so they cross the IPC boundary as one
    /// string rather than as tens of thousands of JSON numbers.
    rgba_base64: String,
}

#[tauri::command]
pub(crate) fn render_status_item_preview(
    segments: Vec<StatusItemSegmentDto>,
    worst_used_percent: u8,
) -> StatusItemImage {
    let (rgba, width, height) = status_item_render::render(
        &to_segments(&segments),
        false,
        worst_used_percent,
        status_item_render::is_dark_mode(),
    );
    StatusItemImage {
        width,
        height,
        rgba_base64: BASE64.encode(&rgba),
    }
}

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

/// Which pin group's chip a left click landed on, if any; see
/// "Clicking a group's chip in the menu bar" in docs/architecture.md
/// for the units the click and the item's box arrive in.
pub(crate) fn group_at_click(
    app: &tauri::AppHandle,
    window: &tauri::WebviewWindow,
    click_x: f64,
    item_x: f64,
    item_y: f64,
    item_width: f64,
) -> Option<String> {
    if item_width <= 0.0 {
        return None;
    }
    let displays = displays_in_points(window);
    let (index, item_left_points, _) = resolve_status_item_point(&displays, item_x, item_y)?;
    let scale = displays[index].scale;
    let state = app.state::<AppState>();
    let spans = state
        .last_status_item_chip_spans
        .lock()
        .expect("last_status_item_chip_spans mutex poisoned")
        .clone();
    let icon_width_px = *state
        .last_icon_width_px
        .lock()
        .expect("last_icon_width_px mutex poisoned");
    let click_in_icon_px = click_x_in_icon_px(
        click_x / scale,
        item_left_points,
        Some(item_width / scale),
        icon_width_px as f64,
    );
    let index = status_item_render::chip_at(&spans, icon_width_px, click_in_icon_px)?;
    state
        .last_status_item_segments
        .lock()
        .expect("last_status_item_segments mutex poisoned")
        .get(index)
        .and_then(|segment| segment.group_id().map(str::to_owned))
}

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

    let tooltip = state
        .last_status_item_tooltip
        .lock()
        .expect("last_status_item_tooltip mutex poisoned")
        .clone();
    status_item
        .set_tooltip(Some(&tooltip))
        .map_err(|e| e.to_string())?;

    let segs = to_segments(&segments);

    // Cached from the same layout the chips were drawn at, so a click
    // resolves against where they really landed.
    *state
        .last_status_item_chip_spans
        .lock()
        .expect("last_status_item_chip_spans mutex poisoned") =
        status_item_render::chip_spans(&segs, worst_used_percent);

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

    sync_status_item_length(status_item, icon_width_px);

    if width_changed {
        schedule_resync_after_icon_change(app, status_item.clone());
    }
    Ok(())
}

/// Sets the item's own width, then re-fits the view that catches its
/// clicks, which `tray-icon` re-fits only while setting an icon. See
/// "Clicking a group's chip in the menu bar" in docs/architecture.md.
#[cfg(target_os = "macos")]
pub(crate) fn sync_status_item_length(status_item: &tauri::tray::TrayIcon, icon_width_px: u32) {
    use objc2_foundation::MainThreadMarker;

    let width_points = icon_width_px as f64 / status_item_render::RENDER_SCALE;
    let _ = status_item.with_inner_tray_icon(move |inner| {
        let Some(ns_status_item) = inner.ns_status_item() else {
            return;
        };
        ns_status_item.setLength(width_points);
        let Some(button) = MainThreadMarker::new().and_then(|mtm| ns_status_item.button(mtm))
        else {
            return;
        };
        let bounds = button.bounds();
        for view in button.subviews() {
            view.setFrame(bounds);
        }
    });
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn sync_status_item_length(_status_item: &tauri::tray::TrayIcon, _icon_width_px: u32) {}

#[cfg(target_os = "macos")]
pub(crate) fn disable_status_item_native_highlight(status_item: &tauri::tray::TrayIcon) {
    use objc2_app_kit::NSCellStyleMask;
    use objc2_foundation::MainThreadMarker;

    let _ = status_item.with_inner_tray_icon(move |inner| {
        let Some(mtm) = MainThreadMarker::new() else {
            return;
        };
        let Some(button) = inner.ns_status_item().and_then(|item| item.button(mtm)) else {
            return;
        };
        let Some(cell) = button.cell() else {
            return;
        };
        let _: () =
            unsafe { objc2::msg_send![&*cell, setHighlightsBy: NSCellStyleMask::NoCellMask] };
    });
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn disable_status_item_native_highlight(_status_item: &tauri::tray::TrayIcon) {}

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

#[tauri::command]
pub(crate) fn end_window_drag(state: tauri::State<'_, AppState>) {
    *state
        .manual_drag_anchor
        .lock()
        .expect("manual_drag_anchor mutex poisoned") = None;
}

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

pub(crate) fn clear_docked_target(app: &tauri::AppHandle) {
    *app.state::<AppState>()
        .docked_target
        .lock()
        .expect("docked_target mutex poisoned") = None;
}

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
        // Insurance against ordering the window front moving it from where
        // the call above just placed it.
        apply_docked_position(app, window, layout);
    }
    log_docked_placement(app, window, item_x, item_y, layout);
    let _ = window.emit("panel-visibility", true);
    set_status_item_highlighted(app, true);
}

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
