---
id: decision.c103ee057256acf2d0d5c251511a07b9
type: decision
state: active
kind: shape
title: A pin group folds from its chip in the menu bar, not from the panel
by: Maksim Yaromin
created: 2026-09-12
updated: 2026-09-12
---

A pin group is folded and unfolded from its own chip in the menu bar,
with no panel involved. The chip is the button; a figure beside it
reports a number and nothing else, and a click on one opens the panel
like a click on any other part of the status item.

The constraint a group exists to relieve is menu bar width, so the
control that relieves it lives in the menu bar too. Requiring the panel
to be open to fold a group would mean opening a window to reclaim a few
points of a bar the user is already looking at, and would hide the
current arrangement behind the very surface the group is meant to make
unnecessary.

Three earlier shapes did this job and were each replaced for a reason
worth keeping:

- A colour bar under every figure a group owned, with the figure itself
  as the click target. A colour says which figures belong together only
  to a reader who already knows the legend, and the menu bar has no room
  to carry one. A figure also cannot be both a report and a button
  without the number becoming unclickable-looking or the button
  ambiguous.
- A bare slug in front of the group's figures. This answers *which*
  group without a legend, but reads as loose text beside the digits with
  nothing marking it as a target.
- A rolled-up figure standing for the whole group. It spends a number's
  worth of width to say something a name says better, and it invites the
  reading that the group has one limit of its own.

The chip keeps the name, marks itself as a target, and carries the
group's colour as identity rather than as the only signal. It follows
the badge language the panel already uses, so the same colour and shape
mean the same thing on both surfaces. Rolled up, the chip is the whole
of what the group draws.

One layout rule follows from the chip being the button: opening a group
must never move its chip. Every item a group adds is appended after the
chip, never inserted before it, so the target does not jump out from
under the pointer that just hit it.

How the chip is drawn, measured and hit-tested is in
docs/status-item-rendering.md, under "The group chip" and "Resolving a
click back to a chip".

Because the chip is the only button in the item, a click has to be
resolved back to it exactly; the decision "Resolve a status item click
in the image's pixel space, never as a fraction of the button's width"
(decision.495adb8c6508226107933a32cbfaa493) governs that arithmetic.
