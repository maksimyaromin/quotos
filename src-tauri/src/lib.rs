mod accounts;
mod atomic_write;
mod geometry;
mod launch_at_login;
mod panel_window;
mod persistence;
mod providers;
mod ratelimit;
mod scheduler;
mod shell;
mod signin;
mod single_instance;
mod status_item_render;
pub mod statusline;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

use tauri::image::Image;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{TrayIconBuilder, TrayIconEvent};
use tauri::{Emitter, Manager};

use persistence::Store;
use ratelimit::RateLimiter;
use scheduler::Scheduler;

use geometry::{DockedLayout, DragAnchor, PANEL_WINDOW_HEIGHT_LOGICAL, PANEL_WINDOW_WIDTH_LOGICAL};

struct AppState {
    http: reqwest::Client,
    rate_limiter: RateLimiter,
    profile_cache: Mutex<HashMap<String, serde_json::Value>>,
    /// While the panel is detached into a real, freestanding window, focus
    /// loss must never hide it.
    detached: Mutex<bool>,
    tracked_store: Store,
    /// Quotos's own app-support directory; see `statusline.rs`, which
    /// stores the installed helper, per-account feed readings, and install
    /// backups underneath it.
    statusline_root: PathBuf,
    scheduler: Scheduler,
    /// In-progress `claude setup-token` sessions, keyed by account id; see
    /// `signin.rs`.
    sign_in: signin::SignInRegistry,
    /// The status item's own rect, updated on every status item event, not
    /// just clicks: `set_detached`'s snap-back has no event of its own to
    /// read from, so it needs this cached value to know where to re-dock.
    /// `None` until the first such event arrives.
    last_status_item_rect: Mutex<Option<(f64, f64)>>,
    /// The pixel width of the status item image last handed to `set_icon`,
    /// always at the fixed "2x of an 18pt-tall image" convention.
    /// `compute_docked_layout` uses it to separate the item's own AppKit
    /// margin from the image itself.
    last_icon_width_px: Mutex<u32>,
    /// Whether the panel is currently visible, the only input to the
    /// "panel open" highlight not already known at repaint time from
    /// segments alone. Flipped by `set_status_item_highlighted`.
    status_item_highlighted: Mutex<bool>,
    /// The segments `set_status_item_state` last received, cached so toggling
    /// `status_item_highlighted` can repaint with the same digits without
    /// the frontend resending them.
    last_status_item_segments: Mutex<Vec<shell::StatusItemSegmentDto>>,
    /// The glyph's own arc fill last set by `set_status_item_state`, 0 to 100,
    /// cached for the same reason as `last_status_item_segments`.
    last_status_item_worst_used_percent: Mutex<u8>,
    /// The status item's hover and VoiceOver text last set by
    /// `set_status_item_state`, cached the same way. Starts as the plain product
    /// name, matching the builder's own pre-any-data baseline.
    last_status_item_tooltip: Mutex<String>,
    /// The layout the window is supposed to be at right now, while docked
    /// and visible, in global points; see `DisplayPoints`. `None` whenever
    /// hidden or detached, since dragging must never fight this. Read by
    /// the debounced correction in the `WindowEvent::Moved` handler, which
    /// undoes anything that relocates the window while it is supposed to
    /// stay docked.
    docked_target: Mutex<Option<DockedLayout>>,
    /// How many `WindowEvent::Moved` events have fired so far, bumped on
    /// every one and read back by a debounced correction task to tell
    /// whether it is still the last one scheduled.
    move_generation: Mutex<u64>,
    /// The most recent position `WindowEvent::Moved` reported, converted
    /// to global points, so the debounced correction compares against the
    /// latest observed position after its delay, not a value captured at
    /// scheduling time.
    last_known_position: Mutex<(f64, f64)>,
    /// Anchor for `drag_window_step`'s manual, frame-based detached-window
    /// drag. `None` whenever no manual drag is in progress.
    manual_drag_anchor: Mutex<Option<DragAnchor>>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            accounts::list_accounts,
            accounts::fetch_snapshot,
            accounts::load_tracked,
            accounts::save_tracked,
            accounts::kick_scheduler,
            shell::hide_panel,
            shell::set_status_item_state,
            shell::set_detached,
            shell::drag_window_step,
            shell::end_window_drag,
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

