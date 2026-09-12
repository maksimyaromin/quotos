use std::process::Command;

const GLYPH_PX: u32 = 36;
const SIDE_PAD_PX: u32 = 10;
/// Every buffer here is drawn at two pixels per point, which is also
/// the size the status item's image is presented at, so a length in
/// this module's pixels halves into points and vice versa.
pub const RENDER_SCALE: f64 = 2.0;
pub const GLYPH_LEFT_INSET_POINTS: f64 = SIDE_PAD_PX as f64 / RENDER_SCALE;
const GLYPH_GAP_PX: u32 = 19;
const FIGURE_GAP_PX: u32 = 11;
const GROUP_GUTTER_PRE_PX: u32 = 10;
const HAIRLINE_WIDTH_PX: u32 = 2;
const GROUP_GUTTER_POST_PX: u32 = 10;
const HAIRLINE_HEIGHT_PX: u32 = 22;
/// Between a group's slug and the first figure it leads. Tighter than
/// the gap between two figures, so the slug reads as belonging to the
/// figures after it rather than standing between two of them.
const SLUG_GAP_PX: u32 = 7;

const _: () = assert!(
    GLYPH_GAP_PX > FIGURE_GAP_PX,
    "the glyph must read as a separate shape from the figures via a wider gap than sits between two figures"
);

const _: () = assert!(
    FIGURE_GAP_PX > SLUG_GAP_PX,
    "a slug must sit closer to the figures it leads than those figures sit to each other"
);

#[cfg(target_os = "macos")]
const TEXT_FONT_SIZE_PT: f64 = 12.0 * 2.0;
/// A slug names its group; it never reports a number. Drawn smaller and
/// lighter than the digits so it leads them instead of competing.
#[cfg(target_os = "macos")]
const SLUG_FONT_SIZE_PT: f64 = 10.0 * 2.0;
const SLUG_INK_FRACTION: f64 = 0.62;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StatusItemColor {
    Neutral,
    Amber,
    Red,
}

impl StatusItemColor {
    fn rgba(self, dark_mode: bool) -> (u8, u8, u8, u8) {
        match self {
            StatusItemColor::Amber => (0xe0, 0xa9, 0x2b, 0xff),
            StatusItemColor::Red => (0xe5, 0x64, 0x6a, 0xff),
            StatusItemColor::Neutral if dark_mode => (0xff, 0xff, 0xff, 235),
            StatusItemColor::Neutral => (0x00, 0x00, 0x00, 217),
        }
    }
}

pub struct StatusItemSegment {
    pub text: String,
    pub color: StatusItemColor,
    pub group_start: bool,
    /// The group's short name, drawn just before this figure and the
    /// only thing a click folds that group by. Carried by the first
    /// figure of a group's cluster and by nothing else.
    pub slug: Option<String>,
}

fn glyph_coverage(canvas_px: u32, used_fraction: f64) -> Vec<u8> {
    const TRACK_RADIUS_SVG: f64 = 5.4;
    const TRACK_STROKE_SVG: f64 = 1.4;
    const TRACK_OPACITY: f64 = 0.26;
    const ARC_RADIUS_SVG: f64 = 5.4;
    const ARC_STROKE_SVG: f64 = 1.9;
    const GAP_CENTER_RAD: f64 = std::f64::consts::FRAC_PI_4;
    const GAP_HALF_WIDTH_RAD: f64 = 31.0 * std::f64::consts::PI / 180.0;
    const ARC_GAP_LOW_RAD: f64 = GAP_CENTER_RAD - GAP_HALF_WIDTH_RAD;
    const ARC_GAP_HIGH_RAD: f64 = GAP_CENTER_RAD + GAP_HALF_WIDTH_RAD;
    const TAIL_ANGLE_RAD: f64 = GAP_CENTER_RAD;
    const TAIL_R_INNER_SVG: f64 = 3.2;
    const TAIL_R_OUTER_SVG: f64 = 8.0;
    const TAIL_STROKE_SVG: f64 = ARC_STROKE_SVG;
    const TARGET_INK_DIAMETER_CSS_PX: f64 = 15.0;
    const AA_HALF_WIDTH_PX: f64 = 0.75;

    let natural_outer_diameter = (ARC_RADIUS_SVG + ARC_STROKE_SVG / 2.0) * 2.0;
    let px_per_css_px = canvas_px as f64 / 18.0;
    let scale = (TARGET_INK_DIAMETER_CSS_PX * px_per_css_px) / natural_outer_diameter;
    let center = canvas_px as f64 / 2.0;

    let gap_width = ARC_GAP_HIGH_RAD - ARC_GAP_LOW_RAD;
    let max_sweep = std::f64::consts::TAU - gap_width;
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
                let d_start = ((ux - cap_start_x).powi(2) + (uy - cap_start_y).powi(2)).sqrt();
                let d_end = ((ux - cap_end_x).powi(2) + (uy - cap_end_y).powi(2)).sqrt();
                let d = d_start.min(d_end);
                (1.0 - ((d - ARC_STROKE_SVG / 2.0) * scale) / AA_HALF_WIDTH_PX).clamp(0.0, 1.0)
            };

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

/// The glyph's own rightmost ink pixel, scanned from its rendered coverage.
/// See "Spacing the figures evenly" in docs/status-item-rendering.md.
fn glyph_ink_right_edge_px(coverage: &[u8], canvas_px: u32) -> f64 {
    const INK_EDGE_COVERAGE_THRESHOLD: u8 = 127;
    let mut max_x = None;
    for y in 0..canvas_px {
        for x in 0..canvas_px {
            if coverage[(y * canvas_px + x) as usize] > INK_EDGE_COVERAGE_THRESHOLD {
                max_x = Some(max_x.map_or(x, |m: u32| m.max(x)));
            }
        }
    }
    max_x.map_or(canvas_px as f64 / 2.0, |x| x as f64 + 1.0)
}

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

