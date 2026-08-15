//! Docked-panel placement arithmetic: the global-point coordinate space, the
//! displays that define it, and the layout math that decides where the panel
//! and its beak go. Everything here is pure math or a read-only screen query
//! — the code that actually moves windows stays in `lib.rs` — which is what
//! keeps the placement rules unit-testable against real measured geometry.

use crate::tray_render;

/// The window's own fixed logical size, from `tauri.conf.json`'s `width`/
/// `height` — duplicated here (rather than read back from the window, which
/// would just report whatever size it currently, possibly wrongly, is) so
/// the Resized-event guard in `run`'s `setup` has a ground truth to snap
/// back to.
pub(crate) const PANEL_WINDOW_WIDTH_LOGICAL: f64 = 360.0;
pub(crate) const PANEL_WINDOW_HEIGHT_LOGICAL: f64 = 560.0;

/// Followup-1 fix (round 2 regression, the panel-never-opens defect): the
/// round-2 handoff moved the popover's beak off-center — it now sits at the
/// panel's own left edge, under the glyph, with the panel unfolding
/// rightward (`Panel.jsx`'s `beakLeft`, `App.tsx`'s `BEAK_LEFT`).
///
/// This used to go through `tauri-plugin-positioner`'s
/// `move_window_constrained(Position::TrayBottomLeft)`, primed by its own
/// `on_tray_event` cache. On a real click that produced a window position
/// wildly inconsistent with the tray icon's own rect (e.g. tray at physical
/// x=2444 on a 3456-wide monitor placed the window at physical x=1368 —
/// nowhere near clamped-to-monitor math could explain), landing the panel
/// off in empty space on the correct monitor, which reads on screen as
/// "nothing opened" — exactly this bug. Confirmed with the tray rect and
/// computed target logged and diffed against the window's real post-move
/// bounds (read independently of Tauri's own position getters, via
/// `CGWindowListCopyWindowInfo`, since `WebviewWindow::outer_position()`
/// itself was also observed misreporting immediately after a window's
/// first-ever `show()`); root cause narrowed to the plugin's own
/// `calculate_position`/`get_monitor_for_tray_icon` path, not chased further
/// upstream. Rather than depend on that plugin at all (the whole
/// `tauri-plugin-positioner` dependency is dropped by this fix), this now
/// computes the position itself directly from the tray icon's own `Rect`
/// (handed to us fresh on every click via `TrayIconEvent::Click`'s `rect`
/// field — always physical pixels, per `tray-icon` v0.24.2's own `Rect`
/// type) plus `monitor_from_point` on that same rect, per the handoff: left
/// edge = icon left − 6px (clamped to no further right than screen right −
/// 340px, never left of the monitor's own left edge — this is what keeps
/// both displays working, including an external display positioned such
/// that its coordinates are negative), top pinned at a fixed 32px from that
/// monitor's own top edge (6px under the menu bar, not flush against the
/// icon's bottom). The reposition happens after `show()`/`set_focus()`
/// rather than before — empirically, positioning a still-hidden window on
/// its very first ever appearance was less reliable than repositioning it
/// once already shown; the `Moved`/`Focused(true)` window events and the
/// WindowServer's own reported bounds both confirm the final position lands
/// exactly on target either way, but this ordering was what was verified.
///
/// Also returns the beak's horizontal offset (logical/CSS px, relative to
/// the panel's own left edge) so the caller can tell the frontend where to
/// draw it. B3/B5: the beak must stay centered under the tray glyph's own
/// center, not the whole button's center.
///
/// R3-1 fix: this used to assume the glyph sat a fixed "6px button padding"
/// inside the item, i.e. `icon_left + 15pt` — a number carried over from a
/// design-system mockup, never checked against a real tray item. Measuring
/// the real thing (a pixel-precise screenshot of the bare glyph, cross-checked
/// against `CGWindowListCopyWindowInfo` and the item's own Accessibility
/// rect — see `RESULT.md`) found the *actual* glyph center sitting almost
/// exactly at the item's own geometric center instead, ~18pt from its left
/// edge, not 15 — because `NSStatusItem` centers the whole composited image
/// (glyph, or glyph+digits once pinned) within a button that's wider than
/// the image by a small system-chosen margin on each side. Pinning a second
/// data point (item width 36pt bare vs 64pt with one pinned segment, right
/// edge unchanged either way — status items lay out right-to-left) confirmed
/// that margin isn't the same fixed distance from the item's own left edge
/// once digits are added, since digits only widen the image, not the glyph
/// portion of it.
///
/// So rather than hardcode a second (equally guessable) constant, this now
/// derives the margin fresh from the two pieces of ground truth Quotos
/// actually has: the item's own current width (`tray.rect()`, queried here
/// so it's paired with the same call's position rather than a value cached
/// at a different instant) and the composited image's own known width
/// (`AppState.last_icon_width_px`, set by `set_tray_status` right before
/// `set_icon` — always at the fixed "2x of an 18pt-tall image" convention,
/// so `/2` gives its real width in points regardless of monitor scale). The
/// glyph is always that image's leftmost 18pt (`tray_render.rs`'s `render`
/// draws digits only after it) — assuming `NSStatusItem` centers the image
/// (confirmed above), half of whatever's left over after the image is the
/// left margin, and the glyph's own center is 9pt further in from there.
/// This is exact for however many digits are pinned, not just the bare-glyph
/// case it was checked against, because it's computed from the actual
/// numbers each time rather than assumed constant.
///
/// If `tray.rect()` is unavailable (already `None` in this same run's other
/// call — see `resync_docked_position_after_icon_change`), this falls back
/// to treating the glyph as flush with the item's own left edge (no margin)
/// rather than failing outright — a plausible-worst-case position, not a
/// crash.
///
/// R3-7: everything here is in **points**, not "physical pixels" — see
/// `DisplayPoints`'s doc comment for why this whole module stopped speaking
/// physical pixels at all. The scale factor used to appear on both sides of
/// this arithmetic and cancel out anyway; dropping it removes a place where
/// the *wrong* scale factor could be supplied.
fn glyph_center_offset_from_item_left_points(item_width_points: Option<f64>, icon_width_px: f64) -> f64 {
    const GLYPH_WIDTH_POINTS: f64 = 18.0; // fixed: tray_render's glyph is always drawn at this size
    // `last_icon_width_px` is a buffer width at the fixed "2x of an 18pt-tall
    // image" convention `set_icon_for_ns_status_item_button` imposes, so /2
    // is its real width in points on any display.
    let image_width_points = icon_width_px / 2.0;
    let margin_points = item_width_points.map(|w| ((w - image_width_points) / 2.0).max(0.0)).unwrap_or(0.0);
    // R3-11: the glyph is no longer the image's leftmost pixel — the image
    // carries its own side padding so A11's highlight has horizontal air. That
    // inset comes from `tray_render`, which owns it, rather than being
    // duplicated as a number here.
    margin_points + tray_render::GLYPH_LEFT_INSET_POINTS + GLYPH_WIDTH_POINTS / 2.0
}

