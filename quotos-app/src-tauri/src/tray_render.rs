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

const GLYPH_PNG: &[u8] = include_bytes!("../icons/tray/tray-icon.png");

/// The crate always requests an 18pt button height; rendering at 2x that
/// keeps the composited bitmap crisp on Retina (matches the design system's
/// own "18x18 CSS-px, 36x36 @2x" spec for the glyph).
const GLYPH_PX: u32 = 36;
const GAP_PX: u32 = 10; // 5pt gap between glyph and digits, at 2x
const SEGMENT_GAP_PX: u32 = 8; // 4pt gap between separately-pinned segments, at 2x

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
            // --amber (design/system/tokens/colors.css) — same in light and dark.
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
}

struct DecodedGlyph {
    width: u32,
    height: u32,
    /// Alpha-only mask: the bundled tray glyph is a template image (a
    /// uniform-color silhouette carried entirely in the alpha channel), so
    /// alpha alone is enough to know which pixels are "ink".
    alpha: Vec<u8>,
}

fn glyph() -> &'static DecodedGlyph {
    use std::sync::OnceLock;
    static GLYPH: OnceLock<DecodedGlyph> = OnceLock::new();
    GLYPH.get_or_init(|| {
        let img = tauri::image::Image::from_bytes(GLYPH_PNG).expect("bundled tray glyph must decode");
        let rgba = img.rgba();
        let mut alpha = Vec::with_capacity((img.width() * img.height()) as usize);
        for px in rgba.chunks_exact(4) {
            alpha.push(px[3]);
        }
        DecodedGlyph { width: img.width(), height: img.height(), alpha }
    })
}

/// Read-only check of the current menu bar appearance. `defaults read` is a
/// read of user preferences, not a write — it changes nothing on the
/// machine. Absence of the key (the default light-mode case) makes the
/// command fail, which `unwrap_or(false)` correctly treats as "not dark".
fn is_dark_mode() -> bool {
    Command::new("defaults")
        .args(["read", "-g", "AppleInterfaceStyle"])
        .output()
        .map(|o| o.status.success() && String::from_utf8_lossy(&o.stdout).trim().eq_ignore_ascii_case("dark"))
        .unwrap_or(false)
}

