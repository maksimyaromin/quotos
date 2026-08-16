//! R2-2: composites the tray glyph plus colored percentage digits into a raw
//! RGBA buffer.
//!
//! `tray-icon` v0.24.2's macOS `set_title` (`platform_impl/macos/mod.rs`)
//! calls `NSStatusItem`'s button `setTitle:` with a plain `NSString` — there
//! is no attributed-string / color path anywhere in the crate's public API
//! (verified by reading the crate source directly, the same way the
//! `set_title(None)` no-op documented in CLAUDE.md was found). So a colored
//! tray digit is not reachable through `set_title` at all; the only route
//! left is to paint the glyph and the digits ourselves and hand macOS a
//! finished bitmap via `set_icon`.
//!
//! That crate's `set_icon_for_ns_status_item_button` always asks for an
//! 18pt-tall `NSImage` regardless of the source bitmap's own pixel size
//! (`icon_height: f64 = 18.0`) — so supplying a denser buffer than 18x18 is
//! what makes the result crisp on a Retina menu bar; the width follows the
//! source aspect ratio automatically.
//!
//! Neutral-colored digits (nothing elevated, nothing stale) still go
//! through this path rather than the old plain-text `set_title`, because a
//! single tray title string can't mix colors across pinned subscriptions —
//! composing is the only way to give each pinned account's digits their own
//! color.
//!
//! R2-T1: the digits themselves are rendered with real system text (Core
//! Text) instead of a hand-rolled 3x5 bitmap font — see the `text` submodule
//! below. Font selection, glyph shaping and CoreGraphics drawing are macOS
//! platform APIs reached through the `objc2`/`objc2-core-text`/
//! `objc2-core-graphics`/`objc2-app-kit` crates; the rest of this module
//! (glyph compositing, color math, buffer layout) stays platform-agnostic.

use std::process::Command;

/// The crate always requests an 18pt button height; rendering at 2x that
/// keeps the composited bitmap crisp on Retina (matches the design system's
/// own "18x18 CSS-px, 36x36 @2x" spec for the glyph).
const GLYPH_PX: u32 = 36;
/// v4 docs/design/NOTES.md §1's measurement table (ink-to-ink, at 12px MonoLisa —
/// see `verification.md` for how this Rust renderer's own output was
/// measured against it). All of the constants below are CSS-px values
/// doubled for this buffer's fixed 2x-of-18pt convention (matches `GLYPH_PX`
/// itself). R3-11's rationale for *some* side air is unchanged (below); v4
/// changed the number from 6 to 5 CSS-px, matching the handoff's "5px of air
/// inside the pill at each end".
const SIDE_PAD_PX: u32 = 10; // 5 CSS-px air each end
/// R3-11: horizontal air on each side of the glyph+digits, inside the
/// composited image. Two things need it, and one of them is not optional:
///
/// * A11's "panel open" highlight is painted across this whole buffer, so
///   without padding it hugs the ink and reads as a box drawn round the glyph
///   rather than as a pressed menu bar button. macOS fills the status item's
///   own width for its own open items. Firstmate's on-screen pass: *"no
///   horizontal air"*.
/// * It is applied **unconditionally**, highlighted or not, so the glyph's
///   position inside the item cannot shift when the highlight toggles — a
///   shift there would move the beak, which is exactly the drift the captain
///   spent a whole round reporting.
///
/// `geometry.rs`'s `glyph_center_offset_from_item_left_points` reads
/// `GLYPH_LEFT_INSET_POINTS` rather than assuming the glyph is the image's
/// leftmost 18pt.
pub const GLYPH_LEFT_INSET_POINTS: f64 = SIDE_PAD_PX as f64 / 2.0;
/// Structural gap from the glyph's own bbox to the first figure cell's own
/// bbox — tuned against a real on-device capture (see `verification.md`) so
/// the *ink-to-ink* distance (the glyph's own ink margin plus this gap plus
/// the first digit's own centring margin) lands near the table's 6px; an
/// initial guess of 6 physical px measured 8 CSS-px ink-to-ink live, so this
/// is 2 less.
const GLYPH_TO_CELL_GAP_PX: u32 = 2;
/// **The cell reserve — never resize this per digit count.** Fixed 30 CSS-px
/// width per pinned figure, text centred inside it, regardless of whether
/// the figure reads one digit or three: this is what keeps the status item's
/// own width (and therefore the beak's position, and every other digit's
/// position) from moving on a value change (§5 "things not to undo";
/// `every_bare_glyph_path_produces_the_same_image_width` guards the
/// zero-segment case this reserve is the general form of). Consecutive
/// figures inside one group sit in adjacent cells with no additional gap —
/// the visible ~8px ink-to-ink gap between them falls out of each cell's own
/// centring margin, not a separate constant.
const CELL_WIDTH_PX: u32 = 60; // 30 CSS-px
/// Between two adjacent-cell groups: gutter, hairline, gutter — 5 + 1 + 5
/// CSS-px, matching the table's "19px across an 11px hairline" once each
/// side's own cell-centring margin is added on top of this 11px gutter.
const GROUP_GUTTER_PRE_PX: u32 = 10; // 5 CSS-px
const HAIRLINE_WIDTH_PX: u32 = 2; // 1 CSS-px thick
const GROUP_GUTTER_POST_PX: u32 = 10; // 5 CSS-px
/// The hairline's own drawn length — 11 CSS-px tall, centred in the row.
const HAIRLINE_HEIGHT_PX: u32 = 22;

/// The design system specifies 12 CSS-px digits; this buffer is rendered at
/// 2x for Retina throughout (see `GLYPH_PX`), so the actual CoreText point
/// size used in this bitmap's coordinate space is doubled to match.
#[cfg(target_os = "macos")]
const TEXT_FONT_SIZE_PT: f64 = 12.0 * 2.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TrayColor {
    Neutral,
    Amber,
    Red,
}

impl TrayColor {
    fn rgba(self, dark_mode: bool) -> (u8, u8, u8, u8) {
        match self {
            // --amber (docs/design/system/tokens/colors.css) — same in light and dark.
            TrayColor::Amber => (0xe0, 0xa9, 0x2b, 0xff),
            // --red — same in light and dark.
            TrayColor::Red => (0xe5, 0x64, 0x6a, 0xff),
            // Handoff spec: rgba(255,255,255,0.92) dark / rgba(0,0,0,0.85) light.
            TrayColor::Neutral if dark_mode => (0xff, 0xff, 0xff, 235),
            TrayColor::Neutral => (0x00, 0x00, 0x00, 217),
        }
    }
}

pub struct TraySegment {
    pub text: String,
    pub color: TrayColor,
    /// v4 docs/design/NOTES.md §1/§4: true for the first segment of a new
    /// subscription's group — `render` draws the hairline immediately before
    /// any segment with this set (never before the very first segment
    /// overall, even if the frontend happened to set it there).
    pub group_start: bool,
}

