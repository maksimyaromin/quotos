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
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use tauri::image::Image;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, WebviewWindow};

use persistence::Store;
use ratelimit::RateLimiter;
use scheduler::Scheduler;

use geometry::{DockedLayout, DragAnchor, PANEL_WINDOW_HEIGHT_LOGICAL, PANEL_WINDOW_WIDTH_LOGICAL};

struct AppState {
    http: reqwest::Client,
    rate_limiter: RateLimiter,
    profile_cache: Mutex<HashMap<String, serde_json::Value>>,
    detached: Mutex<bool>,
    tracked_store: Store,
    statusline_root: PathBuf,
    scheduler: Scheduler,
    sign_in: signin::SignInRegistry,
    last_status_item_rect: Mutex<Option<(f64, f64)>>,
    last_icon_width_px: Mutex<u32>,
    status_item_highlighted: Mutex<bool>,
    last_status_item_segments: Mutex<Vec<shell::StatusItemSegmentDto>>,
    last_status_item_worst_used_percent: Mutex<u8>,
    last_status_item_tooltip: Mutex<String>,
    docked_target: Mutex<Option<DockedLayout>>,
    move_generation: Mutex<u64>,
    last_known_position: Mutex<(f64, f64)>,
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

            log_status_item_font_choice();

            let app_support_dir = app.path().app_config_dir()?;
            claim_single_instance_or_exit(&app_support_dir);
            let tracked_path = app_support_dir.join("tracked.json");
            let (initial_rgba, initial_w, initial_h) = status_item_render::plain_glyph_rgba(0);
            app.manage(initial_app_state(app_support_dir, tracked_path, initial_w));
            accounts::spawn_scheduler(app.handle().clone());

            let window = app
                .get_webview_window("main")
                .expect("the 'main' window must be declared in tauri.conf.json");
            configure_main_window(&window);
            install_window_event_handlers(&window, app.handle().clone());

            let (menu, launch_item) = build_status_item_menu(app.handle())?;
            let status_item = build_status_item(
                app.handle(),
                (initial_rgba, initial_w, initial_h),
                &menu,
                launch_item,
            )?;
            shell::sync_status_item_length(&status_item, initial_w);
            shell::disable_status_item_native_highlight(&status_item);
            app.manage(status_item);

            spawn_debug_auto_open_if_enabled(app.handle().clone());

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn log_status_item_font_choice() {
    eprintln!(
        "quotos: status item digit font = {}",
        if status_item_render::used_fallback_font() {
            "system fallback (MonoLisa not found)"
        } else {
            "MonoLisa"
        }
    );
}

fn claim_single_instance_or_exit(app_support_dir: &Path) {
    match single_instance::claim(app_support_dir) {
        single_instance::Claim::Held(guard) => {
            std::mem::forget(guard);
        }
        single_instance::Claim::TakenByOther => {
            eprintln!("quotos: another Quotos instance is already running; exiting");
            std::process::exit(0);
        }
        single_instance::Claim::Unavailable(err) => {
            eprintln!("quotos: could not check for another running instance ({err}); continuing");
        }
    }
}

fn initial_app_state(
    app_support_dir: PathBuf,
    tracked_path: PathBuf,
    initial_icon_width: u32,
) -> AppState {
    AppState {
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
        last_icon_width_px: Mutex::new(initial_icon_width),
        status_item_highlighted: Mutex::new(false),
        last_status_item_segments: Mutex::new(Vec::new()),
        last_status_item_worst_used_percent: Mutex::new(0),
        last_status_item_tooltip: Mutex::new("Quotos".to_string()),
        docked_target: Mutex::new(None),
        move_generation: Mutex::new(0),
        last_known_position: Mutex::new((0.0, 0.0)),
        manual_drag_anchor: Mutex::new(None),
    }
}

fn configure_main_window(window: &WebviewWindow) {
    let _ = window.hide();
    panel_window::make_nonactivating_panel(window);
    shell::set_popover_collection_behavior(window);
}

fn install_window_event_handlers(window: &WebviewWindow, app: AppHandle) {
    let blur_window = window.clone();
    let blur_app = app;
    window.on_window_event(move |event| match event {
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
        tauri::WindowEvent::Moved(pos) => {
            let state = blur_app.state::<AppState>();
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
                    tokio::time::sleep(std::time::Duration::from_millis(MOVE_SETTLE_MS)).await;
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
    });
}

fn build_status_item_menu(
    app: &AppHandle,
) -> tauri::Result<(Menu<tauri::Wry>, CheckMenuItem<tauri::Wry>)> {
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
    Ok((menu, launch_item))
}

fn build_status_item(
    app: &AppHandle,
    icon: (Vec<u8>, u32, u32),
    menu: &Menu<tauri::Wry>,
    launch_item: CheckMenuItem<tauri::Wry>,
) -> tauri::Result<TrayIcon> {
    let (rgba, w, h) = icon;
    TrayIconBuilder::with_id("main-status-item")
        .icon(Image::new_owned(rgba, w, h))
        .icon_as_template(true)
        .menu(menu)
        .show_menu_on_left_click(false)
        .tooltip("Quotos")
        .on_menu_event(move |app, event| match event.id.as_ref() {
            "quit" => app.exit(0),
            "launch-at-login" => {
                let target = !launch_at_login::status().is_registered();
                if let Err(message) = launch_at_login::set_registered(target) {
                    eprintln!("quotos: launch at login: {message}");
                }
                let _ = launch_item.set_checked(launch_at_login::status().is_registered());
            }
            _ => {}
        })
        .on_tray_icon_event(|status_item, event| {
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
        .build(app)
}

fn spawn_debug_auto_open_if_enabled(app: AppHandle) {
    if std::env::var_os("QUOTOS_DEBUG_AUTO_OPEN").is_none() {
        return;
    }
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_secs(3)).await;
        let _ = app.clone().run_on_main_thread(move || {
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
            shell::show_panel(&app, &window, x, y);
        });
    });
}