            eprintln!(
                "quotos: status item digit font = {}",
                if status_item_render::used_fallback_font() {
                    "system fallback (MonoLisa not found)"
                } else {
                    "MonoLisa"
                }
            );

            // Built here rather than through the builder's own manage(),
            // since the tracked-list store needs app.path(), not available
            // until setup.
            let app_support_dir = app.path().app_config_dir()?;
            // Before anything else touches shared state: if a Quotos is
            // already running, this one must bow out, since two instances
            // would each run their own rate limiter against the same
            // shared 5-per-300s allowance and spend it double-speed. See
            // single_instance.rs.
            match single_instance::claim(&app_support_dir) {
                single_instance::Claim::Held(guard) => {
                    // The OS lock lives exactly as long as this handle
                    // stays open and is owned by this process, so the
                    // handle is deliberately never closed.
                    std::mem::forget(guard);
                }
                single_instance::Claim::TakenByOther => {
                    eprintln!("quotos: another Quotos instance is already running; exiting");
                    std::process::exit(0);
                }
                single_instance::Claim::Unavailable(err) => {
                    eprintln!(
                        "quotos: could not check for another running instance ({err}); continuing"
                    );
                }
            }
            let tracked_path = app_support_dir.join("tracked.json");
            // Computed here, rather than by the status item builder below,
            // so AppState.last_icon_width_px starts at the exact width of
            // the icon the builder actually sets; both reuse this one
            // (rgba, w, h) rather than calling plain_glyph_rgba() twice.
            let (initial_rgba, initial_w, initial_h) = status_item_render::plain_glyph_rgba(0);
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
                last_status_item_rect: Mutex::new(None),
                last_icon_width_px: Mutex::new(initial_w),
                status_item_highlighted: Mutex::new(false),
                last_status_item_segments: Mutex::new(Vec::new()),
                last_status_item_worst_used_percent: Mutex::new(0),
                last_status_item_tooltip: Mutex::new("Quotos".to_string()),
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
            // Before anything else touches the window: the class swap is
            // what lets every later show avoid activating the
            // application. See panel_window.rs.
            panel_window::make_nonactivating_panel(&window);
            shell::set_popover_collection_behavior(&window);

