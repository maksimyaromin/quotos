//! Docked-panel placement arithmetic, pure math and read-only screen
//! queries only; `shell.rs` moves windows. See docs/architecture.md's
//! "Coordinate spaces and panel placement" and "The beak".

use crate::status_item_render;

/// The window's own fixed logical size, from `tauri.conf.json`. Duplicated
/// rather than read back from the window, so the Resized-event guard in
/// `run`'s `setup` has a ground truth to snap back to.
pub(crate) const PANEL_WINDOW_WIDTH_LOGICAL: f64 = 360.0;
pub(crate) const PANEL_WINDOW_HEIGHT_LOGICAL: f64 = 560.0;

/// The status item glyph's own center, as an offset in points from the
/// item's own left edge, not the whole button's center. Returns points,
/// never physical pixels; see docs/architecture.md.
fn glyph_center_offset_from_item_left_points(
    item_width_points: Option<f64>,
    icon_width_px: f64,
) -> f64 {
    const GLYPH_WIDTH_POINTS: f64 = 18.0;
    let image_width_points = icon_width_px / 2.0;
    let margin_points = item_width_points
        .map(|w| ((w - image_width_points) / 2.0).max(0.0))
        .unwrap_or(0.0);
    margin_points + status_item_render::GLYPH_LEFT_INSET_POINTS + GLYPH_WIDTH_POINTS / 2.0
}

/// The coordinate space this whole module speaks: global points, never
/// physical pixels, since the APIs this module depends on disagree on
/// which display's scale factor a `Physical*` value used. See docs/architecture.md.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct DisplayPoints {
    /// Top-left corner in the global point space.
    pub(crate) origin: (f64, f64),
    /// Size in points.
    pub(crate) size: (f64, f64),
    /// This display's own backing scale factor. Needed only to undo the
    /// multiplication `tray-icon` applied to the status item rect.
    pub(crate) scale: f64,
    /// Global-point y of this display's own menu bar bottom, read live
    /// from `NSScreen.visibleFrame`, never hardcoded: a notched built-in's
    /// bar is noticeably taller than an external display's. `None` off macOS.
    pub(crate) menu_bar_bottom: Option<f64>,
}

impl DisplayPoints {
    fn contains(&self, x: f64, y: f64) -> bool {
        x >= self.origin.0
            && x < self.origin.0 + self.size.0
            && y >= self.origin.1
            && y < self.origin.1 + self.size.1
    }
}

/// Recovers the global-point position of a `tray-icon` rect and the
/// display it belongs to, by trying each display's own scale factor and
/// keeping the one whose quotient lands inside that display. See docs/architecture.md.
pub(crate) fn resolve_status_item_point(
    displays: &[DisplayPoints],
    item_x: f64,
    item_y: f64,
) -> Option<(usize, f64, f64)> {
    let mut best: Option<(usize, f64, f64, f64)> = None; // (index, x, y, distance below that display's top)
    for (index, display) in displays.iter().enumerate() {
        if display.scale <= 0.0 {
            continue;
        }
        let (x, y) = (item_x / display.scale, item_y / display.scale);
        if !display.contains(x, y) {
            continue;
        }
        let from_top = y - display.origin.1;
        if best.is_none_or(|(_, _, _, best_from_top)| from_top < best_from_top) {
            best = Some((index, x, y, from_top));
        }
    }
    best.map(|(index, x, y, _)| (index, x, y))
}

