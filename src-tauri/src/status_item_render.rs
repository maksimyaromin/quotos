//! Composites the status item's glyph plus colored percentage digits into
//! a raw RGBA buffer. `tray-icon` v0.24.2's macOS `set_title` calls
//! `NSStatusItem`'s button `setTitle:` with a plain `NSString`, and there
//! is no attributed-string or color path anywhere in the crate's public
//! API, so the only route to a colored digit is to paint it directly and
//! hand macOS a finished bitmap through `set_icon`.
//!
//! `set_icon_for_ns_status_item_button` always asks for an 18pt-tall
//! `NSImage` regardless of the source bitmap's own pixel size, so
//! supplying a denser buffer than 18x18 is what keeps the result crisp on
//! a Retina menu bar.
//!
//! Digits are rendered with real system text through Core Text. See the
//! `text` submodule below.

use std::process::Command;

/// The crate always requests an 18pt button height. Rendering at 2x that
/// keeps the composited bitmap crisp on Retina, matching the design
/// system's own "18x18 CSS-px, 36x36 @2x" spec for the glyph.
const GLYPH_PX: u32 = 36;
/// All of the constants below are CSS-px values doubled for this buffer's
/// fixed 2x-of-18pt convention, matching GLYPH_PX itself. 5 CSS-px of air
/// sits at each end.
const SIDE_PAD_PX: u32 = 10; // 5 CSS-px air each end
/// Horizontal air on each side of the glyph and digits, inside the
/// composited image. Two things need it, and one of them is not optional:
///
/// * The "panel open" highlight is painted across this whole buffer, so
///   without padding it hugs the ink and reads as a box drawn around the
///   glyph rather than as a pressed menu bar button. macOS fills the
///   status item's own width for its own open items.
/// * It is applied unconditionally, highlighted or not, so the glyph's
///   position inside the item cannot shift when the highlight toggles. A
///   shift there would move the beak too.
///
/// `geometry.rs`'s `glyph_center_offset_from_item_left_points` reads
/// `GLYPH_LEFT_INSET_POINTS` rather than assuming the glyph is the image's
/// leftmost 18pt.
pub const GLYPH_LEFT_INSET_POINTS: f64 = SIDE_PAD_PX as f64 / 2.0;
/// Structural gap from the glyph's own bounding box to the first figure
/// cell's own bounding box. Tuned so the ink-to-ink distance, the glyph's
/// own ink margin plus this gap plus the first digit's own centering
/// margin, lands near the target of 6 CSS-px.
const GLYPH_TO_CELL_GAP_PX: u32 = 2;
/// The cell reserve. Never resize this per digit count, for every segment
/// except the trailing one: a digit count change in an earlier segment
/// must never move the beak, which tracks the glyph rather than the
/// segments.
///
/// The trailing segment does not get this reserve. `render()` sizes its
/// cell to that one segment's own measured width instead, since nothing
/// sits to its right for it to shift.
const CELL_WIDTH_PX: u32 = 60; // 30 CSS-px
/// Between two adjacent-cell groups: gutter, hairline, gutter, 5 + 1 + 5
/// CSS-px. The visible 19px across the hairline includes each side's own
/// cell-centering margin added on top of this 11px gutter.
const GROUP_GUTTER_PRE_PX: u32 = 10; // 5 CSS-px
const HAIRLINE_WIDTH_PX: u32 = 2; // 1 CSS-px thick
const GROUP_GUTTER_POST_PX: u32 = 10; // 5 CSS-px
/// The hairline's own drawn length, 11 CSS-px tall, centered in the row.
const HAIRLINE_HEIGHT_PX: u32 = 22;

/// The design system specifies 12 CSS-px digits; this buffer is rendered at
/// 2x for Retina throughout (see `GLYPH_PX`), so the actual CoreText point
/// size used in this bitmap's coordinate space is doubled to match.
#[cfg(target_os = "macos")]
const TEXT_FONT_SIZE_PT: f64 = 12.0 * 2.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StatusItemColor {
    Neutral,
    Amber,
    Red,
}

