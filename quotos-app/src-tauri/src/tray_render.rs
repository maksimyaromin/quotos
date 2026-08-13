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

use std::process::Command;
use std::sync::OnceLock;

const GLYPH_PNG: &[u8] = include_bytes!("../icons/tray/tray-icon.png");

/// The crate always requests an 18pt button height; rendering at 2x that
/// keeps the composited bitmap crisp on Retina (matches the design system's
/// own "18x18 CSS-px, 36x36 @2x" spec for the glyph).
const GLYPH_PX: u32 = 36;
const GAP_PX: u32 = 10; // 5pt gap between glyph and digits, at 2x
const SEGMENT_GAP_PX: u32 = 8; // 4pt gap between separately-pinned segments, at 2x
const TEXT_SCALE: u32 = 4; // each font pixel becomes a TEXT_SCALE x TEXT_SCALE block

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

/// 3x5 pixel font, bits packed MSB-left per row. Covers exactly what tray
/// digits ever need: 0-9, '%', '!' (the "broken" mark), and space.
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

const CHAR_W: u32 = 3;
const CHAR_H: u32 = 5;

fn text_width(text: &str) -> u32 {
    let n = text.chars().count() as u32;
    if n == 0 {
        return 0;
    }
    n * CHAR_W * TEXT_SCALE + (n - 1) * TEXT_SCALE
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

fn draw_text(buf: &mut [u8], buf_w: u32, buf_h: u32, x0: u32, text: &str, rgba: (u8, u8, u8, u8)) {
    let y0 = (buf_h.saturating_sub(CHAR_H * TEXT_SCALE)) / 2;
    let mut x = x0;
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
                        put_pixel(
                            buf,
                            buf_w,
                            buf_h,
                            x + col * TEXT_SCALE + sx,
                            y0 + row as u32 * TEXT_SCALE + sy,
                            rgba,
                        );
                    }
                }
            }
        }
        x += CHAR_W * TEXT_SCALE;
    }
}

/// Composites the glyph plus every segment's colored digits into one RGBA
/// buffer. Returns `(rgba, width, height)`. `segments` empty still draws the
/// bare glyph (callers that truly have nothing pinned should prefer the
/// cheaper template-icon path in `lib.rs` instead of calling this).
pub fn render(segments: &[TraySegment]) -> (Vec<u8>, u32, u32) {
    let dark = is_dark_mode();
    let g = glyph();

    let widths: Vec<u32> = segments.iter().map(|s| text_width(&s.text)).collect();
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
        draw_text(&mut buf, total_w, total_h, x, &seg.text, seg.color.rgba(dark));
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
    fn a_broken_mark_renders_some_red_pixels() {
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
}
