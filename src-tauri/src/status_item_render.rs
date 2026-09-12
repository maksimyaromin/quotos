use std::process::Command;

const GLYPH_PX: u32 = 36;
const SIDE_PAD_PX: u32 = 10;
/// Every buffer here is drawn at two pixels per point, which is also
/// the size the status item's image is presented at, so a length in
/// this module's pixels halves into points and vice versa.
pub const RENDER_SCALE: f64 = 2.0;
pub const GLYPH_LEFT_INSET_POINTS: f64 = SIDE_PAD_PX as f64 / RENDER_SCALE;
const GLYPH_GAP_PX: u32 = 19;
/// Between any two adjacent items, whichever kinds they are; see
/// "Spacing the items evenly" in docs/status-item-rendering.md.
const ITEM_GAP_PX: u32 = 11;

const _: () = assert!(
    GLYPH_GAP_PX > ITEM_GAP_PX,
    "the glyph must read as a separate shape from the items via a wider gap than sits between two of them"
);

/// A chip's frame and the alpha its fill tints the colour to, both
/// following the panel's own badge; see "The group chip" in
/// docs/status-item-rendering.md.
const CHIP_HEIGHT_PX: u32 = 26;
const CHIP_RADIUS_PX: f64 = 6.0;
const CHIP_PAD_X_PX: f64 = 10.0;
const CHIP_FILL_ALPHA: f64 = 0.18;

const _: () = assert!(
    CHIP_HEIGHT_PX < GLYPH_PX,
    "a chip must leave air above and below it inside the image"
);

#[cfg(target_os = "macos")]
const TEXT_FONT_SIZE_PT: f64 = 12.0 * 2.0;
/// A chip names its group; it never reports a number. Set smaller than
/// the digits, and in the interface face; see "The group chip" in
/// docs/status-item-rendering.md.
#[cfg(target_os = "macos")]
const SLUG_FONT_SIZE_PT: f64 = 10.0 * 2.0;

/// The palette a pin group is drawn in, mirroring `GROUP_COLORS` in
/// `types/entities.ts` and the colour tokens the panel uses.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GroupColor {
    Teal,
    Blue,
    Violet,
    Amber,
    Red,
}

impl GroupColor {
    pub fn parse(name: &str) -> GroupColor {
        match name {
            "blue" => GroupColor::Blue,
            "violet" => GroupColor::Violet,
            "amber" => GroupColor::Amber,
            "red" => GroupColor::Red,
            _ => GroupColor::Teal,
        }
    }

    fn rgb(self, dark_mode: bool) -> (u8, u8, u8) {
        match (self, dark_mode) {
            (GroupColor::Teal, true) => (0x4e, 0x9c, 0x8d),
            (GroupColor::Teal, false) => (0x18, 0x5f, 0x55),
            (GroupColor::Blue, true) => (0x5b, 0x8d, 0xef),
            (GroupColor::Blue, false) => (0x3a, 0x6f, 0xd8),
            (GroupColor::Violet, true) => (0x8b, 0x7a, 0xd8),
            (GroupColor::Violet, false) => (0x5f, 0x4b, 0xbd),
            (GroupColor::Amber, true) => (0xe0, 0xa9, 0x2b),
            (GroupColor::Amber, false) => (0xb9, 0x79, 0x1a),
            (GroupColor::Red, true) => (0xe5, 0x64, 0x6a),
            (GroupColor::Red, false) => (0xd3, 0x3a, 0x41),
        }
    }
}

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

/// One item in the status item, left to right; see "What the item is
/// made of" in docs/status-item-rendering.md.
#[derive(Clone, Debug, PartialEq)]
pub enum StatusItemSegment {
    /// A pin group's chip: the only thing a click folds a group by,
    /// and, collapsed, the whole of what that group draws.
    Chip { slug: String, color: GroupColor },
    /// One pinned limit window's percentage.
    Figure {
        text: String,
        color: StatusItemColor,
    },
}

