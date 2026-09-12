# The panel and status item lifecycle

`shell.rs` is one module rather than two because its two halves are
mutually recursive: repainting the status item can move the open panel,
through `repaint_status_item`, `schedule_resync_after_icon_change`, and
`reposition_under_status_item`, and showing or hiding the panel repaints
the status item, through `show_panel`, `hide_panel`, and
`set_status_item_highlighted`. The pure layers stay out of this module:
coordinate math lives in `geometry.rs`, bitmap composition in
`status_item_render.rs`, the `NSPanel` class swap in `panel_window.rs`.

## Repainting the status item

`repaint_status_item` is the one place the icon actually gets redrawn
and the one place the panel-open highlight is set, shared by its two
independent inputs, the pinned-subscription digits and whether the panel
is open, neither of which knows the other's current value, so it always
reads both fresh from `AppState` rather than taking either as a
parameter. `record_if_changed` compares the new state
against the last-painted one first, since the frontend fires
`set_status_item_state` on every state change, most of which leave the
pinned digits identical, and a full bitmap composite plus `set_icon` is
main-thread work worth skipping. The tooltip participates in that
comparison too, since it can change alone: renaming a subscription
rewrites its tooltip line while leaving every digit byte-identical.

The repaint always paints a composed bitmap and never an
`icon_as_template` one: the mark carries two inks and its gauge a third
tone, which a template image would flatten to one. The highlight is
re-applied after every repaint rather than once when the panel opens,
since `set_icon` clears it; see "The panel-open highlight" in
platform-constraints.md. The title is always cleared with `Some("")`,
never `None`; see "tray-icon 0.24.2 on macOS" there too.

## Keeping the item's length synced

Pinning or unpinning a subscription, or the highlight toggling, can
change the status item's own width, which on macOS shifts the item's
own on-screen x too, since status items lay out right-to-left. Nothing
about that resize goes through `TrayIconEvent`, so the last-known item
position goes stale the instant a repaint runs, and the beak and panel
silently drift off the glyph until the next real click or hover unless
the position is re-synced whenever the panel is open and docked. That
resync is gated on the width having actually changed, since a
same-width repaint cannot have moved the item and re-docking after one
costs three `status_item.rect()` reads and up to three frame-placement
calls on the main thread; the highlight toggle never changes the width,
by construction, and fires on every open and close.

A single synchronous read right after `set_icon()` still reports the
item's previous width and position for one to a few runloop turns,
since `set_icon`'s own dispatch guarantees the image is set, not that
`NSStatusItem`'s width-driven layout pass has already run.
`schedule_resync_after_icon_change` makes one immediate attempt plus a
couple of short-delay retries rather than trusting the first read; each
retry just re-reads and re-applies, harmless if an earlier attempt
already landed on the right numbers.

## Detached mode

Tearing the panel off into a real, freestanding window, `detached =
true`, takes it out of the hide-on-blur path and lets it show up in
Cmd+Tab, so it can be parked on screen and watched while the user works
elsewhere. Decorations stay off in both states: with `titleBarStyle:
Overlay` in `tauri.conf.json`, turning decorations on paints real
traffic lights and a native title-bar strip over the content regardless
of the window's own transparency, so dragging comes entirely from
`app.tsx`'s header calling `drag_window_step` on every `mousemove`.

Entering detached mode sets `NSFloatingWindowLevel`, right for a
free-floating window and wrong for the docked popover. Snapping back
restores `NSStatusWindowLevel` and must be the only level-setting call
on that path: `tao`'s `set_always_on_top`, called on entry, ends in an
async dispatch to the main queue, and calling it again on the way back
would schedule a later runloop turn that drops the level back to
floating right after the synchronous restore, undoing it. Snapping back
also re-docks the window under the status item from the last rect seen
by any status item event, since that path has no fresh click to read a
rect from; it is triggered by the panel's own header button.

## The beak offset in the browser mock harness

`app.tsx` takes the beak's horizontal offset from the native
`panel-beak-offset` event, pushed on every dock and re-dock since it
depends on the status item's real position; see `compute_docked_layout`
in `shell.rs`. The browser mock harness has no real status item to push
that event from, so it seeds a fallback constant instead. The native
build starts at `null`, not that same fallback: a hardcoded value there
would draw the beak confidently in the wrong place if the event were
ever missed, where `null` fails visibly instead.

## Placing the window synchronously

`tao`'s own `set_outer_position` ends in an async dispatch of
`setFrameTopLeftPoint:` onto the main queue, while `show()` runs inline.
Called in either order from the click handler, already on the main
thread, the window would become visible at its stale position first and
only move a runloop turn later, a guaranteed one-frame flash at wherever
it last was. `place_window_top_left_sync` places the window directly
through `NSWindow` instead, closing that gap: by the time `show()` runs
the frame is already right. Callers fall back to Tauri's own async
`set_position` on non-macOS, or when the call arrives off the main
thread, where `setFrameTopLeftPoint:` is not safe.

`show_panel` positions the window before showing it, since `show()`
reveals the window wherever it was last left, and reapplies the
position once more after `show()`, a no-op when nothing moved the
window, since setting the frame to its current origin emits no `Moved`
event, cheap insurance against anything ordering-front does to the
frame.

## Manual dragging

`drag_window_step` moves a detached window by hand, one `mousemove` at a
time, instead of calling AppKit's
`-[NSWindow performWindowDragWithEvent:]`, which `tao`'s
`startDragging()` bottoms out in. `performWindowDragWithEvent:` does
move the window, but doing so while the mouse stays down measurably
reactivates the application, the same Space-losing failure
`panel_window.rs` exists to prevent, and the same problem reproduces
for `place_window_top_left_sync` itself: relocating a window while the
mouse button is held over it appears to carry an implicit activation
outside `NSWindowStyleMaskNonactivatingPanel`'s own promise, which is
scoped to key and main status, not window-server-level drag handling.
Reactivation is therefore scoped to the physical gesture's duration
only, never on show and never on an external window-manager move.

The first `mousemove` of a gesture only records where the cursor and
the window each started; every later call sets the window's frame
directly from the live delta, through the same synchronous
`place_window_top_left_sync` the docking path already uses.

## Staying on the popover's own Space

A full-screen application owns its Space, and macOS does not order
another application's window into it just because that application
asks. `NSWindowCollectionBehaviorFullScreenAuxiliary`, the standard
utility and palette window behavior, grants that; paired with
`CanJoinAllSpaces`, the window then exists on every Space at once, which
is both what a menu bar popover actually is and the option with no
asynchronous Space transition to race, unlike `MoveToActiveSpace`.
`.Transient` keeps it out of Mission Control and Exposé's per-Space
window list, matching the system's own Volume and Wi-Fi popovers, and
`.IgnoresCycle` keeps it out of Cmd-` window cycling.
`set_popover_collection_behavior` reapplies this on every show rather
than only at launch, idempotent and cheap insurance against anything
resetting it after first realization.

`alwaysOnTop` in `tauri.conf.json` gets the window `NSFloatingWindowLevel`,
right for a panel floating over ordinary windows and not enough to be
seen over another application's full-screen Space; a status-item
popover belongs at `NSStatusWindowLevel`, the level the system's own
menu bar popovers use.
