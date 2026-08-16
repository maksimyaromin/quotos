//! Docked-panel placement arithmetic: the global-point coordinate space, the
//! displays that define it, and the layout math that decides where the
//! panel and its beak go. Everything here is pure math or a read-only
//! screen query. The code that actually moves windows stays in `shell.rs`,
//! which keeps the placement rules unit-testable against real measured
//! geometry.

use crate::tray_render;

/// The window's own fixed logical size, from `tauri.conf.json`'s `width`
/// and `height`. Duplicated here rather than read back from the window,
/// which would just report whatever size it currently, possibly wrongly,
/// is, so the Resized-event guard in `run`'s `setup` has a ground truth to
/// snap back to.
pub(crate) const PANEL_WINDOW_WIDTH_LOGICAL: f64 = 360.0;
pub(crate) const PANEL_WINDOW_HEIGHT_LOGICAL: f64 = 560.0;

/// Returns the beak's horizontal offset in points, relative to the panel's
/// own left edge, so the caller can tell the frontend where to draw it.
/// The beak must stay centered under the tray glyph's own center, not the
/// whole button's center.
///
/// `NSStatusItem` centers the whole composited image, glyph alone or glyph
/// plus pinned digits, within a button that is wider than the image by a
/// small system-chosen margin on each side. That margin is not a fixed
/// distance from the item's own left edge once digits are added, since
/// digits only widen the image, not the glyph portion of it. So rather
/// than hardcode a margin constant, this derives it fresh from two pieces
/// of ground truth: the item's own current width, from `tray.rect()`,
/// queried by the caller so it is paired with the same call's position
/// rather than a value cached at a different instant, and the composited
/// image's own known width, from `AppState.last_icon_width_px`, set by
/// `set_tray_status` right before `set_icon`, always at the fixed "2x of
/// an 18pt-tall image" convention, so dividing by two gives its real width
/// in points regardless of monitor scale. The glyph is always that image's
/// leftmost 18pt, since `tray_render.rs`'s `render` draws digits only
/// after it. Assuming `NSStatusItem` centers the image, half of whatever
/// is left over after the image is the left margin, and the glyph's own
/// center is 9 points further in from there. This holds for however many
/// digits are pinned, not just the bare-glyph case, because it is computed
/// from the actual numbers each time rather than assumed constant.
///
/// If `tray.rect()` is unavailable, this falls back to treating the glyph
/// as flush with the item's own left edge, no margin, rather than failing
/// outright. That is a plausible worst-case position, not a crash.
///
/// Everything here is in points, not physical pixels. See
/// `DisplayPoints`'s doc comment for why this whole module speaks only
/// points. A scale factor that appeared on both sides of this arithmetic
/// and canceled out anyway would still be a place where the wrong scale
/// factor could be supplied by mistake.
fn glyph_center_offset_from_item_left_points(
    item_width_points: Option<f64>,
    icon_width_px: f64,
) -> f64 {
    // tray_render's glyph is always drawn at this size.
    const GLYPH_WIDTH_POINTS: f64 = 18.0;
    // icon_width_px is a buffer width at the fixed "2x of an 18pt-tall
    // image" convention set_icon_for_ns_status_item_button imposes, so
    // dividing by two gives its real width in points on any display.
    let image_width_points = icon_width_px / 2.0;
    let margin_points = item_width_points
        .map(|w| ((w - image_width_points) / 2.0).max(0.0))
        .unwrap_or(0.0);
    // The glyph is not the image's leftmost pixel. The image carries its
    // own side padding so the tray highlight has horizontal air. That
    // inset comes from tray_render, which owns it, rather than being
    // duplicated as a number here.
    margin_points + tray_render::GLYPH_LEFT_INSET_POINTS + GLYPH_WIDTH_POINTS / 2.0
}