/// R3-7 — the coordinate space this whole module speaks, and why it had to
/// change.
///
/// macOS has exactly one coordinate space in which a multi-display layout has
/// a single, consistent meaning: the **global point space** (`CGDisplayBounds`
/// / `NSScreen.frame`), top-left origin at the main display's top-left. There
/// is no global *pixel* space at all — "physical pixels" are only ever defined
/// relative to one display's own backing scale factor.
///
/// The three APIs this module depends on each hand out a `Physical*` type that
/// is really "global points × **some** display's scale factor", and they do not
/// agree on *which* display's:
///
/// | source | value | scale used |
/// |---|---|---|
/// | `TrayIconEvent`'s `rect` (`tray-icon` 0.24.2 `get_tray_rect`) | tray item frame | the **menu bar display**'s `backingScaleFactor` |
/// | `Monitor::position()`/`size()` (`tao` 0.35.3 `monitor.rs`) | `CGDisplayBounds` / `CGDisplayPixelsWide` | **that monitor's own** scale factor |
/// | `set_position(Physical)` / `WindowEvent::Moved` (`tao` `window.rs`, `window_delegate.rs`) | window frame | the **window's current** `backingScaleFactor` |
///
/// On a single-display machine all three coincide and the bug is invisible.
/// On the captain's actual setup — built-in Retina at point `(0,0,1728,1117)`
/// scale 2, external LG at point `(-2560,-908,2560,2880)` scale 1 — they
/// diverge, and every symptom he reported falls straight out of it:
///
/// * A tray click on the built-in yields `tray_x = 2366` ("1183 points × 2").
///   Handed to `set_position(Physical(…))` while the window happens to be on
///   the *LG* (scale 1), `tao` divides by **1** and places the window at point
///   x = 2354 — past the right edge of every display, i.e. nowhere. The window
///   is revealed at its stale position first (below) and then vanishes:
///   *"панель мерцает только"*, and it flickers **on the other monitor**,
///   which is where it was last left — *"кликаю на макбуке, глитч на другом"*.
/// * The mirror case (tray on the LG, window on the built-in) divides by 2 and
///   lands the panel at half the intended offset from the LG's own origin —
///   visible, on the right display, in the wrong place; the debounced
///   correction then re-runs it with the window's now-correct scale factor and
///   it snaps across: *"она прыгает по экрану"*.
///
/// So the fix is not another retry or another delay: it is to stop speaking a
/// unit that does not exist. Everything below converts to points at the edges
/// (`DisplayPoints`, `resolve_tray_point`) and never leaves them —
/// `apply_docked_position` places the window with a `LogicalPosition`, which
/// `tao`'s `Position::to_logical` passes through untouched, so no scale factor
/// is ever consulted on the way out either.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct DisplayPoints {
    /// Top-left corner in the global point space.
    pub(crate) origin: (f64, f64),
    /// Size in points.
    pub(crate) size: (f64, f64),
    /// This display's own backing scale factor — needed only to *undo* the
    /// multiplication `tray-icon` applied to the tray rect.
    pub(crate) scale: f64,
    /// Global-point y of the **bottom edge of this display's own menu bar**,
    /// read live from `NSScreen.visibleFrame`; `None` on a display that has no
    /// menu bar, or when the screen list wasn't reachable (off the main
    /// thread, or off macOS).
    ///
    /// R3-8: the menu bar's height is not a constant and must never be written
    /// down as one. `compute_docked_layout` used to place the panel's top at a
    /// flat `32pt` from the display's top — the handoff's own number, which it
    /// describes as *"6px под меню-баром"*, i.e. a menu bar height plus a gap.
    /// Measured on this machine's two displays: the notched built-in's menu bar
    /// is **33pt** tall, so `32` put the panel's top edge *inside* the menu bar
    /// and AppKit clamped it back out to exactly 33 — a panel flush against the
    /// bar with no gap at all. An unnotched display's is ~24pt, where the same
    /// constant leaves an 8pt gap instead of 6. Same constant, two different
    /// wrong answers on one machine, which is exactly the display-dependent
    /// *"не появляется где должна"*. Derived per display now, so it is right on
    /// a notched screen, an unnotched one, a scaled one, and a machine that
    /// isn't this one.
    pub(crate) menu_bar_bottom: Option<f64>,
}

