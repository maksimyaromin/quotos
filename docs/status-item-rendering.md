# Status item rendering

<img src="images/menu-bar.png" alt="The composited status item in the menu bar: the capacity glyph followed by two accounts' percentage figures, separated by a hairline">

`status_item_render.rs` composites the status item's glyph and colored
percentage digits into a raw RGBA buffer. `tray-icon` 0.24.2's macOS
`set_title` calls `NSStatusItem`'s button `setTitle:` with a plain
`NSString`, with no attributed-string or color path anywhere in the
crate's public API, so the only route to a colored digit is to paint it
directly and hand macOS a finished bitmap through `set_icon`.

`set_icon_for_ns_status_item_button` always asks for an 18pt-tall
`NSImage` regardless of the source bitmap's own pixel size, so supplying
a denser buffer than 18x18 is what keeps the result crisp on a Retina
menu bar. Digits render as real system text through Core Text; see
"Text rendering" below for the non-macOS fallback.

## Layout padding

Horizontal air on each side of the glyph and digits, inside the
composited image, is not optional. The "panel open" highlight paints
across the whole buffer, so without padding it hugs the ink and reads
as a box drawn around the glyph rather than a pressed menu bar button,
the way macOS fills a status item's own width for its own open items.
The padding also applies unconditionally, highlighted or not, so the
glyph's position inside the item cannot shift when the highlight
toggles; a shift there would move the beak too.

`geometry.rs`'s `glyph_center_offset_from_item_left_points` reads
`GLYPH_LEFT_INSET_POINTS` rather than assuming the glyph sits at the
image's leftmost 18pt.

## The capacity glyph

`glyph_coverage` computes per-pixel alpha coverage for the
capacity-gauge mark procedurally from its exact vector geometry,
`src/design-system/assets/menubar-glyph.svg`: a faint full-circle track
plus a bold, round-capped arc with a gap. Drawing the exact shape at
the target resolution means there is no raster source to be too small
or too soft, and the target ink size, `TARGET_INK_DIAMETER_CSS_PX`, is
a direct, tunable parameter instead of whatever a fixed asset happened
to contain.

The SVG's path, `M4.46 12.02 a5.4 5.4 0 1 1 7.08 0`, was converted to
the two gap-endpoint angles in the code by hand: the vector from the
center `(8,8)` to each endpoint, through `atan2`. Both are in the
module's plain math convention, y-down, 0 at the +x axis, which already
matches the SVG's own y-down convention with no flip needed, since this
buffer is top-left-origin throughout.

The mark reads as a Q, not an O: a ring with a gap, plus a second
stroke, the tail, through the gap's own diagonal, which is what a
counter needs to read as a Q rather than a bare ring. The gap is 62
degrees wide, centered at 45 degrees, lower-right, where the tail sits.
The tail is a second capsule, radius 3.2 to 8.0 along that same
45-degree diagonal, same stroke weight as the arc, round caps, drawn
unconditionally, even at 0% used, since the tail is what reads as a Q
rather than an O regardless of fill state. Its own reach at 45 degrees,
outer radius plus half-stroke, projected onto either axis, stays just
inside the ring's own axis-aligned reach, so it needs no separate
accounting in `natural_outer_diameter` or `scale`. The arc's own end
angle is data, `used_fraction`, clamped to `0.0..=1.0`, rather than a
constant 100%. It starts right where the gap ends, sweeps forward by
`used_fraction` of the maximum possible sweep, `TAU` minus the gap's
own width, and lands exactly on the gap's other edge at 100%.

## Spacing the figures evenly

The rule: every horizontal gap in the item is a fixed distance from one
shape's own rendered ink to the next shape's own rendered ink, never
from either shape's wider advance box or reserved cell. `GLYPH_GAP_PX`
is that distance from the glyph to the first figure, `FIGURE_GAP_PX`
from one figure to the next, and `GROUP_GUTTER_PRE_PX`/
`GROUP_GUTTER_POST_PX` from a figure to the hairline marking a new
subscription group, in place of the plain figure gap that boundary
would otherwise have gotten. `layout_figures` computes every position
by this one rule; nothing downstream hand-adjusts a value it produces.