/// Per-pixel alpha coverage (0-255, row-major, `canvas_px` square) for the
/// capacity-gauge mark, drawn procedurally from its exact vector geometry
/// (`docs/design/system/assets/menubar-glyph.svg`: a faint full-circle track plus
/// a bold, round-capped arc with a gap at the bottom) rather than rasterized
/// from a fixed-size source and scaled — which is what firstmate's on-screen
/// pass (round 3, `data/quotos-tray-t1/firstmate-findings-1.md`) measured as
/// both undersized (11.5×10.5pt ink vs neighbouring menu bar icons'
/// 13.5-20pt — checklist A1/A2, and the handoff's own opening complaint,
/// *"Иконка в трее мельче соседних системных иконок"*) and visibly soft: the
/// old asset was a 22×22 PNG, nearest-neighbor-sampled up into the 36×36
/// composited buffer for the "digits pinned" case, and hand off to macOS's
/// own (unscaled, so even softer) `NSImage` scaling for the "nothing
/// pinned" bare-glyph case — two different blurry paths for what should be
/// the same mark. Drawing the exact shape at the target resolution fixes
/// both at once: there is no raster source to be too small or too soft, and
/// the target ink size (`TARGET_INK_DIAMETER_CSS_PX`) is a direct, tunable
/// parameter instead of whatever a fixed asset happened to contain.
///
/// The SVG's path (`M4.46 12.02 a5.4 5.4 0 1 1 7.08 0`) was converted to the
/// two gap-endpoint angles below by hand (see the commit that introduced
/// this: vector from centre (8,8) to each endpoint, `atan2`) — both in this
/// module's plain math (y-down, 0 = +x axis) convention, which already
/// matches the SVG's own y-down convention with no flip needed since this
/// buffer is top-left-origin throughout (`blend_pixel`, the text module, etc).
///
/// v4 docs/design/NOTES.md §3 corrected the mark: what shipped read as an O (a
/// ring with a gap, but no stroke through it) because a Q is "a counter with
/// a stroke through it, and the stroke was missing." Three changes from the
/// pre-v4 shape, cross-checked against `docs/design/assets/tray-glyph-*.svg`
/// (the two gap-endpoint angles reproduce that SVG's own arc-cap coordinates
/// to 2 decimal places, and the tail's start/end points are exact):
///  - The gap moved from 82.75° centred at 90° (bottom) to 62° centred at
///    45° (lower-right) — where the tail (below) now sits.
///  - A second capsule — the tail, `r` 3.2→8.0 along that same 45° diagonal,
///    same stroke weight as the arc, round caps — is drawn unconditionally
///    (even at 0% used: "Empty quota, empty letter", the tail is what reads
///    as a Q rather than an O regardless of fill state). Its own reach at
///    45° (outer radius + half-stroke, projected onto either axis) stays
///    just inside the ring's own axis-aligned reach, so it needed no change
///    to `natural_outer_diameter`/`scale` below.
///  - The arc's own end angle is now data (`used_fraction`, 0.0-1.0 —
///    clamped) instead of an implicit constant 100%: it starts right where
///    the gap ends (`ARC_GAP_HIGH_RAD`) and sweeps forward by
///    `used_fraction` of the maximum possible sweep (`TAU` minus the gap's
///    own width, "298/360" in the handoff's degrees) — landing exactly on
///    the gap's other edge at 100%, same as the old always-full arc did.
fn glyph_coverage(canvas_px: u32, used_fraction: f64) -> Vec<u8> {
    const TRACK_RADIUS_SVG: f64 = 5.4;
    const TRACK_STROKE_SVG: f64 = 1.4;
    const TRACK_OPACITY: f64 = 0.26;
    const ARC_RADIUS_SVG: f64 = 5.4;
    const ARC_STROKE_SVG: f64 = 1.9;
    // Gap: 62° centred on 45° (docs/design/NOTES.md §3) — 45° ± 31°.
    const GAP_CENTER_RAD: f64 = std::f64::consts::FRAC_PI_4;
    const GAP_HALF_WIDTH_RAD: f64 = 31.0 * std::f64::consts::PI / 180.0;
    const ARC_GAP_LOW_RAD: f64 = GAP_CENTER_RAD - GAP_HALF_WIDTH_RAD; // ~14°
    const ARC_GAP_HIGH_RAD: f64 = GAP_CENTER_RAD + GAP_HALF_WIDTH_RAD; // ~76°
                                                                       // The tail: same diagonal as the gap's own centre, r 3.2 → 8.0, same
                                                                       // stroke weight as the arc.
    const TAIL_ANGLE_RAD: f64 = GAP_CENTER_RAD;
    const TAIL_R_INNER_SVG: f64 = 3.2;
    const TAIL_R_OUTER_SVG: f64 = 8.0;
    const TAIL_STROKE_SVG: f64 = ARC_STROKE_SVG;
    // Middle of the 14-16pt ink band firstmate's neighbour comparison calls
    // for, measured as the bold arc's own outer edge (its dominant visible
    // silhouette) on the 18 CSS-px canvas.
    const TARGET_INK_DIAMETER_CSS_PX: f64 = 15.0;
    // Antialiasing transition half-width, in physical (canvas_px) pixels —
    // a soft coverage ramp across roughly 1.5 physical px either side of
    // each edge, rather than a hard-edged/jagged threshold.
    const AA_HALF_WIDTH_PX: f64 = 0.75;

    let natural_outer_diameter = (ARC_RADIUS_SVG + ARC_STROKE_SVG / 2.0) * 2.0;
    // GLYPH_PX (canvas_px) is always 2x an 18-CSS-px canvas.
    let px_per_css_px = canvas_px as f64 / 18.0;
    let scale = (TARGET_INK_DIAMETER_CSS_PX * px_per_css_px) / natural_outer_diameter;
    let center = canvas_px as f64 / 2.0;

    let gap_width = ARC_GAP_HIGH_RAD - ARC_GAP_LOW_RAD;
    let max_sweep = std::f64::consts::TAU - gap_width; // "full sweep 298/360"
    let sweep = used_fraction.clamp(0.0, 1.0) * max_sweep;
    let arc_start = ARC_GAP_HIGH_RAD;

    let cap_point = |a: f64| (ARC_RADIUS_SVG * a.cos(), ARC_RADIUS_SVG * a.sin());
    let (cap_start_x, cap_start_y) = cap_point(arc_start);
    let (cap_end_x, cap_end_y) = cap_point(arc_start + sweep);

    let (tail_x1, tail_y1) = (
        TAIL_R_INNER_SVG * TAIL_ANGLE_RAD.cos(),
        TAIL_R_INNER_SVG * TAIL_ANGLE_RAD.sin(),
    );
    let (tail_x2, tail_y2) = (
        TAIL_R_OUTER_SVG * TAIL_ANGLE_RAD.cos(),
        TAIL_R_OUTER_SVG * TAIL_ANGLE_RAD.sin(),
    );
    let (tail_dx, tail_dy) = (tail_x2 - tail_x1, tail_y2 - tail_y1);
    let tail_len_sq = tail_dx * tail_dx + tail_dy * tail_dy;

    let mut cov = vec![0u8; (canvas_px * canvas_px) as usize];
    for y in 0..canvas_px {
        for x in 0..canvas_px {
            // Pixel center, converted back into the SVG's own unit space.
            let ux = (x as f64 + 0.5 - center) / scale;
            let uy = (y as f64 + 0.5 - center) / scale;
            let dist = (ux * ux + uy * uy).sqrt();
            let mut angle = uy.atan2(ux);
            if angle < 0.0 {
                angle += std::f64::consts::TAU;
            }

            let track_edge = (dist - TRACK_RADIUS_SVG).abs() - TRACK_STROKE_SVG / 2.0;
            let track_cov =
                (1.0 - (track_edge * scale) / AA_HALF_WIDTH_PX).clamp(0.0, 1.0) * TRACK_OPACITY;

            // Where along the swept arc this pixel's angle falls, measured
            // forward from `arc_start` (0..TAU). The un-swept remainder of
            // the fixed 62° gap band falls out of this automatically: it's
            // exactly the tail end of this range, from `sweep` to `max_sweep`.
            let mut angle_rel = angle - arc_start;
            if angle_rel < 0.0 {
                angle_rel += std::f64::consts::TAU;
            }
            let arc_cov = if sweep <= 0.0 {
                0.0
            } else if angle_rel <= sweep {
                let edge = (dist - ARC_RADIUS_SVG).abs() - ARC_STROKE_SVG / 2.0;
                (1.0 - (edge * scale) / AA_HALF_WIDTH_PX).clamp(0.0, 1.0)
            } else {
                // Round caps at the two ends of the *current* sweep (not the
                // gap's own fixed edges, since the sweep is data now):
                // whichever endpoint is nearer, tested as a plain 2D
                // distance so the cap is a true half-circle.
                let d_start = ((ux - cap_start_x).powi(2) + (uy - cap_start_y).powi(2)).sqrt();
                let d_end = ((ux - cap_end_x).powi(2) + (uy - cap_end_y).powi(2)).sqrt();
                let d = d_start.min(d_end);
                (1.0 - ((d - ARC_STROKE_SVG / 2.0) * scale) / AA_HALF_WIDTH_PX).clamp(0.0, 1.0)
            };

            // The tail: a straight capsule (point-to-segment distance),
            // always drawn regardless of `used_fraction` — "empty quota,
            // empty letter".
            let t = (((ux - tail_x1) * tail_dx + (uy - tail_y1) * tail_dy) / tail_len_sq)
                .clamp(0.0, 1.0);
            let (cx, cy) = (tail_x1 + t * tail_dx, tail_y1 + t * tail_dy);
            let tail_dist = ((ux - cx).powi(2) + (uy - cy).powi(2)).sqrt();
            let tail_cov = (1.0 - ((tail_dist - TAIL_STROKE_SVG / 2.0) * scale) / AA_HALF_WIDTH_PX)
                .clamp(0.0, 1.0);

            let coverage = track_cov.max(arc_cov).max(tail_cov);
            if coverage <= 0.0 {
                continue;
            }
            cov[(y * canvas_px + x) as usize] = (coverage * 255.0).round().clamp(0.0, 255.0) as u8;
        }
    }
    cov
}

