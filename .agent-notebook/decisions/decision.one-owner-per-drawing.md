---
id: decision.one-owner-per-drawing
type: decision
state: active
kind: rule
title: Ask the thing that owns a drawing or a measurement; never keep a second description of it
by: Maksim Yaromin
created: 2026-09-12
updated: 2026-09-12
---

Anything already drawn, measured or owned somewhere gets asked, not
described a second time. Where a second surface needs the same picture
or the same geometry, it calls the thing that produces it. Two
descriptions of one idea have to be kept in agreement by hand, and they
are not. The drift then arrives as a bug in which each description
looks correct read on its own, and only the pair is wrong.

Four places in the status item and its panel settle this, and they are
the reason the rule is worth stating rather than rediscovering:

- The frame behind an open panel is AppKit's own `highlighted`
  property, set from `shell.rs`, not a pill painted into the bitmap.
  This one could never have been matched by hand: what this app paints
  is the item's composited image, and AppKit's plate is the item's
  *slot*, which covers the spacing the menu bar reserves between items
  — space that lies outside the image entirely. Measured across four
  content widths the plate is that item inflated by a constant 10pt to
  each side and 3pt above and below; no constant available inside the
  image reaches it. Setting the same property AppKit sets on its own
  mouse-down makes the pressed state and the panel-open state one
  drawing instead of two geometries. `setHighlighted:` does not survive
  `set_icon`, so every repaint re-applies it.
- The Customize display screen's menu bar preview is the compositor's
  own output, through the `render_status_item_preview` command, and is
  handed the very segment list `set_status_item_state` is called with.
  A CSS strip re-describing a chip, a figure and a gap stood there
  before and drifted, which is what two separately-authored renderers
  do.
- Chip spans come from `layout_segments`, the pass `render` itself
  runs, so a click is tested against the frames the chips were drawn
  at. Nothing measures geometry at click time; the rule "Resolve a
  status item click in the image's pixel space, never as a fraction of
  the button's width" (decision.click-in-image-pixels) is what puts a
  click into that same space.
- On the Customize display screen, what a release would do and what a
  release does are one answer from `resolveDrop`. Every highlight, the
  space that opens, the lit group box, the paired row, is read off it,
  so the hint cannot promise something the drop does not do.

The canonical explanations live with the code they belong to:
docs/platform-constraints.md's "The panel-open highlight",
docs/status-item-rendering.md's "One renderer, two surfaces" and
"Resolving a click back to a chip", and docs/customize-display.md's
"What a drop means".
