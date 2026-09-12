# Status item rendering

<img src="images/menu-bar.png" width="353" alt="The composited status item: the capacity glyph, an opened group's chip and its two figures, a rolled-up group's chip on its own, and one standalone pin's figure">

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
composited image, is not optional: `SIDE_PAD_PX` is what keeps the mark
and the trailing figure off the item's own edges, and so off the icons
either side of it in the menu bar.

Nothing else is painted in this buffer. The frame behind an open panel
is AppKit's own, which it draws around the item rather than inside the
image; see "The panel-open highlight" in
[platform-constraints.md](platform-constraints.md). A pill drawn in here
would sit inside that frame as a second, smaller one.

`geometry.rs`'s `glyph_center_offset_from_item_left_points` reads
`GLYPH_LEFT_INSET_POINTS` and `GLYPH_WIDTH_POINTS` rather than
assuming either, and the beak it derives follows the glyph's own box
whatever that box is resized to.

## The capacity glyph

`glyph_coverage` computes per-pixel alpha coverage for the brand mark
procedurally from its exact vector geometry,
`src/design-system/assets/menubar-glyph.svg`: three chevrons on a
28-unit grid, 2.6 units of stroke, round caps and joins. Drawing the
exact shape at the target resolution means there is no raster source to
be too small or too soft, and the target ink size,
`TARGET_INK_HEIGHT_CSS_PX`, is a direct, tunable parameter instead of
whatever a fixed asset happened to contain. The mark's own ink box is
tall and narrow, 12.6 by 24.1 units, so the target is set on its height
and the width follows; `INK_CENTER_Y_SVG` is what centres it
vertically, since the mark's ink is not centred in its own grid.

The canvas is that ink box and not a square. `GLYPH_W_PX` is the mark's
own drawn width plus `GLYPH_BLEED_PX` of air on either side for the
antialiased edge, where a square 18pt canvas — the shape the round
capacity ring this replaced actually filled — stood a 8pt-wide mark in
nearly twice its own width of dead air, on top of `SIDE_PAD_PX`. Next
to Dropbox, 1Password, the battery and the wifi in a real menu bar,
that read as a small icon inside an oversized frame, which is the
complaint `the_glyph_canvas_is_the_marks_own_width_and_not_a_square`
now holds the line on. `TARGET_INK_HEIGHT_CSS_PX` is set against those
same neighbours: their own ink measures a little under half the bar's
height, and the mark, being the narrow one, sits at the top of that
band rather than the middle of it.

`glyph_coverage`'s own `AA_HALF_WIDTH_PX` is half a pixel, the width a
box filter covers, not the three-quarters it started at. At this size
the difference is not softness but geometry: three quarters of a pixel
of spill on either side of a stroke closes the 1.4-unit gap the mark
leaves between its peach chevron and the grey one under it, and the
three chevrons run together. Rasterising the source SVG at the size the
status item actually draws at is how that was measured, and is how a
change here gets checked against the mark rather than against taste.

The mark says two things at once, and only one of them is a gauge. The
two grey chevrons pointing up are spend rising: `SPEND_CHEVRONS`, drawn
at full colour whatever the reading is, identical at 0% and at 100%. The
one peach chevron pointing down is the limit pressing back, and it is
the gauge. It is drawn twice, the way the ring it replaced was: once as
a track at `LIMIT_TRACK_OPACITY`, at full extent, so the mark always
reads as three chevrons even at 0%, and once fully saturated over the
part `used_fraction` has reached. That opacity is set against this
image's own ground rather than against a white page: the menu bar is
dark and translucent, and a track much fainter than this one does not
read as a faded chevron there, it reads as a missing one, which takes a
third of the mark's height with it.

That bright pass runs from the chevron's own vertex, the bottom of its
wedge, up toward its two arm-tips, so at 0% only the vertex is lit and
at 100% the whole chevron is. `Chevron::nearest` is what clips it:
a pixel belongs to the fill when the nearest point on the chevron's
centreline is at or above the threshold `used_fraction` sets. Measuring
along the centreline rather than slicing the rendered stroke by y keeps
the cut perpendicular to the arm it lands on, and keeps the round join
at the vertex whole at 0%.