fn put_pixel(buf: &mut [u8], w: u32, h: u32, x: u32, y: u32, rgba: (u8, u8, u8, u8)) {
    if x >= w || y >= h {
        return;
    }
    let idx = ((y * w + x) * 4) as usize;
    buf[idx] = rgba.0;
    buf[idx + 1] = rgba.1;
    buf[idx + 2] = rgba.2;
    buf[idx + 3] = rgba.3;
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
fn composite_mask(buf: &mut [u8], buf_w: u32, buf_h: u32, x0: u32, mask: &[u8], mask_w: u32, rgba: (u8, u8, u8, u8)) {
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
            put_pixel(buf, buf_w, buf_h, x0 + x, y, (rgba.0, rgba.1, rgba.2, alpha));
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
    use super::put_pixel;
    use objc2::rc::Retained;
    use objc2_app_kit::{NSFont, NSFontWeightMedium};
    use objc2_core_foundation::{CFArray, CFAttributedString, CFDictionary, CFNumber, CFRetained, CFString, CFType};
    use objc2_core_graphics::{
        kCGColorSpaceSRGB, CGBitmapContextCreateWithData, CGColor, CGColorSpace, CGContext, CGImageAlphaInfo,
    };
    use objc2_core_text::{
        kCTFontAttributeName, kCTFontFamilyNameAttribute, kCTForegroundColorAttributeName, kCTFontTraitsAttribute,
        kCTFontWeightTrait, CTFont, CTFontDescriptor, CTFontManagerCopyAvailableFontFamilyNames, CTLine,
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
            &[unsafe { kCTFontFamilyNameAttribute }, unsafe { kCTFontTraitsAttribute }],
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
            return LoadedFont { handle: FontHandle::Mono(font), used_fallback: false };
        }
        let font = NSFont::monospacedDigitSystemFontOfSize_weight(size_pt, medium_weight());
        LoadedFont { handle: FontHandle::Fallback(font), used_fallback: true }
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
            &[unsafe { kCTFontAttributeName }, unsafe { kCTForegroundColorAttributeName }],
            &[font.as_ref(), color.as_ref()],
        );
        let attr_string = unsafe { CFAttributedString::new(None, Some(&string), Some(attrs.as_opaque())) }?;
        Some(unsafe { CTLine::with_attributed_string(&attr_string) })
    }

    /// Renders `text` with `font` in color `rgba` and composites it onto
    /// `buf` at horizontal offset `x0` (buffer height `buf_h`), returning
    /// the pixel width it occupied so callers can lay out the next segment
    /// after it.
    fn draw_text_impl(buf: &mut [u8], buf_w: u32, buf_h: u32, x0: u32, font: &CTFont, text: &str, rgba: (u8, u8, u8, u8)) -> u32 {
        let Some(line) = make_line(font, text, rgba) else {
            return 0;
        };
        let line_width = unsafe { line.typographic_bounds(std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut()) };
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
                let unpremul = |c: u8| ((c as u32 * 255 + (a as u32 / 2)) / a as u32).min(255) as u8;
                put_pixel(buf, buf_w, buf_h, x0 + x, y, (unpremul(pixels[idx]), unpremul(pixels[idx + 1]), unpremul(pixels[idx + 2]), a));
            }
        }

        width
    }

    /// Renders `text` and composites it onto `buf` at horizontal offset
    /// `x0`, returning the pixel width it occupied (so callers can lay out
    /// the next segment after it).
    pub fn draw_text(buf: &mut [u8], buf_w: u32, buf_h: u32, x0: u32, font: &LoadedFont, text: &str, rgba: (u8, u8, u8, u8)) -> u32 {
        draw_text_impl(buf, buf_w, buf_h, x0, font.handle.as_ct_font(), text, rgba)
    }

    /// Measures `text` without drawing it (used by `render()` to lay out
    /// segments before the final buffer is allocated).
    pub fn measure(font: &LoadedFont, text: &str) -> u32 {
        let Some(line) = make_line(font.handle.as_ct_font(), text, (0, 0, 0, 0)) else {
            return 0;
        };
        let width = unsafe { line.typographic_bounds(std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut()) };
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

    pub fn draw_text(buf: &mut [u8], buf_w: u32, buf_h: u32, x0: u32, _font: &LoadedFont, text: &str, rgba: (u8, u8, u8, u8)) -> u32 {
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
/// cheaper template-icon path in `lib.rs` instead of calling this).
pub fn render(segments: &[TraySegment]) -> (Vec<u8>, u32, u32) {
    let dark = is_dark_mode();
    let g = glyph();
    let font = text::load_font(text_font_size_pt());

    let widths: Vec<u32> = segments.iter().map(|s| text::measure(&font, &s.text)).collect();
    let text_total: u32 = if segments.is_empty() {
        0
    } else {
        widths.iter().sum::<u32>() + (segments.len() as u32 - 1) * SEGMENT_GAP_PX
    };
    let total_w = GLYPH_PX + if text_total > 0 { GAP_PX + text_total } else { 0 };
    let total_h = GLYPH_PX;
    let mut buf = vec![0u8; (total_w * total_h * 4) as usize];

    // The glyph itself always stays neutral — only the digits carry
    // severity/staleness color. Nearest-neighbor sampled from the source
    // asset's native resolution; only ever visible while colored digits are
    // shown (i.e. not the common/quiet case), so a little softness here is
    // an acceptable trade for not carrying a second higher-res glyph asset.
    let ink = TrayColor::Neutral.rgba(dark);
    for y in 0..GLYPH_PX {
        for x in 0..GLYPH_PX {
            let sx = x * g.width / GLYPH_PX;
            let sy = y * g.height / GLYPH_PX;
            let a = g.alpha[(sy * g.width + sx) as usize];
            if a == 0 {
                continue;
            }
            let blended = ((ink.3 as u16 * a as u16) / 255) as u8;
            put_pixel(&mut buf, total_w, total_h, x, y, (ink.0, ink.1, ink.2, blended));
        }
    }

    let mut x = GLYPH_PX + GAP_PX;
    for (seg, w) in segments.iter().zip(widths.iter()) {
        text::draw_text(&mut buf, total_w, total_h, x, &font, &seg.text, seg.color.rgba(dark));
        x += w + SEGMENT_GAP_PX;
    }

    (buf, total_w, total_h)
}

/// The bare glyph alone (no digits), for reverting to the quiet state. Kept
/// as raw RGBA + template flag rather than the original PNG bytes so both
/// paths share the exact same decoded source.
pub fn plain_glyph_rgba() -> (Vec<u8>, u32, u32) {
    let g = glyph();
    (g.alpha.iter().flat_map(|&a| [0xffu8, 0xff, 0xff, a]).collect(), g.width, g.height)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_glyph_decodes_and_is_square_with_some_ink() {
        let g = glyph();
        assert_eq!(g.width, g.height, "tray glyph asset should be square");
        assert!(g.alpha.iter().any(|&a| a > 0), "glyph must have some opaque pixels");
    }

    #[test]
    fn render_with_no_segments_is_exactly_the_glyph_square() {
        let (buf, w, h) = render(&[]);
        assert_eq!((w, h), (GLYPH_PX, GLYPH_PX));
        assert_eq!(buf.len(), (w * h * 4) as usize);
    }

    #[test]
    fn render_grows_width_per_segment_and_never_touches_height() {
        let one = render(&[TraySegment { text: "2%".into(), color: TrayColor::Neutral }]);
        let two = render(&[
            TraySegment { text: "2%".into(), color: TrayColor::Neutral },
            TraySegment { text: "78%".into(), color: TrayColor::Amber },
        ]);
        assert!(one.1 > GLYPH_PX, "adding a segment must widen the image beyond the bare glyph");
        assert!(two.1 > one.1, "a second segment must widen it further");
        assert_eq!(one.2, GLYPH_PX);
        assert_eq!(two.2, GLYPH_PX);
    }

    #[test]
    fn a_digit_and_percent_segment_renders_some_exact_colored_pixels() {
        let (buf, w, h) = render(&[TraySegment { text: "78%".into(), color: TrayColor::Red }]);
        let red = TrayColor::Red.rgba(true);
        let found = buf.chunks_exact(4).any(|px| (px[0], px[1], px[2], px[3]) == red);
        assert!(found, "expected at least one pixel painted in the red channel across a {w}x{h} buffer");
    }

    #[test]
    fn a_broken_mark_renders_some_red_pixels() {
        // '!' is very likely dead in practice (CLAUDE.md: a broken pin
        // contributes no segment at all now), but should still render
        // correctly rather than being special-cased away.
        let (buf, w, h) = render(&[TraySegment { text: "!".into(), color: TrayColor::Red }]);
        let red = TrayColor::Red.rgba(true);
        let found = buf.chunks_exact(4).any(|px| (px[0], px[1], px[2], px[3]) == red);
        assert!(found, "expected at least one pixel painted in the red channel across a {w}x{h} buffer");
    }

    #[test]
    fn neutral_color_differs_between_dark_and_light() {
        assert_ne!(TrayColor::Neutral.rgba(true), TrayColor::Neutral.rgba(false));
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
        let one = render(&[TraySegment { text: "1".into(), color: TrayColor::Red }]);
        let eight = render(&[TraySegment { text: "8".into(), color: TrayColor::Red }]);
        assert_eq!(one.1, eight.1, "'1' and '8' must render at the same width (tabular figures)");
    }

    #[test]
    fn font_fallback_detection_does_not_panic() {
        // Machine-dependent (whether MonoLisa is actually installed), so this
        // only asserts the lookup completes cleanly and returns a bool either
        // way — not which branch was taken.
        let _ = used_fallback_font();
    }
}

