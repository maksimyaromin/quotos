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
use tauri_plugin_positioner::{Position, WindowExt};

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
fn hide_panel(window: tauri::WebviewWindow) {
    let _ = window.hide();
    let _ = window.emit("panel-visibility", false);
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TraySegmentDto {
    text: String,
    color: String, // "neutral" | "amber" | "red"
}

/// Sets what's shown beside the tray glyph — the pinned-subscriptions
/// feature (R2-2). Empty `segments` clears it back to just the plain,
/// theme-tinted glyph.
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
/// `tray_render.rs`'s module doc) — with any segments present this drops
/// `icon_as_template` and paints a composed bitmap instead; with none, it
/// reverts to the plain template glyph exactly as before.
#[tauri::command]
fn set_tray_status(app: tauri::AppHandle, segments: Vec<TraySegmentDto>) -> Result<(), String> {
    let Some(tray) = app.tray_by_id("main-tray") else {
        return Ok(());
    };
    tray.set_title(Some("")).map_err(|e| e.to_string())?;

    if segments.is_empty() {
        let (rgba, w, h) = tray_render::plain_glyph_rgba();
        tray.set_icon(Some(Image::new_owned(rgba, w, h))).map_err(|e| e.to_string())?;
        tray.set_icon_as_template(true).map_err(|e| e.to_string())?;
        return Ok(());
    }

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
    let (rgba, w, h) = tray_render::render(&segs);
    tray.set_icon(Some(Image::new_owned(rgba, w, h))).map_err(|e| e.to_string())?;
    tray.set_icon_as_template(false).map_err(|e| e.to_string())?;
    Ok(())
}

/// I7: tear the panel off into a real, freestanding window (`detached =
/// true`) or fold it back into a popover (`false`). Detached mode gets a
/// title bar (so it can be dragged and is unmistakably a window, not a
/// popover), stays out of the hide-on-blur path, and shows up in Cmd+Tab —
/// the captain's stated need is to park it on screen and watch it while
/// working elsewhere, which a thing that vanishes on focus loss cannot do.
#[tauri::command]
fn set_detached(
    window: tauri::WebviewWindow,
    state: tauri::State<'_, AppState>,
    detached: bool,
) -> Result<(), String> {
    *state.detached.lock().expect("detached mutex poisoned") = detached;
    window.set_decorations(detached).map_err(|e| e.to_string())?;
    window.set_skip_taskbar(!detached).map_err(|e| e.to_string())?;
    window.set_always_on_top(true).map_err(|e| e.to_string())?;
    if detached {
        let _ = window.set_focus();
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

/// Followup-1: the round-2 handoff moved the popover's beak off-center — it
/// now sits at the panel's own left edge, under the glyph, with the panel
/// unfolding rightward (`Panel.jsx`'s `beakLeft`, `App.tsx`'s `BEAK_LEFT`).
/// `Position::TrayBottomCenter` centers the window under the icon instead,
/// which now visibly contradicts the beak (confirmed live on the captain's
/// screen — see `data/quotos-fixes-f2/measurements.md`). `TrayBottomLeft`
/// gets us the right *edge* alignment (window left = icon left), then two
/// handoff-specified adjustments get applied on top: the left edge sits 6px
/// inside the icon's own left edge, and the top is pinned at a fixed 32px
/// from the screen top (6px under the menu bar) rather than flush against
/// the icon's bottom. The x nudge is re-clamped to the monitor's left edge;
/// the plugin's own `move_window_constrained` already clamped the right
/// edge using the window's real width, and nudging further left can only
/// keep that satisfied, never violate it.
fn show_panel(window: &tauri::WebviewWindow) {
    let _ = window.move_window_constrained(Position::TrayBottomLeft);
    if let (Ok(pos), Ok(Some(monitor))) = (window.outer_position(), window.current_monitor()) {
        let scale = monitor.scale_factor();
        let left_offset_px = (6.0 * scale).round() as i32;
        let top_px = (32.0 * scale).round() as i32;
        let monitor_pos = monitor.position();
        let x = (pos.x - left_offset_px).max(monitor_pos.x);
        let y = monitor_pos.y + top_px;
        let _ = window.set_position(tauri::PhysicalPosition::new(x, y));
    }
    let _ = window.show();
    let _ = window.set_focus();
    let _ = window.emit("panel-visibility", true);
}

/// Left-clicking the tray icon while detached brings the window forward
/// instead of hiding it — closing a window the captain deliberately parked
/// on screen must be an explicit action, not an accidental side effect of
/// clicking the tray glyph again.
fn toggle_panel(window: &tauri::WebviewWindow, detached: bool) {
    let visible = window.is_visible().unwrap_or(false);
    if visible && !detached {
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

            // Built here rather than via the builder's own `.manage()`
            // because the tracked-list store needs `app.path()`, which
            // isn't available until setup — see persistence.rs for why a
            // plain, `fsync`'d file is what R2-5's reproduction called for.
            let tracked_path = app.path().app_config_dir()?.join("tracked.json");
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
            });
            spawn_scheduler(app.handle().clone());

            let window = app
                .get_webview_window("main")
                .expect("the 'main' window must be declared in tauri.conf.json");
            let _ = window.hide();

            {
                let blur_window = window.clone();
                let blur_app = app.handle().clone();
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::Focused(false) = event {
                        let detached = blur_app
                            .state::<AppState>()
                            .detached
                            .lock()
                            .map(|d| *d)
                            .unwrap_or(false);
                        if !detached {
                            let _ = blur_window.hide();
                            let _ = blur_window.emit("panel-visibility", false);
                        }
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
                            let detached = app
                                .state::<AppState>()
                                .detached
                                .lock()
                                .map(|d| *d)
                                .unwrap_or(false);
                            toggle_panel(&window, detached);
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