/// Every display, in global points. Reads `NSScreen` directly on macOS,
/// the only API that also reports `visibleFrame`, needing the main
/// thread; the `tao` path below is the fallback, losing the bar height.
#[cfg(target_os = "macos")]
pub(crate) fn displays_in_points(window: &tauri::WebviewWindow) -> Vec<DisplayPoints> {
    use objc2_app_kit::NSScreen;
    use objc2_foundation::MainThreadMarker;

    let Some(mtm) = MainThreadMarker::new() else {
        return displays_in_points_via_tao(window);
    };
    let screens = NSScreen::screens(mtm);
    // AppKit's global space is y-up; this module is y-down. The first
    // screen's own height is the flip constant, its origin being (0,0).
    let Some(flip) = screens.iter().next().map(|s| s.frame().size.height) else {
        return displays_in_points_via_tao(window);
    };
    screens
        .iter()
        .map(|screen| {
            let frame = screen.frame();
            let visible = screen.visibleFrame();
            let top = flip - (frame.origin.y + frame.size.height);
            // visibleFrame also excludes the Dock, but the Dock never sits
            // at the top, so the difference at the top edge is the menu
            // bar and nothing else.
            let menu_bar_height =
                (frame.origin.y + frame.size.height) - (visible.origin.y + visible.size.height);
            DisplayPoints {
                origin: (frame.origin.x, top),
                size: (frame.size.width, frame.size.height),
                scale: screen.backingScaleFactor(),
                menu_bar_bottom: (menu_bar_height > 0.0).then_some(top + menu_bar_height),
            }
        })
        .collect()
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn displays_in_points(window: &tauri::WebviewWindow) -> Vec<DisplayPoints> {
    displays_in_points_via_tao(window)
}

fn displays_in_points_via_tao(window: &tauri::WebviewWindow) -> Vec<DisplayPoints> {
    window
        .available_monitors()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|m| {
            let scale = m.scale_factor();
            if scale <= 0.0 {
                return None;
            }
            let pos = *m.position();
            let size = *m.size();
            Some(DisplayPoints {
                origin: (pos.x as f64 / scale, pos.y as f64 / scale),
                size: (size.width as f64 / scale, size.height as f64 / scale),
                scale,
                menu_bar_bottom: None,
            })
        })
        .collect()
}

/// Where the docked panel window belongs, all in global points (see
/// `DisplayPoints`): the window's own top-left, plus the beak's CSS `left`
/// inside the panel.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct DockedLayout {
    pub(crate) x: f64,
    pub(crate) y: f64,
    pub(crate) beak_left: f64,
}

/// The pure half of `compute_docked_layout`. Takes no `tauri` handles, so
/// the placement rules are unit-testable against real two-display
/// geometry instead of only on a live screen.
pub(crate) fn docked_layout_in_points(
    display: DisplayPoints,
    item_left: f64,
    item_top: f64,
    item_bottom: f64,
    item_width_points: Option<f64>,
    icon_width_px: f64,
) -> DockedLayout {
    const PANEL_WIDTH: f64 = 332.0;
    // However far left the beak wants the panel, it can never hang off
    // the display's right edge.
    const RIGHT_CLAMP: f64 = 340.0;

    // x is derived from the beak's wanted position, not the panel's; see
    // docs/architecture.md's "The beak" for why that construction is required.
    const BEAK_BASE_WIDTH: f64 = 20.0; // BEAK_BASE_HALF * 2 in Panel.jsx
    const BEAK_HEIGHT: f64 = 10.0; // BEAK_HEIGHT in Panel.jsx
    const BEAK_INSET_IN_PANEL: f64 = 20.0; // notch's left edge, from the panel's own left edge
    const BEAK_TIP_CLEARANCE: f64 = 2.0; // how far the tip stops short of the menu bar

    // Mirrors app.css's own insets: shadow-blur margin, and headroom for
    // the beak. Both are load-bearing, since the window is what's positioned.
    const PANEL_INSET_X: f64 = (PANEL_WINDOW_WIDTH_LOGICAL - PANEL_WIDTH) / 2.0;
    const PANEL_INSET_TOP: f64 = 12.0; // app.css's body { padding-top }, Panel.jsx's NOTCH_RESERVE

    let glyph_center =
        item_left + glyph_center_offset_from_item_left_points(item_width_points, icon_width_px);

    // Places the window so the beak lands at its wanted inset on the
    // glyph's center, then clamps x to the display.
    let wanted_panel_left = glyph_center - BEAK_INSET_IN_PANEL - BEAK_BASE_WIDTH / 2.0;
    let mut x = wanted_panel_left - PANEL_INSET_X;
    x = x.max(display.origin.0);
    let max_x = (display.origin.0 + display.size.0 - RIGHT_CLAMP).max(display.origin.0);
    x = x.min(max_x);

    // visibleFrame reports no bar height inside a full-screen Space, so
    // this reconstructs it from the item's own centering; see docs/architecture.md.
    let inset_above_item = (item_top - display.origin.1).max(0.0);
    // NEG_INFINITY, not 0: a display left of the primary has negative
    // coordinates, where 0 is not a neutral floor but a point far below it.
    let menu_bar_bottom = display
        .menu_bar_bottom
        .unwrap_or(f64::NEG_INFINITY)
        .max(item_bottom + inset_above_item);
    // Solves for the window top from where the beak's tip should end up:
    // tip = y + PANEL_INSET_TOP - BEAK_HEIGHT, and tip should be
    // BEAK_TIP_CLEARANCE below the bar.
    let y = menu_bar_bottom + BEAK_TIP_CLEARANCE - (PANEL_INSET_TOP - BEAK_HEIGHT);

    // Recomputed from the applied x, not the wanted one; see docs/architecture.md.
    let panel_left = x + PANEL_INSET_X;
    let beak_left = (glyph_center - panel_left - BEAK_BASE_WIDTH / 2.0)
        .clamp(0.0, PANEL_WIDTH - BEAK_BASE_WIDTH);

    DockedLayout { x, y, beak_left }
}