fn draw_highlight_background(buf: &mut [u8], w: u32, h: u32, dark: bool) {
    const RADIUS_PHYSICAL: f64 = 10.0;
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

/// The slug's ink: the neutral figure colour, thinned so the group's
/// name stays legible without reading as loudly as a number.
fn slug_rgba(dark: bool) -> (u8, u8, u8, u8) {
    let (r, g, b, a) = StatusItemColor::Neutral.rgba(dark);
    (r, g, b, (a as f64 * SLUG_INK_FRACTION).round() as u8)
}

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

/// A text run's drawing position: `x0` plus the sub-pixel `local_offset`
/// CoreText's own text position takes. See docs/status-item-rendering.md.
#[derive(Clone, Copy)]
struct TextOrigin {
    x0: u32,
    local_offset: f64,
}

#[cfg(target_os = "macos")]
mod text {
    use super::{TextOrigin, blend_pixel};
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

    fn medium_weight() -> f64 {
        unsafe { NSFontWeightMedium }
    }

    fn monolisa_family_available() -> bool {
        let names: CFRetained<CFArray<CFString>> =
            unsafe { CFRetained::cast_unchecked(CTFontManagerCopyAvailableFontFamilyNames()) };
        names.to_vec().iter().any(|n| n.to_string() == FONT_FAMILY)
    }

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

    pub fn load_font(size_pt: f64) -> LoadedFont {
        if let Some(font) = try_load_monolisa(size_pt) {
            return LoadedFont {
                handle: FontHandle::Mono(font),
                used_fallback: false,
            };
        }
        load_fallback_font(size_pt)
    }

    fn load_fallback_font(size_pt: f64) -> LoadedFont {
        let font = NSFont::monospacedDigitSystemFontOfSize_weight(size_pt, medium_weight());
        LoadedFont {
            handle: FontHandle::Fallback(font),
            used_fallback: true,
        }
    }

    /// Skips `try_load_monolisa` even where it would succeed; see
    /// "Text rendering" in docs/status-item-rendering.md.
    #[cfg(test)]
    pub fn load_font_forcing_fallback(size_pt: f64) -> LoadedFont {
        load_fallback_font(size_pt)
    }

    fn make_line(font: &CTFont, text: &str, rgba: (u8, u8, u8, u8)) -> Option<CFRetained<CTLine>> {
        if text.is_empty() {
            return None;
        }
        let string = CFString::from_str(text);
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

    fn draw_text_impl(
        buf: &mut [u8],
        buf_w: u32,
        buf_h: u32,
        origin: TextOrigin,
        font: &CTFont,
        text: &str,
        rgba: (u8, u8, u8, u8),
    ) -> u32 {
        let TextOrigin { x0, local_offset } = origin;
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
        let width = (local_offset + line_width).ceil().max(0.0) as u32;
        if width == 0 {
            return 0;
        }

        let mut pixels = vec![0u8; (width * buf_h * 4) as usize];
        // Both the fill color and this bitmap context use sRGB explicitly:
        // pairing a generic RGB color with a Device RGB context shifts even
        // opaque pixels off the requested color, not just antialiasing fuzz.
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

        let ascent = unsafe { font.ascent() };
        let descent = unsafe { font.descent() };
        // No CTM flip: flipping to a top-left origin here would mirror the
        // glyphs. The context stays in its native bottom-left/y-up
        // convention, and this baseline is derived for that.
        let baseline_native = ((buf_h as f64) + descent - ascent) / 2.0;
        CGContext::set_text_position(Some(&ctx), local_offset, baseline_native);
        // CTLineDraw over per-glyph CTFontDrawGlyphs: the per-glyph path
        // produced specific corrupted outlines on this font, OS, and
        // binding combination, such as a 7 missing its top bar.
        unsafe { line.draw(&ctx) };

        for y in 0..buf_h {
            for x in 0..width {
                let idx = ((y * width + x) * 4) as usize;
                let a = pixels[idx + 3];
                if a == 0 {
                    continue;
                }
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

    pub fn draw_text(
        buf: &mut [u8],
        buf_w: u32,
        buf_h: u32,
        origin: TextOrigin,
        font: &LoadedFont,
        text: &str,
        rgba: (u8, u8, u8, u8),
    ) -> u32 {
        draw_text_impl(
            buf,
            buf_w,
            buf_h,
            origin,
            font.handle.as_ct_font(),
            text,
            rgba,
        )
    }

    #[cfg(test)]
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

    /// The line's ink bounds, `(left, right)` from a text position of
    /// `(0.0, _)`: the tight box CoreText draws, not `measure`'s wider box.
    /// See "Spacing the figures evenly" in docs/status-item-rendering.md.
    pub fn ink_bounds(font: &LoadedFont, text: &str) -> (f64, f64) {
        let Some(line) = make_line(font.handle.as_ct_font(), text, (0, 0, 0, 0)) else {
            return (0.0, 0.0);
        };
        let bounds = unsafe { line.image_bounds(None) };
        (bounds.origin.x, bounds.origin.x + bounds.size.width)
    }
}

#[cfg(not(target_os = "macos"))]
mod text {
    use super::{TextOrigin, composite_mask};

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
            // A group's slug is letters, so this font carries the
            // uppercase alphabet the slug is built from as well.
            'A' => [0b010, 0b101, 0b111, 0b101, 0b101],
            'B' => [0b110, 0b101, 0b110, 0b101, 0b110],
            'C' => [0b011, 0b100, 0b100, 0b100, 0b011],
            'D' => [0b110, 0b101, 0b101, 0b101, 0b110],
            'E' => [0b111, 0b100, 0b110, 0b100, 0b111],
            'F' => [0b111, 0b100, 0b110, 0b100, 0b100],
            'G' => [0b011, 0b100, 0b101, 0b101, 0b011],
            'H' => [0b101, 0b101, 0b111, 0b101, 0b101],
            'I' => [0b111, 0b010, 0b010, 0b010, 0b111],
            'J' => [0b001, 0b001, 0b001, 0b101, 0b010],
            'K' => [0b101, 0b101, 0b110, 0b101, 0b101],
            'L' => [0b100, 0b100, 0b100, 0b100, 0b111],
            'M' => [0b101, 0b111, 0b111, 0b101, 0b101],
            'N' => [0b101, 0b111, 0b111, 0b111, 0b101],
            'O' => [0b010, 0b101, 0b101, 0b101, 0b010],
            'P' => [0b110, 0b101, 0b110, 0b100, 0b100],
            'Q' => [0b010, 0b101, 0b101, 0b110, 0b011],
            'R' => [0b110, 0b101, 0b110, 0b101, 0b101],
            'S' => [0b011, 0b100, 0b010, 0b001, 0b110],
            'T' => [0b111, 0b010, 0b010, 0b010, 0b010],
            'U' => [0b101, 0b101, 0b101, 0b101, 0b011],
            'V' => [0b101, 0b101, 0b101, 0b101, 0b010],
            'W' => [0b101, 0b101, 0b111, 0b111, 0b101],
            'X' => [0b101, 0b101, 0b010, 0b101, 0b101],
            'Y' => [0b101, 0b101, 0b010, 0b010, 0b010],
            'Z' => [0b111, 0b001, 0b010, 0b100, 0b111],
            _ => [0, 0, 0, 0, 0],
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
        origin: TextOrigin,
        _font: &LoadedFont,
        text: &str,
        rgba: (u8, u8, u8, u8),
    ) -> u32 {
        let (mask, mask_w) = render_mask(text, buf_h);
        composite_mask(
            buf,
            buf_w,
            buf_h,
            origin.x0 + origin.local_offset.round() as u32,
            &mask,
            mask_w,
            rgba,
        );
        mask_w
    }

    #[cfg(test)]
    pub fn measure(_font: &LoadedFont, text: &str) -> u32 {
        text_width(text)
    }

    /// This bitmap font has no side bearings: every glyph fills its cell
    /// edge to edge, so ink bounds and the advance box coincide.
    pub fn ink_bounds(_font: &LoadedFont, text: &str) -> (f64, f64) {
        (0.0, text_width(text) as f64)
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

#[cfg(target_os = "macos")]
fn slug_font_size_pt() -> f64 {
    SLUG_FONT_SIZE_PT
}
#[cfg(not(target_os = "macos"))]
fn slug_font_size_pt() -> f64 {
    0.0
}

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

/// Where one segment's text runs are drawn: the figure always, the
/// group's slug ahead of it when this figure leads a group's cluster.
struct SegmentPlacement {
    slug: Option<TextPlacement>,
    figure: TextPlacement,
    hairline_x0: Option<u32>,
}

struct TextPlacement {
    origin: TextOrigin,
    span: InkSpan,
}

/// The horizontal span one text run's ink occupies in the bitmap
/// `render` draws, so a click in the menu bar can be resolved back to
/// what sits underneath it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InkSpan {
    pub x0: u32,
    pub x1: u32,
}

/// One group slug's ink, and which segment carries it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SlugSpan {
    pub segment: usize,
    pub span: InkSpan,
}

/// A slug's ink is only as wide as its three letters, so a click a hair
/// off one still folds that group rather than falling through to the
/// panel.
const HIT_PADDING_PX: u32 = 4;

const _: () = assert!(
    HIT_PADDING_PX < GROUP_GUTTER_PRE_PX,
    "a slug's forgiving edge must stay inside its own cluster's gutter, never reaching the figure before it"
);

struct SegmentInk {
    slug: Option<(f64, f64)>,
    figure: (f64, f64),
}

fn measure_segments(segments: &[StatusItemSegment]) -> Vec<SegmentInk> {
    let figure_font = text::load_font(text_font_size_pt());
    let slug_font = text::load_font(slug_font_size_pt());
    segments
        .iter()
        .map(|seg| SegmentInk {
            slug: seg
                .slug
                .as_deref()
                .filter(|slug| !slug.is_empty())
                .map(|slug| text::ink_bounds(&slug_font, slug)),
            figure: text::ink_bounds(&figure_font, &seg.text),
        })
        .collect()
}

/// Places the glyph, then every cluster's slug and figures a fixed
/// ink-to-ink gap apart; see "Spacing the figures evenly" in
/// docs/status-item-rendering.md. Also returns where that ink ends.
fn layout_segments(
    segments: &[StatusItemSegment],
    ink: &[SegmentInk],
    glyph_ink_right_edge: f64,
) -> (Vec<SegmentPlacement>, f64) {
    let mut placements = Vec::with_capacity(segments.len());
    let mut ink_cursor = glyph_ink_right_edge;
    let place = |gap: f64, (ink_min_x, ink_max_x): (f64, f64), cursor: &mut f64| {
        let origin = *cursor + gap - ink_min_x;
        *cursor = origin + ink_max_x;
        let x0 = origin.floor();
        TextPlacement {
            origin: TextOrigin {
                x0: x0 as u32,
                local_offset: origin - x0,
            },
            span: InkSpan {
                x0: (origin + ink_min_x).floor().max(0.0) as u32,
                x1: cursor.ceil().max(0.0) as u32,
            },
        }
    };
    for (i, seg) in segments.iter().enumerate() {
        let (lead_gap, hairline_x0) = if i == 0 {
            (GLYPH_GAP_PX as f64, None)
        } else if seg.group_start {
            let hairline_left = ink_cursor + GROUP_GUTTER_PRE_PX as f64;
            ink_cursor = hairline_left + HAIRLINE_WIDTH_PX as f64;
            (
                GROUP_GUTTER_POST_PX as f64,
                Some(hairline_left.round() as u32),
            )
        } else {
            (FIGURE_GAP_PX as f64, None)
        };
        let slug = ink[i]
            .slug
            .map(|bounds| place(lead_gap, bounds, &mut ink_cursor));
        let figure_gap = if slug.is_some() {
            SLUG_GAP_PX as f64
        } else {
            lead_gap
        };
        let figure = place(figure_gap, ink[i].figure, &mut ink_cursor);
        placements.push(SegmentPlacement {
            slug,
            figure,
            hairline_x0,
        });
    }
    (placements, ink_cursor)
}

fn glyph_ink_right_edge_for(worst_used_percent: u8) -> f64 {
    let coverage = glyph_coverage(GLYPH_PX, worst_used_percent as f64 / 100.0);
    SIDE_PAD_PX as f64 + glyph_ink_right_edge_px(&coverage, GLYPH_PX)
}

/// Shares `layout_segments` with `render`, so the spans a click is
/// tested against are the ones the slugs were actually drawn at.
pub fn slug_spans(segments: &[StatusItemSegment], worst_used_percent: u8) -> Vec<SlugSpan> {
    if segments.is_empty() {
        return Vec::new();
    }
    let ink = measure_segments(segments);
    let (placements, _) =
        layout_segments(segments, &ink, glyph_ink_right_edge_for(worst_used_percent));
    placements
        .into_iter()
        .enumerate()
        .filter_map(|(segment, placement)| {
            placement.slug.map(|slug| SlugSpan {
                segment,
                span: slug.span,
            })
        })
        .collect()
}

/// Which segment's slug a click landed on, in the image's own pixel
/// grid, which `geometry.rs`'s `click_x_in_icon_px` puts a click into.
/// A click on a bare figure lands on no slug and so folds nothing.
pub fn slug_at(spans: &[SlugSpan], icon_width_px: u32, x: f64) -> Option<usize> {
    if !(0.0..=icon_width_px as f64).contains(&x) {
        return None;
    }
    spans
        .iter()
        .find(|slug| {
            x >= slug.span.x0.saturating_sub(HIT_PADDING_PX) as f64
                && x <= (slug.span.x1 + HIT_PADDING_PX) as f64
        })
        .map(|slug| slug.segment)
}

pub fn render(
    segments: &[StatusItemSegment],
    highlighted: bool,
    worst_used_percent: u8,
    dark: bool,
) -> (Vec<u8>, u32, u32) {
    let used_fraction = worst_used_percent as f64 / 100.0;
    let coverage = glyph_coverage(GLYPH_PX, used_fraction);
    let figure_font = text::load_font(text_font_size_pt());
    let slug_font = text::load_font(slug_font_size_pt());

    let ink = measure_segments(segments);
    let (placements, content_ink_right) =
        layout_segments(segments, &ink, glyph_ink_right_edge_for(worst_used_percent));

    let total_w = if segments.is_empty() {
        SIDE_PAD_PX * 2 + GLYPH_PX
    } else {
        (content_ink_right + SIDE_PAD_PX as f64).ceil() as u32
    };
    let total_h = GLYPH_PX;
    let mut buf = vec![0u8; (total_w * total_h * 4) as usize];

    if highlighted {
        draw_highlight_background(&mut buf, total_w, total_h, dark);
    }

    let ink_color = StatusItemColor::Neutral.rgba(dark);
    for y in 0..GLYPH_PX {
        for x in 0..GLYPH_PX {
            let a = coverage[(y * GLYPH_PX + x) as usize];
            if a == 0 {
                continue;
            }
            let blended = ((ink_color.3 as u16 * a as u16) / 255) as u8;
            blend_pixel(
                &mut buf,
                total_w,
                total_h,
                SIDE_PAD_PX + x,
                y,
                (ink_color.0, ink_color.1, ink_color.2, blended),
            );
        }
    }

    for (seg, placement) in segments.iter().zip(placements.iter()) {
        if let Some(hairline_x0) = placement.hairline_x0 {
            draw_hairline(&mut buf, total_w, total_h, hairline_x0, dark);
        }
        if let (Some(slug), Some(text)) = (&placement.slug, seg.slug.as_deref()) {
            text::draw_text(
                &mut buf,
                total_w,
                total_h,
                slug.origin,
                &slug_font,
                text,
                slug_rgba(dark),
            );
        }
        text::draw_text(
            &mut buf,
            total_w,
            total_h,
            placement.figure.origin,
            &figure_font,
            &seg.text,
            seg.color.rgba(dark),
        );
    }

    (buf, total_w, total_h)
}

pub fn plain_glyph_rgba(worst_used_percent: u8) -> (Vec<u8>, u32, u32) {
    let used_fraction = worst_used_percent as f64 / 100.0;
    let coverage = glyph_coverage(GLYPH_PX, used_fraction);
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

    fn seg(text: &str, color: StatusItemColor) -> StatusItemSegment {
        StatusItemSegment {
            text: text.into(),
            color,
            group_start: false,
            slug: None,
        }
    }

    fn led(slug: &str, text: &str) -> StatusItemSegment {
        StatusItemSegment {
            slug: Some(slug.into()),
            ..seg(text, StatusItemColor::Neutral)
        }
    }

    fn ink_with(
        figure_font: &text::LoadedFont,
        slug_font: &text::LoadedFont,
        segs: &[StatusItemSegment],
    ) -> Vec<SegmentInk> {
        segs.iter()
            .map(|s| SegmentInk {
                slug: s
                    .slug
                    .as_deref()
                    .map(|slug| text::ink_bounds(slug_font, slug)),
                figure: text::ink_bounds(figure_font, &s.text),
            })
            .collect()
    }

    /// Every segment's own absolute figure ink edges, computed the same
    /// way `render` places them; see "Spacing the figures evenly" in
    /// docs/status-item-rendering.md.
    fn figure_ink_edges(font: &text::LoadedFont, segs: &[StatusItemSegment]) -> Vec<(f64, f64)> {
        let slug_font = text::load_font(slug_font_size_pt());
        let ink = ink_with(font, &slug_font, segs);
        let (placements, _) = layout_segments(segs, &ink, glyph_ink_right_edge_for(0));
        placements
            .iter()
            .zip(&ink)
            .map(|(p, i)| {
                let origin = p.figure.origin.x0 as f64 + p.figure.origin.local_offset;
                (origin + i.figure.0, origin + i.figure.1)
            })
            .collect()
    }

    fn figure_spans(segs: &[StatusItemSegment], worst_used_percent: u8) -> Vec<InkSpan> {
        let ink = measure_segments(segs);
        let (placements, _) =
            layout_segments(segs, &ink, glyph_ink_right_edge_for(worst_used_percent));
        placements.into_iter().map(|p| p.figure.span).collect()
    }

    #[test]
    fn figure_spans_land_on_the_ink_the_figures_are_drawn_with() {
        let segs = [
            seg("18%", StatusItemColor::Neutral),
            seg("84%", StatusItemColor::Amber),
        ];
        let font = text::load_font(text_font_size_pt());
        let edges = figure_ink_edges(&font, &segs);
        let spans = figure_spans(&segs, 0);

        assert_eq!(spans.len(), 2);
        for (span, (ink_min, ink_max)) in spans.iter().zip(edges) {
            assert!(
                (span.x0 as f64 - ink_min).abs() <= 1.0,
                "span {span:?} should start at the ink's own left edge {ink_min}"
            );
            assert!(
                (span.x1 as f64 - ink_max).abs() <= 1.0,
                "span {span:?} should end at the ink's own right edge {ink_max}"
            );
        }
    }

    mod slugs {
        use super::*;

        #[test]
        fn a_slug_is_drawn_just_before_the_figure_it_leads() {
            let segs = [led("FAB", "55%")];
            let slugs = slug_spans(&segs, 0);
            let figures = figure_spans(&segs, 0);

            assert_eq!(slugs.len(), 1);
            assert_eq!(slugs[0].segment, 0);
            assert!(
                slugs[0].span.x1 <= figures[0].x0,
                "the slug leads its figure: {:?} then {:?}",
                slugs[0].span,
                figures[0]
            );
            assert!(
                (figures[0].x0 - slugs[0].span.x1).abs_diff(SLUG_GAP_PX) <= 1,
                "a slug sits SLUG_GAP_PX ({SLUG_GAP_PX}px) from the figure it leads, give or take how each edge rounds, got {}px",
                figures[0].x0 - slugs[0].span.x1
            );
        }

        #[test]
        fn a_slug_span_lands_on_the_ink_the_slug_is_drawn_with() {
            let segs = [led("CUR", "99%")];
            let slug_font = text::load_font(slug_font_size_pt());
            let ink = measure_segments(&segs);
            let (placements, _) = layout_segments(&segs, &ink, glyph_ink_right_edge_for(0));
            let placed = placements[0].slug.as_ref().expect("the slug is placed");
            let origin = placed.origin.x0 as f64 + placed.origin.local_offset;
            let (ink_min, ink_max) = text::ink_bounds(&slug_font, "CUR");
            let span = slug_spans(&segs, 0)[0].span;

            assert!((span.x0 as f64 - (origin + ink_min)).abs() <= 1.0);
            assert!((span.x1 as f64 - (origin + ink_max)).abs() <= 1.0);
        }

        #[test]
        fn only_the_figures_given_a_slug_report_one() {
            let segs = [
                led("FAB", "55%"),
                seg("18%", StatusItemColor::Neutral),
                StatusItemSegment {
                    group_start: true,
                    ..led("CUR", "99%")
                },
                StatusItemSegment {
                    group_start: true,
                    ..seg("12%", StatusItemColor::Neutral)
                },
            ];
            assert_eq!(
                slug_spans(&segs, 0)
                    .iter()
                    .map(|s| s.segment)
                    .collect::<Vec<_>>(),
                vec![0, 2],
                "an opened group's later members and a standalone pin carry no slug"
            );
        }

        #[test]
        fn an_empty_slug_is_drawn_and_hit_tested_as_no_slug_at_all() {
            let segs = [StatusItemSegment {
                slug: Some(String::new()),
                ..seg("55%", StatusItemColor::Neutral)
            }];
            assert!(slug_spans(&segs, 0).is_empty());
            assert_eq!(
                render(&segs, false, 0, true).1,
                render(&[seg("55%", StatusItemColor::Neutral)], false, 0, true).1,
                "an empty slug reserves no width"
            );
        }

        #[test]
        fn slug_at_folds_the_group_whose_slug_was_clicked() {
            let segs = [
                led("FAB", "55%"),
                StatusItemSegment {
                    group_start: true,
                    ..led("CUR", "99%")
                },
            ];
            let spans = slug_spans(&segs, 0);
            let (_, width, _) = render(&segs, false, 0, false);

            for slug in &spans {
                let middle = (slug.span.x0 + slug.span.x1) as f64 / 2.0;
                assert_eq!(
                    slug_at(&spans, width, middle),
                    Some(slug.segment),
                    "the middle of a slug folds its own group"
                );
            }
        }

        /// The whole point of moving the click target: the figure is no
        /// longer a button, so a click on the digits opens the panel
        /// like any other click on the item.
        #[test]
        fn a_click_on_the_bare_figure_folds_nothing() {
            let segs = [led("FAB", "55%")];
            let slugs = slug_spans(&segs, 0);
            let figure = figure_spans(&segs, 0)[0];
            let (_, width, _) = render(&segs, false, 0, false);

            for x in [
                figure.x0 as f64 + HIT_PADDING_PX as f64 + 1.0,
                (figure.x0 + figure.x1) as f64 / 2.0,
                figure.x1 as f64,
            ] {
                assert_eq!(
                    slug_at(&slugs, width, x),
                    None,
                    "icon pixel {x} is the figure's own ink, which folds nothing"
                );
            }
        }

        #[test]
        fn a_click_on_the_glyph_or_past_either_end_folds_nothing() {
            let segs = [led("FAB", "55%")];
            let spans = slug_spans(&segs, 0);
            let (_, width, _) = render(&segs, false, 0, false);

            assert_eq!(
                slug_at(&spans, width, (SIDE_PAD_PX + GLYPH_PX / 2) as f64),
                None
            );
            assert_eq!(slug_at(&spans, width, -0.1), None);
            assert_eq!(slug_at(&spans, width, width as f64 + 0.1), None);
            assert_eq!(slug_at(&spans, width, width as f64), None);
        }

        #[test]
        fn a_click_just_off_a_slug_still_folds_its_group() {
            let segs = [led("FAB", "55%")];
            let spans = slug_spans(&segs, 0);
            let (_, width, _) = render(&segs, false, 0, false);
            for off in 1..HIT_PADDING_PX {
                assert_eq!(
                    slug_at(&spans, width, spans[0].span.x0 as f64 - off as f64),
                    Some(0),
                    "a click {off}px shy of the letters still folds the group"
                );
            }
        }

        #[test]
        fn slug_at_finds_nothing_when_nothing_is_drawn() {
            let width = SIDE_PAD_PX * 2 + GLYPH_PX;
            assert_eq!(slug_at(&[], width, width as f64 / 2.0), None);
            assert!(slug_spans(&[], 0).is_empty());
        }

        #[test]
        fn two_slugs_never_claim_the_same_pixel_even_padded() {
            let segs = [
                led("FAB", "55%"),
                StatusItemSegment {
                    group_start: true,
                    ..led("CUR", "99%")
                },
            ];
            let spans = slug_spans(&segs, 0);
            assert!(
                spans[0].span.x1 + HIT_PADDING_PX < spans[1].span.x0.saturating_sub(HIT_PADDING_PX),
                "padded slug spans must stay disjoint: {spans:?}"
            );
        }

        #[test]
        fn a_slug_widens_the_image_by_its_own_ink_plus_its_gap() {
            let bare = render(&[seg("55%", StatusItemColor::Neutral)], false, 0, false);
            let with_slug = render(&[led("FAB", "55%")], false, 0, false);
            let slug_font = text::load_font(slug_font_size_pt());
            let (ink_min, ink_max) = text::ink_bounds(&slug_font, "FAB");
            let slug_ink = (ink_max - ink_min).round() as u32;

            assert!(with_slug.1 > bare.1, "a slug has to cost width");
            assert_eq!(with_slug.2, bare.2, "and no height");
            assert!(
                (with_slug.1 - bare.1).abs_diff(slug_ink + SLUG_GAP_PX) <= 2,
                "the slug should cost its own ink plus its gap: {} vs {}",
                with_slug.1 - bare.1,
                slug_ink + SLUG_GAP_PX
            );
        }

        #[test]
        fn a_slug_paints_ink_of_its_own_lighter_than_the_digits_beside_it() {
            let segs = [led("FAB", "55%")];
            let (buf, w, _) = render(&segs, false, 0, true);
            let span = slug_spans(&segs, 0)[0].span;
            let alpha_at = |x: u32, y: u32| buf[(((y * w) + x) * 4 + 3) as usize];
            let column_ink = |x: u32| (0..GLYPH_PX).map(|y| alpha_at(x, y)).max().unwrap_or(0);

            let slug_ink = (span.x0..span.x1).map(column_ink).max().unwrap_or(0);
            assert!(slug_ink > 0, "the slug's letters have to be drawn");
            assert!(
                slug_ink < StatusItemColor::Neutral.rgba(true).3,
                "the slug is drawn lighter than a figure's own ink"
            );
        }

        /// The colour bar that used to sit under a grouped figure is
        /// gone: a group is named in the menu bar, never tinted there.
        #[test]
        fn nothing_is_painted_under_a_grouped_figure_any_more() {
            let segs = [led("FAB", "55%")];
            let (buf, w, h) = render(&segs, false, 0, true);
            let figure = figure_spans(&segs, 0)[0];
            // The band the bar used to occupy: the bottom few rows
            // under the digits' own ink.
            for y in (h - 6)..h {
                for x in figure.x0..figure.x1.min(w) {
                    assert_eq!(
                        buf[(((y * w) + x) * 4 + 3) as usize],
                        0,
                        "pixel ({x}, {y}) under the figure must stay clear"
                    );
                }
            }
        }

        #[test]
        fn a_slug_does_not_disturb_the_figures_own_ink() {
            let bare_segs = [seg("55%", StatusItemColor::Neutral)];
            let led_segs = [led("FAB", "55%")];
            let bare = render(&bare_segs, false, 0, true);
            let with_slug = render(&led_segs, false, 0, true);
            let bare_span = figure_spans(&bare_segs, 0)[0];
            let led_span = figure_spans(&led_segs, 0)[0];

            // The slug shifts the digits into a different sub-pixel
            // phase, so their antialiasing differs column by column;
            // what must not change is how much ink they are drawn with.
            let ink_mass = |buf: &[u8], w: u32, span: InkSpan| -> u64 {
                (0..GLYPH_PX)
                    .flat_map(|y| (span.x0..span.x1).map(move |x| (y, x)))
                    .map(|(y, x)| buf[(((y * w) + x) * 4 + 3) as usize] as u64)
                    .sum()
            };
            let before = ink_mass(&bare.0, bare.1, bare_span);
            let after = ink_mass(&with_slug.0, with_slug.1, led_span);

            assert!(
                (bare_span.x1 - bare_span.x0).abs_diff(led_span.x1 - led_span.x0) <= 1,
                "the digits keep their own width, give or take how each edge rounds"
            );
            assert!(
                before.abs_diff(after) * 100 < before * 3,
                "the digits keep their own weight: {before} then {after}"
            );
        }
    }

    /// The click geometry the first pin-groups version got wrong: a
    /// bare fraction of the button's width does not map onto the image
    /// the button centres inside itself.
    mod clicks_through_the_buttons_margin {
        use super::*;
        use crate::geometry::click_x_in_icon_px;

        /// What AppKit hands back around a status item of this shape:
        /// eight points of button either side of the image, the same
        /// measurement the beak's own offset is derived from.
        const MARGIN_POINTS: f64 = 8.0;
        const ITEM_LEFT_POINTS: f64 = 1183.0;

        struct Scene {
            segments: Vec<StatusItemSegment>,
            spans: Vec<SlugSpan>,
            icon_width_px: u32,
            item_width_points: f64,
        }

        fn scene() -> Scene {
            let segments = vec![
                led("FAB", "55%"),
                StatusItemSegment {
                    group_start: true,
                    ..led("CUR", "99%")
                },
                StatusItemSegment {
                    group_start: true,
                    ..led("MON", "84%")
                },
                StatusItemSegment {
                    group_start: true,
                    ..seg("12%", StatusItemColor::Neutral)
                },
            ];
            let spans = slug_spans(&segments, 50);
            let (_, icon_width_px, _) = render(&segments, false, 50, true);
            Scene {
                segments,
                spans,
                icon_width_px,
                item_width_points: icon_width_px as f64 / RENDER_SCALE + MARGIN_POINTS * 2.0,
            }
        }

        /// Where a click has to land, in points across the display, to
        /// sit on a given pixel of the composited image.
        fn click_landing_on(icon_px: f64) -> f64 {
            ITEM_LEFT_POINTS + MARGIN_POINTS + icon_px / RENDER_SCALE
        }

        fn resolve(scene: &Scene, click_x_points: f64) -> Option<usize> {
            slug_at(
                &scene.spans,
                scene.icon_width_px,
                click_x_in_icon_px(
                    click_x_points,
                    ITEM_LEFT_POINTS,
                    Some(scene.item_width_points),
                    scene.icon_width_px as f64,
                ),
            )
        }

        /// The arithmetic this replaced: the click's offset as a plain
        /// fraction of the button's width, multiplied back out by the
        /// image's own width.
        fn resolve_by_bare_fraction(scene: &Scene, click_x_points: f64) -> Option<usize> {
            let fraction = (click_x_points - ITEM_LEFT_POINTS) / scene.item_width_points;
            slug_at(
                &scene.spans,
                scene.icon_width_px,
                fraction * scene.icon_width_px as f64,
            )
        }

        #[test]
        fn a_click_anywhere_on_a_slug_folds_that_slugs_own_group() {
            let scene = scene();
            for slug in &scene.spans {
                for icon_px in [
                    slug.span.x0 as f64,
                    (slug.span.x0 + slug.span.x1) as f64 / 2.0,
                    slug.span.x1 as f64,
                ] {
                    assert_eq!(
                        resolve(&scene, click_landing_on(icon_px)),
                        Some(slug.segment),
                        "icon pixel {icon_px} is slug {}'s own ink",
                        slug.segment
                    );
                }
            }
        }

        #[test]
        fn the_bare_fraction_of_the_buttons_width_misses_the_slug_clicked() {
            let scene = scene();
            let goes_astray = scene.spans.iter().any(|slug| {
                (slug.span.x0..=slug.span.x1).any(|icon_px| {
                    let click = click_landing_on(icon_px as f64);
                    resolve(&scene, click) == Some(slug.segment)
                        && resolve_by_bare_fraction(&scene, click) != Some(slug.segment)
                })
            });
            assert!(
                goes_astray,
                "the margin-blind arithmetic has to send at least one click on a slug's own letters somewhere else; that is the bug it caused"
            );
        }

        #[test]
        fn a_click_in_the_buttons_own_margin_folds_nothing() {
            let scene = scene();
            assert_eq!(
                resolve(&scene, ITEM_LEFT_POINTS + MARGIN_POINTS / 2.0),
                None
            );
            assert_eq!(
                resolve(
                    &scene,
                    ITEM_LEFT_POINTS + scene.item_width_points - MARGIN_POINTS / 2.0
                ),
                None
            );
        }

        /// Through the same coordinate space the slugs are resolved in:
        /// a click on the digits themselves opens the panel.
        #[test]
        fn a_click_on_a_figure_across_the_margin_folds_nothing_either() {
            let scene = scene();
            for figure in figure_spans(&scene.segments, 50) {
                let middle = (figure.x0 + figure.x1) as f64 / 2.0;
                assert_eq!(resolve(&scene, click_landing_on(middle)), None);
            }
        }

        #[test]
        fn without_the_buttons_width_the_image_is_taken_as_flush_left() {
            let scene = scene();
            let click = ITEM_LEFT_POINTS + 30.0;
            assert_eq!(
                click_x_in_icon_px(click, ITEM_LEFT_POINTS, None, scene.icon_width_px as f64),
                30.0 * RENDER_SCALE
            );
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
    fn a_non_trailing_figures_same_digit_count_barely_moves_what_follows_it() {
        let a = render(
            &[
                seg("42%", StatusItemColor::Neutral),
                seg("50%", StatusItemColor::Neutral),
            ],
            false,
            0,
            false,
        );
        let b = render(
            &[
                seg("77%", StatusItemColor::Neutral),
                seg("50%", StatusItemColor::Neutral),
            ],
            false,
            0,
            false,
        );
        assert!(
            a.1.abs_diff(b.1) <= 1,
            "a non-trailing segment's digit count staying put must not jitter the image width by more than rounding: {} vs {}",
            a.1,
            b.1
        );
    }

    #[test]
    fn a_non_trailing_figures_different_digit_count_now_moves_what_follows_it() {
        let font = text::load_font(text_font_size_pt());
        let narrow_width = text::measure(&font, "9%");
        let wide_width = text::measure(&font, "100%");

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
        let actual_delta = wide_first.1 - narrow_first.1;
        let advance_delta = wide_width - narrow_width;
        assert!(
            actual_delta.abs_diff(advance_delta) <= 3,
            "a non-trailing segment's own digit count should shift what follows by roughly its own advance delta ({advance_delta}px), not {actual_delta}px; a small gap remains from each digit's own ink bearing, not from digit count"
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
            .as_chunks::<4>()
            .0
            .iter()
            .any(|px| (px[0], px[1], px[2], px[3]) == red);
        assert!(
            found,
            "expected at least one pixel painted in the red channel across a {w}x{h} buffer"
        );
    }

    #[test]
    fn a_broken_mark_renders_some_red_pixels() {
        let (buf, w, h) = render(&[seg("!", StatusItemColor::Red)], false, 0, false);
        let red = StatusItemColor::Red.rgba(true);
        let found = buf
            .as_chunks::<4>()
            .0
            .iter()
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
            GROUP_GUTTER_PRE_PX + HAIRLINE_WIDTH_PX + GROUP_GUTTER_POST_PX - FIGURE_GAP_PX,
            "a group boundary must replace the plain figure gap it displaces with the gutter+hairline width"
        );

        let (buf, w, h) = two_groups;
        let segs = [seg("9%", StatusItemColor::Neutral), {
            let mut s = seg("9%", StatusItemColor::Neutral);
            s.group_start = true;
            s
        }];
        let ink = measure_segments(&segs);
        let (placements, _) = layout_segments(&segs, &ink, glyph_ink_right_edge_for(0));
        let gutter_x = placements[1]
            .hairline_x0
            .expect("the second group's leading segment must carry the hairline");
        let mid_row = h / 2;
        let painted = (gutter_x..gutter_x + HAIRLINE_WIDTH_PX)
            .any(|x| buf[(((mid_row * w) + x) * 4 + 3) as usize] > 0);
        assert!(
            painted,
            "expected hairline pixels at the layout's own computed gutter position {gutter_x}"
        );
    }

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
        for font in [
            text::load_font(text_font_size_pt()),
            text::load_font_forcing_fallback(text_font_size_pt()),
        ] {
            let widths: Vec<u32> = "0123456789"
                .chars()
                .map(|c| text::measure(&font, &c.to_string()))
                .collect();
            assert!(
                widths.iter().all(|&w| w == widths[0]),
                "every digit must advance the same width (tabular figures), got {widths:?}"
            );
        }
    }

    #[test]
    fn font_fallback_detection_does_not_panic() {
        let _ = used_fallback_font();
    }

    #[test]
    fn highlighted_bare_glyph_paints_translucent_pixels_behind_the_ink() {
        let (buf, w, h) = render(&[], true, 0, false);
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
            .as_chunks::<4>()
            .0
            .iter()
            .any(|px| (px[0], px[1], px[2], px[3]) == red);
        assert!(
            found,
            "expected an unmodified red digit pixel somewhere in a {w}x{h} highlighted buffer"
        );
    }

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

    #[test]
    fn every_figure_to_figure_ink_gap_is_equal_across_digit_count_mixes() {
        for font in [
            text::load_font(text_font_size_pt()),
            text::load_font_forcing_fallback(text_font_size_pt()),
        ] {
            for figures in [
                ["2%", "74%", "100%"],
                ["9%", "9%", "9%"],
                ["100%", "1%", "50%"],
            ] {
                let segs = figures.map(|t| seg(t, StatusItemColor::Neutral));
                let edges = figure_ink_edges(&font, &segs);
                let gaps: Vec<f64> = edges.windows(2).map(|w| w[1].0 - w[0].1).collect();
                for gap in &gaps {
                    assert!(
                        (gap - FIGURE_GAP_PX as f64).abs() < 1e-6,
                        "{figures:?}: every figure-to-figure ink gap must equal FIGURE_GAP_PX ({FIGURE_GAP_PX}px), got {gaps:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn the_glyph_to_first_figure_gap_is_its_own_larger_constant() {
        for font in [
            text::load_font(text_font_size_pt()),
            text::load_font_forcing_fallback(text_font_size_pt()),
        ] {
            let segs = [seg("42%", StatusItemColor::Neutral)];
            let glyph_ink_right_edge = SIDE_PAD_PX as f64
                + glyph_ink_right_edge_px(&glyph_coverage(GLYPH_PX, 0.0), GLYPH_PX);
            let glyph_gap = figure_ink_edges(&font, &segs)[0].0 - glyph_ink_right_edge;
            assert!(
                (glyph_gap - GLYPH_GAP_PX as f64).abs() < 1e-6,
                "the glyph-to-figure gap must equal GLYPH_GAP_PX ({GLYPH_GAP_PX}px), got {glyph_gap}"
            );
        }
    }

    #[test]
    fn the_glyphs_own_ink_is_unaffected_by_whether_figures_follow_it() {
        let bare = render(&[], false, 50, false);
        let with_figures = render(
            &[
                seg("9%", StatusItemColor::Neutral),
                seg("67%", StatusItemColor::Neutral),
                seg("96%", StatusItemColor::Red),
            ],
            false,
            50,
            false,
        );
        let glyph_region = |buf: &[u8], w: u32| -> Vec<u8> {
            (0..GLYPH_PX)
                .flat_map(|y| (0..GLYPH_PX).map(move |x| (y, x)))
                .map(|(y, x)| buf[(((y * w) + SIDE_PAD_PX + x) * 4 + 3) as usize])
                .collect()
        };
        assert_eq!(
            glyph_region(&bare.0, bare.1),
            glyph_region(&with_figures.0, with_figures.1),
            "the glyph's own drawn pixels must not change when figures are appended after it"
        );
    }
}
