use std::process::Command;

const GLYPH_PX: u32 = 36;
const SIDE_PAD_PX: u32 = 10;
pub const GLYPH_LEFT_INSET_POINTS: f64 = SIDE_PAD_PX as f64 / 2.0;
const GLYPH_GAP_PX: u32 = 19;
const FIGURE_GAP_PX: u32 = 11;
const GROUP_GUTTER_PRE_PX: u32 = 10;
const HAIRLINE_WIDTH_PX: u32 = 2;
const GROUP_GUTTER_POST_PX: u32 = 10;
const HAIRLINE_HEIGHT_PX: u32 = 22;

const _: () = assert!(
    GLYPH_GAP_PX > FIGURE_GAP_PX,
    "the glyph must read as a separate shape from the figures via a wider gap than sits between two figures"
);

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

struct FigurePlacement {
    origin: TextOrigin,
    hairline_x0: Option<u32>,
}

/// Places the glyph and every figure a fixed ink-to-ink gap apart; see
/// "Spacing the figures evenly" in docs/status-item-rendering.md. Returns
/// each placement and the absolute x just past the last figure's own ink.
fn layout_figures(
    segments: &[StatusItemSegment],
    ink_bounds: &[(f64, f64)],
    glyph_ink_right_edge: f64,
) -> (Vec<FigurePlacement>, f64) {
    let mut placements = Vec::with_capacity(segments.len());
    let mut ink_cursor = glyph_ink_right_edge;
    for (i, seg) in segments.iter().enumerate() {
        let (gap, hairline_x0) = if i == 0 {
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
        let (ink_min_x, ink_max_x) = ink_bounds[i];
        let origin = ink_cursor + gap - ink_min_x;
        ink_cursor = origin + ink_max_x;
        let x0 = origin.floor();
        placements.push(FigurePlacement {
            origin: TextOrigin {
                x0: x0 as u32,
                local_offset: origin - x0,
            },
            hairline_x0,
        });
    }
    (placements, ink_cursor)
}

pub fn render(
    segments: &[StatusItemSegment],
    highlighted: bool,
    worst_used_percent: u8,
    dark: bool,
) -> (Vec<u8>, u32, u32) {
    let used_fraction = worst_used_percent as f64 / 100.0;
    let coverage = glyph_coverage(GLYPH_PX, used_fraction);
    let font = text::load_font(text_font_size_pt());

    let ink_bounds: Vec<(f64, f64)> = segments
        .iter()
        .map(|s| text::ink_bounds(&font, &s.text))
        .collect();
    let glyph_ink_right_edge = SIDE_PAD_PX as f64 + glyph_ink_right_edge_px(&coverage, GLYPH_PX);
    let (placements, content_ink_right) =
        layout_figures(segments, &ink_bounds, glyph_ink_right_edge);

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

    for (seg, placement) in segments.iter().zip(placements.iter()) {
        if let Some(hairline_x0) = placement.hairline_x0 {
            draw_hairline(&mut buf, total_w, total_h, hairline_x0, dark);
        }
        text::draw_text(
            &mut buf,
            total_w,
            total_h,
            placement.origin,
            &font,
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
        }
    }

    /// Every segment's own absolute ink edges, computed the same way
    /// `render` places them; see "Spacing the figures evenly" in
    /// docs/status-item-rendering.md.
    fn figure_ink_edges(font: &text::LoadedFont, segs: &[StatusItemSegment]) -> Vec<(f64, f64)> {
        let ink_bounds: Vec<(f64, f64)> = segs
            .iter()
            .map(|s| text::ink_bounds(font, &s.text))
            .collect();
        let glyph_ink_right_edge =
            SIDE_PAD_PX as f64 + glyph_ink_right_edge_px(&glyph_coverage(GLYPH_PX, 0.0), GLYPH_PX);
        let (placements, _) = layout_figures(segs, &ink_bounds, glyph_ink_right_edge);
        placements
            .iter()
            .zip(&ink_bounds)
            .map(|(p, &(min_x, max_x))| {
                let origin = p.origin.x0 as f64 + p.origin.local_offset;
                (origin + min_x, origin + max_x)
            })
            .collect()
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
        let font = text::load_font(text_font_size_pt());
        let ink_bounds: Vec<(f64, f64)> = segs
            .iter()
            .map(|s| text::ink_bounds(&font, &s.text))
            .collect();
        let glyph_ink_right_edge =
            SIDE_PAD_PX as f64 + glyph_ink_right_edge_px(&glyph_coverage(GLYPH_PX, 0.0), GLYPH_PX);
        let (placements, _) = layout_figures(&segs, &ink_bounds, glyph_ink_right_edge);
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