/// Where a header-drag gesture started: the live cursor and the window's
/// own top-left, both in global points. Held in `AppState.manual_drag_anchor`.
#[derive(Clone, Copy)]
pub(crate) struct DragAnchor {
    pub(crate) mouse: (f64, f64),
    pub(crate) window_top_left: (f64, f64),
}

/// Pure delta math for `drag_window_step`. The target keeps the same
/// offset from the cursor it had when the gesture began, so a drag never
/// accumulates rounding drift the way re-anchoring to the last step would.
pub(crate) fn drag_target_from_anchor(anchor: DragAnchor, current_mouse: (f64, f64)) -> (f64, f64) {
    (
        anchor.window_top_left.0 + (current_mouse.0 - anchor.mouse.0),
        anchor.window_top_left.1 + (current_mouse.1 - anchor.mouse.1),
    )
}

#[cfg(test)]
mod tests {
    mod glyph_offset {
        use super::super::glyph_center_offset_from_item_left_points;

        #[test]
        fn bare_glyph_offset_matches_the_measured_item_center() {
            let offset = glyph_center_offset_from_item_left_points(Some(46.0), 60.0);
            assert!(
                (offset - 22.0).abs() < 0.01,
                "expected ~22pt offset, got {offset}pt"
            );
        }

        #[test]
        fn pinned_digits_offset_scales_with_the_wider_image_not_a_fixed_constant() {
            let offset = glyph_center_offset_from_item_left_points(Some(74.0), 58.0 * 2.0);
            assert!(
                (offset - 22.0).abs() < 0.01,
                "expected ~22pt offset, got {offset}pt"
            );
        }

        #[test]
        fn the_offset_is_independent_of_any_display_scale_factor() {
            let retina = glyph_center_offset_from_item_left_points(Some(46.0), 60.0);
            let non_retina = glyph_center_offset_from_item_left_points(Some(46.0), 60.0);
            assert_eq!(retina, non_retina);
        }

        #[test]
        fn missing_item_rect_falls_back_to_glyph_flush_with_the_left_edge() {
            let offset = glyph_center_offset_from_item_left_points(None, 60.0);
            assert!((offset - 14.0).abs() < 0.01, "got {offset}");
        }
    }

    mod docked_layout {
        use super::super::{DisplayPoints, docked_layout_in_points, resolve_status_item_point};