`used_fraction` is `icon_fill_percent`, whatever the frontend has
decided that should be: `lib/status-item-segments.ts`'s
`computeIconFillPercent` reads it from the one window a person has
designated, and otherwise from the arithmetic mean of every tracked
subscription's headline figure. Nothing in this module knows which.

`GlyphCoverage` carries those three layers separately rather than one
buffer, because the mark is not painted in a single colour; `draw_glyph`
stacks them, track then fill then spend. That is also why the status
item is never a template image any more: macOS would flatten the two
inks and the gauge to one tone. The colours come from `spend_rgb` and
`limit_rgb`, which mirror `tokens/colors.css`'s own `--spend` and
`--peach`, read per appearance at each repaint rather than watched for,
the same as every other colour in this image.

## What the item is made of

The item is the capacity glyph, then a row of items, left to right. An
item is one of two things, and there is no third: a pin group's **chip**,
its slug in a small rounded rectangle of the group's own colour, or one
pinned limit window's **figure**, a bare percentage. A group rolled up
is its chip and nothing else — no rolled-up number; opened out, the same
chip stays exactly where it was and its members' own figures follow it.
A standalone pin is a bare figure and is not a click target. With no
groups at all the row is nothing but figures, which is what the item was
before groups existed.

Nothing is ever drawn *between* two items. An earlier version parted
clusters with a vertical hairline; adjacent items are parted by spacing
alone, and the `no_dividers` tests hold the gap between every pair of
items empty so a divider cannot come back by accident.

### Crossing the IPC boundary

`shell.rs`'s `StatusItemSegmentDto` is the Rust side of that list, and
every field in it answers to the camelCase name `types/entities.ts`
sends, `groupId` included. Serde spells that with two attributes, not
one: `rename_all` on an enum renames its *variants*, and
`rename_all_fields` is what reaches the fields inside them. With only
the first, a chip's `group_id` silently stops matching, `segments`
fails to deserialize as a whole, and `set_status_item_state` is never
entered — so the menu bar freezes on whatever it last drew, from the
moment a person makes their first group, with nothing on screen to say
why. `shell.rs`'s own tests deserialize the frontend's exact payload
rather than one written in the Rust spelling, which is the only version
of that test that would have caught it.

## Spacing the items evenly

The rule: every horizontal gap in the item is a fixed distance from one
shape's own rendered edge to the next shape's own rendered edge, never
from either shape's wider advance box or reserved cell. An edge is a
figure's own ink, and a chip's own filled rectangle rather than the
letters inside it. `GLYPH_GAP_PX` is that distance from the glyph to the
first item; `ITEM_GAP_PX` is that distance between any two adjacent
items, whichever kinds they are — chip to chip, chip to figure, figure
to figure. There is one such constant and the spacing is uniform, which
is what makes the rhythm read as deliberate rather than as three
different rules meeting. `layout_segments` computes every position by
this one rule; nothing downstream hand-adjusts a value it produces.

An item with no text to draw — a group whose name yields no slug — is
placed nowhere and costs no gap, so an empty item cannot open a hole in
an otherwise even row.

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
antialiasing places the visible edge to the eye. `layout_segments`
threads a cursor through the glyph and then every item in order, each
one's origin computed from the previous shape's own drawn edge plus the
gap constant minus the new shape's own leading bearing, so the
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

## Resolving a click back to a chip

A click on a pin group's chip folds that group in place instead of
opening the panel, so a click has to be resolved back to the chip
underneath it. A click on a figure resolves to nothing and opens the
panel like any other click on the item: the figure reports a number,
the chip is the button. `chip_spans` reports the horizontal span each
chip's frame occupies, sharing `layout_segments` with `render` rather
than re-deriving the geometry, so the spans a click is tested against
are the ones the chips were drawn at. `shell.rs` computes them once per
repaint and caches them; nothing measures anything at click time.

