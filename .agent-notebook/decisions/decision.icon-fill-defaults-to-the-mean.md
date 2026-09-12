---
id: decision.icon-fill-defaults-to-the-mean
type: decision
state: active
kind: rule
title: The mark's fill is the mean of every tracked subscription unless one window is designated
by: Maksim Yaromin
created: 2026-09-12
updated: 2026-09-12
---

How full the mark's limit chevron is drawn is the arithmetic mean of
every tracked subscription's headline figure, unless one limit window
has been designated to drive it. `lib/status-item-segments.ts`'s
`computeIconFillPercent` is the whole of that choice; nothing in the
renderer knows which of the two it is looking at. What the reading does
to the mark is the shape "The mark's limit chevron is the gauge"
(decision.limit-chevron-is-the-gauge).

The mean is deliberate and it is deliberately blunt. Averaging a busy
subscription with an idle one describes neither of them, and that is
accepted: the mark is an ambient shape read at a glance from across a
room, while the pinned figures beside it are what report actual numbers
and carry the severity colour. The obvious alternative, the worst
member, is worse for the job. It pegs the mark at full whenever any one
window is exhausted, and a gauge that spends most of its time at its
own ceiling has stopped saying anything; it also promotes a single
narrow window into the app's headline without anyone having asked for
that.

Anyone who wants the mark to mean one specific thing says so: a toggle
on a limit window's row designates that window, and the choice persists
beside the tracked list. Untracking that subscription, or choosing the
same window again, returns the mark to the mean. A designated window
that stops reporting a figure falls back to the mean too, rather than
freezing on its last value, since a gauge that holds still is
indistinguishable from one that is up to date.

So the mean is not a placeholder waiting to be improved into a
worst-of. Replacing the default with the worst member is a change of
what the mark is for, and needs to be argued as one.