        // Deliberately different menu bar heights per display: 33pt built-in,
        // 24pt external, so a single wrong constant would fail one of them.
        const BUILT_IN: DisplayPoints = DisplayPoints {
            origin: (0.0, 0.0),
            size: (1728.0, 1117.0),
            scale: 2.0,
            menu_bar_bottom: Some(33.0),
        };
        const EXTERNAL: DisplayPoints = DisplayPoints {
            origin: (-2560.0, -855.0),
            size: (2560.0, 2880.0),
            scale: 1.0,
            menu_bar_bottom: Some(-831.0),
        };

        // A bare status item as it really measures: the 18pt glyph padded
        // to a 30pt image, status_item_render's SIDE_PAD_PX, inside an
        // NSStatusItem 8pt wider on each side again.
        const ITEM_W: f64 = 46.0;
        const ICON_PX: f64 = 60.0; // that 30pt image at the fixed 2x convention

        fn glyph_center(item_left: f64, item_w: f64, icon_px: f64) -> f64 {
            item_left
                + super::super::glyph_center_offset_from_item_left_points(Some(item_w), icon_px)
        }

        fn item_top(display: DisplayPoints) -> f64 {
            let bar_height = display
                .menu_bar_bottom
                .map(|b| b - display.origin.1)
                .unwrap_or(33.0);
            display.origin.1 + (bar_height - 24.0).max(0.0) / 2.0
        }

        fn item_bottom(display: DisplayPoints) -> f64 {
            item_top(display) + 24.0
        }

        fn displays() -> Vec<DisplayPoints> {
            vec![BUILT_IN, EXTERNAL]
        }

        // tray-icon multiplies 1183 by the built-in's scale, arriving as
        // 2366; only dividing by that same display's scale recovers 1183.
        #[test]
        fn a_status_item_rect_from_the_retina_built_in_resolves_to_that_display_in_points() {
            let (index, x, y) = resolve_status_item_point(&displays(), 2366.0, 0.0)
                .expect("built-in status item point must resolve");
            assert_eq!(index, 0);
            assert!((x - 1183.0).abs() < 0.01, "got {x}");
            assert!(y.abs() < 0.01, "got {y}");
        }

        // The mirror case: negative point coordinates, and a 1x scale
        // where the raw value is already the answer.
        #[test]
        fn a_status_item_rect_from_the_1x_external_display_resolves_to_that_display_in_points() {
            let (index, x, y) = resolve_status_item_point(&displays(), -400.0, -855.0)
                .expect("external status item point must resolve");
            assert_eq!(index, 1);
            assert!((x + 400.0).abs() < 0.01, "got {x}");
            assert!((y + 855.0).abs() < 0.01, "got {y}");
        }

        #[test]
        fn a_status_item_rect_matching_no_display_resolves_to_nothing_rather_than_a_guess() {
            assert!(resolve_status_item_point(&displays(), 90_000.0, 90_000.0).is_none());
        }

        // tip = y + (panel top inset 12 - beak height 10) lands the
        // window's own top flush with the bar, at that display's own
        // menu_bar_bottom plus BEAK_TIP_CLEARANCE.
        #[test]
        fn the_beaks_tip_sits_just_under_the_menu_bar_of_its_own_display() {
            let tip = |l: super::super::DockedLayout| l.y + (12.0 - 10.0);

            let built_in = docked_layout_in_points(
                BUILT_IN,
                1183.0,
                item_top(BUILT_IN),
                item_bottom(BUILT_IN),
                Some(ITEM_W),
                ICON_PX,
            );
            assert!(
                (tip(built_in) - (33.0 + 2.0)).abs() < 0.01,
                "got {}",
                tip(built_in)
            );

            let external = docked_layout_in_points(
                EXTERNAL,
                -400.0,
                item_top(EXTERNAL),
                item_bottom(EXTERNAL),
                Some(ITEM_W),
                ICON_PX,
            );
            assert!(
                (tip(external) - (-831.0 + 2.0)).abs() < 0.01,
                "got {}",
                tip(external)
            );
        }

