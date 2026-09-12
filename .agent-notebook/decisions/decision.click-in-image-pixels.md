---
id: decision.click-in-image-pixels
type: decision
state: active
kind: rule
title: Resolve a status item click in the image's pixel space, never as a fraction of the button's width
by: Maksim Yaromin
created: 2026-09-12
updated: 2026-09-12
---

A click on the status item is resolved in the composited image's own
pixel grid, through `geometry.rs`'s `click_x_in_icon_px`. It is never
resolved by taking the click's offset as a fraction of the status item
button's width and scaling that fraction back out by the image's width.

The two widths are not the same. macOS centres the image inside a
button wider than it by a system margin — the same margin
`glyph_center_offset_from_item_left_points` already derives for the
panel's beak. At the measured eight points a side, the fraction-based
arithmetic is off by about sixteen image pixels at either end, which is
wider than a chip. The failure that produces is intermittent by nature:
a click near the middle of the image lands correctly and the error
grows towards the ends, so a group folds sometimes and opens the panel
other times, depending on where its chip happens to sit that repaint.
Anything that widens the image — one more pinned figure, a longer group
name — moves the boundary again.

The fraction form looks simpler, and it is the natural thing to write
from `TrayIconEvent::Click`, which hands over the click point and the
item's box already converted by the same scale factor. Their difference
is the offset across the *button*, which is not the offset into the
*image*. Subtract the derived margin first, scale into image pixels,
and only then test against the chip spans.

Those spans are reported in that same image pixel space and come from
`layout_segments`, the layout pass `render` itself uses, so a click is
tested against the frames the chips were actually drawn at; nothing
re-derives geometry at click time. `HIT_PADDING_PX` widens each span
before the test, since a click a point or two shy of a chip still
plainly means that group, and a compile-time assertion keeps two padded
spans from meeting in the gap between them.

docs/architecture.md's "Clicking a group's chip in the menu bar"
carries the full derivation. What the resolved click is for is the
shape "A pin group folds from its chip in the menu bar, not from the
panel" (decision.group-folds-from-its-chip).
