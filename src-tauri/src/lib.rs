mod accounts;
mod atomic_write;
mod geometry;
mod idle;
mod launch_at_login;
mod panel_window;
mod persistence;
mod providers;
mod scheduler;
mod shell;
mod signin;
mod single_instance;
/// Public for `examples/dump_tray_scenes.rs`; see "Looking at the
/// result" in docs/status-item-rendering.md.
pub mod status_item_render;
pub mod statusline;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tauri::image::Image;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, WebviewWindow};

use persistence::Store;
use scheduler::Scheduler;

use geometry::{DockedLayout, DragAnchor, PANEL_WINDOW_HEIGHT_LOGICAL, PANEL_WINDOW_WIDTH_LOGICAL};

struct AppState {
    http: reqwest::Client,
    profile_cache: Mutex<HashMap<String, serde_json::Value>>,
    detached: Mutex<bool>,
    tracked_store: Store,
    statusline_root: PathBuf,
    last_ok_snapshot: Mutex<HashMap<String, providers::RawSnapshot>>,
    scheduler: Scheduler,
    screen_locked: Arc<AtomicBool>,
    sign_in: signin::SignInRegistry,
    last_status_item_rect: Mutex<Option<(f64, f64)>>,
    last_icon_width_px: Mutex<u32>,
    status_item_highlighted: Mutex<bool>,
    last_status_item_segments: Mutex<Vec<shell::StatusItemSegmentDto>>,
    last_status_item_chip_spans: Mutex<Vec<status_item_render::ChipSpan>>,
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
            accounts::load_pin_groups,
            accounts::save_pin_groups,
            accounts::kick_scheduler,
            shell::hide_panel,
            shell::set_status_item_state,
            shell::render_status_item_preview,
            shell::set_detached,
            shell::drag_window_step,
            shell::end_window_drag,
            accounts::start_sign_in,
            accounts::submit_sign_in_code,
            accounts::cancel_sign_in,
            accounts::forget_sign_in,
            accounts::statusline_status,
            accounts::statusline_enable,
            accounts::statusline_disable,
        ])
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            log_status_item_font_choice();

            let app_support_dir = app.path().app_config_dir()?;
            let _ = std::fs::create_dir_all(&app_support_dir);
            claim_single_instance_or_exit(&app_support_dir);
            migrate_legacy_statusline(&app_support_dir);
            let tracked_path = app_support_dir.join("tracked.json");
            let (initial_rgba, initial_w, initial_h) = status_item_render::plain_glyph_rgba(0);
            let state = initial_app_state(app_support_dir, tracked_path, initial_w);
            idle::watch_screen_lock_state(state.screen_locked.clone());
            app.manage(state);
            accounts::spawn_scheduler(app.handle().clone());
            accounts::spawn_statusline_watcher(app.handle().clone());

            let window = app
                .get_webview_window("main")
                .expect("the 'main' window must be declared in tauri.conf.json");
            configure_main_window(&window);
            install_window_event_handlers(&window, app.handle().clone());

            let (menu, launch_item) = build_status_item_menu(app.handle())?;
            let status_item = build_status_item(
                app.handle(),
                (initial_rgba, initial_w, initial_h),
                menu,
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
            // Leaked on purpose: the lock must outlive this function, and
            // dropping the guard would release it immediately.
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

fn migrate_legacy_statusline(app_support_dir: &Path) {
    let candidates: Vec<PathBuf> = providers::claude::discover_accounts()
        .into_iter()
        .map(|a| PathBuf::from(a.config_dir))
        .collect();
    statusline::migrate_legacy(app_support_dir, &candidates);
}

fn initial_app_state(
    app_support_dir: PathBuf,
    tracked_path: PathBuf,
    initial_icon_width: u32,
) -> AppState {
    AppState {
        http: reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .expect("failed to build HTTP client"),
        profile_cache: Mutex::new(HashMap::new()),
        detached: Mutex::new(false),
        tracked_store: Store::load(tracked_path),
        statusline_root: app_support_dir,
        last_ok_snapshot: Mutex::new(HashMap::new()),
        scheduler: Scheduler::new(),
        screen_locked: Arc::new(AtomicBool::new(false)),
        sign_in: signin::SignInRegistry::new(),
        last_status_item_rect: Mutex::new(None),
        last_icon_width_px: Mutex::new(initial_icon_width),
        status_item_highlighted: Mutex::new(false),
        last_status_item_segments: Mutex::new(Vec::new()),
        last_status_item_chip_spans: Mutex::new(Vec::new()),
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

/// What a click on the status item is asking for. The menu appears
/// because this says so, not because one is attached to the item; see
/// "Clicking a group's slug in the menu bar" in docs/architecture.md.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum TrayClickIntent {
    ShowMenu,
    OpenPanelOrFoldGroup,
    Ignore,
}

fn tray_click_intent(
    button: tauri::tray::MouseButton,
    button_state: tauri::tray::MouseButtonState,
) -> TrayClickIntent {
    use tauri::tray::{MouseButton, MouseButtonState};
    match (button, button_state) {
        // On press, the way every other menu bar menu opens.
        (MouseButton::Right, MouseButtonState::Down) => TrayClickIntent::ShowMenu,
        // On release, so a press that turns into a drag is not a click.
        (MouseButton::Left, MouseButtonState::Up) => TrayClickIntent::OpenPanelOrFoldGroup,
        _ => TrayClickIntent::Ignore,
    }
}

/// Whether AppKit may open the item's own menu on a left click.
/// `tray-icon` defaults this to true; a left click belongs to the panel
/// and to a group's chip, so it is false here.
const SHOW_MENU_ON_LEFT_CLICK: bool = false;

/// Attached only for as long as the menu is on screen. `show_menu`
/// runs the menu's own tracking loop and returns once it closes, so
/// the item is left owning no menu and cannot pop one on its own.
fn show_status_item_menu(status_item: &TrayIcon, menu: &Menu<tauri::Wry>) {
    if status_item.set_menu(Some(menu.clone())).is_err() {
        return;
    }
    let _ = status_item.with_inner_tray_icon(|inner| inner.show_menu());
    let _ = status_item.set_menu(None::<Menu<tauri::Wry>>);
}

fn build_status_item(
    app: &AppHandle,
    icon: (Vec<u8>, u32, u32),
    menu: Menu<tauri::Wry>,
    launch_item: CheckMenuItem<tauri::Wry>,
) -> tauri::Result<TrayIcon> {
    let (rgba, w, h) = icon;
    TrayIconBuilder::with_id("main-status-item")
        .show_menu_on_left_click(SHOW_MENU_ON_LEFT_CLICK)
        .icon(Image::new_owned(rgba, w, h))
        .icon_as_template(true)
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
        .on_tray_icon_event(move |status_item, event| {
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

            let TrayIconEvent::Click {
                button,
                button_state,
                position,
                rect,
                ..
            } = &event
            else {
                return;
            };
            match tray_click_intent(*button, *button_state) {
                TrayClickIntent::Ignore => {}
                TrayClickIntent::ShowMenu => show_status_item_menu(status_item, &menu),
                TrayClickIntent::OpenPanelOrFoldGroup => {
                    let (Some((item_x, item_y)), Some(window)) =
                        (item_xy, app.get_webview_window("main"))
                    else {
                        return;
                    };
                    // A click on a group's chip folds that group; one
                    // anywhere else, a bare figure included, opens the
                    // panel. See docs/architecture.md.
                    let item_width = match rect.size {
                        tauri::Size::Physical(s) => s.width as f64,
                        tauri::Size::Logical(s) => s.width,
                    };
                    if let Some(group_id) =
                        shell::group_at_click(app, &window, position.x, item_x, item_y, item_width)
                    {
                        let _ = app.emit("status-item-group-clicked", group_id);
                        return;
                    }
                    let detached = app
                        .state::<AppState>()
                        .detached
                        .lock()
                        .map(|d| *d)
                        .unwrap_or(false);
                    shell::toggle_panel(app, &window, detached, item_x, item_y);
                }
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

#[cfg(test)]
mod tests {
    use super::{SHOW_MENU_ON_LEFT_CLICK, TrayClickIntent, tray_click_intent};
    use tauri::tray::{MouseButton, MouseButtonState};

    /// The gap the last round's fix left: AppKit opens a menu of its
    /// own off a left click whenever the item owns one, which it does
    /// while a right-click's menu is being tracked.
    #[test]
    fn nothing_at_all_opens_the_menu_except_a_right_press() {
        for button in [MouseButton::Left, MouseButton::Right, MouseButton::Middle] {
            for state in [MouseButtonState::Down, MouseButtonState::Up] {
                // Every route to the menu there is: the intent this
                // module reads, and AppKit's own behaviour on an item
                // that owns one.
                let by_this_module = tray_click_intent(button, state) == TrayClickIntent::ShowMenu;
                let by_appkit = SHOW_MENU_ON_LEFT_CLICK && button == MouseButton::Left;
                let expected = button == MouseButton::Right && state == MouseButtonState::Down;
                assert_eq!(
                    by_this_module || by_appkit,
                    expected,
                    "{button:?} {state:?}: by this module's own intent or by AppKit's"
                );
            }
        }
    }

    #[test]
    fn a_right_press_is_the_only_thing_that_shows_the_menu() {
        assert_eq!(
            tray_click_intent(MouseButton::Right, MouseButtonState::Down),
            TrayClickIntent::ShowMenu
        );
        for (button, state) in [
            (MouseButton::Right, MouseButtonState::Up),
            (MouseButton::Middle, MouseButtonState::Down),
            (MouseButton::Middle, MouseButtonState::Up),
        ] {
            assert_ne!(
                tray_click_intent(button, state),
                TrayClickIntent::ShowMenu,
                "{button:?} {state:?} must not show the menu"
            );
        }
    }

    /// The reported regression: the Launch at Login / Quit menu came up
    /// on a plain left click, where the panel belongs.
    #[test]
    fn no_left_click_of_any_kind_shows_the_menu() {
        assert_eq!(
            tray_click_intent(MouseButton::Left, MouseButtonState::Up),
            TrayClickIntent::OpenPanelOrFoldGroup
        );
        assert_eq!(
            tray_click_intent(MouseButton::Left, MouseButtonState::Down),
            TrayClickIntent::Ignore
        );
    }

    #[test]
    fn a_release_of_a_button_other_than_the_left_one_does_nothing() {
        assert_eq!(
            tray_click_intent(MouseButton::Middle, MouseButtonState::Up),
            TrayClickIntent::Ignore
        );
    }
}