impl StatusItemColor {
    fn rgba(self, dark_mode: bool) -> (u8, u8, u8, u8) {
        match self {
            // --amber, defined in src/design-system/tokens/colors.css. Same in light and dark.
            StatusItemColor::Amber => (0xe0, 0xa9, 0x2b, 0xff),
            // --red. Same in light and dark.
            StatusItemColor::Red => (0xe5, 0x64, 0x6a, 0xff),
            StatusItemColor::Neutral if dark_mode => (0xff, 0xff, 0xff, 235),
            StatusItemColor::Neutral => (0x00, 0x00, 0x00, 217),
        }
    }
}

pub struct StatusItemSegment {
    pub text: String,
    pub color: StatusItemColor,
    /// True for the first segment of a new subscription's group. `render`
    /// draws the hairline immediately before any segment with this set,
    /// never before the very first segment overall, even if the frontend
    /// happened to set it there.
    pub group_start: bool,
}

/// Per-pixel alpha coverage, 0-255, row-major, for a `canvas_px` square,
/// for the capacity-gauge mark. Drawn procedurally from its exact vector
/// geometry, `src/design-system/assets/menubar-glyph.svg`: a faint
/// full-circle track plus a bold, round-capped arc with a gap, rather than
/// rasterized from a fixed-size source and scaled. Drawing the exact shape
/// at the target resolution means there is no raster source to be too
/// small or too soft, and the target ink size, `TARGET_INK_DIAMETER_CSS_PX`,
/// is a direct, tunable parameter instead of whatever a fixed asset
/// happened to contain.
///
/// The SVG's path, `M4.46 12.02 a5.4 5.4 0 1 1 7.08 0`, was converted to
/// the two gap-endpoint angles below by hand: the vector from the center
/// (8,8) to each endpoint, through `atan2`. Both are in this module's
/// plain math convention, y-down, 0 at the +x axis, which already matches
/// the SVG's own y-down convention with no flip needed, since this buffer
/// is top-left-origin throughout. See `blend_pixel` and the `text` module.
///
/// The mark reads as a Q, not an O: a ring with a gap, plus a second
/// stroke, the tail, through the gap's own diagonal, which is what a
/// counter needs to read as a Q rather than a bare ring. The gap is 62
/// degrees wide, centered at 45 degrees, lower-right, which is where the
/// tail sits. The tail is a second capsule, radius 3.2 to 8.0 along that
/// same 45-degree diagonal, same stroke weight as the arc, round caps,
/// drawn unconditionally, even at 0% used, since the tail is what reads as
/// a Q rather than an O regardless of fill state. Its own reach at 45
/// degrees, outer radius plus half-stroke, projected onto either axis,
/// stays just inside the ring's own axis-aligned reach, so it needs no
/// separate accounting in `natural_outer_diameter` or `scale` below. The
/// arc's own end angle is data, `used_fraction`, 0.0 to 1.0, clamped,
/// rather than a constant 100%. It starts right where the gap ends,
/// `ARC_GAP_HIGH_RAD`, and sweeps forward by `used_fraction` of the
/// maximum possible sweep, `TAU` minus the gap's own width, landing
/// exactly on the gap's other edge at 100%.
fn glyph_coverage(canvas_px: u32, used_fraction: f64) -> Vec<u8> {
    const TRACK_RADIUS_SVG: f64 = 5.4;
    const TRACK_STROKE_SVG: f64 = 1.4;
    const TRACK_OPACITY: f64 = 0.26;
    const ARC_RADIUS_SVG: f64 = 5.4;
    const ARC_STROKE_SVG: f64 = 1.9;
    // Gap: 62 degrees centered on 45 degrees, meaning 45 +/- 31 degrees.
    const GAP_CENTER_RAD: f64 = std::f64::consts::FRAC_PI_4;
    const GAP_HALF_WIDTH_RAD: f64 = 31.0 * std::f64::consts::PI / 180.0;
    const ARC_GAP_LOW_RAD: f64 = GAP_CENTER_RAD - GAP_HALF_WIDTH_RAD; // ~14°
    const ARC_GAP_HIGH_RAD: f64 = GAP_CENTER_RAD + GAP_HALF_WIDTH_RAD; // ~76°
                                                                       // The tail: same diagonal as the gap's own center, radius 3.2 to 8.0,
                                                                       // same stroke weight as the arc.
    const TAIL_ANGLE_RAD: f64 = GAP_CENTER_RAD;
    const TAIL_R_INNER_SVG: f64 = 3.2;
    const TAIL_R_OUTER_SVG: f64 = 8.0;
    const TAIL_STROKE_SVG: f64 = ARC_STROKE_SVG;
    // The middle of a 14-16pt ink band, measured as the bold arc's own
    // outer edge, its dominant visible silhouette, on the 18 CSS-px
    // canvas.
    const TARGET_INK_DIAMETER_CSS_PX: f64 = 15.0;
    // Antialiasing transition half-width, in physical (canvas_px) pixels.
    // A soft coverage ramp across roughly 1.5 physical px either side of
    // each edge, rather than a hard-edged, jagged threshold.
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
                // Round caps at the two ends of the current sweep, not the
                // gap's own fixed edges, since the sweep is data now.
                // Whichever endpoint is nearer, tested as a plain 2D
                // distance so the cap is a true half-circle.
                let d_start = ((ux - cap_start_x).powi(2) + (uy - cap_start_y).powi(2)).sqrt();
                let d_end = ((ux - cap_end_x).powi(2) + (uy - cap_end_y).powi(2)).sqrt();
                let d = d_start.min(d_end);
                (1.0 - ((d - ARC_STROKE_SVG / 2.0) * scale) / AA_HALF_WIDTH_PX).clamp(0.0, 1.0)
            };

            // The tail is a straight capsule, using point-to-segment
            // distance, always drawn regardless of used_fraction.
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
/// read of user preferences, not a write, so it changes nothing on the
/// machine. Absence of the key, the default light-mode case, makes the
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

