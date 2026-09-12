# The customize display screen

Pin groups are arranged here and nowhere else: every pinned limit
window is a row, grouped rows boxed under their group's header,
standalone rows loose below them. Nothing on this screen ever unpins
anything, and nothing here is decided at the moment of pinning.

## The drag

The gesture runs on [`@dnd-kit`](https://dndkit.com): `@dnd-kit/core`
for the context, sensors and drag overlay, `@dnd-kit/sortable` for the
lists, `@dnd-kit/modifiers` to keep the drag on one axis. An earlier
version hand-rolled the whole thing on `mousedown`/`mousemove`, which
was correct and read as arbitrary: rows jumped rather than moved, a
drop target had to be guessed at, and nothing animated. The library
brings the parts that are hard to hand-roll — a lifted overlay that
follows the pointer, the FLIP transition that slides other rows out of
the way, a drop animation that settles rather than snaps, and keyboard
dragging for free.

Two levels of `SortableContext` mirror the two levels on screen: an
outer one holding every group box and every standalone row, and one per
group holding that group's members. A row is picked up by its own
handle, never by its body, so clicking a group's name opens the rename
and nothing else; the pointer sensor additionally needs four pixels of
travel before a press becomes a drag.

Collision detection is `pointerWithin` with `rectIntersection` behind
it: what is under the pointer is what a drop lands on, innermost first,
so a member row is never shadowed by the group box around it. The
fallback only matters for a drag that has left every target at once.

## What a drop means

`lib/customize-drag.ts` is the whole of that decision, kept apart from
the library that reports the drag. `resolveDrop` takes what is being
dragged and what it is over, and returns one of: group two loose pins
together, join a group (at the end, or ahead of the member dropped
onto), leave a group, reorder two groups, or nothing at all.

The screen asks it twice: once per pointer move, to light up what a
release would do, and once on release, to do it. That is why the
highlight can never disagree with the outcome — the insertion line, the
lit group box, the paired row and the way-out strip are all read off
the same answer the drop acts on.

## Testing it

jsdom has no layout, so the drag library has no geometry to report
from: its collision detection would resolve every drop to whichever
droppable happened to be measured first. `customize-display-screen.spec.tsx`
and `app.spec.tsx` therefore stand in for `DndContext` itself and hand
the screen the answers a real drag would reach, which exercises every
id and every payload the screen attaches and everything it does with a
drop. `customize-drag.spec.ts` covers the decision on its own. What the
library does between those two points — measuring, animating, settling
— is the library's own and is verified by hand in the browser harness,
the way [contributing.md](contributing.md#testing) requires of anything
that depends on real on-screen geometry.