        #[test]
        fn the_top_follows_each_displays_own_menu_bar_height_not_one_constant() {
            let built_in = docked_layout_in_points(
                BUILT_IN,
                1183.0,
                item_top(BUILT_IN),
                item_bottom(BUILT_IN),
                Some(ITEM_W),
                ICON_PX,
            );
            let external = docked_layout_in_points(
                EXTERNAL,
                -400.0,
                item_top(EXTERNAL),
                item_bottom(EXTERNAL),
                Some(ITEM_W),
                ICON_PX,
            );
            let below_own_top = |l: super::super::DockedLayout, d: DisplayPoints| l.y - d.origin.1;
            assert!(
                (below_own_top(built_in, BUILT_IN) - 33.0).abs() < 0.01,
                "got {}",
                below_own_top(built_in, BUILT_IN)
            );
            assert!(
                (below_own_top(external, EXTERNAL) - 24.0).abs() < 0.01,
                "got {}",
                below_own_top(external, EXTERNAL)
            );
            assert_ne!(
                below_own_top(built_in, BUILT_IN),
                below_own_top(external, EXTERNAL)
            );
        }

        // Reconstructing the bar from the status item, since a status item
        // is centered in its bar, must land on the same answer as reading
        // the bar directly.
        #[test]
        fn a_hidden_menu_bar_is_reconstructed_from_the_status_item_to_the_same_answer() {
            let hidden_bar = DisplayPoints {
                menu_bar_bottom: None,
                ..BUILT_IN
            };
            let with_bar = docked_layout_in_points(
                BUILT_IN,
                1183.0,
                item_top(BUILT_IN),
                item_bottom(BUILT_IN),
                Some(ITEM_W),
                ICON_PX,
            );
            let without = docked_layout_in_points(
                hidden_bar,
                1183.0,
                item_top(hidden_bar),
                item_bottom(hidden_bar),
                Some(ITEM_W),
                ICON_PX,
            );
            assert!(
                (without.y - with_bar.y).abs() < 0.01,
                "{} vs {}",
                without.y,
                with_bar.y
            );
        }

        #[test]
        fn the_fallback_floor_works_on_a_negative_coordinate_display() {
            let hidden_bar = DisplayPoints {
                menu_bar_bottom: None,
                ..EXTERNAL
            };
            let layout = docked_layout_in_points(
                hidden_bar,
                -400.0,
                item_top(hidden_bar),
                item_bottom(hidden_bar),
                Some(ITEM_W),
                ICON_PX,
            );
            assert!(
                layout.y < 0.0 && layout.y > EXTERNAL.origin.1,
                "got {}",
                layout.y
            );
        }

        #[test]
        fn the_beak_sits_clear_of_the_panels_left_corner_by_moving_the_panel_not_the_beak() {
            let layout = docked_layout_in_points(
                BUILT_IN,
                1183.0,
                item_top(BUILT_IN),
                item_bottom(BUILT_IN),
                Some(ITEM_W),
                ICON_PX,
            );
            assert!(
                (layout.beak_left - 20.0).abs() < 0.01,
                "got {}",
                layout.beak_left
            );
            assert!(layout.beak_left > 12.0, "must clear the 12pt corner radius");
            let beak_center = layout.x + 14.0 + layout.beak_left + 10.0;
            assert!(
                (beak_center - glyph_center(1183.0, ITEM_W, ICON_PX)).abs() < 0.01,
                "got {beak_center}"
            );
        }

