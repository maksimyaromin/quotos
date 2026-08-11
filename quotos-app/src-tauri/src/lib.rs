mod providers;
mod ratelimit;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

use tauri::menu::{Menu, MenuItem};
use tauri::tray::{TrayIconBuilder, TrayIconEvent};
use tauri::{Emitter, Manager};
use tauri_plugin_positioner::{Position, WindowExt};

use providers::{AccountDescriptor, FetchError, RawSnapshot};
use ratelimit::RateLimiter;

struct AppState {
    http: reqwest::Client,
    rate_limiter: RateLimiter,
    profile_cache: Mutex<HashMap<String, serde_json::Value>>,
}

#[tauri::command]
fn list_accounts() -> Vec<AccountDescriptor> {
    providers::claude::discover_accounts()
}

#[tauri::command]
async fn fetch_snapshot(
    state: tauri::State<'_, AppState>,
    account_id: String,
    provider: String,
    config_dir: String,
) -> Result<RawSnapshot, FetchError> {
    if provider != "claude" {
        return Err(FetchError::Other {
            message: format!("no adapter for provider '{provider}'"),
        });
    }

    // The 5-per-300s budget is shared with Claude Code itself; reserving a
    // slot before the network call keeps Quotos from ever being the reason
    // the captain's own /usage view starts 429ing.
    if let Err(retry_after_secs) = state.rate_limiter.try_acquire(&account_id) {
        return Err(FetchError::RateLimited { retry_after_secs });
    }

    let path = PathBuf::from(&config_dir);
    let usage = providers::claude::fetch_usage(&state.http, &path).await?;

    let profile = {
        let cached = {
            let cache = state.profile_cache.lock().expect("profile cache poisoned");
            cache.get(&account_id).cloned()
        };
        match cached {
            Some(p) => Some(p),
            None => {
                let fetched = providers::claude::fetch_profile(&state.http, &path).await;
                if let Some(p) = &fetched {
                    let mut cache = state.profile_cache.lock().expect("profile cache poisoned");
                    cache.insert(account_id.clone(), p.clone());
                }
                fetched
            }
        }
    };

    Ok(RawSnapshot {
        account_id,
        provider,
        config_dir,
        fetched_at: chrono::Utc::now().to_rfc3339(),
        usage,
        profile,
    })
}

#[tauri::command]
fn hide_panel(window: tauri::WebviewWindow) {
    let _ = window.hide();
    let _ = window.emit("panel-visibility", false);
}

/// Sets the text shown beside the tray glyph (macOS `NSStatusItem` title) —
/// the pinned-subscriptions stretch goal. Empty string clears it back to
/// just the glyph.
#[tauri::command]
fn set_tray_title(app: tauri::AppHandle, title: String) -> Result<(), String> {
    if let Some(tray) = app.tray_by_id("main-tray") {
        let value = if title.is_empty() { None } else { Some(title.as_str()) };
        tray.set_title(value).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn show_panel(window: &tauri::WebviewWindow) {
    let _ = window.move_window(Position::TrayBottomRight);
    let _ = window.show();
    let _ = window.set_focus();
    let _ = window.emit("panel-visibility", true);
}

fn toggle_panel(window: &tauri::WebviewWindow) {
    let visible = window.is_visible().unwrap_or(false);
    if visible {
        let _ = window.hide();
        let _ = window.emit("panel-visibility", false);
    } else {
        show_panel(window);
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_positioner::init())
        .manage(AppState {
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(10))
                .build()
                .expect("failed to build HTTP client"),
            rate_limiter: RateLimiter::new(5, Duration::from_secs(300)),
            profile_cache: Mutex::new(HashMap::new()),
        })
        .invoke_handler(tauri::generate_handler![
            list_accounts,
            fetch_snapshot,
            hide_panel,
            set_tray_title,
        ])
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let window = app
                .get_webview_window("main")
                .expect("the 'main' window must be declared in tauri.conf.json");
            let _ = window.hide();

            {
                let blur_window = window.clone();
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::Focused(false) = event {
                        let _ = blur_window.hide();
                        let _ = blur_window.emit("panel-visibility", false);
                    }
                });
            }

            let quit_item = MenuItem::with_id(app, "quit", "Quit Quotos", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&quit_item])?;

            let tray_icon = tauri::include_image!("icons/tray/tray-icon.png");
            let tray = TrayIconBuilder::with_id("main-tray")
                .icon(tray_icon)
                .icon_as_template(true)
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| {
                    if event.id.as_ref() == "quit" {
                        app.exit(0);
                    }
                })
                .on_tray_icon_event(|tray, event| {
                    tauri_plugin_positioner::on_tray_event(tray.app_handle(), &event);
                    if let TrayIconEvent::Click {
                        button: tauri::tray::MouseButton::Left,
                        button_state: tauri::tray::MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            toggle_panel(&window);
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
