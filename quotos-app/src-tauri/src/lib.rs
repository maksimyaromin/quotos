mod accounts;
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
pub mod statusline;
mod tray_render;

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
    last_tray_segments: Mutex<Vec<shell::TraySegmentDto>>,
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
    /// `performWindowDragWithEvent:` dragging isn't used). `None` whenever
    /// no manual drag is in progress.
    manual_drag_anchor: Mutex<Option<DragAnchor>>,
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
            shell::hide_panel,
            shell::set_tray_status,
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

            // R2-T1: log once at startup which font the tray's percentage
            // digits actually resolved to on this machine — MonoLisa if
            // installed, otherwise the system tabular-figure UI font (see
            // `tray_render.rs`'s `text` submodule for the lookup/fallback).
            eprintln!(
                "quotos: tray digit font = {}",
                if tray_render::used_fallback_font() {
                    "system fallback (MonoLisa not found)"
                } else {
                    "MonoLisa"
                }
            );

            // Built here rather than via the builder's own `.manage()`
            // because the tracked-list store needs `app.path()`, which
            // isn't available until setup — see persistence.rs for why a
            // plain, `fsync`'d file is what R2-5's reproduction called for.
            let app_support_dir = app.path().app_config_dir()?;
            // Before anything else touches shared state (the tracked store,
            // the scheduler): if a Quotos is already running, this one must
            // bow out — two instances each run their own rate limiter
            // against the same shared 5-per-300s allowance and spend it
            // double-speed (see single_instance.rs for the full argument).
            match single_instance::claim(&app_support_dir) {
                single_instance::Claim::Held(guard) => {
                    // The OS lock lives exactly as long as this handle stays
                    // open, and its owner is the process itself — so the
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
                                shell::set_tray_highlighted(&blur_app, false);
                                shell::clear_docked_target(&blur_app);
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
                            if (w - PANEL_WINDOW_WIDTH_LOGICAL).abs() > 0.5
                                || (h - PANEL_WINDOW_HEIGHT_LOGICAL).abs() > 0.5
                            {
                                let _ = blur_window.set_size(tauri::LogicalSize::new(
                                    PANEL_WINDOW_WIDTH_LOGICAL,
                                    PANEL_WINDOW_HEIGHT_LOGICAL,
                                ));
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
                                        return; // a newer Moved event superseded this one — let it debounce instead
                                    }
                                    let Some(target) = *state2
                                        .docked_target
                                        .lock()
                                        .expect("docked_target mutex poisoned")
                                    else {
                                        return; // hidden or detached by the time this fired
                                    };
                                    let current = *state2
                                        .last_known_position
                                        .lock()
                                        .expect("last_known_position mutex poisoned");
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
                                        shell::apply_docked_position(&app2, &window2, target);
                                    }
                                });
                            }
                        }
                        _ => {}
                    }
                });
            }

            // The tray's right-click menu. "Launch at Login" drives the
            // OS's own login-item registry (`launch_at_login.rs`); its
            // checkmark is read from the OS at build time and re-read after
            // every toggle, never assumed from the click.
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
                .on_menu_event(move |app, event| match event.id.as_ref() {
                    "quit" => app.exit(0),
                    "launch-at-login" => {
                        // Toggle relative to what the OS currently reports
                        // (the native click already flipped the checkmark
                        // optimistically), then set the checkmark from what
                        // the OS says afterwards — a refused registration
                        // (e.g. an unbundled dev binary) reads as still-off
                        // rather than lying.
                        let target = !launch_at_login::status().is_registered();
                        if let Err(message) = launch_at_login::set_registered(target) {
                            eprintln!("quotos: launch at login: {message}");
                        }
                        let _ = launch_item.set_checked(launch_at_login::status().is_registered());
                    }
                    _ => {}
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
                        *app.state::<AppState>()
                            .last_tray_rect
                            .lock()
                            .expect("last_tray_rect mutex poisoned") = Some(xy);
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
                            shell::toggle_panel(app, &window, detached, tray_x, tray_y);
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
                        let (Some(window), Some(tray)) =
                            (app.get_webview_window("main"), app.tray_by_id("main-tray"))
                        else {
                            return;
                        };
                        let Some(rect) = tray.rect().ok().flatten() else {
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
