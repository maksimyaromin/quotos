---
id: decision.limit-chevron-is-the-gauge
type: decision
state: active
kind: shape
title: "The mark's limit chevron is the gauge: a full-extent track with a reveal clipped along its centreline"
by: Maksim Yaromin
created: 2026-09-12
updated: 2026-09-12
---

The menu bar mark is three chevrons, and only one of them is a gauge.
The two grey chevrons rising from below are spend, drawn at full colour
whatever the reading is and identical at 0% and at 100%. The single
peach chevron pressing down from above is the limit, and it is the part
that reports `icon_fill_percent`.

That one chevron is drawn twice. A faint track at `LIMIT_TRACK_OPACITY`
covers its full extent, so the mark reads as three chevrons at every
reading, and a fully saturated pass covers the part the reading has
reached. The reveal runs from the chevron's own vertex up toward its
two arm-tips, and `Chevron::nearest` is what clips it: a pixel belongs
to the fill when the nearest point on the chevron's centreline is at or
past the threshold. Measuring along the centreline rather than slicing
the rendered stroke by y keeps the cut perpendicular to whichever arm it
lands on, and keeps the round join at the vertex whole at 0%.

Both halves of that are the reusable part, for any procedural glyph
that also has to report a percentage. A glyph that only draws the part
it has reached stops being a glyph at low readings — here the mark
would lose a third of its height and read as two chevrons, which is a
different mark, not an emptier one. And a reveal clipped along the
shape's own centreline follows the shape, where a clip along a screen
axis cuts across it and exposes the stroke's flat end.

The track's opacity is set against this image's own ground and not
against a white page: the menu bar is dark and translucent, and a track
much fainter than this one does not read as a faded chevron there, it
reads as a missing one.

The mark is painted procedurally from its own vector geometry,
`src/design-system/assets/menubar-glyph.svg`, rather than composited
from a raster asset, and it can never be a template image: macOS would
flatten its two inks and the gauge to one tone. Drawing the exact shape
at the target resolution also makes the ink size a parameter rather
than a property of whatever asset was exported.

docs/status-item-rendering.md's "The capacity glyph" holds the
constants, the antialiasing width and the layer stacking. What the
reading itself is comes from the rule "The mark's fill is the mean of
every tracked subscription unless one window is designated"
(decision.icon-fill-defaults-to-the-mean).
