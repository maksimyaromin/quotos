# The customize display screen

Pin groups are arranged here and nowhere else: every pinned limit
window is a row, grouped rows boxed under their group's header,
standalone rows loose below them. Nothing on this screen ever unpins
anything, and nothing here is decided at the moment of pinning.

## The preview

The strip at the top is the menu bar, drawn by the menu bar's own
compositor: the screen hands the segment list it is currently showing
to the `render_status_item_preview` command and paints the bitmap that
comes back. It re-describes nothing, which is the point; see "One
renderer, two surfaces" in
[status-item-rendering.md](status-item-rendering.md#one-renderer-two-surfaces).

## The drag

The gesture runs on [`@dnd-kit`](https://dndkit.com): `@dnd-kit/core`
for the context, sensors and drag overlay, `@dnd-kit/sortable` for the
lists, `@dnd-kit/modifiers` to keep the drag on one axis. An earlier
version hand-rolled the whole thing on `mousedown`/`mousemove`, which
was correct and read as arbitrary: rows jumped rather than moved, a
drop target had to be guessed at, and nothing animated.

Two levels of `SortableContext` mirror the two levels on screen: an
outer one holding every group box and every standalone row, and one per
group holding that group's members. A row is picked up by its own
handle, never by its body, so clicking a group's name opens the rename
and nothing else; the pointer sensor additionally needs four pixels of
travel before a press becomes a drag. Every row in the list, group
header and member alike, puts its handle in the same column: a member
is indented by its own content moving, never its handle.

Collision detection is `pointerWithin` and nothing else. What is under
the pointer is what a drop lands on, innermost first, so a member row
is never shadowed by the group box around it, and a drag that is over
nothing is over nothing. The obvious-looking fallback, taking whichever
box the dragged row happens to overlap, is what makes a drop target
stop being something anyone can aim at.

## Motion

`drag-motion.ts` holds the whole of the screen's motion language, and
there is only one: two springs, driven by
[`framer-motion`](https://motion.dev). Picking a row up springs it to
`PICKED_UP`, a small scale and tilt, on the stiffer `GRAB_SPRING`; the
rows that move aside for it, and the space that opens where it will
land, travel on the heavier `REFLOW_SPRING`; the drop settles on the
same overshoot expressed as a bezier, since that one moment is animated
by the drag library rather than by us. Nothing on the screen eases and
nothing jumps.

### Nothing changes width while a drag is on

Every drag-relevant box states its own width rather than taking one
from its content, and every layout animation is `layout="position"`:
framer-motion animates a size change by scaling the box, which squeezes
the text inside it, so only position is ever animated and only heights
are ever set. The drop hint that appears on a paired row is out of the
row's flow entirely and stands over the figure, which keeps its place
hidden beneath it.

Together those are why nothing shakes under a stationary pointer. A row
that sized itself to its content answered every mid-drag change — a
hint appearing, a space opening, a box taking one more member — with a
width change, and a width change is a reflow of everything beside it.

macOS's Reduce Motion setting reaches the stylesheet through the
`--dur-*` tokens, which a spring driven in JavaScript never reads, so
every one of those transitions is asked for through
`grabTransition`/`reflowTransition`/`dropSettle`, which answer with no
motion at all when `useReducedMotion` says so.

### What the list does while a row is in the air

The row that was picked up closes up behind it: what is under the
pointer is the row itself, and leaving a copy of it in the list would
show it twice. Where it would land, a space opens holding that same
row's name and figure, so the list already reads the way it will once
the pointer is released. Both are one height animating on the reflow
spring, which is also the whole of the "make room" feel.

The space is drawn, never actually moved into: the rows themselves stay
where the drag started. Rearranging them for real mid-drag feeds the
library its own output — the row changes which list it belongs to, the
targets re-register, the answer changes, and the screen loops until
React gives up. A drawn space costs nothing in the drag library's own
bookkeeping, which is also why the way out of a group arrives at its
full height at once rather than growing into it: a target is measured
the moment it appears, and a strip still growing would be measured as
the sliver it was.

## What a drop means

`lib/customize-drag.ts` is the whole of that decision, kept apart from
the library that reports the drag. `resolveDrop` takes what is being
dragged, what it is over, and the order the members are in, and returns
one of: group two loose pins together, join a group, leave a group,
reorder two groups, or nothing at all. `landingSlot` turns that answer
into where the space opens.

Joining a group lands the pin ahead of the member it was dropped onto,
with one exception: a pin already in that group, dropped onto a member
*below* itself, lands after that member instead. The order matters
because the list closes up behind a drag before the drop is applied, so
"ahead of it" would be exactly where the row already was — the space
would open under the pointer and the release would then move nothing.
That is why `resolveDrop` needs the arrangement and not just the two
ends of the gesture: the same answer decides both the space shown and
the move carried out, so a space that opens is always a move that
happens.

The screen asks it on every pointer move, to show what a release would
do, and again on release, to do it. That is why the highlight can never
disagree with the outcome — the space, the lit group box, the paired
row and the way-out strip are all read off the same answer the drop
acts on. `sameDrop` keeps the screen from redrawing when the answer has
not actually changed.

Pairing two loose pins is the one drop that opens no space: the group
it would make does not exist yet, so the row it would pair with is
marked instead.

## Testing it

jsdom has no layout, so the drag library has no geometry to report
from: its collision detection would resolve every drop to whichever
droppable happened to be measured first. `customize-display-screen.spec.tsx`
and `app.spec.tsx` therefore stand in for `DndContext` itself and hand
the screen the answers a real drag would reach, which exercises every
id and every payload the screen attaches, everything it draws while a
drag is in the air, and everything it does with a drop.
`customize-drag.spec.ts` covers the decision on its own, and
`drag-motion.spec.ts` the motion language.

What the library does between those two points — measuring, animating,
settling — is the library's own and is verified by hand in the browser
harness, the way [contributing.md](contributing.md#testing) requires of
anything that depends on real on-screen geometry.