/// The coordinate space this whole module speaks, and why.
///
/// macOS has exactly one coordinate space in which a multi-display layout
/// has a single, consistent meaning: the global point space
/// (`CGDisplayBounds` / `NSScreen.frame`), top-left origin at the main
/// display's top-left. There is no global pixel space at all. "Physical
/// pixels" are only ever defined relative to one display's own backing
/// scale factor.
///
/// The three APIs this module depends on each hand out a `Physical*` type
/// that is really "global points times some display's scale factor", and
/// they do not agree on which display's:
///
/// | source | value | scale used |
/// |---|---|---|
/// | `TrayIconEvent`'s `rect` (`tray-icon` 0.24.2 `get_tray_rect`) | tray item frame | the menu bar display's `backingScaleFactor` |
/// | `Monitor::position()`/`size()` (`tao` 0.35.3 `monitor.rs`) | `CGDisplayBounds` / `CGDisplayPixelsWide` | that monitor's own scale factor |
/// | `set_position(Physical)` / `WindowEvent::Moved` (`tao` `window.rs`, `window_delegate.rs`) | window frame | the window's current `backingScaleFactor` |
///
/// On a single-display machine all three coincide and any mismatch is
/// invisible. On a machine with displays at different scale factors they
/// diverge. A tray click at point x=1183 on a scale-2 display arrives here
/// as physical x=2366. Dividing that by the wrong display's scale, for
/// example 1 instead of 2, places a window at point x=2366, off the right
/// edge of every real display: the window is revealed at its stale
/// position first and then vanishes onto whichever display it was last
/// on. Dividing by half the correct scale instead lands the window at
/// half the intended offset from a display's own origin, visibly in the
/// wrong place.
///
/// So the fix is not another retry or another delay. It is to stop
/// speaking a unit that does not exist. Everything below converts to
/// points at the edges, `DisplayPoints` and `resolve_tray_point`, and
/// never leaves them. `apply_docked_position` places the window with a
/// `LogicalPosition`, which `tao`'s `Position::to_logical` passes through
/// untouched, so no scale factor is ever consulted on the way out either.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct DisplayPoints {
    /// Top-left corner in the global point space.
    pub(crate) origin: (f64, f64),
    /// Size in points.
    pub(crate) size: (f64, f64),
    /// This display's own backing scale factor. Needed only to undo the
    /// multiplication `tray-icon` applied to the tray rect.
    pub(crate) scale: f64,
    /// Global-point y of the bottom edge of this display's own menu bar,
    /// read live from `NSScreen.visibleFrame`. `None` on a display that
    /// has no menu bar, or when the screen list was not reachable, either
    /// off the main thread or off macOS.
    ///
    /// The menu bar's height is not a constant and must never be written
    /// down as one. A notched built-in display's menu bar can be
    /// noticeably taller than an unnotched external display's, so a
    /// single hardcoded offset from the display's top edge is wrong on at
    /// least one of them: too small a constant places the panel's top
    /// edge inside the menu bar, where AppKit clamps it back out flush
    /// against the bar with no gap, and too large a constant leaves an
    /// oversized gap on a shorter bar. Deriving this per display instead
    /// makes it correct on a notched screen, an unnotched one, a scaled
    /// one, and any other machine.
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