/// Read-only check of the current menu bar appearance. `defaults read` is a
/// read of user preferences, not a write — it changes nothing on the
/// machine. Absence of the key (the default light-mode case) makes the
/// command fail, which `unwrap_or(false)` correctly treats as "not dark".
fn is_dark_mode() -> bool {
    Command::new("defaults")
        .args(["read", "-g", "AppleInterfaceStyle"])
        .output()
        .map(|o| {
            o.status.success()
                && String::from_utf8_lossy(&o.stdout)
                    .trim()
                    .eq_ignore_ascii_case("dark")
        })
        .unwrap_or(false)
}

/// A11 needs two translucent layers in one buffer now (the "panel open"
/// highlight, then the glyph/digits drawn over it) — `put_pixel`'s plain
/// overwrite would just discard whichever layer drew second wherever they
/// overlap, losing the highlight everywhere the glyph or a digit covers it.
/// Standard "src-over" alpha compositing instead: on a fully-opaque `src` or
/// a fully-transparent destination pixel this reduces to exactly what
/// `put_pixel` already did, so every existing single-layer caller (glyph
/// ink onto a blank buffer, digit text onto a blank buffer) is unaffected —
/// only the new highlighted case actually exercises the blend math.
fn blend_pixel(buf: &mut [u8], w: u32, h: u32, x: u32, y: u32, rgba: (u8, u8, u8, u8)) {
    if x >= w || y >= h {
        return;
    }
    let idx = ((y * w + x) * 4) as usize;
    let src_a = rgba.3 as u32;
    if src_a == 0 {
        return;
    }
    if src_a == 255 {
        buf[idx] = rgba.0;
        buf[idx + 1] = rgba.1;
        buf[idx + 2] = rgba.2;
        buf[idx + 3] = 255;
        return;
    }
    let dst_a = buf[idx + 3] as u32;
    let out_a = src_a + dst_a * (255 - src_a) / 255;
    if out_a == 0 {
        buf[idx] = 0;
        buf[idx + 1] = 0;
        buf[idx + 2] = 0;
        buf[idx + 3] = 0;
        return;
    }
    let blend_channel = |src_c: u8, dst_c: u8| -> u8 {
        (((src_c as u32 * src_a) + (dst_c as u32 * dst_a * (255 - src_a) / 255)) / out_a).min(255)
            as u8
    };
    buf[idx] = blend_channel(rgba.0, buf[idx]);
    buf[idx + 1] = blend_channel(rgba.1, buf[idx + 1]);
    buf[idx + 2] = blend_channel(rgba.2, buf[idx + 2]);
    buf[idx + 3] = out_a as u8;
}

/// A11: "У иконки нет состояния «панель открыта»" — the handoff's own fix is
/// a system-style highlight behind the whole glyph+digits image while the
/// panel is open: rgba(255,255,255,0.20) dark / rgba(0,0,0,0.14) light,
/// radius 5 CSS-px. Drawn ourselves (a standard rounded-box signed-distance
/// field, same AA approach as `glyph_coverage`) rather than reached for via
/// any native `NSStatusItem` highlighted state, because the icon is already
/// a custom composited bitmap and the design calls for these exact tokens,
/// not whatever tint macOS's own default selection style would draw.
fn draw_highlight_background(buf: &mut [u8], w: u32, h: u32, dark: bool) {
    const RADIUS_PHYSICAL: f64 = 10.0; // 5 CSS-px * 2 (this buffer's usual 2x)
    const AA_HALF_WIDTH_PX: f64 = 0.75;
    let rgba = if dark {
        (0xffu8, 0xffu8, 0xffu8, (0.20f64 * 255.0).round() as u8)
    } else {
        (0x00u8, 0x00u8, 0x00u8, (0.14f64 * 255.0).round() as u8)
    };
    let (bx, by) = (w as f64 / 2.0, h as f64 / 2.0);
    for y in 0..h {
        for x in 0..w {
            let px = x as f64 + 0.5 - bx;
            let py = y as f64 + 0.5 - by;
            let qx = px.abs() - (bx - RADIUS_PHYSICAL);
            let qy = py.abs() - (by - RADIUS_PHYSICAL);
            let outside_len = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt();
            let signed_dist = outside_len + qx.max(qy).min(0.0) - RADIUS_PHYSICAL;
            let coverage = (0.5 - signed_dist / AA_HALF_WIDTH_PX).clamp(0.0, 1.0);
            if coverage <= 0.0 {
                continue;
            }
            let alpha = ((rgba.3 as f64) * coverage).round().clamp(0.0, 255.0) as u8;
            blend_pixel(buf, w, h, x, y, (rgba.0, rgba.1, rgba.2, alpha));
        }
    }
}

/// v4 docs/design/NOTES.md §1: the vertical rule that parts two pinned
/// subscriptions' figure groups — "11px hairline at white 34%", drawn at
/// `HAIRLINE_WIDTH_PX` wide and `HAIRLINE_HEIGHT_PX` tall, centred in the
/// row. Only the dark-appearance value is in the handoff; the light value
/// mirrors `draw_highlight_background`'s own light-below-dark pattern (a
/// black line reads at a slightly lower opacity than an equally-weighted
/// white one) rather than inventing an unrelated number.
fn draw_hairline(buf: &mut [u8], w: u32, h: u32, x0: u32, dark: bool) {
    let rgba = if dark {
        (0xffu8, 0xffu8, 0xffu8, (0.34f64 * 255.0).round() as u8)
    } else {
        (0x00u8, 0x00u8, 0x00u8, (0.28f64 * 255.0).round() as u8)
    };
    let top = h.saturating_sub(HAIRLINE_HEIGHT_PX) / 2;
    for y in top..(top + HAIRLINE_HEIGHT_PX).min(h) {
        for x in x0..(x0 + HAIRLINE_WIDTH_PX).min(w) {
            blend_pixel(buf, w, h, x, y, rgba);
        }
    }
}

