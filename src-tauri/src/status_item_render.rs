//! Composites the status item's glyph plus colored percentage digits into
//! a raw RGBA buffer, since `tray-icon` has no colored-title path. See
//! docs/status-item-rendering.md.

use std::process::Command;

/// The crate always requests an 18pt button height. Rendering at 2x that
/// keeps the composited bitmap crisp on Retina, matching the design
/// system's own "18x18 CSS-px, 36x36 @2x" spec for the glyph.
const GLYPH_PX: u32 = 36;
/// All of the constants below are CSS-px values doubled for this buffer's
/// fixed 2x-of-18pt convention, matching GLYPH_PX itself. 5 CSS-px of air
/// sits at each end.
const SIDE_PAD_PX: u32 = 10;
/// Padding is not optional: it keeps the "panel open" highlight from
/// hugging the ink, and keeps the glyph from shifting when the highlight
/// toggles. See docs/status-item-rendering.md.
pub const GLYPH_LEFT_INSET_POINTS: f64 = SIDE_PAD_PX as f64 / 2.0;
/// Gap from the glyph's bounding box to the first figure cell's bounding
/// box, tuned so the ink-to-ink distance lands near a target of 6 CSS-px.
const GLYPH_TO_CELL_GAP_PX: u32 = 2;
/// The cell reserve for every segment but the trailing one: a digit-count
/// change in an earlier segment must never move the beak, which tracks
/// the glyph. `render()` sizes the trailing cell to its own measured width.
const CELL_WIDTH_PX: u32 = 60; // 30 CSS-px
/// Between two adjacent-cell groups: gutter, hairline, gutter, 5 + 1 + 5
/// CSS-px. The visible 19px across the hairline includes each side's own
/// cell-centering margin added on top of this 11px gutter.
const GROUP_GUTTER_PRE_PX: u32 = 10; // 5 CSS-px
const HAIRLINE_WIDTH_PX: u32 = 2; // 1 CSS-px thick
const GROUP_GUTTER_POST_PX: u32 = 10; // 5 CSS-px
/// The hairline's own drawn length, 11 CSS-px tall, centered in the row.
const HAIRLINE_HEIGHT_PX: u32 = 22;

/// The design system specifies 12 CSS-px digits, doubled to match this
/// buffer's 2x-for-Retina convention; see `GLYPH_PX`.
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
    /// never draws the hairline before the very first segment overall,
    /// even if the frontend set this there.
    pub group_start: bool,
}

/// Per-pixel alpha coverage for the capacity-gauge mark, drawn
/// procedurally from its exact vector geometry rather than rasterized
/// from a fixed-size source. See docs/status-item-rendering.md.
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
    // Antialiasing transition half-width, in `canvas_px` physical pixels.
    // A soft coverage ramp across roughly 1.5 physical px either side of
    // each edge, rather than a hard-edged, jagged threshold.
    const AA_HALF_WIDTH_PX: f64 = 0.75;

    let natural_outer_diameter = (ARC_RADIUS_SVG + ARC_STROKE_SVG / 2.0) * 2.0;
    // GLYPH_PX, this function's own canvas_px, is always 2x an 18-CSS-px canvas.
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
            // forward from `arc_start`. The un-swept remainder of the fixed
            // gap band falls out of this automatically as the tail end.
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
                // gap's fixed edges, since the sweep is data. Nearer
                // endpoint wins, by plain 2D distance for a true half-circle.
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