/// Recovers the global-point position of a `tray-icon` rect, along with
/// the display it belongs to.
///
/// `tray-icon` multiplies the true point coordinate by the menu bar
/// display's scale factor and reports nothing about which display that
/// was, so the division cannot be done blind. This tries each display's
/// own scale factor and keeps the one whose quotient actually lands
/// inside that display. Where an unusual arrangement could make two
/// candidates both land in bounds, the tie-break is the one property a
/// menu bar always has: it hugs its own display's top edge.
pub(crate) fn resolve_tray_point(
    displays: &[DisplayPoints],
    tray_x: f64,
    tray_y: f64,
) -> Option<(usize, f64, f64)> {
    let mut best: Option<(usize, f64, f64, f64)> = None; // (index, x, y, distance below that display's top)
    for (index, display) in displays.iter().enumerate() {
        if display.scale <= 0.0 {
            continue;
        }
        let (x, y) = (tray_x / display.scale, tray_y / display.scale);
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

/// Every display, in global points. On macOS this reads `NSScreen`
/// directly rather than going through `tao`'s `Monitor`, since it is one
/// API, in one coordinate space, and the only one that also reports
/// `visibleFrame`, which is where the menu bar height comes from. See
/// `DisplayPoints::menu_bar_bottom`. It needs the main thread. The `tao`
/// path below is the fallback for anywhere else, and loses only the menu
/// bar height.
#[cfg(target_os = "macos")]
pub(crate) fn displays_in_points(window: &tauri::WebviewWindow) -> Vec<DisplayPoints> {
    use objc2_app_kit::NSScreen;
    use objc2_foundation::MainThreadMarker;

    let Some(mtm) = MainThreadMarker::new() else {
        return displays_in_points_via_tao(window);
    };
    let screens = NSScreen::screens(mtm);
    // AppKit's global space is y-up from the first screen's bottom-left.
    // The rest of this module is y-down from its top-left. That screen's
    // own height is the flip constant, since its origin is (0,0) by
    // definition.
    let Some(flip) = screens.iter().next().map(|s| s.frame().size.height) else {
        return displays_in_points_via_tao(window);
    };
    screens
        .iter()
        .map(|screen| {
            let frame = screen.frame();
            let visible = screen.visibleFrame();
            let top = flip - (frame.origin.y + frame.size.height);
            // `visibleFrame` also excludes the Dock, but the Dock never sits at
            // the top, so the difference at the *top* edge is the menu bar and
            // nothing else.
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
    tray_left: f64,
    tray_top: f64,
    tray_bottom: f64,
    item_width_points: Option<f64>,
    icon_width_px: f64,
) -> DockedLayout {
    const PANEL_WIDTH: f64 = 332.0;
    // However far left the beak wants the panel, it can never hang off
    // the display's right edge.
    const RIGHT_CLAMP: f64 = 340.0;

    // The beak is held a comfortable distance inside the panel's own left
    // edge instead of pressed against it, BEAK_INSET_IN_PANEL, and the
    // panel is pulled up until the beak's tip, not the panel's top edge,
    // sits just under the menu bar, BEAK_TIP_CLEARANCE.
    //
    // These two constraints interact, which is why x below is not simply
    // "icon left minus a constant". The beak's center must stay exactly
    // on the glyph's center, so the only way to also move the beak away
    // from the corner is to move the whole panel further left. Deriving x
    // from the beak's wanted position makes both hold by construction, at
    // whatever glyph offset the tray happens to report, rather than by a
    // second constant that would have to be retuned every time the first
    // one moves.
    const BEAK_BASE_WIDTH: f64 = 20.0; // BEAK_BASE_HALF * 2 in Panel.jsx
    const BEAK_HEIGHT: f64 = 10.0; // BEAK_HEIGHT in Panel.jsx
    const BEAK_INSET_IN_PANEL: f64 = 20.0; // notch's left edge, from the panel's own left edge
    const BEAK_TIP_CLEARANCE: f64 = 2.0; // how far the tip stops short of the menu bar

    // The panel is inset inside its own window, which is deliberately
    // wider and taller at the top: (window width - panel width) / 2 of
    // empty margin per side for the drop shadow's blur to fade into
    // rather than be clipped by, and padding-top worth of headroom for
    // the beak. Both live in app.css, and both are load-bearing here
    // because the window is what gets positioned while the panel is what
    // is actually visible.
    const PANEL_INSET_X: f64 = (PANEL_WINDOW_WIDTH_LOGICAL - PANEL_WIDTH) / 2.0;
    const PANEL_INSET_TOP: f64 = 12.0; // app.css's body { padding-top }, Panel.jsx's NOTCH_RESERVE

    let glyph_center =
        tray_left + glyph_center_offset_from_item_left_points(item_width_points, icon_width_px);

    // Places the window so the beak lands at its wanted inset with its
    // center on the glyph's center, then clamps to the display. The beak
    // offset recomputed below from the clamped x is what keeps it
    // tracking the glyph even when the panel itself is clamped at the
    // screen edge.
    let wanted_panel_left = glyph_center - BEAK_INSET_IN_PANEL - BEAK_BASE_WIDTH / 2.0;
    let mut x = wanted_panel_left - PANEL_INSET_X;
    x = x.max(display.origin.0);
    let max_x = (display.origin.0 + display.size.0 - RIGHT_CLAMP).max(display.origin.0);
    x = x.min(max_x);

    // Both candidates for the menu bar's own bottom edge are read from the
    // running system, never assumed. visibleFrame is the direct answer
    // where it exists, but it does not exist inside a full-screen Space:
    // the menu bar is auto-hidden there, visibleFrame equals frame, and
    // there is no bar height to read even while the bar sits revealed on
    // screen under the cursor. The tray item is still there and still
    // measured, though, and macOS centers a status item vertically in its
    // bar, so the bar's bottom is the item's bottom plus the same inset
    // that sits above it. Derived either way, constant neither way.
    let inset_above_item = (tray_top - display.origin.1).max(0.0);
    // NEG_INFINITY, not 0. A display left of the primary has negative
    // coordinates, where 0 is not a neutral floor but a point far below
    // it.
    let menu_bar_bottom = display
        .menu_bar_bottom
        .unwrap_or(f64::NEG_INFINITY)
        .max(tray_bottom + inset_above_item);
    // Solves for the window top from where the beak's tip should end up:
    // tip = y + PANEL_INSET_TOP - BEAK_HEIGHT, and tip should be
    // BEAK_TIP_CLEARANCE below the bar.
    let y = menu_bar_bottom + BEAK_TIP_CLEARANCE - (PANEL_INSET_TOP - BEAK_HEIGHT);

    // Recomputed from the applied x, not the wanted one, so a screen-edge
    // clamp moves the beak across the panel instead of dragging it off
    // the glyph. Clamped only to keep the notch on the panel at all.
    // buildPanelOutlinePath shrinks whichever top corner the notch
    // encroaches on rather than the notch giving way, since clamping the
    // notch position instead moves the beak visibly off the glyph.
    let panel_left = x + PANEL_INSET_X;
    let beak_left = (glyph_center - panel_left - BEAK_BASE_WIDTH / 2.0)
        .clamp(0.0, PANEL_WIDTH - BEAK_BASE_WIDTH);

    DockedLayout { x, y, beak_left }
}

/// Where a header-drag gesture started: the live cursor and the window's own
/// top-left at that moment, both in global points (see `DisplayPoints`).
/// Captured on the gesture's first `mousemove`, held in
/// `AppState.manual_drag_anchor` until mouseup.
#[derive(Clone, Copy)]
pub(crate) struct DragAnchor {
    pub(crate) mouse: (f64, f64),
    pub(crate) window_top_left: (f64, f64),
}

/// Pure delta math for `drag_window_step`, split out so it's testable without
/// a real window: the target keeps the same offset from the live cursor that
/// it had when the gesture began, so a drag never accumulates rounding drift
/// across many small steps the way repeatedly re-anchoring to the previous
/// step would.
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

        /// A wider, pinned-digits image still centers correctly as long as the
        /// item's own measured width and the image's own known width are both
        /// fresh. This is the whole point of deriving the margin instead of
        /// reusing a single constant: it has to hold for any segment count,
        /// not just the bare-glyph case above.
        #[test]
        fn pinned_digits_offset_scales_with_the_wider_image_not_a_fixed_constant() {
            let offset = glyph_center_offset_from_item_left_points(Some(74.0), 58.0 * 2.0);
            assert!(
                (offset - 22.0).abs() < 0.01,
                "expected ~22pt offset, got {offset}pt"
            );
        }

        // The offset is a property of the tray item and its own composited
        // image, both of which are already in points. No display scale factor
        // may enter into it. Two displays of different scale must give the
        // same answer for the same item.
        #[test]
        fn the_offset_is_independent_of_any_display_scale_factor() {
            let retina = glyph_center_offset_from_item_left_points(Some(46.0), 60.0);
            let non_retina = glyph_center_offset_from_item_left_points(Some(46.0), 60.0);
            assert_eq!(retina, non_retina);
        }

        #[test]
        fn missing_item_rect_falls_back_to_glyph_flush_with_the_left_edge() {
            let offset = glyph_center_offset_from_item_left_points(None, 60.0);
            // No margin data available: the image's own known padding plus half
            // the glyph's width is the best available answer, not a crash or a
            // wildly wrong guess.
            assert!((offset - 14.0).abs() < 0.01, "got {offset}");
        }
    }

    mod docked_layout {
        use super::super::{docked_layout_in_points, resolve_tray_point, DisplayPoints};

        // A two-display arrangement in the global point space macOS actually
        // uses, NSScreen.frame or CGDisplayBounds: a Retina display at (0,0)
        // 1728x1117 scale 2, and an external display at (-2560,-908) 2560x2880
        // scale 1. Every number below is expressed the way one of the three
        // real APIs would hand it over. See DisplayPoints. The menu bar
        // heights are deliberately different, 33pt on the notched built-in
        // display and a standard 24pt on the unnotched external one, since a
        // single constant being wrong on one of them is exactly the bug this
        // module exists to avoid.
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

        // A tray item as macOS lays one out: a 24pt-tall button centered in
        // whatever menu bar it is in, so it can never extend below that bar.
        // A bare tray item as it really measures: the 18pt glyph padded to a
        // 30pt image, tray_render's SIDE_PAD_PX, inside an NSStatusItem 8pt
        // wider on each side again.
        const ITEM_W: f64 = 46.0;
        const ICON_PX: f64 = 60.0; // that 30pt image at the fixed 2x convention

        fn glyph_centre(tray_left: f64, item_w: f64, icon_px: f64) -> f64 {
            tray_left
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

        // tray-icon multiplies the tray item's true point position, 1183 here,
        // by the menu bar display's scale factor, so the value arriving here
        // is 2366, which is not a coordinate in any real space. Dividing by
        // the built-in's own scale recovers 1183. Dividing by the external's
        // leaves 2366, off every display.
        #[test]
        fn a_tray_rect_from_the_retina_built_in_resolves_to_that_display_in_points() {
            let (index, x, y) = resolve_tray_point(&displays(), 2366.0, 0.0)
                .expect("built-in tray point must resolve");
            assert_eq!(index, 0);
            assert!((x - 1183.0).abs() < 0.01, "got {x}");
            assert!(y.abs() < 0.01, "got {y}");
        }

        // The mirror case: the menu bar living on the 1x external display,
        // whose point coordinates are negative. Here tray-icon's
        // multiplication is by 1 and the raw value is already the answer, but
        // only if the external display's scale is the one used to undo it.
        // Halving it, the built-in's scale, lands somewhere else entirely.
        #[test]
        fn a_tray_rect_from_the_1x_external_display_resolves_to_that_display_in_points() {
            let (index, x, y) = resolve_tray_point(&displays(), -400.0, -855.0)
                .expect("external tray point must resolve");
            assert_eq!(index, 1);
            assert!((x + 400.0).abs() < 0.01, "got {x}");
            assert!((y + 855.0).abs() < 0.01, "got {y}");
        }

        #[test]
        fn a_tray_rect_matching_no_display_resolves_to_nothing_rather_than_a_guess() {
            assert!(resolve_tray_point(&displays(), 90_000.0, 90_000.0).is_none());
        }

        // The beak's tip sits BEAK_TIP_CLEARANCE below the menu bar of the
        // display the icon is actually on. Tip = y + (panel top inset 12 -
        // beak height 10), so the window's own top lands flush with the bar:
        // 33 on the notched built-in, -825 on the external, whose bar bottom
        // is -831 and whose coordinates are negative with a bar of a
        // different height again.
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

        // Two displays whose menu bars differ in height must get different
        // tops relative to their own origin. A constant would give the same
        // number for both.
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

        // Inside a full-screen Space the menu bar is auto-hidden and
        // visibleFrame reports no bar at all, even while the bar is sitting
        // revealed under the cursor. The tray item is still measured, and
        // reconstructing the bar from it, since a status item is centered in
        // its bar, must land on the same answer as reading the bar directly,
        // not on some degraded approximation.
        #[test]
        fn a_hidden_menu_bar_is_reconstructed_from_the_tray_item_to_the_same_answer() {
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

        // A display left of the primary has negative coordinates throughout.
        // A "no menu bar reported" floor of 0 would read as far below such a
        // display's real bar rather than as neutral.
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

        // The beak is held a comfortable distance inside the panel's own left
        // edge rather than pressed against it, which, since its center must
        // still be the glyph's center, is achieved by moving the whole panel
        // left rather than by moving the beak.
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
            // Well clear of the 12pt corner radius.
            assert!(layout.beak_left > 12.0);
            // Still exactly on the glyph.
            let beak_centre = layout.x + 14.0 + layout.beak_left + 10.0;
            assert!(
                (beak_centre - glyph_centre(1183.0, ITEM_W, ICON_PX)).abs() < 0.01,
                "got {beak_centre}"
            );
        }

        // Pinning digits widens the tray item, and macOS lays status items
        // out right-to-left, so the item's own left edge moves left, carrying
        // the glyph with it, since NSStatusItem centers the whole composited
        // image and the glyph is that image's leftmost 18pt. Nothing can hold
        // the beak still and under the glyph, because the glyph itself moved.
        // What must hold is that the beak lands on the glyph's real center in
        // both states, with the panel tracking the item the same way in both.
        #[test]
        fn the_beak_lands_on_the_glyph_centre_both_before_and_after_pinning() {
            // The absolute on-screen centre of the notch = window x + the 14pt
            // shadow-blur inset + beak_left + half the 12pt notch.
            let beak_centre = |l: super::super::DockedLayout| l.x + 14.0 + l.beak_left + 10.0;

            let bare = docked_layout_in_points(
                BUILT_IN,
                1183.0,
                item_top(BUILT_IN),
                item_bottom(BUILT_IN),
                Some(ITEM_W),
                ICON_PX,
            );
            let bare_glyph_centre: f64 = glyph_centre(1183.0, ITEM_W, ICON_PX);
            assert!(
                (beak_centre(bare) - bare_glyph_centre).abs() < 0.01,
                "bare: {}",
                beak_centre(bare)
            );
            assert!(
                (bare.x - (bare_glyph_centre - 44.0)).abs() < 0.01,
                "bare panel: {}",
                bare.x
            );

            // A pinned segment: the item grows with the same right edge, so
            // its left edge moves left, and the composited image widens to
            // fit the glyph plus one segment.
            let pinned = docked_layout_in_points(
                BUILT_IN,
                1155.0,
                item_top(BUILT_IN),
                item_bottom(BUILT_IN),
                Some(74.0),
                58.0 * 2.0,
            );
            let pinned_glyph_centre = glyph_centre(1155.0, 74.0, 58.0 * 2.0);
            assert!(
                (beak_centre(pinned) - pinned_glyph_centre).abs() < 0.01,
                "pinned: {}",
                beak_centre(pinned)
            );
            assert!(
                (pinned.x - (pinned_glyph_centre - 44.0)).abs() < 0.01,
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

        // At the right screen edge the panel stops, clamped to screen right
        // minus 340, and the beak keeps tracking the glyph past the panel's
        // own center.
        #[test]
        fn at_the_right_screen_edge_the_panel_stops_and_the_beak_keeps_tracking() {
            // Icon hard against the built-in's right edge.
            let tray_left = 1728.0 - 40.0;
            let layout = docked_layout_in_points(
                BUILT_IN,
                tray_left,
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
            let beak_centre = layout.x + 14.0 + layout.beak_left + 10.0;
            assert!(
                (beak_centre - glyph_centre(tray_left, ITEM_W, ICON_PX)).abs() < 0.01,
                "beak must still track the glyph, got {beak_centre}"
            );
        }

        // The notch can never leave the panel, however extreme the geometry.
        #[test]
        fn the_beak_stays_within_the_panel() {
            for tray_left in [-3000.0f64, -2560.0, 0.0, 900.0, 1727.0, 5000.0] {
                let layout = docked_layout_in_points(
                    BUILT_IN,
                    tray_left,
                    item_top(BUILT_IN),
                    item_bottom(BUILT_IN),
                    Some(ITEM_W),
                    ICON_PX,
                );
                assert!(
                    layout.beak_left >= 0.0 && layout.beak_left <= 332.0 - 20.0,
                    "tray_left={tray_left} gave {}",
                    layout.beak_left
                );
            }
        }
    }

    mod manual_drag {
        use super::super::{drag_target_from_anchor, DragAnchor};

        // The window must follow the cursor 1:1. Whatever offset it had from
        // the cursor when the gesture began, its own top-left minus the
        // anchor mouse point, has to be exactly preserved at every later
        // cursor position, on both axes and through negative deltas, such as
        // dragging up and left toward a display's negative coordinate space.
        // See `DisplayPoints`.
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

        // A cursor position identical to the anchor must be a true no-op.
        // This is what keeps a stationary mousedown followed by an immediate
        // move, a plain click, from nudging the window at all.
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