        // Pinning digits widens the item and moves its left edge, and the
        // glyph, left; what must hold is the beak on the glyph's real center.
        #[test]
        fn the_beak_lands_on_the_glyph_center_both_before_and_after_pinning() {
            // window x + the 14pt shadow-blur inset + beak_left + half the 12pt notch
            let beak_center = |l: super::super::DockedLayout| l.x + 14.0 + l.beak_left + 10.0;

            let bare = docked_layout_in_points(
                BUILT_IN,
                1183.0,
                item_top(BUILT_IN),
                item_bottom(BUILT_IN),
                Some(ITEM_W),
                ICON_PX,
            );
            let bare_glyph_center: f64 = glyph_center(1183.0, ITEM_W, ICON_PX);
            assert!(
                (beak_center(bare) - bare_glyph_center).abs() < 0.01,
                "bare: {}",
                beak_center(bare)
            );
            assert!(
                (bare.x - (bare_glyph_center - 44.0)).abs() < 0.01,
                "bare panel: {}",
                bare.x
            );

            // The item's right edge holds and its left edge moves left as
            // the composited image widens to fit the glyph plus one
            // pinned segment.
            let pinned = docked_layout_in_points(
                BUILT_IN,
                1155.0,
                item_top(BUILT_IN),
                item_bottom(BUILT_IN),
                Some(74.0),
                58.0 * 2.0,
            );
            let pinned_glyph_center = glyph_center(1155.0, 74.0, 58.0 * 2.0);
            assert!(
                (beak_center(pinned) - pinned_glyph_center).abs() < 0.01,
                "pinned: {}",
                beak_center(pinned)
            );
            assert!(
                (pinned.x - (pinned_glyph_center - 44.0)).abs() < 0.01,
                "pinned panel: {}",
                pinned.x
            );

            // Unpinning restores the bare geometry exactly, no hysteresis,
            // since every input is re-derived rather than accumulated.
            let unpinned = docked_layout_in_points(
                BUILT_IN,
                1183.0,
                item_top(BUILT_IN),
                item_bottom(BUILT_IN),
                Some(ITEM_W),
                ICON_PX,
            );
            assert_eq!(bare, unpinned);
        }

        #[test]
        fn at_the_right_screen_edge_the_panel_stops_and_the_beak_keeps_tracking() {
            let item_left = 1728.0 - 40.0; // icon hard against the built-in's right edge
            let layout = docked_layout_in_points(
                BUILT_IN,
                item_left,
                item_top(BUILT_IN),
                item_bottom(BUILT_IN),
                Some(ITEM_W),
                ICON_PX,
            );
            assert!(
                (layout.x - (1728.0 - 340.0)).abs() < 0.01,
                "panel must clamp, got {}",
                layout.x
            );
            let beak_center = layout.x + 14.0 + layout.beak_left + 10.0;
            assert!(
                (beak_center - glyph_center(item_left, ITEM_W, ICON_PX)).abs() < 0.01,
                "beak must still track the glyph, got {beak_center}"
            );
        }

        #[test]
        fn the_beak_stays_within_the_panel() {
            for item_left in [-3000.0f64, -2560.0, 0.0, 900.0, 1727.0, 5000.0] {
                let layout = docked_layout_in_points(
                    BUILT_IN,
                    item_left,
                    item_top(BUILT_IN),
                    item_bottom(BUILT_IN),
                    Some(ITEM_W),
                    ICON_PX,
                );
                assert!(
                    layout.beak_left >= 0.0 && layout.beak_left <= 332.0 - 20.0,
                    "item_left={item_left} gave {}",
                    layout.beak_left
                );
            }
        }
    }

    mod manual_drag {
        use super::super::{DragAnchor, drag_target_from_anchor};

        #[test]
        fn the_target_preserves_the_grab_offset_across_a_move() {
            // Grabbed 80pt right, 50pt down of the window's own top-left.
            let anchor = DragAnchor {
                mouse: (500.0, 200.0),
                window_top_left: (420.0, 150.0),
            };
            let target = drag_target_from_anchor(anchor, (650.0, 120.0));
            assert_eq!(target, (570.0, 70.0));
        }

        #[test]
        fn no_cursor_movement_yields_no_window_movement() {
            let anchor = DragAnchor {
                mouse: (100.0, 100.0),
                window_top_left: (10.0, 10.0),
            };
            assert_eq!(
                drag_target_from_anchor(anchor, anchor.mouse),
                anchor.window_top_left
            );
        }
    }
}