/// Read-only: `defaults read` only reads preferences, never writes. Absence
/// of the key, the light-mode default, fails the command, which
/// `unwrap_or(false)` correctly treats as not dark.
pub(crate) fn is_dark_mode() -> bool {
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

/// The highlight and then the glyph or digits can land on the same pixel,
/// so a plain overwrite would lose the highlight wherever they overlap.
/// This does standard src-over alpha compositing instead.
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

/// The "panel open" highlight, drawn as a rounded-box signed-distance
/// field rather than a native `NSStatusItem` highlighted state, since the
/// icon is already a custom bitmap and the design calls for exact tokens.
fn draw_highlight_background(buf: &mut [u8], w: u32, h: u32, dark: bool) {
    const RADIUS_PHYSICAL: f64 = 10.0; // 5 CSS-px, doubled for this buffer's usual 2x
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

/// Parts two pinned subscriptions' figure groups. The light-appearance
/// opacity mirrors `draw_highlight_background`'s light-below-dark pattern,
/// since a black line reads heavier than a white one at equal weight.
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

/// Composites an 8-bit coverage mask onto `buf` at `x0`, using `rgba` as
/// the solid color; coverage modulates `rgba`'s own alpha. Used only by
/// the non-macOS fallback text renderer; see docs/status-item-rendering.md.
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

/// Real system text through Core Text and CoreGraphics, via the `objc2`
/// bindings Tauri already pulls in, sharper than a hand-rolled bitmap
/// font next to Apple's own menu-bar text. See docs/status-item-rendering.md.
#[cfg(target_os = "macos")]
mod text {
    use super::blend_pixel;
    use objc2::rc::Retained;
    use objc2_app_kit::{NSFont, NSFontWeightMedium};
    use objc2_core_foundation::{
        CFArray, CFAttributedString, CFDictionary, CFNumber, CFRetained, CFString, CFType,
    };
    use objc2_core_graphics::{
        CGBitmapContextCreateWithData, CGColor, CGColorSpace, CGContext, CGImageAlphaInfo,
        kCGColorSpaceSRGB,
    };
    use objc2_core_text::{
        CTFont, CTFontDescriptor, CTFontManagerCopyAvailableFontFamilyNames, CTLine,
        kCTFontAttributeName, kCTFontFamilyNameAttribute, kCTFontTraitsAttribute,
        kCTFontWeightTrait, kCTForegroundColorAttributeName,
    };
    use std::ffi::c_void;

    pub const FONT_FAMILY: &str = "MonoLisa";

    /// Either the requested MonoLisa font or, if not installed, the
    /// system's tabular-figure UI font. `NSFont` and `CTFont` are
    /// toll-free bridged, so both expose an identical `&CTFont` for drawing.
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

    /// `NSFontWeight` and `CTFontWeightTrait` share the same -1.0..1.0
    /// normalized scale, so Apple's medium-weight constant is reused for
    /// both the CoreText and AppKit paths, rather than a hand-picked number.
    fn medium_weight() -> f64 {
        unsafe { NSFontWeightMedium }
    }

    /// A pure CoreText, thread-safe query, unlike `NSFontManager`, whose
    /// `sharedFontManager` requires a `MainThreadMarker`. This matters since
    /// `set_status_item_state` has no guarantee of running on the main thread.
    fn monolisa_family_available() -> bool {
        let names: CFRetained<CFArray<CFString>> =
            unsafe { CFRetained::cast_unchecked(CTFontManagerCopyAvailableFontFamilyNames()) };
        names.to_vec().iter().any(|n| n.to_string() == FONT_FAMILY)
    }

    /// `CTFontCreateWithFontDescriptor` never returns null; on a mismatch it
    /// silently substitutes a default font instead, so this double-checks
    /// the resolved font's own family name against a fluke substitution.
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

    /// Tries MonoLisa first, falls back to the system's tabular-figure UI
    /// font. No caching across calls; see docs/status-item-rendering.md for
    /// why staying stateless is the deliberate choice here.
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

    /// Builds a `CTLine` laying out `text` with `font` in color `rgba`,
    /// CoreText's standard path for a short run. Not manual per-glyph
    /// plotting: see docs/status-item-rendering.md for why that corrupts.
    fn make_line(font: &CTFont, text: &str, rgba: (u8, u8, u8, u8)) -> Option<CFRetained<CTLine>> {
        if text.is_empty() {
            return None;
        }
        let string = CFString::from_str(text);
        // sRGB, not generic RGB: the two don't share a gamma curve, which
        // shifts even opaque pixels off the requested color. See
        // docs/status-item-rendering.md for the measured drift.
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

    /// Composites `text` onto `buf` at `x0`. Returns the pixel width it
    /// occupied so callers can lay out the next segment after it.
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
                // An alpha-only context corrupts CoreText glyph shapes;
                // see docs/platform-constraints.md.
                CGImageAlphaInfo::PremultipliedLast.0,
                None,
                std::ptr::null_mut(),
            )
        };
        let Some(ctx) = ctx else {
            return 0;
        };

        // Deliberately not flipping the CTM: that mirrors the glyphs
        // themselves. Drawing stays in the context's native bottom-left/y-up
        // convention instead; see docs/status-item-rendering.md.
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
    /// `x0`. Returns the pixel width it occupied so callers can lay out
    /// the next segment after it.
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

    /// Measures `text` without drawing it. `render()` uses this to lay out
    /// segments before the final buffer is allocated.
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

/// Lets a non-mac `cargo check` still compile despite the CoreText
/// bindings above being macOS-only. A hand-rolled bitmap font, producing
/// a coverage mask so it shares `composite_mask` with the real path above.
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
/// font instead of finding MonoLisa, so a caller can report which font
/// actually rendered on a given machine.
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

/// `text::measure`'s widths center text inside each segment's own reserve,
/// never size it. A `group_start` flag on the very first segment is
/// excluded, since there is no prior group for it to part from.
fn text_area_width(segments: &[StatusItemSegment], widths: &[u32]) -> u32 {
    if segments.is_empty() {
        return 0;
    }
    let boundaries = segments.iter().skip(1).filter(|s| s.group_start).count() as u32;
    let gutter_px = GROUP_GUTTER_PRE_PX + HAIRLINE_WIDTH_PX + GROUP_GUTTER_POST_PX;
    let last_width = *widths.last().unwrap_or(&0);
    let non_last_segments = segments.len().saturating_sub(1) as u32;
    GLYPH_TO_CELL_GAP_PX + CELL_WIDTH_PX * non_last_segments + last_width + boundaries * gutter_px
}

/// Composites the glyph plus every segment's colored digits into one RGBA
/// buffer. An empty `segments` still draws the bare glyph as a non-template
/// image when `highlighted`, since a template image carries no background tint.
pub fn render(
    segments: &[StatusItemSegment],
    highlighted: bool,
    worst_used_percent: u8,
    dark: bool,
) -> (Vec<u8>, u32, u32) {
    let used_fraction = worst_used_percent as f64 / 100.0;
    let coverage = glyph_coverage(GLYPH_PX, used_fraction);
    let font = text::load_font(text_font_size_pt());

    let widths: Vec<u32> = segments
        .iter()
        .map(|s| text::measure(&font, &s.text))
        .collect();
    let total_w = SIDE_PAD_PX * 2 + GLYPH_PX + text_area_width(segments, &widths);
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

/// The bare glyph alone, no digits, for the quiet state. White RGB plus
/// alpha, a template image's convention: macOS tints it from alpha alone.
pub fn plain_glyph_rgba(worst_used_percent: u8) -> (Vec<u8>, u32, u32) {
    let used_fraction = worst_used_percent as f64 / 100.0;
    let coverage = glyph_coverage(GLYPH_PX, used_fraction);
    // Padded identically to render's output: the two paths swap places on
    // every panel toggle, and a width mismatch would move the beak.
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

    /// Measures the ink's bounding box out of the composited buffer rather
    /// than trusting `TARGET_INK_DIAMETER_CSS_PX` alone, since antialiasing
    /// and round caps could push the real bounds off from what's intended.
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
        // either side for the antialiasing threshold this test uses,
        // greater than 32 out of 255 rather than full opacity.
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
        let (buf, w, h) = render(&[], false, 0, false);
        assert_eq!((w, h), (SIDE_PAD_PX * 2 + GLYPH_PX, GLYPH_PX));
        assert_eq!(buf.len(), (w * h * 4) as usize);
    }

    #[test]
    fn every_bare_glyph_path_produces_the_same_image_width() {
        assert_eq!(plain_glyph_rgba(50).1, render(&[], false, 50, false).1);
        assert_eq!(plain_glyph_rgba(50).1, render(&[], true, 50, false).1);
    }

    // shell::sync_status_item_length sets the item's native length from
    // this function's returned width, so highlighted must never change it.
    #[test]
    fn click_highlight_and_panel_open_share_one_frame_width_with_segments_pinned() {
        let segs = [
            seg("51%", StatusItemColor::Neutral),
            seg("67%", StatusItemColor::Neutral),
            seg("96%", StatusItemColor::Red),
        ];
        let unhighlighted = render(&segs, false, 96, false);
        let highlighted = render(&segs, true, 96, false);
        assert_eq!(
            unhighlighted.1, highlighted.1,
            "the image width driving the status item's length must not change when the panel-open pill is drawn"
        );
        assert_eq!(unhighlighted.2, highlighted.2);
    }

    #[test]
    fn worst_used_percent_never_changes_the_bare_glyph_width() {
        assert_eq!(plain_glyph_rgba(0).1, plain_glyph_rgba(100).1);
        assert_eq!(
            render(&[], false, 0, false).1,
            render(&[], false, 100, false).1
        );
    }

    #[test]
    fn the_highlight_reaches_into_the_side_padding_where_the_glyph_never_draws() {
        let (buf, w, h) = render(&[], true, 0, false);
        let mid_row = h / 2;
        let alpha_at = |x: u32| buf[(((mid_row * w) + x) * 4 + 3) as usize];
        assert!(alpha_at(2) > 0, "highlight must cover the left padding");
        assert!(
            alpha_at(w - 3) > 0,
            "highlight must cover the right padding"
        );
        let (bare, _, _) = render(&[], false, 0, false);
        assert_eq!(
            bare[(((mid_row * w) + 2) * 4 + 3) as usize],
            0,
            "unhighlighted padding stays fully transparent"
        );
    }

    #[test]
    fn render_grows_width_per_segment_and_never_touches_height() {
        let one = render(&[seg("2%", StatusItemColor::Neutral)], false, 0, false);
        let two = render(
            &[
                seg("2%", StatusItemColor::Neutral),
                seg("78%", StatusItemColor::Amber),
            ],
            false,
            0,
            false,
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
        let one_digit = render(&[seg("9%", StatusItemColor::Neutral)], false, 0, false);
        let two_digit = render(&[seg("42%", StatusItemColor::Neutral)], false, 0, false);
        let three_digit = render(&[seg("100%", StatusItemColor::Neutral)], false, 0, false);
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
            false,
        );
        let wide_first = render(
            &[
                seg("100%", StatusItemColor::Neutral),
                seg("50%", StatusItemColor::Neutral),
            ],
            false,
            0,
            false,
        );
        assert_eq!(
            narrow_first.1, wide_first.1,
            "a non-trailing segment's own digit count must not change the image width"
        );
    }

    #[test]
    fn click_highlight_and_panel_open_share_one_frame_width_even_as_the_trailing_digit_count_varies()
     {
        for text in ["0%", "9%", "42%", "100%"] {
            let segs = [
                seg("51%", StatusItemColor::Neutral),
                seg(text, StatusItemColor::Neutral),
            ];
            let unhighlighted = render(&segs, false, 50, false);
            let highlighted = render(&segs, true, 50, false);
            assert_eq!(
                unhighlighted.1, highlighted.1,
                "trailing text {text:?}: image width must not depend on `highlighted`"
            );
        }
    }

    #[test]
    fn a_digit_and_percent_segment_renders_some_exact_colored_pixels() {
        let (buf, w, h) = render(&[seg("78%", StatusItemColor::Red)], false, 0, false);
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
        let (buf, w, h) = render(&[seg("!", StatusItemColor::Red)], false, 0, false);
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
            false,
        );
        let mut second_group = seg("9%", StatusItemColor::Neutral);
        second_group.group_start = true;
        let two_groups = render(
            &[seg("9%", StatusItemColor::Neutral), second_group],
            false,
            0,
            false,
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

    // The frontend's own buildTraySegments never sets group_start on the
    // first segment, but the Rust layer should not rely on that alone.
    #[test]
    fn a_group_start_first_segment_draws_no_leading_hairline() {
        let mut first = seg("9%", StatusItemColor::Neutral);
        first.group_start = true;
        let with_flag = render(&[first], false, 0, false);
        let without_flag = render(&[seg("9%", StatusItemColor::Neutral)], false, 0, false);
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
        let one = render(&[seg("1", StatusItemColor::Red)], false, 0, false);
        let eight = render(&[seg("8", StatusItemColor::Red)], false, 0, false);
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
        let (buf, w, h) = render(&[], true, 0, false);
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
        let (buf, _, _) = render(&[], false, 0, false);
        assert_eq!(
            buf[3], 0,
            "no highlight requested, corner should stay fully transparent"
        );
    }

    #[test]
    fn highlighted_digits_still_render_their_own_color_on_top() {
        let (buf, w, h) = render(&[seg("78%", StatusItemColor::Red)], true, 0, false);
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
            false,
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
