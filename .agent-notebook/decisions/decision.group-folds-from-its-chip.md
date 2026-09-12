---
id: decision.group-folds-from-its-chip
type: decision
state: active
kind: shape
title: A pin group folds from its chip in the menu bar, not from the panel
by: Maksim Yaromin
created: 2026-09-12
updated: 2026-09-12
---

A pin group is folded and unfolded from its own chip in the menu bar,
with no panel involved. The chip is the button. A figure beside it
reports a number and nothing else, and a click on a figure opens the
panel exactly like a click on any other part of the status item.

The constraint a group exists to relieve is menu bar width, so the
control that relieves it lives in the menu bar too. Requiring the panel
to be open in order to fold a group would mean opening a window to
reclaim a few points of a bar the user is already looking at, and would
hide the current arrangement behind the very surface the group is meant
to make unnecessary.

Three earlier shapes did this job and were each replaced for a reason
worth keeping:

- A colour bar drawn under every figure a group owned, with the figure
  itself as the click target. A colour says which figures belong
  together only to a reader who already knows the legend, and the menu
  bar has no room to carry one. A figure also cannot be both a report
  and a button without the number looking unclickable or the button
  reading as ambiguous.
- A bare slug in front of the group's figures. That answers *which*
  group without a legend, but it reads as loose text beside the digits,
  with nothing marking it as a target.
- A rolled-up figure standing for the whole group, its worst member's.
  It spends a number's worth of width to say something the name says
  better, and it invites the reading that the group has a limit of its
  own. Rolled up, the chip is now the whole of what a group draws, and
  the members' numbers wait in the tooltip until the group is opened.

The chip keeps the name, marks itself as a target, and carries the
group's colour as identity rather than as the only signal. It follows
the badge language the panel already uses, so the same colour and the
same shape mean the same thing on both surfaces.

One layout rule follows from the chip being the button: opening a group
must never move its chip. Every item a group adds is appended after the
chip, never inserted before it, so the target does not jump out from
under the pointer that just hit it.

How the chip is drawn, measured and hit-tested is in
docs/status-item-rendering.md, under "The group chip" and "Resolving a
click back to a chip". Because the chip is the only button in the item,
a click has to be resolved back to it exactly; the rule "Resolve a
status item click in the image's pixel space, never as a fraction of
the button's width" (decision.click-in-image-pixels) governs that
arithmetic.