An earlier design measured a figure's own advance box, the space
`CTLineGetTypographicBounds` reports a run occupies, and stepped a
fixed constant from box edge to box edge. That looked uneven because a
glyph's ink sits inside its advance box with its own left and right
side bearing, and that bearing differs per glyph even in a tabular
font: a leading `1` and a leading `3` carry different amounts of empty
space before their own ink starts. Since every figure gap sits between
a `%` and the next figure's leading digit, and the `%` glyph's own
right bearing is constant while the leading digit's left bearing is
not, a fixed advance-to-advance step produced a different visible gap
depending on which digit happened to lead the next figure, even though
the constant never changed. Measuring from ink to ink removes the
bearing from the arithmetic entirely, so the constant is the whole
story and the visible gap no longer depends on which digits are
adjacent.

A figure's own ink bounds come from `CTLineGetImageBounds`, the tight
box CoreText actually draws, not the wider box `measure` returns for
sizing. The glyph has no advance box to begin with, since it is
painted procedurally rather than laid out as text, so its own ink right
edge is scanned directly from its rendered coverage buffer: the
rightmost pixel whose coverage crosses half, matching where
antialiasing places the visible edge to the eye. `layout_figures`
threads an ink cursor through the glyph and every figure in order, each
one's origin computed from the previous shape's own ink edge plus that
boundary's constant minus the new shape's own leading bearing, so the
canvas width the buffer is sized to is exactly where the last figure's
own ink actually ends, not a reserved slot that may not agree with it.
Because that final origin is rarely a whole pixel, `TextOrigin` carries
the leftover fraction into CoreText's own text position, so the ink
lands at the exact position the arithmetic calls for rather than
snapping to the nearest device pixel.

`GLYPH_GAP_PX` is a separate, larger constant than `FIGURE_GAP_PX`,
not the same value applied a second time. A mark is a different kind
of shape than a digit, and the eye expects a wider break between a
symbol and a run of numbers than between two numbers; where that
expectation and a shared constant would disagree, the wider break
wins. Its value, like `FIGURE_GAP_PX`'s, is chosen by looking at
rendered output at several value sets, not derived from another
constant.

The trade against the ink measurement: a leading digit's own bearing
now nudges everything after it by a sub-pixel amount when that digit
changes, even at the same digit count, since the ink cursor is
threaded through every figure's own measured bearing rather than a
digit-count-only advance. That shift is bounded by the spread between
two digits' own side bearings, under a point, well under what a digit
count actually changing moves things by; the alternative, positioning
from the advance box, is exactly the uneven-gap bug this rule replaces.
The glyph itself stays put regardless of any of this, since `render`
draws it at a fixed offset before the ink cursor exists, so
`geometry.rs`'s `glyph_center_offset_from_item_left_points` and the
beak it derives are unaffected by anything happening to its right.

A boundary between two different subscriptions' figure groups draws a
hairline between the two gutters: 5-CSS-px, a 1-CSS-px hairline, then
another 5-CSS-px, doubled for this buffer's 2x convention. `render`
never draws a hairline before the very first segment overall,
regardless of what the frontend sets on it, since there is no prior
group for the first segment to part from.

## Compositing

The buffer can carry two translucent layers, the "panel open" highlight
and then the glyph or digits drawn over it, so a plain overwrite would
discard whichever layer drew second wherever they overlap, losing the
highlight everywhere the glyph or a digit covers it. `blend_pixel` does
standard src-over alpha compositing instead. A fully-opaque source or a
fully-transparent destination pixel reduces to a plain overwrite, so a
single-layer caller, such as glyph ink or digit text onto a blank
buffer, is unaffected; only a highlighted, multi-layer case exercises
the blend math.