/// Composites an 8-bit coverage mask (`mask`, `mask_w` wide x `buf_h` tall,
/// row-major, one byte per pixel) onto `buf` at horizontal offset `x0`,
/// using `rgba` as the solid color. Coverage modulates `rgba`'s own alpha
/// (so `TrayColor::Neutral`'s sub-255 base alpha is preserved), and the RGB
/// channels are always exactly `rgba`'s — the mask carries no color
/// information of its own. Used by the non-macOS fallback text renderer
/// below; the real (macOS) CoreText renderer draws already-colored RGBA
/// straight from CoreGraphics instead (see its `draw_text_impl`'s doc
/// comment for why a coverage-mask-only approach doesn't work for text).
#[cfg(not(target_os = "macos"))]
fn composite_mask(
    buf: &mut [u8],
    buf_w: u32,
    buf_h: u32,
    x0: u32,
    mask: &[u8],
    mask_w: u32,
    rgba: (u8, u8, u8, u8),
) {
    if mask_w == 0 {
        return;
    }
    for y in 0..buf_h {
        for x in 0..mask_w {
            let coverage = mask[(y * mask_w + x) as usize];
            if coverage == 0 {
                continue;
            }
            let alpha = ((coverage as u16 * rgba.3 as u16) / 255) as u8;
            blend_pixel(
                buf,
                buf_w,
                buf_h,
                x0 + x,
                y,
                (rgba.0, rgba.1, rgba.2, alpha),
            );
        }
    }
}

/// Real system text rendering for the tray's percentage digits, via Core
/// Text / CoreGraphics (through the `objc2` bindings tauri already pulls in
/// transitively). Replaces the old hand-rolled 3x5 bitmap font, which read
/// as blocky next to Apple's own menu-bar text (and whose `%` glyph looked
/// like "a mangled colon").
#[cfg(target_os = "macos")]
mod text {
    use super::blend_pixel;
    use objc2::rc::Retained;
    use objc2_app_kit::{NSFont, NSFontWeightMedium};
    use objc2_core_foundation::{
        CFArray, CFAttributedString, CFDictionary, CFNumber, CFRetained, CFString, CFType,
    };
    use objc2_core_graphics::{
        kCGColorSpaceSRGB, CGBitmapContextCreateWithData, CGColor, CGColorSpace, CGContext,
        CGImageAlphaInfo,
    };
    use objc2_core_text::{
        kCTFontAttributeName, kCTFontFamilyNameAttribute, kCTFontTraitsAttribute,
        kCTFontWeightTrait, kCTForegroundColorAttributeName, CTFont, CTFontDescriptor,
        CTFontManagerCopyAvailableFontFamilyNames, CTLine,
    };
    use std::ffi::c_void;

    pub const FONT_FAMILY: &str = "MonoLisa";

    /// Either the requested MonoLisa font, or (if it isn't installed on this
    /// machine) the system's own tabular-figure UI font. `NSFont` and
    /// `CTFont` are toll-free bridged (same underlying object), so both
    /// variants expose an identical `&CTFont` for drawing.
    enum FontHandle {
        Mono(CFRetained<CTFont>),
        Fallback(Retained<NSFont>),
    }

    impl FontHandle {
        fn as_ct_font(&self) -> &CTFont {
            match self {
                FontHandle::Mono(f) => f,
                FontHandle::Fallback(f) => f.as_ref(),
            }
        }
    }

    pub struct LoadedFont {
        handle: FontHandle,
        pub used_fallback: bool,
    }

    /// `NSFontWeight` and `CTFontWeightTrait` share the same documented
    /// -1.0..1.0 normalized scale, so Apple's own medium-weight constant is
    /// reused for both the CoreText descriptor trait (MonoLisa path) and the
    /// AppKit fallback call, rather than hand-picking a magic number for one
    /// of the two.
    fn medium_weight() -> f64 {
        unsafe { NSFontWeightMedium }
    }

    /// Pure CoreText, thread-safe query — unlike `NSFontManager` (whose
    /// `sharedFontManager` requires a `MainThreadMarker` in these bindings,
    /// i.e. is genuinely main-thread-restricted), `CTFontManagerCopy*` is a
    /// plain C function with no such requirement, which matters here because
    /// `set_tray_status` runs inside a Tauri command handler with no
    /// guarantee of being on the main thread.
    fn monolisa_family_available() -> bool {
        let names: CFRetained<CFArray<CFString>> =
            unsafe { CFRetained::cast_unchecked(CTFontManagerCopyAvailableFontFamilyNames()) };
        names.to_vec().iter().any(|n| n.to_string() == FONT_FAMILY)
    }

    /// Builds a font descriptor for family "MonoLisa" at the medium weight
    /// trait and resolves it to a concrete font. `CTFontCreateWithFontDescriptor`
    /// (like `CTFontCreateWithName`) never returns null — on a mismatch it
    /// silently substitutes a default font instead — so availability is
    /// checked up front via `CTFontManagerCopyAvailableFontFamilyNames`, and
    /// double-checked after creation by comparing the resolved font's own
    /// family name, in case of a fluke substitution.
    fn try_load_monolisa(size_pt: f64) -> Option<CFRetained<CTFont>> {
        if !monolisa_family_available() {
            return None;
        }

        let family = CFString::from_str(FONT_FAMILY);
        let weight = CFNumber::new_f64(medium_weight());
        let traits: CFRetained<CFDictionary<CFString, CFType>> =
            CFDictionary::from_slices(&[unsafe { kCTFontWeightTrait }], &[weight.as_ref()]);
        let attrs: CFRetained<CFDictionary<CFString, CFType>> = CFDictionary::from_slices(
            &[unsafe { kCTFontFamilyNameAttribute }, unsafe {
                kCTFontTraitsAttribute
            }],
            &[family.as_ref(), traits.as_ref()],
        );
        let descriptor = unsafe { CTFontDescriptor::with_attributes(attrs.as_ref()) };
        let font = unsafe { CTFont::with_font_descriptor(&descriptor, size_pt, std::ptr::null()) };

        let actual_family = unsafe { font.family_name() };
        if actual_family.to_string() == FONT_FAMILY {
            Some(font)
        } else {
            None
        }
    }

    /// Tries MonoLisa first; falls back to the system's tabular-figure UI
    /// font (`NSFont.monospacedDigitSystemFontOfSize:weight:`) if MonoLisa
    /// isn't installed. No caching across calls — font matching here is
    /// infrequent (once per tray repaint, at most once a minute) and cheap
    /// enough that keeping this stateless sidesteps `CFRetained`/`Retained`
    /// not being `Send + Sync` (Core Foundation/AppKit object wrappers
    /// aren't declared thread-safe for storage in a shared `static`, even
    /// though the lookups themselves are safe to call off the main thread).
    pub fn load_font(size_pt: f64) -> LoadedFont {
        if let Some(font) = try_load_monolisa(size_pt) {
            return LoadedFont {
                handle: FontHandle::Mono(font),
                used_fallback: false,
            };
        }
        let font = NSFont::monospacedDigitSystemFontOfSize_weight(size_pt, medium_weight());
        LoadedFont {
            handle: FontHandle::Fallback(font),
            used_fallback: true,
        }
    }

