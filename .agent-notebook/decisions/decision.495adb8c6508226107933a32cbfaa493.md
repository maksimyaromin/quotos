---
id: decision.495adb8c6508226107933a32cbfaa493
type: decision
state: active
kind: rule
title: Resolve a status item click in the image's pixel space, never as a fraction of the button's width
by: Maksim Yaromin
created: 2026-09-12
updated: 2026-09-12
---

A click on the status item is resolved in the composited image's own
pixel grid, through geometry.rs's `click_x_in_icon_px`. It is never
resolved by taking the click's offset as a fraction of the status item
button's width and scaling that back out by the image's width.

The two widths are not the same. macOS centres the image inside a button
that is wider than it by a system margin — the same margin
`glyph_center_offset_from_item_left_points` already derives for the
panel's beak. At the measured eight points a side, the fraction-based
arithmetic is off by about sixteen image pixels at either end, which is
wider than a chip. The failure it produces is intermittent by nature: a
click near the middle of the image lands correctly, and the error grows
towards the ends, so a group folds sometimes and opens the panel other
times depending on where its chip happens to sit that repaint. Anything
that widens the image — one more pinned figure, a longer name — moves
the boundary again.

The fraction form looks simpler and is the natural thing to write from
`TrayIconEvent::Click`, which hands over both the click point and the
item's box in the same units. Their difference is the offset across the
*button*, which is not the offset into the *image*. Subtract the derived
margin first, then scale into image pixels, and only then test against
the chip spans.

Chip spans are reported in that same image pixel space and are produced
by the layout pass `render` itself uses, so the spans a click is tested
against are the frames the chips were actually drawn at; nothing
re-derives geometry at click time.

docs/architecture.md's "Clicking a group's chip in the menu bar" carries
the full derivation. `geometry.rs`'s `click_offset` tests pin the rule
by asserting that the fraction-based arithmetic visibly disagrees.

What the resolved click is for is the decision "A pin group folds from
its chip in the menu bar, not from the panel"
(decision.c103ee057256acf2d0d5c251511a07b9).