/// This buffer can carry two translucent layers, the "panel open"
/// highlight and then the glyph or digits drawn over it, so a plain
/// overwrite would discard whichever layer drew second wherever they
/// overlap, losing the highlight everywhere the glyph or a digit covers
/// it. This does standard "src-over" alpha compositing instead. On a
/// fully-opaque `src` or a fully-transparent destination pixel this
/// reduces to a plain overwrite, so a single-layer caller, such as glyph
/// ink or digit text onto a blank buffer, is unaffected. Only a
/// highlighted, multi-layer case actually exercises the blend math.
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

/// A system-style highlight behind the whole glyph and digits image while
/// the panel is open: rgba(255,255,255,0.20) dark, rgba(0,0,0,0.14) light,
/// radius 5 CSS-px. Drawn here as a standard rounded-box signed-distance
/// field, the same antialiasing approach as `glyph_coverage`, rather than
/// reached through any native `NSStatusItem` highlighted state, because
/// the icon is already a custom composited bitmap and the design calls for
/// these exact tokens, not whatever tint macOS's own default selection
/// style would draw.
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

/// The vertical rule that parts two pinned subscriptions' figure groups:
/// an 11px hairline at white 34% in dark appearance, drawn
/// `HAIRLINE_WIDTH_PX` wide and `HAIRLINE_HEIGHT_PX` tall, centered in the
/// row. The light-appearance value mirrors `draw_highlight_background`'s
/// own light-below-dark pattern. A black line reads at a slightly lower
/// opacity than an equally-weighted white one, rather than inventing an
/// unrelated number.
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

/// Composites an 8-bit coverage mask, `mask`, `mask_w` wide by `buf_h`
/// tall, row-major, one byte per pixel, onto `buf` at horizontal offset
/// `x0`, using `rgba` as the solid color. Coverage modulates `rgba`'s own
/// alpha, so `StatusItemColor::Neutral`'s sub-255 base alpha is preserved, and
/// the RGB channels are always exactly `rgba`'s. The mask carries no color
/// information of its own. Used by the non-macOS fallback text renderer
/// below. The real macOS CoreText renderer draws already-colored RGBA
/// straight from CoreGraphics instead. See its `draw_text_impl`'s doc
/// comment for why a coverage-mask-only approach does not work for text.
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