    /// Builds a `CTLine` laying out `text` with `font` in color `rgba`.
    /// `CTLineCreateWithAttributedString` + `CTLineDraw` is CoreText's own
    /// standard, documented path for drawing a short text run — used here
    /// instead of manually resolving glyph IDs and advances via
    /// `CTFontGetGlyphsForCharacters`/`CTFontGetAdvancesForGlyphs`/
    /// `CTFontDrawGlyphs`, which was tried first and, verified visually
    /// (rendered a labeled PNG dump of `render()`'s actual output at large
    /// point sizes, since neither the tray nor the panel can be photographed
    /// live in this environment — see RESULT.md), produced specific glyphs
    /// with wrong/incomplete outlines (e.g. a "7" missing its top bar, while
    /// "8" right next to it, same font, same call sequence, rendered
    /// perfectly) — a real defect in that lower-level path on this
    /// font/OS/binding combination, not a premultiply or coordinate-flip
    /// issue (both were independently ruled out: the corruption was
    /// identical with the system fallback font, not just MonoLisa, and
    /// identical with a straight RGBA context, not just the alpha-only one
    /// tried before that). `CTLine` goes through CoreText's normal text
    /// layout/shaping engine instead of raw per-glyph plotting, which is
    /// both simpler and — per this same visual verification — correct.
    fn make_line(font: &CTFont, text: &str, rgba: (u8, u8, u8, u8)) -> Option<CFRetained<CTLine>> {
        if text.is_empty() {
            return None;
        }
        let string = CFString::from_str(text);
        // sRGB, not `new_generic_rgb`: verified visually (see RESULT.md) that
        // pairing a Generic-RGB fill color with a Device-RGB bitmap context
        // (the two color spaces don't share a gamma curve) shifted even
        // fully-opaque glyph-interior pixels well off the requested color —
        // e.g. requested (229,100,106) came back (236,123,125), a 20+ point
        // shift on G/B, not just antialiasing fuzz. `TrayColor::rgba`'s
        // values are plain CSS hex tokens, i.e. already sRGB by convention,
        // so both the fill color and the bitmap context below use sRGB
        // explicitly and agree with each other.
        let color = CGColor::new_srgb(
            rgba.0 as f64 / 255.0,
            rgba.1 as f64 / 255.0,
            rgba.2 as f64 / 255.0,
            rgba.3 as f64 / 255.0,
        );
        let attrs: CFRetained<CFDictionary<CFString, CFType>> = CFDictionary::from_slices(
            &[unsafe { kCTFontAttributeName }, unsafe {
                kCTForegroundColorAttributeName
            }],
            &[font.as_ref(), color.as_ref()],
        );
        let attr_string =
            unsafe { CFAttributedString::new(None, Some(&string), Some(attrs.as_opaque())) }?;
        Some(unsafe { CTLine::with_attributed_string(&attr_string) })
    }