## Text rendering

Digits render through Core Text and CoreGraphics, reached through the
`objc2` bindings Tauri already pulls in transitively, since real system
text is sharper than a hand-rolled bitmap font next to Apple's own
menu-bar text. A non-macOS fallback module renders a 3x5 pixel bitmap
font instead, gated `cfg(not(target_os = "macos"))` so a non-mac `cargo
check` still compiles; this crate only ever ships for macOS as a menu
bar app.

`load_font` tries the MonoLisa family first and falls back to the
system's tabular-figure UI font, `NSFont.monospacedDigitSystemFontOfSize:weight:`,
if MonoLisa is not installed, with no caching across calls. Font
matching happens at most once a minute, cheap enough that staying
stateless sidesteps `CFRetained` and `Retained` not being `Send + Sync`:
Core Foundation and AppKit object wrappers are not declared thread-safe
for storage in a shared `static`, even though the lookups themselves
are safe to call off the main thread, which matters because
`set_status_item_state` runs inside a Tauri command handler with no
guarantee of running on the main thread. `try_load_monolisa` checks
availability up front through `CTFontManagerCopyAvailableFontFamilyNames`
and double-checks after creation by comparing the resolved font's own
family name, since `CTFontCreateWithFontDescriptor` never returns null
and silently substitutes a default font on a mismatch instead.

CI has no MonoLisa license to install, so every run there exercises the
fallback path regardless of test intent; a developer's own machine, with
MonoLisa installed, never does on its own. A test asserting a layout
property that is supposed to hold under either font calls
`load_font_forcing_fallback`, a `#[cfg(test)]` path that skips
`try_load_monolisa` outright, so that property is proven on a MonoLisa
machine too rather than only on the next CI run.

`make_line` builds a `CTLine` through `CTLineCreateWithAttributedString`
plus `CTLineDraw`, CoreText's own standard path for drawing a short text
run, rather than manually resolving glyph IDs and advances through
`CTFontGetGlyphsForCharacters`, `CTFontGetAdvancesForGlyphs`, and
`CTFontDrawGlyphs`. That lower-level path produces specific glyphs with
wrong or incomplete outlines on this font, OS, and binding combination,
for example a "7" missing its top bar while an "8" right next to it,
same font, same call sequence, renders perfectly. The corruption is not
a premultiply or coordinate-flip issue, since it appears identically
with the system fallback font and with a straight RGBA context.

The fill color and the bitmap context both use sRGB explicitly, not
generic RGB. Pairing a Generic-RGB fill color with a Device-RGB bitmap
context, which do not share a gamma curve, shifts even fully-opaque
glyph-interior pixels well off the requested color: a requested
`(229,100,106)` can come back as `(236,123,125)`, a shift of more than
20 on the green and blue channels, not just antialiasing fuzz.
`StatusItemColor::rgba`'s values are plain CSS hex tokens, already
sRGB by convention, so both sides agree.

`draw_text_impl` deliberately does not flip the bitmap context's CTM,
the usual translate-plus-`scale(1,-1)` trick that turns a
`CGBitmapContext`'s bottom-left/y-up default into top-left/y-down.
Doing so mirrors the glyphs themselves vertically, since `CTLineDraw`
and `CTFontDrawGlyphs` orient glyph outlines relative to the CTM's
handedness rather than compensating for it; the alternative fix would
be a flipped `CGContextSetTextMatrix`, but it is simpler to draw in the
context's native, bottom-left/y-up, convention and account for that in
the baseline math instead. `CGBitmapContextCreateWithData`'s backing
memory is always laid out top-row-first regardless of the drawing CTM,
a fixed property of the pixel buffer rather than of how it is drawn
into, so the row-major copy loop needs no inversion either way; only
the baseline math accounts for native y-up.