/// Real system text rendering for the status item's percentage digits, through
/// Core Text and CoreGraphics, reached through the `objc2` bindings Tauri
/// already pulls in transitively. This is sharper than a hand-rolled
/// bitmap font next to Apple's own menu-bar text. See the non-macOS
/// fallback module below for the bitmap alternative this platform does
/// not need.
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

    /// Either the requested MonoLisa font, or, if it is not installed on
    /// this machine, the system's own tabular-figure UI font. `NSFont` and
    /// `CTFont` are toll-free bridged, the same underlying object, so both
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

    /// A pure CoreText, thread-safe query. Unlike `NSFontManager`, whose
    /// `sharedFontManager` requires a `MainThreadMarker` in these bindings
    /// and is genuinely main-thread-restricted, `CTFontManagerCopy*` is a
    /// plain C function with no such requirement. This matters here because
    /// `set_status_item_state` runs inside a Tauri command handler with no
    /// guarantee of being on the main thread.
    fn monolisa_family_available() -> bool {
        let names: CFRetained<CFArray<CFString>> =
            unsafe { CFRetained::cast_unchecked(CTFontManagerCopyAvailableFontFamilyNames()) };
        names.to_vec().iter().any(|n| n.to_string() == FONT_FAMILY)
    }

    /// Builds a font descriptor for family "MonoLisa" at the medium weight
    /// trait and resolves it to a concrete font.
    /// `CTFontCreateWithFontDescriptor`, like `CTFontCreateWithName`, never
    /// returns null. On a mismatch it silently substitutes a default font
    /// instead, so availability is checked up front through
    /// `CTFontManagerCopyAvailableFontFamilyNames`, and double-checked
    /// after creation by comparing the resolved font's own family name, in
    /// case of a fluke substitution.
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

    /// Tries MonoLisa first. Falls back to the system's tabular-figure UI
    /// font, `NSFont.monospacedDigitSystemFontOfSize:weight:`, if MonoLisa
    /// is not installed. No caching across calls. Font matching here is
    /// infrequent, once per status item repaint, at most once a minute, and cheap
    /// enough that keeping this stateless sidesteps `CFRetained` and
    /// `Retained` not being `Send + Sync`. Core Foundation and AppKit
    /// object wrappers are not declared thread-safe for storage in a
    /// shared `static`, even though the lookups themselves are safe to
    /// call off the main thread.
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
    /// `CTLineCreateWithAttributedString` plus `CTLineDraw` is CoreText's
    /// own standard, documented path for drawing a short text run. Used
    /// here instead of manually resolving glyph IDs and advances through
    /// `CTFontGetGlyphsForCharacters`, `CTFontGetAdvancesForGlyphs`, and
    /// `CTFontDrawGlyphs`: that lower-level path produces specific glyphs
    /// with wrong or incomplete outlines on this font, OS, and binding
    /// combination, for example a "7" missing its top bar while an "8"
    /// right next to it, same font, same call sequence, renders perfectly.
    /// The corruption is not a premultiply or coordinate-flip issue, since
    /// it appears identically with the system fallback font and with a
    /// straight RGBA context. `CTLine` goes through CoreText's normal text
    /// layout and shaping engine instead of raw per-glyph plotting, which
    /// is both simpler and correct here.
    fn make_line(font: &CTFont, text: &str, rgba: (u8, u8, u8, u8)) -> Option<CFRetained<CTLine>> {
        if text.is_empty() {
            return None;
        }
        let string = CFString::from_str(text);
        // sRGB, not new_generic_rgb. Pairing a Generic-RGB fill color with
        // a Device-RGB bitmap context, which do not share a gamma curve,
        // shifts even fully-opaque glyph-interior pixels well off the
        // requested color: a requested (229,100,106) can come back as
        // (236,123,125), a shift of more than 20 on the green and blue
        // channels, not just antialiasing fuzz. StatusItemColor::rgba's values
        // are plain CSS hex tokens, already sRGB by convention, so both the
        // fill color and the bitmap context below use sRGB explicitly and
        // agree with each other.
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

        // Deliberately not flipping the CTM here, the usual translate plus
        // scale(1,-1) trick that turns a CGBitmapContext's bottom-left/y-up
        // default into top-left/y-down. Doing so mirrors the glyphs
        // themselves vertically, since CTLineDraw and CTFontDrawGlyphs
        // orient glyph outlines relative to the CTM's handedness rather
        // than compensating for it. The alternative fix would be to also
        // set a flipped CGContextSetTextMatrix, but it is simpler to draw
        // in the context's native, bottom-left/y-up, convention and
        // account for that in baseline_native below.
        // CGBitmapContextCreateWithData's backing memory is always laid
        // out top-row-first regardless of the drawing CTM, a fixed
        // property of the pixel buffer rather than of how it is drawn
        // into, so the row-major copy loop further down needs no
        // inversion either way. Only the baseline math needs to account
        // for native y-up.
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

/// This crate only ever ships for macOS, as a menu bar app, but the
/// CoreText bindings above are gated `target_os = "macos"` so that a
/// non-mac `cargo check`, such as a contributor on Linux, still compiles.
/// This is a hand-rolled bitmap font, restructured to produce a coverage
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
    /// status item digits ever need: 0-9, percent, the broken mark, and space.
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
/// font instead of finding MonoLisa. Exposed so the caller can report
/// which font actually rendered on a given machine. See the `text`
/// submodule's `load_font` for the lookup-and-fallback logic itself.
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
/// buffer. Returns `(rgba, width, height)`. An empty `segments` still
/// draws the bare glyph, as a non-template colored image whenever
/// `highlighted` is true, since a plain template image cannot carry a
/// background tint of its own. A caller with truly nothing pinned and no
/// highlight should prefer the cheaper template-icon path in `shell.rs`
/// instead of calling this.
///
/// Each segment gets a fixed `CELL_WIDTH_PX` reserve, never the raw text
/// width. `text::measure` is only used to center text inside that reserve,
/// not to size the layout.
pub fn render(
    segments: &[StatusItemSegment],
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
    // A group_start flag on the very first segment means nothing, since
    // there is no prior group to part from, so it is excluded here the
    // same way the draw loop below excludes it.
    let boundaries = segments.iter().skip(1).filter(|s| s.group_start).count() as u32;
    let gutter_px = GROUP_GUTTER_PRE_PX + HAIRLINE_WIDTH_PX + GROUP_GUTTER_POST_PX;
    // Every cell except the trailing one keeps the fixed CELL_WIDTH_PX
    // reserve. The trailing cell is sized to that one segment's own
    // measured width instead. See CELL_WIDTH_PX's own doc comment for why
    // only the last one is safe to trim.
    let last_width = *widths.last().unwrap_or(&0);
    let non_last_segments = segments.len().saturating_sub(1) as u32;
    let text_area = if segments.is_empty() {
        0
    } else {
        GLYPH_TO_CELL_GAP_PX
            + CELL_WIDTH_PX * non_last_segments
            + last_width
            + boundaries * gutter_px
    };
    let total_w = SIDE_PAD_PX * 2 + GLYPH_PX + text_area;
    let total_h = GLYPH_PX;
    let mut buf = vec![0u8; (total_w * total_h * 4) as usize];

    if highlighted {
        draw_highlight_background(&mut buf, total_w, total_h, dark);
    }

    // The glyph itself always stays neutral. Only the digits carry
    // severity or staleness color.
    let ink = StatusItemColor::Neutral.rgba(dark);
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
    let last_index = segments.len().saturating_sub(1);
    for (i, (seg, w)) in segments.iter().zip(widths.iter()).enumerate() {
        if seg.group_start && i > 0 {
            x += GROUP_GUTTER_PRE_PX;
            draw_hairline(&mut buf, total_w, total_h, x, dark);
            x += HAIRLINE_WIDTH_PX + GROUP_GUTTER_POST_PX;
        }
        // The trailing segment's cell is exactly its own width, so no
        // centering offset falls out of that automatically. Every earlier
        // segment keeps the fixed reserve. See CELL_WIDTH_PX's doc
        // comment.
        let cell_width = if i == last_index { *w } else { CELL_WIDTH_PX };
        let text_x = x + cell_width.saturating_sub(*w) / 2;
        text::draw_text(
            &mut buf,
            total_w,
            total_h,
            text_x,
            &font,
            &seg.text,
            seg.color.rgba(dark),
        );
        x += cell_width;
    }

    (buf, total_w, total_h)
}

/// The bare glyph alone, no digits, for reverting to the quiet state, the
/// most common state, so this path matters at least as much as `render()`'s
/// embedded glyph. Rendered at full `GLYPH_PX` resolution. See
/// `glyph_coverage`'s doc comment. White RGB plus alpha, matching a
/// template image's convention: macOS tints template images itself from
/// alpha alone, per appearance. `worst_used_percent`: see `render`'s doc
/// comment. The empty state is still the glyph alone, its arc filled to
/// this value.
pub fn plain_glyph_rgba(worst_used_percent: u8) -> (Vec<u8>, u32, u32) {
    let used_fraction = worst_used_percent as f64 / 100.0;
    let coverage = glyph_coverage(GLYPH_PX, used_fraction);
    // Padded identically to render's output. See SIDE_PAD_PX. The two
    // paths swap places whenever the panel opens or a pin changes, and an
    // image width that changed between them would move the glyph, and
    // with it the beak, on every toggle.
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

    /// A `StatusItemSegment` literal helper. `group_start` defaults to false,
    /// which is what most tests below want: a single group.
    fn seg(text: &str, color: StatusItemColor) -> StatusItemSegment {
        StatusItemSegment {
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

    #[test]
    fn the_tail_renders_even_at_zero_percent_used() {
        let cov = glyph_coverage(GLYPH_PX, 0.0);
        assert!(
            cov.iter().any(|&a| a > 0),
            "the tail (and track) must still draw at 0% used"
        );
    }

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

    /// Measures the ink's own bounding box directly out of the composited
    /// buffer, rather than trusting `TARGET_INK_DIAMETER_CSS_PX` alone,
    /// since antialiasing and the round caps could push the real ink
    /// bounds off from what was intended.
    #[test]
    fn glyph_ink_bounding_box_is_in_the_target_band() {
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

    // render and plain_glyph_rgba swap places whenever the panel opens or
    // closes. A width mismatch would resize the status item on every
    // toggle, moving the glyph and the beak with it.
    #[test]
    fn every_bare_glyph_path_produces_the_same_image_width() {
        assert_eq!(plain_glyph_rgba(50).1, render(&[], false, 50).1);
        assert_eq!(plain_glyph_rgba(50).1, render(&[], true, 50).1);
    }

    // shell::sync_status_item_length sets the status item's native length
    // from this function's own returned width, so highlighted must never
    // change that width, for a real segment set too, not just the bare
    // glyph case above.
    #[test]
    fn click_highlight_and_panel_open_share_one_frame_width_with_segments_pinned() {
        let segs = [
            seg("51%", StatusItemColor::Neutral),
            seg("67%", StatusItemColor::Neutral),
            seg("96%", StatusItemColor::Red),
        ];
        let unhighlighted = render(&segs, false, 96);
        let highlighted = render(&segs, true, 96);
        assert_eq!(
            unhighlighted.1, highlighted.1,
            "the image width driving the status item's length must not change when the panel-open pill is drawn"
        );
        assert_eq!(unhighlighted.2, highlighted.2);
    }

    #[test]
    fn worst_used_percent_never_changes_the_bare_glyph_width() {
        assert_eq!(plain_glyph_rgba(0).1, plain_glyph_rgba(100).1);
        assert_eq!(render(&[], false, 0).1, render(&[], false, 100).1);
    }

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
        let one = render(&[seg("2%", StatusItemColor::Neutral)], false, 0);
        let two = render(
            &[
                seg("2%", StatusItemColor::Neutral),
                seg("78%", StatusItemColor::Amber),
            ],
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

    // compute_docked_layout derives the beak's position from the glyph's
    // own, unrelated, fixed-offset position, so it is safe for the
    // trailing segment's own width to move.
    #[test]
    fn a_trailing_figures_width_now_tracks_its_own_digit_count() {
        let one_digit = render(&[seg("9%", StatusItemColor::Neutral)], false, 0);
        let two_digit = render(&[seg("42%", StatusItemColor::Neutral)], false, 0);
        let three_digit = render(&[seg("100%", StatusItemColor::Neutral)], false, 0);
        assert!(
            one_digit.1 < two_digit.1,
            "a single (and so trailing) segment's own wider text should now widen the image"
        );
        assert!(
            two_digit.1 < three_digit.1,
            "three digits should reserve more than two, now that the trailing cell is tight"
        );
    }

    #[test]
    fn a_non_trailing_figures_digit_count_never_moves_what_follows_it() {
        let narrow_first = render(
            &[
                seg("9%", StatusItemColor::Neutral),
                seg("50%", StatusItemColor::Neutral),
            ],
            false,
            0,
        );
        let wide_first = render(
            &[
                seg("100%", StatusItemColor::Neutral),
                seg("50%", StatusItemColor::Neutral),
            ],
            false,
            0,
        );
        assert_eq!(
            narrow_first.1, wide_first.1,
            "a non-trailing segment's own digit count must not change the image width"
        );
    }

    #[test]
    fn click_highlight_and_panel_open_share_one_frame_width_even_as_the_trailing_digit_count_varies(
    ) {
        for text in ["0%", "9%", "42%", "100%"] {
            let segs = [
                seg("51%", StatusItemColor::Neutral),
                seg(text, StatusItemColor::Neutral),
            ];
            let unhighlighted = render(&segs, false, 50);
            let highlighted = render(&segs, true, 50);
            assert_eq!(
                unhighlighted.1, highlighted.1,
                "trailing text {text:?}: image width must not depend on `highlighted`"
            );
        }
    }

    #[test]
    fn a_digit_and_percent_segment_renders_some_exact_colored_pixels() {
        let (buf, w, h) = render(&[seg("78%", StatusItemColor::Red)], false, 0);
        let red = StatusItemColor::Red.rgba(true);
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
        // '!' is very likely unreachable in practice, since a broken pin
        // contributes no segment at all, but should still render
        // correctly rather than being special-cased away.
        let (buf, w, h) = render(&[seg("!", StatusItemColor::Red)], false, 0);
        let red = StatusItemColor::Red.rgba(true);
        let found = buf
            .chunks_exact(4)
            .any(|px| (px[0], px[1], px[2], px[3]) == red);
        assert!(
            found,
            "expected at least one pixel painted in the red channel across a {w}x{h} buffer"
        );
    }

    #[test]
    fn a_group_start_segment_widens_the_image_by_the_hairline_gutter() {
        let same_group = render(
            &[
                seg("9%", StatusItemColor::Neutral),
                seg("9%", StatusItemColor::Neutral),
            ],
            false,
            0,
        );
        let mut second_group = seg("9%", StatusItemColor::Neutral);
        second_group.group_start = true;
        let two_groups = render(
            &[seg("9%", StatusItemColor::Neutral), second_group],
            false,
            0,
        );
        assert_eq!(
            two_groups.1 - same_group.1,
            GROUP_GUTTER_PRE_PX + HAIRLINE_WIDTH_PX + GROUP_GUTTER_POST_PX,
            "a group boundary must add exactly the gutter+hairline width"
        );

        // The reserved width alone does not prove the hairline was
        // actually painted into it, only that space was left for it.
        let (buf, w, h) = two_groups;
        let gutter_x =
            SIDE_PAD_PX + GLYPH_PX + GLYPH_TO_CELL_GAP_PX + CELL_WIDTH_PX + GROUP_GUTTER_PRE_PX;
        let mid_row = h / 2;
        let painted = (gutter_x..gutter_x + HAIRLINE_WIDTH_PX)
            .any(|x| buf[(((mid_row * w) + x) * 4 + 3) as usize] > 0);
        assert!(
            painted,
            "expected hairline pixels within the reserved gutter"
        );
    }

    // group_start on the very first segment, where there is no prior
    // group to part from, must never draw a hairline before it. The
    // frontend's own buildTraySegments never sets it there, but the Rust
    // layer should not rely on that alone.
    #[test]
    fn a_group_start_first_segment_draws_no_leading_hairline() {
        let mut first = seg("9%", StatusItemColor::Neutral);
        first.group_start = true;
        let with_flag = render(&[first], false, 0);
        let without_flag = render(&[seg("9%", StatusItemColor::Neutral)], false, 0);
        assert_eq!(
            with_flag.1, without_flag.1,
            "group_start on the first segment must not add a gutter"
        );
    }

    #[test]
    fn neutral_color_differs_between_dark_and_light() {
        assert_ne!(
            StatusItemColor::Neutral.rgba(true),
            StatusItemColor::Neutral.rgba(false)
        );
    }

    #[test]
    fn amber_and_red_are_identical_regardless_of_appearance() {
        assert_eq!(
            StatusItemColor::Amber.rgba(true),
            StatusItemColor::Amber.rgba(false)
        );
        assert_eq!(
            StatusItemColor::Red.rgba(true),
            StatusItemColor::Red.rgba(false)
        );
    }

    #[test]
    fn digit_widths_are_tabular() {
        // MonoLisa is monospace already, and the system fallback font is
        // requested via monospacedDigitSystemFontOfSize:weight:, which
        // guarantees tabular figures.
        let one = render(&[seg("1", StatusItemColor::Red)], false, 0);
        let eight = render(&[seg("8", StatusItemColor::Red)], false, 0);
        assert_eq!(
            one.1, eight.1,
            "'1' and '8' must render at the same width (tabular figures)"
        );
    }

    #[test]
    fn font_fallback_detection_does_not_panic() {
        // Machine-dependent, so this only asserts the lookup completes
        // cleanly, not which branch it took.
        let _ = used_fallback_font();
    }

    #[test]
    fn highlighted_bare_glyph_paints_translucent_pixels_behind_the_ink() {
        let (buf, w, h) = render(&[], true, 0);
        // Sampled just inset from the flat middle of an edge, inside the
        // highlight's rounded rect but outside the glyph's own ink.
        let (x, y) = (2u32, h / 2);
        let idx = ((y * w + x) * 4) as usize;
        let edge_alpha = buf[idx + 3];
        assert!(
            edge_alpha > 0,
            "expected the highlight to paint near the canvas edge, got alpha {edge_alpha}"
        );
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
        let (buf, w, h) = render(&[seg("78%", StatusItemColor::Red)], true, 0);
        let red = StatusItemColor::Red.rgba(true);
        let found = buf
            .chunks_exact(4)
            .any(|px| (px[0], px[1], px[2], px[3]) == red);
        assert!(
            found,
            "expected an unmodified red digit pixel somewhere in a {w}x{h} highlighted buffer"
        );
    }

    // The bound is looser than SIDE_PAD_PX alone so a font-fallback
    // machine does not make this flaky, while still failing hard against
    // a leftover slice of CELL_WIDTH_PX's much larger reserve.
    #[test]
    fn nothing_but_the_side_pad_survives_after_the_trailing_segments_own_ink() {
        let (buf, w, h) = render(
            &[
                seg("51%", StatusItemColor::Neutral),
                seg("0%", StatusItemColor::Neutral),
            ],
            false,
            0,
        );
        let mut last_ink_x = 0u32;
        for y in 0..h {
            for x in 0..w {
                let a = buf[(((y * w) + x) * 4 + 3) as usize];
                if a > 0 {
                    last_ink_x = last_ink_x.max(x);
                }
            }
        }
        let trailing_gap = w - 1 - last_ink_x;
        assert!(
            trailing_gap <= SIDE_PAD_PX + 8,
            "trailing gap {trailing_gap}px should track SIDE_PAD_PX ({SIDE_PAD_PX}px), not a leftover cell reserve (would be tens of px)"
        );
    }
}