    /// Renders `text` with `font` in color `rgba` and composites it onto
    /// `buf` at horizontal offset `x0` (buffer height `buf_h`), returning
    /// the pixel width it occupied so callers can lay out the next segment
    /// after it.
    fn draw_text_impl(
        buf: &mut [u8],
        buf_w: u32,
        buf_h: u32,
        x0: u32,
        font: &CTFont,
        text: &str,
        rgba: (u8, u8, u8, u8),
    ) -> u32 {
        let Some(line) = make_line(font, text, rgba) else {
            return 0;
        };
        let line_width = unsafe {
            line.typographic_bounds(
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        let width = line_width.ceil().max(0.0) as u32;
        if width == 0 {
            return 0;
        }

        let mut pixels = vec![0u8; (width * buf_h * 4) as usize];
        let Some(colorspace) = CGColorSpace::with_name(Some(unsafe { kCGColorSpaceSRGB })) else {
            return 0;
        };
        let ctx = unsafe {
            CGBitmapContextCreateWithData(
                pixels.as_mut_ptr() as *mut c_void,
                width as usize,
                buf_h as usize,
                8,
                (width * 4) as usize,
                Some(&colorspace),
                CGImageAlphaInfo::PremultipliedLast.0,
                None,
                std::ptr::null_mut(),
            )
        };
        let Some(ctx) = ctx else {
            return 0;
        };

        // Deliberately NOT flipping the CTM here (translate+scale(1,-1), the
        // usual trick to turn a CGBitmapContext's bottom-left/y-up default
        // into top-left/y-down): verified visually (see RESULT.md) that
        // doing so mirrors the glyphs themselves vertically, since
        // `CTLineDraw`/`CTFontDrawGlyphs` orient glyph outlines relative to
        // the CTM's handedness rather than compensating for it — the usual
        // fix would be to also set a flipped `CGContextSetTextMatrix`, but
        // it's simpler to just draw in the context's native (bottom-left/
        // y-up) convention and account for that in `baseline_native` below.
        // `CGBitmapContextCreateWithData`'s backing memory is always laid
        // out top-row-first regardless of the drawing CTM (that's a fixed
        // property of the pixel buffer, not of how you draw into it), so
        // the row-major copy loop further down needs no inversion either
        // way — only the baseline math needs to account for native y-up.
        let ascent = unsafe { font.ascent() };
        let descent = unsafe { font.descent() };
        let baseline_native = ((buf_h as f64) + descent - ascent) / 2.0;
        CGContext::set_text_position(Some(&ctx), 0.0, baseline_native);
        unsafe { line.draw(&ctx) };

        for y in 0..buf_h {
            for x in 0..width {
                let idx = ((y * width + x) * 4) as usize;
                let a = pixels[idx + 3];
                if a == 0 {
                    continue;
                }
                // Unpremultiply: premultiplied RGB = straight RGB * a/255.
                let unpremul =
                    |c: u8| ((c as u32 * 255 + (a as u32 / 2)) / a as u32).min(255) as u8;
                blend_pixel(
                    buf,
                    buf_w,
                    buf_h,
                    x0 + x,
                    y,
                    (
                        unpremul(pixels[idx]),
                        unpremul(pixels[idx + 1]),
                        unpremul(pixels[idx + 2]),
                        a,
                    ),
                );
            }
        }

        width
    }

    /// Renders `text` and composites it onto `buf` at horizontal offset
    /// `x0`, returning the pixel width it occupied (so callers can lay out
    /// the next segment after it).
    pub fn draw_text(
        buf: &mut [u8],
        buf_w: u32,
        buf_h: u32,
        x0: u32,
        font: &LoadedFont,
        text: &str,
        rgba: (u8, u8, u8, u8),
    ) -> u32 {
        draw_text_impl(buf, buf_w, buf_h, x0, font.handle.as_ct_font(), text, rgba)
    }

    /// Measures `text` without drawing it (used by `render()` to lay out
    /// segments before the final buffer is allocated).
    pub fn measure(font: &LoadedFont, text: &str) -> u32 {
        let Some(line) = make_line(font.handle.as_ct_font(), text, (0, 0, 0, 0)) else {
            return 0;
        };
        let width = unsafe {
            line.typographic_bounds(
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        width.ceil().max(0.0) as u32
    }
}

/// Non-macOS fallback: this crate only ever ships for macOS (menu bar app),
/// but the CoreText bindings above are gated `target_os = "macos"` so that a
/// non-mac `cargo check` (e.g. a contributor on Linux) still compiles. Kept
/// as the old hand-rolled bitmap font, restructured to produce a coverage
/// mask so it shares `composite_mask` with the real implementation above.
#[cfg(not(target_os = "macos"))]
mod text {
    use super::composite_mask;

    const CHAR_W: u32 = 3;
    const CHAR_H: u32 = 5;
    const TEXT_SCALE: u32 = 4;

    pub struct LoadedFont;

    pub fn load_font(_size_pt: f64) -> LoadedFont {
        LoadedFont
    }

    pub fn used_fallback() -> bool {
        true
    }

    /// 3x5 pixel font, bits packed MSB-left per row. Covers exactly what
    /// tray digits ever need: 0-9, '%', '!' (the "broken" mark), and space.
    fn char_glyph(c: char) -> [u8; 5] {
        match c {
            '0' => [0b111, 0b101, 0b101, 0b101, 0b111],
            '1' => [0b010, 0b110, 0b010, 0b010, 0b111],
            '2' => [0b111, 0b001, 0b111, 0b100, 0b111],
            '3' => [0b111, 0b001, 0b111, 0b001, 0b111],
            '4' => [0b101, 0b101, 0b111, 0b001, 0b001],
            '5' => [0b111, 0b100, 0b111, 0b001, 0b111],
            '6' => [0b111, 0b100, 0b111, 0b101, 0b111],
            '7' => [0b111, 0b001, 0b010, 0b010, 0b010],
            '8' => [0b111, 0b101, 0b111, 0b101, 0b111],
            '9' => [0b111, 0b101, 0b111, 0b001, 0b111],
            '%' => [0b101, 0b001, 0b010, 0b100, 0b101],
            '!' => [0b010, 0b010, 0b010, 0b000, 0b010],
            _ => [0, 0, 0, 0, 0], // space, and anything else: blank
        }
    }

    fn text_width(text: &str) -> u32 {
        let n = text.chars().count() as u32;
        if n == 0 {
            return 0;
        }
        n * CHAR_W * TEXT_SCALE + (n - 1) * TEXT_SCALE
    }

    fn render_mask(text: &str, buf_h: u32) -> (Vec<u8>, u32) {
        let width = text_width(text);
        if width == 0 {
            return (Vec::new(), 0);
        }
        let mut mask = vec![0u8; (width * buf_h) as usize];
        let y0 = (buf_h.saturating_sub(CHAR_H * TEXT_SCALE)) / 2;
        let mut x = 0u32;
        for (i, c) in text.chars().enumerate() {
            if i > 0 {
                x += TEXT_SCALE;
            }
            let rows = char_glyph(c);
            for (row, bits) in rows.iter().enumerate() {
                for col in 0..CHAR_W {
                    if (bits >> (CHAR_W - 1 - col)) & 1 == 0 {
                        continue;
                    }
                    for sy in 0..TEXT_SCALE {
                        for sx in 0..TEXT_SCALE {
                            let px = x + col * TEXT_SCALE + sx;
                            let py = y0 + row as u32 * TEXT_SCALE + sy;
                            if px < width && py < buf_h {
                                mask[(py * width + px) as usize] = 0xff;
                            }
                        }
                    }
                }
            }
            x += CHAR_W * TEXT_SCALE;
        }
        (mask, width)
    }

    pub fn draw_text(
        buf: &mut [u8],
        buf_w: u32,
        buf_h: u32,
        x0: u32,
        _font: &LoadedFont,
        text: &str,
        rgba: (u8, u8, u8, u8),
    ) -> u32 {
        let (mask, mask_w) = render_mask(text, buf_h);
        composite_mask(buf, buf_w, buf_h, x0, &mask, mask_w, rgba);
        mask_w
    }

    pub fn measure(_font: &LoadedFont, text: &str) -> u32 {
        text_width(text)
    }
}

#[cfg(target_os = "macos")]
fn text_font_size_pt() -> f64 {
    TEXT_FONT_SIZE_PT
}
#[cfg(not(target_os = "macos"))]
fn text_font_size_pt() -> f64 {
    0.0
}

/// Whether the last font lookup fell back to the system tabular-figure UI
/// font instead of finding MonoLisa. Exposed so the caller (and RESULT.md)
/// can report which font actually rendered on a given machine — see the
/// `text` submodule's `load_font` for the lookup-and-fallback logic itself.
pub fn used_fallback_font() -> bool {
    #[cfg(target_os = "macos")]
    {
        text::load_font(text_font_size_pt()).used_fallback
    }
    #[cfg(not(target_os = "macos"))]
    {
        text::used_fallback()
    }
}

/// Composites the glyph plus every segment's colored digits into one RGBA
/// buffer. Returns `(rgba, width, height)`. `segments` empty still draws the
/// bare glyph (callers that truly have nothing pinned should prefer the
/// cheaper template-icon path in `shell.rs` instead of calling this).
/// `highlighted` is A11's "panel open" state — draws the design's own
/// translucent rounded-rect behind everything else when true (see
/// `draw_highlight_background`); `segments` empty here still draws the bare
/// glyph as a non-template colored image whenever `highlighted` is true
/// (a plain template image can't carry a background tint of its own — see
/// `shell.rs`'s `repaint_tray_icon` for why that case routes here rather than
/// through `plain_glyph_rgba`'s template path at all). `worst_used_percent`
/// (0-100) is the glyph's own arc fill — v4 docs/design/NOTES.md §1: "filled to
/// your worst active limit across everything tracked" — drawn regardless of
/// whether `segments` is empty; the glyph's own colour never changes with it
/// (`glyph_coverage`'s ink is composited through `TrayColor::Neutral` alone,
/// same as before).
///
/// v4 §1/§4: each segment gets a fixed `CELL_WIDTH_PX` reserve (never the
/// raw text width — `text::measure` is only used to *centre* text inside
/// that reserve, not to size the layout), and `TraySegment::group_start`
/// segments get a hairline gutter drawn immediately before them.
pub fn render(
    segments: &[TraySegment],
    highlighted: bool,
    worst_used_percent: u8,
) -> (Vec<u8>, u32, u32) {
    let dark = is_dark_mode();
    let used_fraction = worst_used_percent as f64 / 100.0;
    let coverage = glyph_coverage(GLYPH_PX, used_fraction);
    let font = text::load_font(text_font_size_pt());

    let widths: Vec<u32> = segments
        .iter()
        .map(|s| text::measure(&font, &s.text))
        .collect();
    // A `group_start` flag on the very first segment means nothing — there is
    // no prior group to part from — so it's excluded here the same way the
    // draw loop below excludes it.
    let boundaries = segments.iter().skip(1).filter(|s| s.group_start).count() as u32;
    let gutter_px = GROUP_GUTTER_PRE_PX + HAIRLINE_WIDTH_PX + GROUP_GUTTER_POST_PX;
    let text_area = if segments.is_empty() {
        0
    } else {
        GLYPH_TO_CELL_GAP_PX + CELL_WIDTH_PX * segments.len() as u32 + boundaries * gutter_px
    };
    let total_w = SIDE_PAD_PX * 2 + GLYPH_PX + text_area;
    let total_h = GLYPH_PX;
    let mut buf = vec![0u8; (total_w * total_h * 4) as usize];

    if highlighted {
        draw_highlight_background(&mut buf, total_w, total_h, dark);
    }

    // The glyph itself always stays neutral — only the digits carry
    // severity/staleness color.
    let ink = TrayColor::Neutral.rgba(dark);
    for y in 0..GLYPH_PX {
        for x in 0..GLYPH_PX {
            let a = coverage[(y * GLYPH_PX + x) as usize];
            if a == 0 {
                continue;
            }
            let blended = ((ink.3 as u16 * a as u16) / 255) as u8;
            blend_pixel(
                &mut buf,
                total_w,
                total_h,
                SIDE_PAD_PX + x,
                y,
                (ink.0, ink.1, ink.2, blended),
            );
        }
    }

    let mut x = SIDE_PAD_PX + GLYPH_PX + GLYPH_TO_CELL_GAP_PX;
    for (i, (seg, w)) in segments.iter().zip(widths.iter()).enumerate() {
        if seg.group_start && i > 0 {
            x += GROUP_GUTTER_PRE_PX;
            draw_hairline(&mut buf, total_w, total_h, x, dark);
            x += HAIRLINE_WIDTH_PX + GROUP_GUTTER_POST_PX;
        }
        let text_x = x + CELL_WIDTH_PX.saturating_sub(*w) / 2;
        text::draw_text(
            &mut buf,
            total_w,
            total_h,
            text_x,
            &font,
            &seg.text,
            seg.color.rgba(dark),
        );
        x += CELL_WIDTH_PX;
    }

    (buf, total_w, total_h)
}

/// The bare glyph alone (no digits), for reverting to the quiet state — the
/// most common state per the design philosophy ("Спокойный случай молчит"),
/// so this path matters at least as much as `render()`'s embedded glyph.
/// Full `GLYPH_PX` resolution (previously this hand off the original
/// 22×22-source PNG straight to macOS's own `NSImage` scaling — a second,
/// separate, equally-blurry path from `render()`'s own former upscaling;
/// see `glyph_coverage`'s doc comment). White RGB + alpha, matching a
/// template image's convention (macOS tints template images itself from
/// alpha alone, per-appearance). `worst_used_percent`: see `render`'s doc
/// comment — the empty state is still the glyph alone, its arc filled to
/// this value (v4 docs/design/NOTES.md §1's empty-state rule).
pub fn plain_glyph_rgba(worst_used_percent: u8) -> (Vec<u8>, u32, u32) {
    let used_fraction = worst_used_percent as f64 / 100.0;
    let coverage = glyph_coverage(GLYPH_PX, used_fraction);
    // Padded identically to `render`'s output (see `SIDE_PAD_PX`): the two
    // paths swap places whenever the panel opens or a pin changes, and an
    // image width that changed between them would move the glyph — and with
    // it the beak — on every toggle.
    let total_w = SIDE_PAD_PX * 2 + GLYPH_PX;
    let mut rgba = vec![0u8; (total_w * GLYPH_PX * 4) as usize];
    for y in 0..GLYPH_PX {
        for x in 0..GLYPH_PX {
            let a = coverage[(y * GLYPH_PX + x) as usize];
            let idx = (((y * total_w) + SIDE_PAD_PX + x) * 4) as usize;
            rgba[idx] = 0xff;
            rgba[idx + 1] = 0xff;
            rgba[idx + 2] = 0xff;
            rgba[idx + 3] = a;
        }
    }
    (rgba, total_w, GLYPH_PX)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `TraySegment` literal helper — `group_start` defaults to false, which
    /// is what every pre-v4 test below wants (a single group).
    fn seg(text: &str, color: TrayColor) -> TraySegment {
        TraySegment {
            text: text.into(),
            color,
            group_start: false,
        }
    }

    #[test]
    fn glyph_has_some_ink() {
        let cov = glyph_coverage(GLYPH_PX, 0.5);
        assert!(
            cov.iter().any(|&a| a > 0),
            "glyph must have some opaque pixels"
        );
    }

    // v4 docs/design/NOTES.md §3: the tail must render even at 0% used ("empty
    // quota, empty letter") — a Q, never an O, regardless of fill state.
    #[test]
    fn the_tail_renders_even_at_zero_percent_used() {
        let cov = glyph_coverage(GLYPH_PX, 0.0);
        assert!(
            cov.iter().any(|&a| a > 0),
            "the tail (and track) must still draw at 0% used"
        );
    }

    // v4: the arc's own end angle comes from data. 0% used draws no arc ink
    // at all beyond the tail/track (no stray dot at the gap edge); 100%
    // draws substantially more ink than a low percentage, since the swept
    // arc is now most of the ring instead of nothing.
    #[test]
    fn arc_sweep_grows_with_used_fraction() {
        let empty = glyph_coverage(GLYPH_PX, 0.0);
        let full = glyph_coverage(GLYPH_PX, 1.0);
        let count_ink = |cov: &[u8]| cov.iter().filter(|&&a| a > 32).count();
        assert!(
            count_ink(&full) > count_ink(&empty) + 50,
            "a fully-used arc should paint substantially more ink than an empty one"
        );
    }

    /// Round-3 regression guard for firstmate's exact finding
    /// (`data/quotos-tray-t1/firstmate-findings-1.md`): the glyph was
    /// measured at 11.5x10.5pt ink against neighbouring menu bar icons'
    /// 13.5-20pt, "still the smallest thing on the bar." Measures the ink's
    /// own bounding box (any non-zero-coverage pixel) directly out of the
    /// composited buffer and asserts it lands near the 14-16pt band on an
    /// 18pt canvas — i.e. in physical (2x) pixels, roughly 28-32px on a
    /// GLYPH_PX=36 canvas — rather than trusting the target-diameter
    /// constant alone, since antialiasing and the round caps could in
    /// principle push the real ink bounds off from what was intended.
    #[test]
    fn glyph_ink_bounding_box_is_in_the_target_band() {
        // Full sweep (100%) exercises the arc/tail's maximum reach — the
        // relevant case for this bound, since the track alone is smaller.
        let cov = glyph_coverage(GLYPH_PX, 1.0);
        let mut min_x = GLYPH_PX;
        let mut max_x = 0i64;
        let mut min_y = GLYPH_PX;
        let mut max_y = 0i64;
        for y in 0..GLYPH_PX {
            for x in 0..GLYPH_PX {
                if cov[(y * GLYPH_PX + x) as usize] > 32 {
                    min_x = min_x.min(x);
                    max_x = max_x.max(x as i64);
                    min_y = min_y.min(y);
                    max_y = max_y.max(y as i64);
                }
            }
        }
        let width = max_x - min_x as i64 + 1;
        let height = max_y - min_y as i64 + 1;
        // 14-16 CSS-pt at 2x = 28-32 physical px; a couple of px of slack
        // either side for the antialiasing threshold this test uses (>32
        // out of 255, not full opacity).
        assert!(
            (26..=34).contains(&width),
            "ink width {width}px should be roughly 28-32px (14-16pt @2x)"
        );
        assert!(
            (26..=34).contains(&height),
            "ink height {height}px should be roughly 28-32px (14-16pt @2x)"
        );
    }

    #[test]
    fn render_with_no_segments_is_the_glyph_square_plus_its_side_padding() {
        let (buf, w, h) = render(&[], false, 0);
        assert_eq!((w, h), (SIDE_PAD_PX * 2 + GLYPH_PX, GLYPH_PX));
        assert_eq!(buf.len(), (w * h * 4) as usize);
    }

    // R3-11: the highlighted and unhighlighted paths (`render` and
    // `plain_glyph_rgba`) swap places whenever the panel opens or closes. If
    // their widths differed the status item would resize on every toggle,
    // moving the glyph — and with it the beak — which is the exact drift class
    // this whole round exists to stop.
    #[test]
    fn every_bare_glyph_path_produces_the_same_image_width() {
        assert_eq!(plain_glyph_rgba(50).1, render(&[], false, 50).1);
        assert_eq!(plain_glyph_rgba(50).1, render(&[], true, 50).1);
    }

    // v4: the glyph's own arc fill must never affect image width either —
    // only the *segments* do. Same drift class as the test above.
    #[test]
    fn worst_used_percent_never_changes_the_bare_glyph_width() {
        assert_eq!(plain_glyph_rgba(0).1, plain_glyph_rgba(100).1);
        assert_eq!(render(&[], false, 0).1, render(&[], false, 100).1);
    }

    // The highlight has to reach the image's own edges — that is what gives it
    // the horizontal air that makes it read as a pressed menu bar button
    // rather than a box drawn tightly round the glyph.
    #[test]
    fn the_highlight_reaches_into_the_side_padding_where_the_glyph_never_draws() {
        let (buf, w, h) = render(&[], true, 0);
        let mid_row = h / 2;
        let alpha_at = |x: u32| buf[(((mid_row * w) + x) * 4 + 3) as usize];
        assert!(alpha_at(2) > 0, "highlight must cover the left padding");
        assert!(
            alpha_at(w - 3) > 0,
            "highlight must cover the right padding"
        );
        let (bare, _, _) = render(&[], false, 0);
        assert_eq!(
            bare[(((mid_row * w) + 2) * 4 + 3) as usize],
            0,
            "unhighlighted padding stays fully transparent"
        );
    }

    #[test]
    fn render_grows_width_per_segment_and_never_touches_height() {
        let one = render(&[seg("2%", TrayColor::Neutral)], false, 0);
        let two = render(
            &[seg("2%", TrayColor::Neutral), seg("78%", TrayColor::Amber)],
            false,
            0,
        );
        assert!(
            one.1 > GLYPH_PX,
            "adding a segment must widen the image beyond the bare glyph"
        );
        assert!(two.1 > one.1, "a second segment must widen it further");
        assert_eq!(one.2, GLYPH_PX);
        assert_eq!(two.2, GLYPH_PX);
    }

    // v4 §1/§5: "the cell reserve is the important part" — the status item
    // must be the same width whether a figure reads one digit or three,
    // because `compute_docked_layout` derives the beak's position from the
    // glyph's own (unrelated) position, but *other* digits sliding around
    // next to it is exactly the "reflow on a value change" bug §5 forbids.
    #[test]
    fn a_single_figures_width_never_changes_with_its_own_digit_count() {
        let one_digit = render(&[seg("9%", TrayColor::Neutral)], false, 0);
        let two_digit = render(&[seg("42%", TrayColor::Neutral)], false, 0);
        let three_digit = render(&[seg("100%", TrayColor::Neutral)], false, 0);
        assert_eq!(
            one_digit.1, two_digit.1,
            "1 vs 2 digits must reserve the same cell width"
        );
        assert_eq!(
            two_digit.1, three_digit.1,
            "2 vs 3 digits must reserve the same cell width"
        );
    }

    #[test]
    fn a_digit_and_percent_segment_renders_some_exact_colored_pixels() {
        let (buf, w, h) = render(&[seg("78%", TrayColor::Red)], false, 0);
        let red = TrayColor::Red.rgba(true);
        let found = buf
            .chunks_exact(4)
            .any(|px| (px[0], px[1], px[2], px[3]) == red);
        assert!(
            found,
            "expected at least one pixel painted in the red channel across a {w}x{h} buffer"
        );
    }

    #[test]
    fn a_broken_mark_renders_some_red_pixels() {
        // '!' is very likely dead in practice (CLAUDE.md: a broken pin
        // contributes no segment at all now), but should still render
        // correctly rather than being special-cased away.
        let (buf, w, h) = render(&[seg("!", TrayColor::Red)], false, 0);
        let red = TrayColor::Red.rgba(true);
        let found = buf
            .chunks_exact(4)
            .any(|px| (px[0], px[1], px[2], px[3]) == red);
        assert!(
            found,
            "expected at least one pixel painted in the red channel across a {w}x{h} buffer"
        );
    }

    // v4: a `group_start` segment draws its hairline gutter, widening the
    // image further than an equivalent same-group segment would.
    #[test]
    fn a_group_start_segment_widens_the_image_by_the_hairline_gutter() {
        let same_group = render(
            &[seg("9%", TrayColor::Neutral), seg("9%", TrayColor::Neutral)],
            false,
            0,
        );
        let mut second_group = seg("9%", TrayColor::Neutral);
        second_group.group_start = true;
        let two_groups = render(&[seg("9%", TrayColor::Neutral), second_group], false, 0);
        assert_eq!(
            two_groups.1 - same_group.1,
            GROUP_GUTTER_PRE_PX + HAIRLINE_WIDTH_PX + GROUP_GUTTER_POST_PX,
            "a group boundary must add exactly the gutter+hairline width"
        );
    }

    // v4: `group_start` on the very first segment (no prior group to part
    // from) must never draw a hairline before it — the frontend's own
    // `buildTraySegments` never sets it there, but the Rust layer shouldn't
    // rely on that alone.
    #[test]
    fn a_group_start_first_segment_draws_no_leading_hairline() {
        let mut first = seg("9%", TrayColor::Neutral);
        first.group_start = true;
        let with_flag = render(&[first], false, 0);
        let without_flag = render(&[seg("9%", TrayColor::Neutral)], false, 0);
        assert_eq!(
            with_flag.1, without_flag.1,
            "group_start on the first segment must not add a gutter"
        );
    }

    #[test]
    fn neutral_color_differs_between_dark_and_light() {
        assert_ne!(
            TrayColor::Neutral.rgba(true),
            TrayColor::Neutral.rgba(false)
        );
    }

    #[test]
    fn amber_and_red_are_identical_regardless_of_appearance() {
        assert_eq!(TrayColor::Amber.rgba(true), TrayColor::Amber.rgba(false));
        assert_eq!(TrayColor::Red.rgba(true), TrayColor::Red.rgba(false));
    }

    #[test]
    fn digit_widths_are_tabular() {
        // Tabular figures: every digit must occupy the same advance width so
        // percentages don't visibly shift as they tick up/down (design spec
        // calls this "tabular-nums"). MonoLisa is monospace already, and the
        // system fallback font is requested via
        // `monospacedDigitSystemFontOfSize:weight:`, which guarantees this.
        let one = render(&[seg("1", TrayColor::Red)], false, 0);
        let eight = render(&[seg("8", TrayColor::Red)], false, 0);
        assert_eq!(
            one.1, eight.1,
            "'1' and '8' must render at the same width (tabular figures)"
        );
    }

    #[test]
    fn font_fallback_detection_does_not_panic() {
        // Machine-dependent (whether MonoLisa is actually installed), so this
        // only asserts the lookup completes cleanly and returns a bool either
        // way — not which branch was taken.
        let _ = used_fallback_font();
    }

    // A11 regression guards.

    #[test]
    fn highlighted_bare_glyph_paints_translucent_pixels_behind_the_ink() {
        let (buf, w, h) = render(&[], true, 0);
        // The very corner pixel is deliberately *outside* the highlight's
        // own rounded rect (that's what "rounded" means) — sample just
        // inset from the flat middle of an edge instead, which is inside
        // the highlight but outside the glyph's own ink (a circular ring
        // roughly centered on the canvas, per `glyph_coverage`), so any
        // non-zero alpha there can only have come from the highlight layer.
        let (x, y) = (2u32, h / 2);
        let idx = ((y * w + x) * 4) as usize;
        let edge_alpha = buf[idx + 3];
        assert!(
            edge_alpha > 0,
            "expected the highlight to paint near the canvas edge, got alpha {edge_alpha}"
        );
        // And it must be translucent, not a solid fill — a fully opaque
        // pixel there would misread as a filled square, not a soft tint.
        assert!(
            edge_alpha < 255,
            "highlight should be translucent, got fully opaque alpha {edge_alpha}"
        );
    }

    #[test]
    fn unhighlighted_bare_glyph_leaves_the_corner_fully_transparent() {
        let (buf, _, _) = render(&[], false, 0);
        assert_eq!(
            buf[3], 0,
            "no highlight requested, corner should stay fully transparent"
        );
    }

    #[test]
    fn highlighted_digits_still_render_their_own_color_on_top() {
        // The highlight must not wash out or replace the digit color it
        // sits behind — blend_pixel's whole point is that a fully-opaque
        // foreground (the digit glyph's solid interior) still wins outright.
        let (buf, w, h) = render(&[seg("78%", TrayColor::Red)], true, 0);
        let red = TrayColor::Red.rgba(true);
        let found = buf
            .chunks_exact(4)
            .any(|px| (px[0], px[1], px[2], px[3]) == red);
        assert!(
            found,
            "expected an unmodified red digit pixel somewhere in a {w}x{h} highlighted buffer"
        );
    }
}