            {
                let blur_window = window.clone();
                let blur_app = app.handle().clone();
                window.on_window_event(move |event| {
                    match event {
                        tauri::WindowEvent::Focused(focused) => {
                            if std::env::var_os("QUOTOS_DEBUG_POS").is_some() {
                                eprintln!(
                                    "quotos-pos: Focused({focused}) visible={:?}",
                                    blur_window.is_visible()
                                );
                            }
                            if *focused {
                                return;
                            }
                            // Diagnostic escape hatch, off by default, so
                            // the panel can stay open long enough to be
                            // inspected. Never set in a shipped run.
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
                                shell::set_status_item_highlighted(&blur_app, false);
                                shell::clear_docked_target(&blur_app);
                            }
                        }
                        tauri::WindowEvent::Resized(size) => {
                            // resizable: false in tauri.conf.json only
                            // disables the native resize-handle drag, not a
                            // third-party window manager resizing this
                            // window directly, so this snaps the size back
                            // since the panel's layout math assumes
                            // exactly 360x560 logical and cannot reflow.
                            // Fights only a resize, never a move.
                            //
                            // Compared and reasserted in logical units:
                            // the incoming PhysicalSize is the point size
                            // times the window's own scale factor, and
                            // converting with that same factor is the only
                            // comparison that means anything on a
                            // mixed-DPI setup.
                            let scale = blur_window.scale_factor().unwrap_or(1.0);
                            let (w, h) = (size.width as f64 / scale, size.height as f64 / scale);
                            if (w - PANEL_WINDOW_WIDTH_LOGICAL).abs() > 0.5
                                || (h - PANEL_WINDOW_HEIGHT_LOGICAL).abs() > 0.5
                            {
                                let _ = blur_window.set_size(tauri::LogicalSize::new(
                                    PANEL_WINDOW_WIDTH_LOGICAL,
                                    PANEL_WINDOW_HEIGHT_LOGICAL,
                                ));
                            }
                        }
                        // The self-correcting half of AppState.docked_target:
                        // whenever AppKit relocates the window away from
                        // where it should be docked, nudges it back, but
                        // only once relocating has gone quiet for
                        // MOVE_SETTLE_MS, not on every single Moved event.
                        // The generation counter tracks whether this is
                        // still the most recently scheduled correction, so
                        // a burst of AppKit-internal relocations finishes
                        // before anything reasserts.
                        tauri::WindowEvent::Moved(pos) => {
                            let state = blur_app.state::<AppState>();
                            // Moved reports the frame origin in points
                            // multiplied by the window's current backing
                            // scale factor, undone here so everything
                            // downstream compares in the one coordinate
                            // space that exists. See DisplayPoints.
                            let scale = blur_window.scale_factor().unwrap_or(1.0);
                            let observed = (pos.x as f64 / scale, pos.y as f64 / scale);
                            *state
                                .last_known_position
                                .lock()
                                .expect("last_known_position mutex poisoned") = observed;
                            let this_generation = {
                                let mut generation = state
                                    .move_generation
                                    .lock()
                                    .expect("move_generation mutex poisoned");
                                *generation += 1;
                                *generation
                            };
                            let has_target = state
                                .docked_target
                                .lock()
                                .expect("docked_target mutex poisoned")
                                .is_some();
                            if has_target {
                                let app2 = blur_app.clone();
                                let window2 = blur_window.clone();
                                tauri::async_runtime::spawn(async move {
                                    const MOVE_SETTLE_MS: u64 = 180;
                                    tokio::time::sleep(std::time::Duration::from_millis(
                                        MOVE_SETTLE_MS,
                                    ))
                                    .await;
                                    let state2 = app2.state::<AppState>();
                                    let is_still_latest = *state2
                                        .move_generation
                                        .lock()
                                        .expect("move_generation mutex poisoned")
                                        == this_generation;
                                    if !is_still_latest {
                                        return;
                                    }
                                    let Some(target) = *state2
                                        .docked_target
                                        .lock()
                                        .expect("docked_target mutex poisoned")
                                    else {
                                        return;
                                    };
                                    let current = *state2
                                        .last_known_position
                                        .lock()
                                        .expect("last_known_position mutex poisoned");
                                    // A tolerance, not exact equality:
                                    // AppKit settles the window a point or
                                    // so off whatever was requested, and
                                    // correcting a sub-point gap produced
                                    // an endless correct-drift-correct
                                    // loop.
                                    const SETTLE_TOLERANCE_POINTS: f64 = 2.0;
                                    if (current.0 - target.x).abs() > SETTLE_TOLERANCE_POINTS
                                        || (current.1 - target.y).abs() > SETTLE_TOLERANCE_POINTS
                                    {
                                        shell::apply_docked_position(&app2, &window2, target);
                                    }
                                });
                            }
                        }
                        _ => {}
                    }
                });
            }

            // The status item's right-click menu. "Launch at Login" drives
            // the OS's own login-item registry; see launch_at_login.rs.
            // Its checkmark is read from the OS at build time and re-read
            // after every toggle, never assumed from the click.
            let launch_item = CheckMenuItem::with_id(
                app,
                "launch-at-login",
                "Launch at Login",
                true,
                launch_at_login::status().is_registered(),
                None::<&str>,
            )?;
            let quit_item = MenuItem::with_id(app, "quit", "Quit Quotos", true, None::<&str>)?;
            let menu = Menu::with_items(
                app,
                &[
                    &launch_item,
                    &PredefinedMenuItem::separator(app)?,
                    &quit_item,
                ],
            )?;

            // Built from the same procedural glyph set_status_item_state uses,
            // rather than a static bundled asset, so there is no window
            // between launch and the first set_status_item_state call where a
            // stale or blurry fixed-size icon could show.
            let status_item = TrayIconBuilder::with_id("main-status-item")
                .icon(Image::new_owned(initial_rgba, initial_w, initial_h))
                .icon_as_template(true)
                .menu(&menu)
                .show_menu_on_left_click(false)
                // The composited glyph and digits image carries no text
                // VoiceOver can read; set_status_item_state keeps this current
                // as pinned digits change, and this is just the
                // pre-any-data baseline.
                .tooltip("Quotos")
                .on_menu_event(move |app, event| match event.id.as_ref() {
                    "quit" => app.exit(0),
                    "launch-at-login" => {
                        // Toggled relative to what the OS currently
                        // reports, then the checkmark is set from what the
                        // OS says afterwards, so a refused registration
                        // reads as still off rather than lying.
                        let target = !launch_at_login::status().is_registered();
                        if let Err(message) = launch_at_login::set_registered(target) {
                            eprintln!("quotos: launch at login: {message}");
                        }
                        let _ = launch_item.set_checked(launch_at_login::status().is_registered());
                    }
                    _ => {}
                })
                .on_tray_icon_event(|status_item, event| {
                    // Carried through raw, exactly as tray-icon reports
                    // it; the conversion into a coordinate space that
                    // actually means something happens once, in
                    // compute_docked_layout through
                    // resolve_status_item_point. See DisplayPoints.
                    let rect_position = match &event {
                        TrayIconEvent::Click { rect, .. }
                        | TrayIconEvent::DoubleClick { rect, .. }
                        | TrayIconEvent::Enter { rect, .. }
                        | TrayIconEvent::Leave { rect, .. }
                        | TrayIconEvent::Move { rect, .. } => Some(rect.position),
                        _ => None,
                    };
                    let item_xy = rect_position.map(|p| match p {
                        tauri::Position::Physical(p) => (p.x as f64, p.y as f64),
                        tauri::Position::Logical(p) => (p.x, p.y),
                    });
                    let app = status_item.app_handle();
                    if let Some(xy) = item_xy {
                        // Cached so set_detached's snap-back path, not
                        // itself a status item event, still knows where
                        // to re-dock.
                        *app.state::<AppState>()
                            .last_status_item_rect
                            .lock()
                            .expect("last_status_item_rect mutex poisoned") = Some(xy);
                    }

                    if let (
                        TrayIconEvent::Click {
                            button: tauri::tray::MouseButton::Left,
                            button_state: tauri::tray::MouseButtonState::Up,
                            ..
                        },
                        Some((item_x, item_y)),
                    ) = (&event, item_xy)
                        && let Some(window) = app.get_webview_window("main")
                    {
                        let detached = app
                            .state::<AppState>()
                            .detached
                            .lock()
                            .map(|d| *d)
                            .unwrap_or(false);
                        shell::toggle_panel(app, &window, detached, item_x, item_y);
                    }
                })
                .build(app)?;
            // Pins the item to a fixed length matching this initial image
            // right away, so there is no window between launch and the
            // first repaint where the item is still
            // NSVariableStatusItemLength. See
            // shell::sync_status_item_length's own doc comment.
            shell::sync_status_item_length(&status_item, initial_w);
            app.manage(status_item);

            // Diagnostic only, off by default: opens the panel from the
            // status item's own rect a few seconds after launch, with no
            // click at all. The rect is available without a click; only
            // the event needs one.
            if std::env::var_os("QUOTOS_DEBUG_AUTO_OPEN").is_some() {
                let auto_app = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(Duration::from_secs(3)).await;
                    let _ = auto_app.clone().run_on_main_thread(move || {
                        let app = &auto_app;
                        let (Some(window), Some(status_item)) = (
                            app.get_webview_window("main"),
                            app.tray_by_id("main-status-item"),
                        ) else {
                            return;
                        };
                        let Some(rect) = status_item.rect().ok().flatten() else {
                            return;
                        };
                        let (x, y) = match rect.position {
                            tauri::Position::Physical(p) => (p.x as f64, p.y as f64),
                            tauri::Position::Logical(p) => (p.x, p.y),
                        };
                        shell::show_panel(app, &window, x, y);
                    });
                });
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