impl StatusItemSegment {
    /// The text this item draws, whichever kind it is.
    fn text(&self) -> &str {
        match self {
            StatusItemSegment::Chip { slug, .. } => slug,
            StatusItemSegment::Figure { text, .. } => text,
        }
    }
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

/// A chip's rounded rectangle, filled with a muted tint of the group's
/// colour, over the same signed-distance field the highlight uses.
fn draw_chip(buf: &mut [u8], w: u32, h: u32, rect: ChipRect, rgb: (u8, u8, u8)) {
    const AA_HALF_WIDTH_PX: f64 = 0.75;
    let half_height = CHIP_HEIGHT_PX as f64 / 2.0;
    let half_width = (rect.x1 - rect.x0) / 2.0;
    if half_width <= 0.0 {
        return;
    }
    let radius = CHIP_RADIUS_PX.min(half_width).min(half_height);
    let center_x = (rect.x0 + rect.x1) / 2.0;
    let center_y = h as f64 / 2.0;
    let first_x = (rect.x0 - 1.0).floor().max(0.0) as u32;
    let last_x = (rect.x1 + 1.0).ceil().clamp(0.0, w as f64) as u32;
    let first_y = (center_y - half_height - 1.0).floor().max(0.0) as u32;
    let last_y = (center_y + half_height + 1.0).ceil().clamp(0.0, h as f64) as u32;

    for y in first_y..last_y {
        for x in first_x..last_x {
            let px = x as f64 + 0.5 - center_x;
            let py = y as f64 + 0.5 - center_y;
            let qx = px.abs() - (half_width - radius);
            let qy = py.abs() - (half_height - radius);
            let outside_len = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt();
            let signed_dist = outside_len + qx.max(qy).min(0.0) - radius;
            let coverage = (0.5 - signed_dist / AA_HALF_WIDTH_PX).clamp(0.0, 1.0);
            if coverage <= 0.0 {
                continue;
            }
            let alpha = (255.0 * CHIP_FILL_ALPHA * coverage)
                .round()
                .clamp(0.0, 255.0) as u8;
            blend_pixel(buf, w, h, x, y, (rgb.0, rgb.1, rgb.2, alpha));
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

    /// The interface face, for a chip's letters. A slug is a name, so
    /// it is set the way the panel sets a badge: the UI font at medium
    /// weight, never the tabular one the digits need.
    pub fn load_ui_font(size_pt: f64) -> LoadedFont {
        LoadedFont {
            handle: FontHandle::Fallback(NSFont::systemFontOfSize_weight(size_pt, medium_weight())),
            used_fallback: false,
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

    /// The bitmap fallback has one face, which letters and digits alike
    /// are drawn in.
    pub fn load_ui_font(size_pt: f64) -> LoadedFont {
        load_font(size_pt)
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

/// Where one item is drawn: a figure's text run, or a chip's frame
/// with its letters centred inside it.
enum SegmentPlacement {
    Chip {
        rect: ChipRect,
        letters: TextPlacement,
    },
    Figure(TextPlacement),
}

impl SegmentPlacement {
    /// The item's own drawn edges: a chip's frame, a figure's ink.
    fn span(&self) -> InkSpan {
        match self {
            SegmentPlacement::Chip { rect, .. } => InkSpan {
                x0: rect.x0.floor().max(0.0) as u32,
                x1: rect.x1.ceil().max(0.0) as u32,
            },
            SegmentPlacement::Figure(text) => text.span,
        }
    }
}

/// A chip's filled rectangle, horizontally; it is always centred
/// vertically and `CHIP_HEIGHT_PX` tall.
#[derive(Clone, Copy, Debug)]
struct ChipRect {
    x0: f64,
    x1: f64,
}

struct TextPlacement {
    origin: TextOrigin,
    span: InkSpan,
}

/// The horizontal span one item occupies in the bitmap `render` draws,
/// so a click in the menu bar can be resolved back to what sits
/// underneath it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InkSpan {
    pub x0: u32,
    pub x1: u32,
}

/// One group chip's frame, and which segment carries it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChipSpan {
    pub segment: usize,
    pub span: InkSpan,
}

/// A click a hair off a chip's frame still folds that group rather
/// than falling through to the panel.
const HIT_PADDING_PX: u32 = 4;

const _: () = assert!(
    2 * HIT_PADDING_PX < ITEM_GAP_PX,
    "two adjacent chips' forgiving edges must not meet in the gap between them"
);

/// The ink bounds of the text one item draws, in its own font.
type SegmentInk = (f64, f64);

fn measure_segments(segments: &[StatusItemSegment]) -> Vec<SegmentInk> {
    let figure_font = text::load_font(text_font_size_pt());
    let slug_font = text::load_ui_font(slug_font_size_pt());
    segments
        .iter()
        .map(|seg| match seg {
            StatusItemSegment::Chip { slug, .. } => text::ink_bounds(&slug_font, slug),
            StatusItemSegment::Figure { text, .. } => text::ink_bounds(&figure_font, text),
        })
        .collect()
}

/// Places the glyph, then every item one fixed gap after the last, and
/// returns where the ink ends; see "Spacing the items evenly" in
/// docs/status-item-rendering.md.
fn layout_segments(
    segments: &[StatusItemSegment],
    ink: &[SegmentInk],
    glyph_ink_right_edge: f64,
) -> (Vec<Option<SegmentPlacement>>, f64) {
    let mut placements = Vec::with_capacity(segments.len());
    let mut cursor = glyph_ink_right_edge;
    let mut any_placed = false;

    for (seg, &(ink_min_x, ink_max_x)) in segments.iter().zip(ink) {
        if seg.text().is_empty() {
            placements.push(None);
            continue;
        }
        let gap = if any_placed {
            ITEM_GAP_PX as f64
        } else {
            GLYPH_GAP_PX as f64
        };
        let place_text = |origin: f64| {
            let x0 = origin.floor();
            TextPlacement {
                origin: TextOrigin {
                    x0: x0 as u32,
                    local_offset: origin - x0,
                },
                span: InkSpan {
                    x0: (origin + ink_min_x).floor().max(0.0) as u32,
                    x1: (origin + ink_max_x).ceil().max(0.0) as u32,
                },
            }
        };
        placements.push(Some(match seg {
            StatusItemSegment::Chip { .. } => {
                let rect = ChipRect {
                    x0: cursor + gap,
                    x1: cursor + gap + (ink_max_x - ink_min_x) + CHIP_PAD_X_PX * 2.0,
                };
                cursor = rect.x1;
                let letters = place_text(rect.x0 + CHIP_PAD_X_PX - ink_min_x);
                SegmentPlacement::Chip { rect, letters }
            }
            StatusItemSegment::Figure { .. } => {
                let origin = cursor + gap - ink_min_x;
                cursor = origin + ink_max_x;
                SegmentPlacement::Figure(place_text(origin))
            }
        }));
        any_placed = true;
    }
    (placements, cursor)
}

fn glyph_ink_right_edge_for(worst_used_percent: u8) -> f64 {
    let coverage = glyph_coverage(GLYPH_PX, worst_used_percent as f64 / 100.0);
    SIDE_PAD_PX as f64 + glyph_ink_right_edge_px(&coverage, GLYPH_PX)
}

/// Shares `layout_segments` with `render`, so the spans a click is
/// tested against are the frames the chips were actually drawn at.
pub fn chip_spans(segments: &[StatusItemSegment], worst_used_percent: u8) -> Vec<ChipSpan> {
    if segments.is_empty() {
        return Vec::new();
    }
    let ink = measure_segments(segments);
    let (placements, _) =
        layout_segments(segments, &ink, glyph_ink_right_edge_for(worst_used_percent));
    placements
        .into_iter()
        .enumerate()
        .filter_map(|(segment, placement)| match placement {
            Some(placement @ SegmentPlacement::Chip { .. }) => Some(ChipSpan {
                segment,
                span: placement.span(),
            }),
            _ => None,
        })
        .collect()
}

/// Which segment's chip a click landed on, in the image's own pixel
/// grid, which `geometry.rs`'s `click_x_in_icon_px` puts a click into.
/// A click on a bare figure lands on no chip and so folds nothing.
pub fn chip_at(spans: &[ChipSpan], icon_width_px: u32, x: f64) -> Option<usize> {
    if !(0.0..=icon_width_px as f64).contains(&x) {
        return None;
    }
    spans
        .iter()
        .find(|chip| {
            x >= chip.span.x0.saturating_sub(HIT_PADDING_PX) as f64
                && x <= (chip.span.x1 + HIT_PADDING_PX) as f64
        })
        .map(|chip| chip.segment)
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
    let slug_font = text::load_ui_font(slug_font_size_pt());

    let ink = measure_segments(segments);
    let (placements, content_ink_right) =
        layout_segments(segments, &ink, glyph_ink_right_edge_for(worst_used_percent));

    let placed_anything = placements.iter().any(Option::is_some);
    let total_w = if placed_anything {
        (content_ink_right + SIDE_PAD_PX as f64).ceil() as u32
    } else {
        SIDE_PAD_PX * 2 + GLYPH_PX
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
        let Some(placement) = placement else { continue };
        match (seg, placement) {
            (StatusItemSegment::Chip { slug, color }, SegmentPlacement::Chip { rect, letters }) => {
                let (r, g, b) = color.rgb(dark);
                draw_chip(&mut buf, total_w, total_h, *rect, (r, g, b));
                text::draw_text(
                    &mut buf,
                    total_w,
                    total_h,
                    letters.origin,
                    &slug_font,
                    slug,
                    (r, g, b, 0xff),
                );
            }
            (StatusItemSegment::Figure { text, color }, SegmentPlacement::Figure(placed)) => {
                text::draw_text(
                    &mut buf,
                    total_w,
                    total_h,
                    placed.origin,
                    &figure_font,
                    text,
                    color.rgba(dark),
                );
            }
            _ => {}
        }
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

    fn fig(text: &str, color: StatusItemColor) -> StatusItemSegment {
        StatusItemSegment::Figure {
            text: text.into(),
            color,
        }
    }

    fn chip(slug: &str) -> StatusItemSegment {
        StatusItemSegment::Chip {
            slug: slug.into(),
            color: GroupColor::Blue,
        }
    }

    fn ink_with(
        figure_font: &text::LoadedFont,
        slug_font: &text::LoadedFont,
        segs: &[StatusItemSegment],
    ) -> Vec<SegmentInk> {
        segs.iter()
            .map(|s| match s {
                StatusItemSegment::Chip { slug, .. } => text::ink_bounds(slug_font, slug),
                StatusItemSegment::Figure { text, .. } => text::ink_bounds(figure_font, text),
            })
            .collect()
    }

    /// Every item's own absolute drawn edges, computed the same way
    /// `render` places them: a figure's ink, a chip's frame. See
    /// "Spacing the items evenly" in docs/status-item-rendering.md.
    fn drawn_edges(font: &text::LoadedFont, segs: &[StatusItemSegment]) -> Vec<(f64, f64)> {
        let slug_font = text::load_ui_font(slug_font_size_pt());
        let ink = ink_with(font, &slug_font, segs);
        let (placements, _) = layout_segments(segs, &ink, glyph_ink_right_edge_for(0));
        placements
            .iter()
            .zip(&ink)
            .filter_map(|(placement, ink)| match placement.as_ref()? {
                SegmentPlacement::Chip { rect, .. } => Some((rect.x0, rect.x1)),
                SegmentPlacement::Figure(text) => {
                    let origin = text.origin.x0 as f64 + text.origin.local_offset;
                    Some((origin + ink.0, origin + ink.1))
                }
            })
            .collect()
    }

    fn spans(segs: &[StatusItemSegment], worst_used_percent: u8) -> Vec<Option<InkSpan>> {
        let ink = measure_segments(segs);
        let (placements, _) =
            layout_segments(segs, &ink, glyph_ink_right_edge_for(worst_used_percent));
        placements
            .into_iter()
            .map(|p| p.map(|p| p.span()))
            .collect()
    }

    fn figure_spans(segs: &[StatusItemSegment], worst_used_percent: u8) -> Vec<InkSpan> {
        spans(segs, worst_used_percent)
            .into_iter()
            .zip(segs)
            .filter(|(_, seg)| matches!(seg, StatusItemSegment::Figure { .. }))
            .filter_map(|(span, _)| span)
            .collect()
    }

    fn alpha_at(buf: &[u8], w: u32, x: u32, y: u32) -> u8 {
        buf[(((y * w) + x) * 4 + 3) as usize]
    }

    #[test]
    fn figure_spans_land_on_the_ink_the_figures_are_drawn_with() {
        let segs = [
            fig("18%", StatusItemColor::Neutral),
            fig("84%", StatusItemColor::Amber),
        ];
        let font = text::load_font(text_font_size_pt());
        let edges = drawn_edges(&font, &segs);
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

    /// Adjacent items are parted by spacing and nothing else, so
    /// nothing is ever painted in the gap between two of them.
    mod no_dividers {
        use super::*;

        fn painted_columns(buf: &[u8], w: u32, h: u32) -> Vec<bool> {
            (0..w)
                .map(|x| (0..h).any(|y| alpha_at(buf, w, x, y) > 0))
                .collect()
        }

        #[test]
        fn the_gap_between_two_items_is_empty_whatever_kinds_they_are() {
            for segs in [
                vec![
                    fig("18%", StatusItemColor::Neutral),
                    fig("55%", StatusItemColor::Neutral),
                ],
                vec![chip("FAB"), chip("CUR")],
                vec![chip("FAB"), fig("12%", StatusItemColor::Neutral)],
                vec![
                    chip("FAB"),
                    fig("18%", StatusItemColor::Neutral),
                    fig("55%", StatusItemColor::Neutral),
                    chip("CUR"),
                    fig("12%", StatusItemColor::Neutral),
                ],
            ] {
                let (buf, w, h) = render(&segs, false, 0, true);
                let columns = painted_columns(&buf, w, h);
                let edges: Vec<InkSpan> = spans(&segs, 0).into_iter().flatten().collect();
                for pair in edges.windows(2) {
                    // A span is the item's own edges, and antialiasing
                    // puts a soft pixel just outside them; a divider was
                    // two pixels wide in the middle of the gap.
                    for x in pair[0].x1 + 2..pair[1].x0 - 1 {
                        assert!(
                            !columns[x as usize],
                            "column {x} sits in the gap between {:?} and {:?} and must stay empty",
                            pair[0], pair[1]
                        );
                    }
                }
            }
        }
    }

    mod chips {
        use super::*;

        /// The whole of the captain's second complaint: a collapsed
        /// group used to draw its slug and then a rolled-up figure.
        /// Collapsed, a group is one chip and nothing else.
        #[test]
        fn a_collapsed_group_draws_its_chip_and_no_figure_at_all() {
            let segs = [chip("FAB")];
            let placed: Vec<Option<InkSpan>> = spans(&segs, 0);
            assert_eq!(placed.len(), 1, "a collapsed group is exactly one item");
            assert_eq!(
                chip_spans(&segs, 0).len(),
                1,
                "and that one item is its chip"
            );
            assert!(
                figure_spans(&segs, 0).is_empty(),
                "a collapsed group must draw no figure"
            );
        }

        #[test]
        fn an_opened_group_keeps_its_chip_and_lays_its_members_out_after_it() {
            let segs = [
                chip("FAB"),
                fig("18%", StatusItemColor::Neutral),
                fig("55%", StatusItemColor::Neutral),
            ];
            let placed: Vec<InkSpan> = spans(&segs, 0).into_iter().flatten().collect();
            let chips = chip_spans(&segs, 0);

            assert_eq!(chips.len(), 1);
            assert_eq!(chips[0].segment, 0, "the chip still leads its members");
            assert_eq!(chips[0].span, placed[0]);
            assert!(
                placed[0].x1 < placed[1].x0 && placed[1].x1 < placed[2].x0,
                "the members follow the chip in order: {placed:?}"
            );
        }

        /// Opening a group must not move its chip: the chip is the click
        /// target, and a target that jumps out from under the pointer on
        /// its own click is a target nobody can hit twice.
        #[test]
        fn opening_a_group_leaves_its_chip_exactly_where_it_was() {
            let collapsed = [chip("FAB")];
            let opened = [chip("FAB"), fig("18%", StatusItemColor::Neutral)];
            assert_eq!(
                chip_spans(&collapsed, 0)[0].span,
                chip_spans(&opened, 0)[0].span
            );
        }

        #[test]
        fn a_chip_is_a_frame_around_its_letters_not_bare_text() {
            let segs = [chip("FAB")];
            let (buf, w, h) = render(&segs, false, 0, true);
            let frame = chip_spans(&segs, 0)[0].span;
            let letters = {
                let ink = measure_segments(&segs);
                let (placements, _) = layout_segments(&segs, &ink, glyph_ink_right_edge_for(0));
                match placements[0].as_ref().expect("the chip is placed") {
                    SegmentPlacement::Chip { letters, .. } => letters.span,
                    SegmentPlacement::Figure(_) => unreachable!("a chip is not a figure"),
                }
            };

            assert!(
                frame.x0 < letters.x0 && letters.x1 < frame.x1,
                "the frame has to surround the letters: {frame:?} around {letters:?}"
            );
            // A column inside the frame but clear of every letter: the
            // fill has to be painted there, which bare text never would.
            let padding_column = frame.x0 + (CHIP_PAD_X_PX / 2.0) as u32;
            assert!(
                alpha_at(&buf, w, padding_column, h / 2) > 0,
                "column {padding_column} is the chip's own padding and must carry its fill"
            );
        }

        #[test]
        fn a_chips_fill_is_a_muted_tint_and_its_letters_the_saturated_colour() {
            let segs = [StatusItemSegment::Chip {
                slug: "FAB".into(),
                color: GroupColor::Blue,
            }];
            let (buf, w, h) = render(&segs, false, 0, true);
            let (r, g, b) = GroupColor::Blue.rgb(true);
            let frame = chip_spans(&segs, 0)[0].span;

            let fill_alpha = alpha_at(&buf, w, frame.x0 + (CHIP_PAD_X_PX / 2.0) as u32, h / 2);
            assert!(
                fill_alpha > 0 && fill_alpha < 128,
                "the fill is a muted tint of the colour, got alpha {fill_alpha}"
            );
            let letters_painted = buf
                .as_chunks::<4>()
                .0
                .iter()
                .any(|px| (px[0], px[1], px[2], px[3]) == (r, g, b, 0xff));
            assert!(
                letters_painted,
                "the letters are the fully-saturated colour, unmixed"
            );
        }

        #[test]
        fn a_chip_stands_clear_of_the_top_and_bottom_of_the_image() {
            let segs = [chip("FAB")];
            let (buf, w, h) = render(&segs, false, 0, true);
            let frame = chip_spans(&segs, 0)[0].span;
            for x in frame.x0..frame.x1.min(w) {
                assert_eq!(
                    alpha_at(&buf, w, x, 0),
                    0,
                    "column {x} touches the top edge"
                );
                assert_eq!(
                    alpha_at(&buf, w, x, h - 1),
                    0,
                    "column {x} touches the bottom edge"
                );
            }
        }

        #[test]
        fn each_group_colour_draws_in_its_own_ink_and_differs_by_appearance() {
            let all = [
                GroupColor::Teal,
                GroupColor::Blue,
                GroupColor::Violet,
                GroupColor::Amber,
                GroupColor::Red,
            ];
            for (i, a) in all.iter().enumerate() {
                assert_ne!(a.rgb(true), a.rgb(false), "{a:?} must answer to appearance");
                for b in &all[i + 1..] {
                    assert_ne!(a.rgb(true), b.rgb(true), "{a:?} and {b:?} must differ");
                }
            }
        }

        #[test]
        fn an_unknown_colour_name_falls_back_to_the_first_of_the_palette() {
            assert_eq!(GroupColor::parse("chartreuse"), GroupColor::Teal);
            assert_eq!(GroupColor::parse("violet"), GroupColor::Violet);
        }

        #[test]
        fn an_empty_slug_is_drawn_and_hit_tested_as_no_chip_at_all() {
            let segs = [chip(""), fig("55%", StatusItemColor::Neutral)];
            assert!(chip_spans(&segs, 0).is_empty());
            assert_eq!(
                render(&segs, false, 0, true).1,
                render(&[fig("55%", StatusItemColor::Neutral)], false, 0, true).1,
                "an empty slug reserves no width and costs no gap"
            );
        }

        #[test]
        fn a_chip_widens_the_image_by_its_own_frame_plus_one_gap() {
            let bare = render(&[fig("55%", StatusItemColor::Neutral)], false, 0, false);
            let with_chip = render(
                &[chip("FAB"), fig("55%", StatusItemColor::Neutral)],
                false,
                0,
                false,
            );
            let slug_font = text::load_ui_font(slug_font_size_pt());
            let (ink_min, ink_max) = text::ink_bounds(&slug_font, "FAB");
            let frame = (ink_max - ink_min + CHIP_PAD_X_PX * 2.0).round() as u32;

            assert!(with_chip.1 > bare.1, "a chip has to cost width");
            assert_eq!(with_chip.2, bare.2, "and no height");
            assert!(
                (with_chip.1 - bare.1).abs_diff(frame + ITEM_GAP_PX) <= 2,
                "the chip should cost its own frame plus one gap: {} vs {}",
                with_chip.1 - bare.1,
                frame + ITEM_GAP_PX
            );
        }
    }

    mod spacing {
        use super::*;

        #[test]
        fn every_gap_between_two_adjacent_items_is_the_same_one_constant() {
            for font in [
                text::load_font(text_font_size_pt()),
                text::load_font_forcing_fallback(text_font_size_pt()),
            ] {
                for segs in [
                    vec![
                        fig("2%", StatusItemColor::Neutral),
                        fig("74%", StatusItemColor::Neutral),
                        fig("100%", StatusItemColor::Neutral),
                    ],
                    vec![chip("FAB"), chip("CUR"), chip("MON")],
                    vec![
                        chip("FAB"),
                        fig("18%", StatusItemColor::Neutral),
                        fig("55%", StatusItemColor::Neutral),
                        chip("CUR"),
                        fig("9%", StatusItemColor::Neutral),
                    ],
                ] {
                    let edges = drawn_edges(&font, &segs);
                    let gaps: Vec<f64> = edges.windows(2).map(|w| w[1].0 - w[0].1).collect();
                    for gap in &gaps {
                        assert!(
                            (gap - ITEM_GAP_PX as f64).abs() < 1e-6,
                            "every item-to-item gap must equal ITEM_GAP_PX ({ITEM_GAP_PX}px), got {gaps:?}"
                        );
                    }
                }
            }
        }

        /// With no groups the tray is the row it was before groups
        /// existed: figures, one gap apart, nothing else.
        #[test]
        fn with_no_groups_the_row_is_plain_figures_one_gap_apart() {
            let segs = [
                fig("18%", StatusItemColor::Neutral),
                fig("55%", StatusItemColor::Neutral),
                fig("99%", StatusItemColor::Red),
                fig("47%", StatusItemColor::Neutral),
            ];
            assert!(chip_spans(&segs, 0).is_empty(), "no groups, no chips");
            let font = text::load_font(text_font_size_pt());
            let edges = drawn_edges(&font, &segs);
            for pair in edges.windows(2) {
                assert!((pair[1].0 - pair[0].1 - ITEM_GAP_PX as f64).abs() < 1e-6);
            }
        }

        #[test]
        fn the_glyph_to_first_item_gap_is_its_own_larger_constant() {
            for font in [
                text::load_font(text_font_size_pt()),
                text::load_font_forcing_fallback(text_font_size_pt()),
            ] {
                for segs in [
                    vec![fig("42%", StatusItemColor::Neutral)],
                    vec![chip("FAB")],
                ] {
                    let glyph_ink_right_edge = SIDE_PAD_PX as f64
                        + glyph_ink_right_edge_px(&glyph_coverage(GLYPH_PX, 0.0), GLYPH_PX);
                    let glyph_gap = drawn_edges(&font, &segs)[0].0 - glyph_ink_right_edge;
                    assert!(
                        (glyph_gap - GLYPH_GAP_PX as f64).abs() < 1e-6,
                        "the glyph-to-item gap must equal GLYPH_GAP_PX ({GLYPH_GAP_PX}px), got {glyph_gap}"
                    );
                }
            }
        }

        /// An item that draws nothing must not leave a gap behind it,
        /// which is what would put a hole in an otherwise even row.
        #[test]
        fn an_undrawable_item_leaves_the_rhythm_alone() {
            let with_hole = [
                fig("18%", StatusItemColor::Neutral),
                chip(""),
                fig("55%", StatusItemColor::Neutral),
            ];
            let without = [
                fig("18%", StatusItemColor::Neutral),
                fig("55%", StatusItemColor::Neutral),
            ];
            assert_eq!(
                render(&with_hole, false, 0, true).1,
                render(&without, false, 0, true).1
            );
        }
    }

    mod clicks {
        use super::*;

        #[test]
        fn chip_at_folds_the_group_whose_chip_was_clicked() {
            let segs = [
                chip("FAB"),
                fig("18%", StatusItemColor::Neutral),
                chip("CUR"),
            ];
            let spans = chip_spans(&segs, 0);
            let (_, width, _) = render(&segs, false, 0, false);

            assert_eq!(spans.len(), 2);
            for chip in &spans {
                for x in [
                    chip.span.x0 as f64,
                    (chip.span.x0 + chip.span.x1) as f64 / 2.0,
                    chip.span.x1 as f64,
                ] {
                    assert_eq!(
                        chip_at(&spans, width, x),
                        Some(chip.segment),
                        "pixel {x} is chip {}'s own frame",
                        chip.segment
                    );
                }
            }
        }

        /// The figure is not a button: a click on the digits opens the
        /// panel like any other click on the item.
        #[test]
        fn a_click_on_a_bare_figure_folds_nothing() {
            let segs = [chip("FAB"), fig("55%", StatusItemColor::Neutral)];
            let chips = chip_spans(&segs, 0);
            let figure = figure_spans(&segs, 0)[0];
            let (_, width, _) = render(&segs, false, 0, false);

            for x in [
                figure.x0 as f64 + HIT_PADDING_PX as f64 + 1.0,
                (figure.x0 + figure.x1) as f64 / 2.0,
                figure.x1 as f64,
            ] {
                assert_eq!(chip_at(&chips, width, x), None);
            }
        }

        #[test]
        fn a_click_on_the_glyph_or_past_either_end_folds_nothing() {
            let segs = [chip("FAB")];
            let spans = chip_spans(&segs, 0);
            let (_, width, _) = render(&segs, false, 0, false);

            assert_eq!(
                chip_at(&spans, width, (SIDE_PAD_PX + GLYPH_PX / 2) as f64),
                None
            );
            assert_eq!(chip_at(&spans, width, -0.1), None);
            assert_eq!(chip_at(&spans, width, width as f64 + 0.1), None);
        }

        #[test]
        fn a_click_just_off_a_chip_still_folds_its_group() {
            let segs = [chip("FAB")];
            let spans = chip_spans(&segs, 0);
            let (_, width, _) = render(&segs, false, 0, false);
            for off in 1..HIT_PADDING_PX {
                assert_eq!(
                    chip_at(&spans, width, spans[0].span.x0 as f64 - off as f64),
                    Some(0)
                );
            }
        }

        #[test]
        fn chip_at_finds_nothing_when_nothing_is_drawn() {
            let width = SIDE_PAD_PX * 2 + GLYPH_PX;
            assert_eq!(chip_at(&[], width, width as f64 / 2.0), None);
            assert!(chip_spans(&[], 0).is_empty());
        }

        #[test]
        fn two_chips_never_claim_the_same_pixel_even_padded() {
            let segs = [chip("FAB"), chip("CUR")];
            let spans = chip_spans(&segs, 0);
            assert!(
                spans[0].span.x1 + HIT_PADDING_PX < spans[1].span.x0.saturating_sub(HIT_PADDING_PX),
                "padded chip spans must stay disjoint: {spans:?}"
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
            spans: Vec<ChipSpan>,
            icon_width_px: u32,
            item_width_points: f64,
        }

        fn scene() -> Scene {
            let segments = vec![
                chip("FAB"),
                fig("18%", StatusItemColor::Neutral),
                fig("55%", StatusItemColor::Neutral),
                chip("CUR"),
                chip("MON"),
                fig("12%", StatusItemColor::Neutral),
            ];
            let spans = chip_spans(&segments, 50);
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
            chip_at(
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
            chip_at(
                &scene.spans,
                scene.icon_width_px,
                fraction * scene.icon_width_px as f64,
            )
        }

        #[test]
        fn a_click_anywhere_on_a_chip_folds_that_chips_own_group() {
            let scene = scene();
            for chip in &scene.spans {
                for icon_px in [
                    chip.span.x0 as f64,
                    (chip.span.x0 + chip.span.x1) as f64 / 2.0,
                    chip.span.x1 as f64,
                ] {
                    assert_eq!(
                        resolve(&scene, click_landing_on(icon_px)),
                        Some(chip.segment),
                        "icon pixel {icon_px} is chip {}'s own frame",
                        chip.segment
                    );
                }
            }
        }

        #[test]
        fn the_bare_fraction_of_the_buttons_width_misses_the_chip_clicked() {
            let scene = scene();
            let goes_astray = scene.spans.iter().any(|chip| {
                (chip.span.x0..=chip.span.x1).any(|icon_px| {
                    let click = click_landing_on(icon_px as f64);
                    resolve(&scene, click) == Some(chip.segment)
                        && resolve_by_bare_fraction(&scene, click) != Some(chip.segment)
                })
            });
            assert!(
                goes_astray,
                "the margin-blind arithmetic has to send at least one click on a chip's own frame somewhere else; that is the bug it caused"
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

        /// Through the same coordinate space the chips are resolved in:
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
            chip("FAB"),
            fig("67%", StatusItemColor::Neutral),
            fig("96%", StatusItemColor::Red),
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
        assert!(
            alpha_at(&buf, w, 2, mid_row) > 0,
            "highlight must cover the left padding"
        );
        assert!(
            alpha_at(&buf, w, w - 3, mid_row) > 0,
            "highlight must cover the right padding"
        );
        let (bare, _, _) = render(&[], false, 0, false);
        assert_eq!(
            alpha_at(&bare, w, 2, mid_row),
            0,
            "unhighlighted padding stays fully transparent"
        );
    }

    #[test]
    fn render_grows_width_per_segment_and_never_touches_height() {
        let one = render(&[fig("2%", StatusItemColor::Neutral)], false, 0, false);
        let two = render(
            &[
                fig("2%", StatusItemColor::Neutral),
                fig("78%", StatusItemColor::Amber),
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
        let one_digit = render(&[fig("9%", StatusItemColor::Neutral)], false, 0, false);
        let two_digit = render(&[fig("42%", StatusItemColor::Neutral)], false, 0, false);
        let three_digit = render(&[fig("100%", StatusItemColor::Neutral)], false, 0, false);
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
                fig("42%", StatusItemColor::Neutral),
                fig("50%", StatusItemColor::Neutral),
            ],
            false,
            0,
            false,
        );
        let b = render(
            &[
                fig("77%", StatusItemColor::Neutral),
                fig("50%", StatusItemColor::Neutral),
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
                fig("9%", StatusItemColor::Neutral),
                fig("50%", StatusItemColor::Neutral),
            ],
            false,
            0,
            false,
        );
        let wide_first = render(
            &[
                fig("100%", StatusItemColor::Neutral),
                fig("50%", StatusItemColor::Neutral),
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
                fig("51%", StatusItemColor::Neutral),
                fig(text, StatusItemColor::Neutral),
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
        let (buf, w, h) = render(&[fig("78%", StatusItemColor::Red)], false, 0, false);
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
        let (buf, w, h) = render(&[fig("!", StatusItemColor::Red)], false, 0, false);
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
        let edge_alpha = alpha_at(&buf, w, 2, h / 2);
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
        let (buf, w, h) = render(&[fig("78%", StatusItemColor::Red)], true, 0, false);
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
                fig("51%", StatusItemColor::Neutral),
                fig("0%", StatusItemColor::Neutral),
            ],
            false,
            0,
            false,
        );
        let mut last_ink_x = 0u32;
        for y in 0..h {
            for x in 0..w {
                if alpha_at(&buf, w, x, y) > 0 {
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
    fn the_glyphs_own_ink_is_unaffected_by_whether_items_follow_it() {
        let bare = render(&[], false, 50, false);
        let with_items = render(
            &[
                chip("FAB"),
                fig("67%", StatusItemColor::Neutral),
                fig("96%", StatusItemColor::Red),
            ],
            false,
            50,
            false,
        );
        let glyph_region = |buf: &[u8], w: u32| -> Vec<u8> {
            (0..GLYPH_PX)
                .flat_map(|y| (0..GLYPH_PX).map(move |x| (y, x)))
                .map(|(y, x)| alpha_at(buf, w, SIDE_PAD_PX + x, y))
                .collect()
        };
        assert_eq!(
            glyph_region(&bare.0, bare.1),
            glyph_region(&with_items.0, with_items.1),
            "the glyph's own drawn pixels must not change when items are appended after it"
        );
    }
}