impl DisplayPoints {
    fn contains(&self, x: f64, y: f64) -> bool {
        x >= self.origin.0 && x < self.origin.0 + self.size.0 && y >= self.origin.1 && y < self.origin.1 + self.size.1
    }
}

/// Recovers the global-point position of a `tray-icon` rect, along with the
/// display it belongs to.
///
/// `tray-icon` multiplied the true point coordinate by the menu bar display's
/// scale factor and told us nothing about which display that was, so the
/// division cannot be done blind — this tries each display's own scale factor
/// and keeps the one whose quotient actually lands inside that display. On the
/// captain's two-display layout the answer is unambiguous in both directions
/// (a built-in tray at `2366` gives `1183` on the built-in ✓ and `2366` on the
/// LG ✗; an LG tray at `-400` gives `-400` on the LG ✓ and `-200` on the
/// built-in ✗). Where an unusual arrangement could make two candidates both
/// land in-bounds, the tie-break is the one property a menu bar always has:
/// it hugs its own display's top edge.
pub(crate) fn resolve_tray_point(displays: &[DisplayPoints], tray_x: f64, tray_y: f64) -> Option<(usize, f64, f64)> {
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

/// Every display, in global points. On macOS this reads `NSScreen` directly
/// rather than going through `tao`'s `Monitor` — one API, in one coordinate
/// space, and the only one that also reports `visibleFrame` (hence the menu
/// bar height, see `DisplayPoints::menu_bar_bottom`). It needs the main
/// thread; the `tao` path below is the fallback for anywhere else, and loses
/// only the menu bar height.
#[cfg(target_os = "macos")]
pub(crate) fn displays_in_points(window: &tauri::WebviewWindow) -> Vec<DisplayPoints> {
    use objc2_app_kit::NSScreen;
    use objc2_foundation::MainThreadMarker;

    let Some(mtm) = MainThreadMarker::new() else { return displays_in_points_via_tao(window) };
    let screens = NSScreen::screens(mtm);
    // AppKit's global space is y-up from the first screen's bottom-left; the
    // rest of this module is y-down from its top-left. That screen's own
    // height is the flip constant (its origin is (0,0) by definition).
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
            let menu_bar_height = (frame.origin.y + frame.size.height) - (visible.origin.y + visible.size.height);
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

/// The pure half of `compute_docked_layout` — no `tauri` handles, so the
/// handoff's own placement rules (B5/B7) are unit-testable against the
/// captain's real two-display geometry instead of only on a live screen.
pub(crate) fn docked_layout_in_points(
    display: DisplayPoints,
    tray_left: f64,
    tray_top: f64,
    tray_bottom: f64,
    item_width_points: Option<f64>,
    icon_width_px: f64,
) -> DockedLayout {
    const PANEL_WIDTH: f64 = 332.0;
    // B5: however far left the beak wants the panel, it can never hang off the
    // display's right edge (handoff: "clamped to screen right − 340").
    const RIGHT_CLAMP: f64 = 340.0;

    // R3-10 — three of the handoff's own numbers are **superseded by the
    // captain's own instruction** after he saw the result at real size:
    //
    // > Он во первых маленький, во вторых — слишком близко к левому краю,
    // > в третьих — слишком низко от топбара и иконки. Он должен быть
    // > "почти в иконке" — чуть чуть ниже топбара.
    //
    // So: the beak is bigger (`BEAK_BASE_WIDTH`/`BEAK_HEIGHT`, authored in
    // Panel.jsx and mirrored here), it is held a comfortable distance inside
    // the panel's own left edge instead of pressed against it
    // (`BEAK_INSET_IN_PANEL`, replacing the handoff's "клюв прижат к левому
    // краю" and its `icon_left − 6` docking rule), and the panel is pulled up
    // until the beak's *tip* — not the panel's top edge — sits just under the
    // menu bar (`BEAK_TIP_CLEARANCE`).
    //
    // The two constraints interact, which is why the x below is no longer
    // "icon left minus a constant": the beak's centre must stay exactly on the
    // glyph's centre (his earlier, equally explicit requirement), so the only
    // way to also move the beak away from the corner is to move the whole
    // panel further left. Deriving x *from* the beak's wanted position makes
    // both hold by construction, at whatever glyph offset the tray happens to
    // report, rather than by a second constant that would have to be retuned
    // every time the first one moves.
    const BEAK_BASE_WIDTH: f64 = 20.0; // == BEAK_BASE_HALF * 2 in Panel.jsx
    const BEAK_HEIGHT: f64 = 10.0; // == BEAK_HEIGHT in Panel.jsx
    const BEAK_INSET_IN_PANEL: f64 = 20.0; // notch's left edge, from the panel's own left edge
    const BEAK_TIP_CLEARANCE: f64 = 2.0; // how far the tip stops short of the menu bar

    // The panel is inset inside its own (deliberately wider, and taller at the
    // top) window: `(window width − panel width) / 2` of empty margin per side
    // for the drop shadow's blur to fade into rather than be clipped by, and
    // `padding-top` worth of headroom for the beak — both in app.css, both
    // load-bearing here because the *window* is what gets positioned while the
    // *panel* is what the captain looks at.
    const PANEL_INSET_X: f64 = (PANEL_WINDOW_WIDTH_LOGICAL - PANEL_WIDTH) / 2.0;
    const PANEL_INSET_TOP: f64 = 12.0; // == app.css's `body { padding-top }` / Panel.jsx's NOTCH_RESERVE

    let glyph_center = tray_left + glyph_center_offset_from_item_left_points(item_width_points, icon_width_px);

    // Place the window so the beak lands at its wanted inset with its centre on
    // the glyph's centre, then clamp to the display — the clamp is what B5's
    // right-screen-edge case exercises, and the beak offset recomputed below
    // from the *clamped* x is what keeps it tracking the glyph there.
    let wanted_panel_left = glyph_center - BEAK_INSET_IN_PANEL - BEAK_BASE_WIDTH / 2.0;
    let mut x = wanted_panel_left - PANEL_INSET_X;
    x = x.max(display.origin.0);
    let max_x = (display.origin.0 + display.size.0 - RIGHT_CLAMP).max(display.origin.0);
    x = x.min(max_x);

    // Both candidates for the menu bar's own bottom edge are read from the
    // running system, never assumed. `visibleFrame` is the direct answer where
    // it exists — but it does not exist in the state the captain actually
    // reproduces from: inside a full-screen Space the menu bar is auto-hidden,
    // `visibleFrame` equals `frame`, and there is no bar height to read even
    // while the bar sits revealed on screen under his cursor. The tray item is
    // still there and still measured, though, and macOS centres a status item
    // vertically in its bar — so the bar's bottom is the item's bottom plus the
    // same inset that sits above it. Derived either way, constant neither way.
    let inset_above_item = (tray_top - display.origin.1).max(0.0);
    // `NEG_INFINITY`, not 0 — a display left of the primary has negative
    // coordinates, where 0 is not a neutral floor but a point far below it.
    let menu_bar_bottom = display.menu_bar_bottom.unwrap_or(f64::NEG_INFINITY).max(tray_bottom + inset_above_item);
    // Solve for the window top from where the beak's *tip* should end up:
    // tip = y + PANEL_INSET_TOP − BEAK_HEIGHT, and tip should be
    // BEAK_TIP_CLEARANCE below the bar.
    let y = menu_bar_bottom + BEAK_TIP_CLEARANCE - (PANEL_INSET_TOP - BEAK_HEIGHT);

    // Recomputed from the applied x, not the wanted one, so a screen-edge clamp
    // moves the beak across the panel instead of dragging it off the glyph.
    // Clamped only to keep the notch on the panel at all; `buildPanelOutlinePath`
    // shrinks whichever top corner the notch encroaches on rather than the notch
    // giving way (doing it the other way round moved the beak visibly off the
    // glyph — caught live: "центровки снова нет").
    let panel_left = x + PANEL_INSET_X;
    let beak_left = (glyph_center - panel_left - BEAK_BASE_WIDTH / 2.0).clamp(0.0, PANEL_WIDTH - BEAK_BASE_WIDTH);

    DockedLayout { x, y, beak_left }
}

/// Pure delta math for `drag_window_step`, split out so it's testable without
/// a real window: the target keeps the same offset from the live cursor that
/// it had when the gesture began, so a drag never accumulates rounding drift
/// across many small steps the way repeatedly re-anchoring to the previous
/// step would.
pub(crate) fn drag_target_from_anchor(anchor_mouse: (f64, f64), anchor_window: (f64, f64), current_mouse: (f64, f64)) -> (f64, f64) {
    (anchor_window.0 + (current_mouse.0 - anchor_mouse.0), anchor_window.1 + (current_mouse.1 - anchor_mouse.1))
}

#[cfg(test)]
mod glyph_offset_tests {
    use super::glyph_center_offset_from_item_left_points;

    // R3-1 regression guard: these numbers are the real measurements taken
    // against a live tray item (see RESULT.md) — a bare-glyph item measured
    // 36pt wide with a bare 36px-wide (=18pt) composited image, and the
    // item's own visual center (glyph ink centroid, from a pixel-precise
    // screenshot) landed almost exactly on the item's geometric center, i.e.
    // 18pt in from its own left edge, not the old hardcoded 15pt.
    #[test]
    fn bare_glyph_offset_matches_the_measured_item_center() {
        // R3-11/v4: the composited image is the 18pt glyph plus padding per
        // side (see tray_render's SIDE_PAD_PX — 5pt as of v4, was 6pt),
        // inside an item NSStatusItem makes 8pt wider still on each side.
        // The glyph stays centred in that image, so its centre remains the
        // item's own centre.
        let offset = glyph_center_offset_from_item_left_points(Some(46.0), 60.0);
        assert!((offset - 22.0).abs() < 0.01, "expected ~22pt offset, got {offset}pt");
    }

    // A wider (pinned-digits) image still centers correctly as long as the
    // item's own measured width and the image's own known width are both
    // fresh — this is the whole point of deriving the margin instead of
    // reusing a single constant: it has to hold for any segment count, not
    // just the bare-glyph case above.
    #[test]
    fn pinned_digits_offset_scales_with_the_wider_image_not_a_fixed_constant() {
        // Real measurement: item widened from 36pt to 64pt after pinning one
        // segment, while its right edge (and therefore the implied margin)
        // stayed put — see `glyph_center_offset_from_item_left_points`.
        let offset = glyph_center_offset_from_item_left_points(Some(74.0), 58.0 * 2.0);
        // Same system margin as the bare case (by construction of these two
        // measurements — right edge held fixed), so the glyph's offset from
        // the item's own *current* left edge is unchanged even though the
        // item itself is much wider now.
        assert!((offset - 22.0).abs() < 0.01, "expected ~22pt offset, got {offset}pt");
    }

    // R3-7: the offset is a property of the tray item and its own composited
    // image, both of which are already in points — no display scale factor
    // may enter into it. Two displays of different scale must give the same
    // answer for the same item.
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

#[cfg(test)]
mod docked_layout_tests {
    use super::{docked_layout_in_points, resolve_tray_point, DisplayPoints};

    // The captain's own two-display arrangement, in the global point space
    // macOS actually uses (`NSScreen.frame` / `CGDisplayBounds`): the built-in
    // Retina MacBook display at (0,0) 1728x1117 scale 2, and the external LG
    // at (-2560,-908) 2560x2880 scale 1. Every number below is expressed the
    // way one of the three real APIs would hand it over — see `DisplayPoints`.
    // Menu bar heights are the real measured ones: 33pt on the notched
    // built-in (`NSScreen` frame 1117 vs visibleFrame 1084), and a standard
    // 24pt on an unnotched external — deliberately different, because a single
    // constant being wrong on one of them is the R3-8 bug.
    const BUILT_IN: DisplayPoints =
        DisplayPoints { origin: (0.0, 0.0), size: (1728.0, 1117.0), scale: 2.0, menu_bar_bottom: Some(33.0) };
    const EXTERNAL: DisplayPoints =
        DisplayPoints { origin: (-2560.0, -855.0), size: (2560.0, 2880.0), scale: 1.0, menu_bar_bottom: Some(-831.0) };

    // A tray item as macOS lays one out: a 24pt-tall button centred in
    // whatever menu bar it is in, so it can never extend below that bar.
    // Checked against the real thing on the notched built-in, whose AX rect
    // measured (1183, 4, 36, 24) against a 33pt bar — this gives 4.5.
    // A bare tray item as it really measures: the 18pt glyph padded to a 30pt
    // image (tray_render's SIDE_PAD_PX), inside an NSStatusItem 8pt wider on
    // each side again.
    const ITEM_W: f64 = 46.0;
    const ICON_PX: f64 = 60.0; // that 30pt image at the fixed 2x convention

    fn glyph_centre(tray_left: f64, item_w: f64, icon_px: f64) -> f64 {
        tray_left + super::glyph_center_offset_from_item_left_points(Some(item_w), icon_px)
    }

    fn item_top(display: DisplayPoints) -> f64 {
        let bar_height = display.menu_bar_bottom.map(|b| b - display.origin.1).unwrap_or(33.0);
        display.origin.1 + (bar_height - 24.0).max(0.0) / 2.0
    }

    fn item_bottom(display: DisplayPoints) -> f64 {
        item_top(display) + 24.0
    }

    fn displays() -> Vec<DisplayPoints> {
        vec![BUILT_IN, EXTERNAL]
    }

    // R3-7 regression guard, built-in half. `tray-icon` multiplies the tray
    // item's true point position (1183, 0) by the *menu bar display's* scale
    // factor, so the value arriving here is 2366 — which is not a coordinate
    // in any real space. Dividing by the built-in's own scale recovers 1183;
    // dividing by the external's leaves 2366, which is off every display.
    #[test]
    fn a_tray_rect_from_the_retina_built_in_resolves_to_that_display_in_points() {
        let (index, x, y) = resolve_tray_point(&displays(), 2366.0, 0.0).expect("built-in tray point must resolve");
        assert_eq!(index, 0);
        assert!((x - 1183.0).abs() < 0.01, "got {x}");
        assert!(y.abs() < 0.01, "got {y}");
    }

    // The mirror case: the menu bar living on the 1x external display, whose
    // point coordinates are negative. Here `tray-icon`'s multiplication is by
    // 1 and the raw value is already the answer — but only if the *external*
    // display's scale is the one used to undo it. Halving it (the built-in's
    // scale) lands somewhere else entirely, which is the second half of the
    // captain's "прыгает по экрану".
    #[test]
    fn a_tray_rect_from_the_1x_external_display_resolves_to_that_display_in_points() {
        let (index, x, y) = resolve_tray_point(&displays(), -400.0, -855.0).expect("external tray point must resolve");
        assert_eq!(index, 1);
        assert!((x + 400.0).abs() < 0.01, "got {x}");
        assert!((y + 855.0).abs() < 0.01, "got {y}");
    }

    #[test]
    fn a_tray_rect_matching_no_display_resolves_to_nothing_rather_than_a_guess() {
        assert!(resolve_tray_point(&displays(), 90_000.0, 90_000.0).is_none());
    }

    // R3-10: the beak's *tip* sits `BEAK_TIP_CLEARANCE` below the menu bar of
    // the display the icon is actually on — the captain's "почти в иконке".
    // Tip = y + (panel top inset 12 − beak height 10), so the window's own top
    // lands flush with the bar: 33 on the notched built-in, −825 on the
    // external (bar bottom −831, i.e. a standard 24pt bar), whose coordinates
    // are negative and whose bar is a different height again.
    #[test]
    fn the_beaks_tip_sits_just_under_the_menu_bar_of_its_own_display() {
        let tip = |l: super::DockedLayout| l.y + (12.0 - 10.0);

        let built_in = docked_layout_in_points(BUILT_IN, 1183.0, item_top(BUILT_IN), item_bottom(BUILT_IN), Some(ITEM_W), ICON_PX);
        assert!((tip(built_in) - (33.0 + 2.0)).abs() < 0.01, "got {}", tip(built_in));

        let external = docked_layout_in_points(EXTERNAL, -400.0, item_top(EXTERNAL), item_bottom(EXTERNAL), Some(ITEM_W), ICON_PX);
        assert!((tip(external) - (-831.0 + 2.0)).abs() < 0.01, "got {}", tip(external));
    }

    // R3-8 regression guard: two displays whose menu bars differ in height must
    // get *different* tops relative to their own origin. A constant would give
    // the same number for both, which is the bug.
    #[test]
    fn the_top_follows_each_displays_own_menu_bar_height_not_one_constant() {
        let built_in = docked_layout_in_points(BUILT_IN, 1183.0, item_top(BUILT_IN), item_bottom(BUILT_IN), Some(ITEM_W), ICON_PX);
        let external = docked_layout_in_points(EXTERNAL, -400.0, item_top(EXTERNAL), item_bottom(EXTERNAL), Some(ITEM_W), ICON_PX);
        let below_own_top = |l: super::DockedLayout, d: DisplayPoints| l.y - d.origin.1;
        assert!((below_own_top(built_in, BUILT_IN) - 33.0).abs() < 0.01, "got {}", below_own_top(built_in, BUILT_IN));
        assert!((below_own_top(external, EXTERNAL) - 24.0).abs() < 0.01, "got {}", below_own_top(external, EXTERNAL));
        assert_ne!(below_own_top(built_in, BUILT_IN), below_own_top(external, EXTERNAL));
    }

    // The state the captain actually reproduces from — inside a full-screen
    // Space, where the menu bar is auto-hidden and `visibleFrame` reports no
    // bar at all even while the bar is sitting revealed under his cursor. The
    // tray item is still measured, and reconstructing the bar from it
    // (a status item is centred in its bar) must land on the same answer as
    // reading the bar directly, not on some degraded approximation.
    #[test]
    fn a_hidden_menu_bar_is_reconstructed_from_the_tray_item_to_the_same_answer() {
        let hidden_bar = DisplayPoints { menu_bar_bottom: None, ..BUILT_IN };
        let with_bar = docked_layout_in_points(BUILT_IN, 1183.0, item_top(BUILT_IN), item_bottom(BUILT_IN), Some(ITEM_W), ICON_PX);
        let without = docked_layout_in_points(hidden_bar, 1183.0, item_top(hidden_bar), item_bottom(hidden_bar), Some(ITEM_W), ICON_PX);
        assert!((without.y - with_bar.y).abs() < 0.01, "{} vs {}", without.y, with_bar.y);
    }

    // A display left of the primary has negative coordinates throughout — a
    // "no menu bar reported" floor of 0 would read as far *below* such a
    // display's real bar rather than as neutral.
    #[test]
    fn the_fallback_floor_works_on_a_negative_coordinate_display() {
        let hidden_bar = DisplayPoints { menu_bar_bottom: None, ..EXTERNAL };
        let layout = docked_layout_in_points(hidden_bar, -400.0, item_top(hidden_bar), item_bottom(hidden_bar), Some(ITEM_W), ICON_PX);
        assert!(layout.y < 0.0 && layout.y > EXTERNAL.origin.1, "got {}", layout.y);
    }

    // R3-10: the beak is held a comfortable distance inside the panel's own
    // left edge rather than pressed against it — his "слишком близко к левому
    // краю" — which, since its centre must still be the glyph's centre, is
    // achieved by moving the whole panel left rather than by moving the beak.
    #[test]
    fn the_beak_sits_clear_of_the_panels_left_corner_by_moving_the_panel_not_the_beak() {
        let layout = docked_layout_in_points(BUILT_IN, 1183.0, item_top(BUILT_IN), item_bottom(BUILT_IN), Some(ITEM_W), ICON_PX);
        assert!((layout.beak_left - 20.0).abs() < 0.01, "got {}", layout.beak_left);
        // Well clear of the 12pt corner radius, unlike the handoff's own
        // "pressed to the left edge" rule this replaces.
        assert!(layout.beak_left > 12.0);
        // ...and still exactly on the glyph.
        let beak_centre = layout.x + 14.0 + layout.beak_left + 10.0;
        assert!((beak_centre - glyph_centre(1183.0, ITEM_W, ICON_PX)).abs() < 0.01, "got {beak_centre}");
    }

    // R3-1/the captain's own acceptance bar: pinning digits widens the tray
    // item, and macOS lays status items out right-to-left, so the item's own
    // left edge moves left — carrying the glyph with it, since `NSStatusItem`
    // centres the whole composited image and the glyph is that image's
    // leftmost 18pt (measured live: glyph centre 1238.75 bare → 1209.75
    // pinned, see RESULT.md). Nothing can hold the beak still *and* under the
    // glyph, because the glyph itself moved; what must hold is that the beak
    // lands on the glyph's real centre in **both** states, with the panel
    // tracking the item the same way in both. Getting that wrong is exactly
    // the drift the captain photographed across pin/unpin.
    #[test]
    fn the_beak_lands_on_the_glyph_centre_both_before_and_after_pinning() {
        // The absolute on-screen centre of the notch = window x + the 14pt
        // shadow-blur inset + beak_left + half the 12pt notch.
        let beak_centre = |l: super::DockedLayout| l.x + 14.0 + l.beak_left + 10.0;

        // Bare: 36pt item, 18pt image; right edge at 1183 + 36 = 1219.
        let bare = docked_layout_in_points(BUILT_IN, 1183.0, item_top(BUILT_IN), item_bottom(BUILT_IN), Some(ITEM_W), ICON_PX);
        let bare_glyph_centre: f64 = glyph_centre(1183.0, ITEM_W, ICON_PX);
        assert!((beak_centre(bare) - bare_glyph_centre).abs() < 0.01, "bare: {}", beak_centre(bare));
        assert!((bare.x - (bare_glyph_centre - 44.0)).abs() < 0.01, "bare panel: {}", bare.x);

        // Pinned "29%": the item grows to 64pt with the same right edge, so
        // its left edge moves to 1219 − 64 = 1155, and the composited image
        // is 46pt wide (glyph + one segment).
        let pinned = docked_layout_in_points(BUILT_IN, 1155.0, item_top(BUILT_IN), item_bottom(BUILT_IN), Some(74.0), 58.0 * 2.0);
        let pinned_glyph_centre = glyph_centre(1155.0, 74.0, 58.0 * 2.0);
        assert!((beak_centre(pinned) - pinned_glyph_centre).abs() < 0.01, "pinned: {}", beak_centre(pinned));
        assert!((pinned.x - (pinned_glyph_centre - 44.0)).abs() < 0.01, "pinned panel: {}", pinned.x);

        // Unpinning restores the bare geometry exactly — no hysteresis, since
        // every input is re-derived rather than accumulated.
        let unpinned = docked_layout_in_points(BUILT_IN, 1183.0, item_top(BUILT_IN), item_bottom(BUILT_IN), Some(ITEM_W), ICON_PX);
        assert_eq!(bare, unpinned);
    }

    // B5: at the right screen edge the panel stops (clamped to screen right −
    // 340) and the beak keeps tracking the glyph past the panel's own centre.
    #[test]
    fn at_the_right_screen_edge_the_panel_stops_and_the_beak_keeps_tracking() {
        // Icon hard against the built-in's right edge.
        let tray_left = 1728.0 - 40.0;
        let layout = docked_layout_in_points(BUILT_IN, tray_left, item_top(BUILT_IN), item_bottom(BUILT_IN), Some(ITEM_W), ICON_PX);
        assert!((layout.x - (1728.0 - 340.0)).abs() < 0.01, "panel must clamp, got {}", layout.x);
        let beak_centre = layout.x + 14.0 + layout.beak_left + 10.0;
        assert!((beak_centre - glyph_centre(tray_left, ITEM_W, ICON_PX)).abs() < 0.01, "beak must still track the glyph, got {beak_centre}");
    }

    // The notch can never leave the panel, however extreme the geometry.
    #[test]
    fn the_beak_stays_within_the_panel() {
        for tray_left in [-3000.0f64, -2560.0, 0.0, 900.0, 1727.0, 5000.0] {
            let layout = docked_layout_in_points(BUILT_IN, tray_left, item_top(BUILT_IN), item_bottom(BUILT_IN), Some(ITEM_W), ICON_PX);
            assert!(layout.beak_left >= 0.0 && layout.beak_left <= 332.0 - 20.0, "tray_left={tray_left} gave {}", layout.beak_left);
        }
    }
}

#[cfg(test)]
mod manual_drag_tests {
    use super::drag_target_from_anchor;

    // The window must follow the cursor 1:1: whatever offset it had from the
    // cursor when the gesture began (its own top-left minus the anchor
    // mouse point) has to be exactly preserved at every later cursor
    // position, on both axes and through negative deltas (dragging up/left,
    // e.g. toward the second display's negative coordinate space — see
    // `DisplayPoints`).
    #[test]
    fn the_target_preserves_the_grab_offset_across_a_move() {
        let anchor_mouse = (500.0, 200.0);
        let anchor_window = (420.0, 150.0); // grabbed 80pt right, 50pt down of the window's own top-left
        let target = drag_target_from_anchor(anchor_mouse, anchor_window, (650.0, 120.0));
        assert_eq!(target, (570.0, 70.0));
    }

    // A cursor position identical to the anchor must be a true no-op — this
    // is what keeps a stationary mousedown-then-immediate-move (a plain
    // click) from nudging the window at all.
    #[test]
    fn no_cursor_movement_yields_no_window_movement() {
        let anchor_mouse = (100.0, 100.0);
        let anchor_window = (10.0, 10.0);
        assert_eq!(drag_target_from_anchor(anchor_mouse, anchor_window, anchor_mouse), anchor_window);
    }
}