`chip_at` takes the click as a coordinate in this buffer's own pixels,
which is the space the spans are reported in; `geometry.rs`'s
`click_x_in_icon_px` is what puts a click there, and "Clicking a
group's chip in the menu bar" in
[architecture.md](architecture.md#clicking-a-groups-chip-in-the-menu-bar)
covers why the item's own width cannot stand in for the image's.

`HIT_PADDING_PX` widens every span a little before the test, because a
click a point or two shy of a chip clearly still means that group. A
compile-time assertion keeps two padded spans from meeting in the gap
between them, so a forgiving edge never claims the chip beside it.

Opening a group must not move its chip. The chip is the click target,
and a target that jumped out from under the pointer on its own click is
one nobody could hit twice; the layout puts every item after the chip,
never before it, so opening a group only ever appends.

## The group chip

A group is named in the menu bar rather than tinted there: the first
three characters of its name, uppercased, in a chip of its own colour.
The slug is derived wherever it is drawn or hit-tested, never stored;
`lib/pin-groups.ts`'s `groupSlug` is the one derivation, and the segment
list carries its result down.

The chip follows the badge language the panel already has, in
`design-system/components/indicators/badge.module.css`: a muted tint of
the colour as the fill, the fully-saturated colour as the letters, in a
small rounded rectangle. `CHIP_RADIUS_PX` is that file's `--radius-xs`
read as points at this buffer's 2x; `CHIP_HEIGHT_PX` and
`CHIP_PAD_X_PX` keep the badge's own proportions against a 22pt menu bar
rather than against a 13px panel row, which is a chip a little over half
the bar's height with horizontal padding of about a third of that.

`CHIP_FILL_ALPHA` computes the tint from the colour rather than reading
a `--*-muted` token, because this image composites over the menu bar's
own translucent backdrop: there is no fixed ground an opaque muted hex
could have been mixed against ahead of time. `GroupColor::rgb` mirrors
the `--teal/blue/violet/amber/red` tokens, whose light-appearance values
differ from their dark ones.

The letters are set at `SLUG_FONT_SIZE_PT` in the interface face, not
the tabular one the digits need: three letters are a name, not a column
of numbers to line up. `text::load_ui_font` is that face. A colour bar
under each grouped figure, and then a bare slug beside a rolled-up
figure, each did this job before: a named chip says which group without
a legend, and says it without spending a number's worth of width.

## One renderer, two surfaces

The Customize display screen shows a preview of the menu bar. That
preview is not a second drawing of the same idea — it is this
compositor's own output. `shell.rs`'s `render_status_item_preview`
command takes a *candidate* segment list, the arrangement the screen is
currently showing, runs it through the same `render` the live tray runs
through, and hands back the raw RGBA. The screen paints that bitmap into
a canvas at half size, `RENDER_SCALE`, which is the size the menu bar
presents the image at.

The command reads and writes none of the status item's own state, so
previewing an arrangement never disturbs the tray. Nothing else is
allowed to draw this preview: the CSS strip that stood here before
re-described a chip, a figure and a gap in a second place, and two
separately-authored renderers drift — which is what it did. Outside
Tauri there is no compositor to ask and the preview is simply absent,
rather than approximated.

## Looking at the result

`cargo run --example dump_tray_scenes -- <dir>` writes the raw RGBA of
the no-groups, collapsed-chip and opened-chip scenes, which is how a
change to any of the above gets looked at rather than only asserted
about. Set `QUOTOS_DUMP_LIGHT` for the light-appearance palette. The
image at the top of this page is its `doc-menu-bar` scene composited
over the menu bar's own ground, so it cannot fall behind the code.

## Compositing

The buffer carries translucent layers over each other — the limit
chevron's track, and then its own bright fill over that — so a plain
overwrite would discard whichever drew second wherever they overlap,
and the filled part of the gauge would come out darkened by the track
beneath it rather than reaching the limit colour. `blend_pixel` does
standard src-over alpha compositing instead. A fully-opaque source or a
fully-transparent destination pixel reduces to a plain overwrite, so a
single-layer caller, such as a chip's frame or digit text onto a blank
buffer, is unaffected.

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
