# Platform constraints

Facts about macOS, Tauri, and this app's dependencies that shape how the
code has to be written here, gathered in one place because none of them
belongs to a single source file.

## The webview's content security policy

`tauri.conf.json`'s CSP is enforced only in a packaged build, never under
`npm run tauri dev`: Tauri injects the policy into the built `index.html`,
while the dev server serves the page straight from Vite with no policy
attached at all. A change that violates it passes every dev-mode check
and vitest run, then breaks only in the built `.app`. Test a
CSP-sensitive change against a real bundle, not just `npm run dev`.

Every relaxation the policy carries is load-bearing:

- `style-src 'unsafe-inline'` for React's inline style attributes and the
  design system's keyframe `<style>` elements.
- `font-src data:` because Vite inlines assets under 4KB, and the app's
  variable-weight fonts land in the built CSS as `data:` URIs.
- `connect-src ipc: http://ipc.localhost` for Tauri's own IPC transport;
  removing it breaks every `invoke` call.

`img-src 'self'` grants nothing beyond that default: every icon and glyph
in the webview is inline SVG markup, never a fetched resource, and the
status item's own bitmap is composited in Rust and handed to AppKit
directly, never touching the webview at all. `img-src` stays explicit
rather than left to fall back to `default-src` so a future, more
permissive `default-src` can't silently widen it too. `tools/checks/csp.spec.ts`
pins both this and the next paragraph's claim.

The policy names no remote host anywhere. Every HTTP request this app
makes happens on the Rust side.

## The webview's capability grant

`capabilities/default.json` grants exactly `core:event:allow-listen` and
`allow-unlisten`, nothing else. Its own `description` field is the
source of truth for why: the frontend reaches Rust only through this
app's own commands, which the capability system does not gate, and
through `listen`. A frontend call into any other `@tauri-apps/api`
surface, such as its window, menu, tray, or path modules, needs its own
explicit grant added first, and a missing grant fails silently: the
resulting promise never resolves or rejects in a way that shows up as an
error during ordinary development.

## Tauri commands and the main thread

A `#[tauri::command]` without `(async)` runs inline with IPC on the main
thread, so it blocks the webview's own rendering while it runs. That is
correct for anything that touches `NSStatusItem` directly, and wrong for
everything else. Every command that forks a process or writes to disk in
this codebase is `(async)`.

## `tray-icon` 0.24.2 on macOS

- There is no path to a colored tray title. `set_title` calls
  `NSStatusItem`'s `setTitle` with a plain `NSString`, with no
  attributed-string or color channel anywhere in the crate's public API.
  `status_item_render.rs` composites a bitmap directly instead.
- `set_title(None)` is a silent no-op: it only calls `setTitle` when
  given `Some(..)`, so clearing a title needs `Some("")`.
- A variable-length status item's button is measurably wider than its
  own composited image, by a fixed AppKit margin per side. `shell.rs`'s
  `sync_status_item_length` pins the item's length to the image's own
  width on every repaint, so the button's bounds and the image's bounds
  match.

## The panel-open highlight

The frame behind an open panel is AppKit's own, set through
`shell.rs`'s `set_status_item_native_highlight` — the same `highlighted`
property AppKit sets on its own mouse-down, so the pressed state and the
panel-open state are one drawing rather than two geometries that have to
be kept in agreement.

They cannot be kept in agreement any other way. This app can only paint
inside its own image, and the image is the item's own bounds;
AppKit's plate is the item's *slot*, which includes the spacing the menu
bar reserves between items. Measured against a status item carrying a
bare mark, one chip, one chip and two figures, and two chips and four
figures, the plate is that item inflated by a constant 10pt on each side
and 3pt above and below, whatever the content width. A pill drawn into
the bitmap reached neither: it was the image's own 18pt height, flush
with its own edges, and read as a smaller box nested inside the one a
press produces.

`setHighlighted:` does not survive `set_icon`, so `repaint_status_item`
re-applies it after every repaint rather than once when the panel opens.

## The non-activating panel class swap

A full-screen Space belongs to one application, and activating a different
one while a full-screen Space is frontmost makes macOS leave that Space, the
same transition Cmd-Tab produces. An ordinary `NSWindow` can only become key
while its application is active, so `WebviewWindow::set_focus()` triggers
that transition on every panel open, since it is `tao`'s
`makeKeyAndOrderFront:` followed by `activateIgnoringOtherApps: YES`.
`NSWindowStyleMaskNonactivatingPanel` lifts the constraint, but only on an
`NSPanel`; set on a plain `NSWindow` it is silently inert.

Tauri and `tao` create a plain `NSWindow` subclass with no switch for "make
it a panel", so `panel_window.rs`'s `make_nonactivating_panel` swaps the
window's class after creation, the same approach `tauri-nspanel` takes.
`object_setClass` is only safe when the new class is no larger than the
object's original allocation: `NSPanel` adds no instance storage over
`NSWindow`, the original window class is larger since it adds a `focusable`
ivar, and the replacement class declares no ivars of its own, so the
runtime size check passes today and fails closed if a future AppKit
changes that. The original class overrides `canBecomeKeyWindow` and
`canBecomeMainWindow`, both reimplemented on the replacement, and
`sendEvent:`, whose body is a no-op unless `isMovableByWindowBackground` is
set, which Tauri never sets since this app drags through `startDragging()`
instead. The window delegate is a separate object and stays attached, so
`tao`'s `Focused`, `Moved`, and `Resized` events keep firing unchanged.
`set_focusable()` becomes unusable after the swap, since it writes the
now-absent `focusable` ivar by name; nothing in this crate calls it.

Setting the non-activating style bit on a plain `NSWindow` raises an
Objective-C exception, and an exception crossing the Rust FFI boundary
aborts the process, so the class swap is verified by reading the class
back before the style mask is ever written. The window already carries a
KVO isa-swizzle when `make_nonactivating_panel` runs; the class swap
displaces it, which is safe only because Tauri sets the content view once,
during window creation, before this ever runs.

`QUOTOS_PANEL_MODE=window` restores the old activate-on-show path for a
same-session comparison without a rebuild. `-[NSApplication isActive]`
reads `true` for an accessory app regardless of which path ran, so telling
them apart needs `NSWorkspace.frontmostApplication` read from a separate
process.

`panel_window.rs` sets two more style bits after the class swap.
`NSPanel` defaults to hiding when its application deactivates, which
would fight detached mode's whole point of staying visible while another
application is frontmost, so `setHidesOnDeactivate: false` turns that
default off. `becomesKeyOnlyIfNeeded` would suppress
`windowDidResignKey` until something inside the panel demanded key
focus, and click-away-to-close depends on that event firing, so it is
also set to `false`. Showing the panel itself calls
`orderFrontRegardless()` to front the window without activating the
app, then `makeKeyWindow()` to give it focus, which only a
non-activating panel accepts while its application is inactive;
`WebviewWindow::set_focus()` is not used for this because it activates
the app, undoing the whole point of the class swap.

`ClassBuilder::add_method` takes each callback cast as `extern "C" fn(_,
_) -> _` rather than with its lifetime spelled out: spelling the
lifetime keeps the compiler from inferring a higher-ranked function
pointer type, and the type it infers instead is one `MethodImplementation`
rejects. `tao`'s own class declarations use the same cast for the same
reason.

## Drawing text into an offscreen bitmap

Compositing real text into a `CGBitmapContext` on macOS has several
non-obvious failure modes: an alpha-only context corrupts CoreText glyph
shapes, the standard bottom-left-origin-to-top-left flip mirrors glyphs
drawn through CoreText, and pairing a generic RGB fill color with a
device RGB bitmap context shifts even fully opaque pixels off the
requested color. `status_item_render.rs` documents each of these at the
exact line that works around it; read that file before changing how the
status item renders text.

## The filesystem is case-insensitive

APFS is case-insensitive by default. `rm -f src/App.css` also deletes
`src/app.css` if both exist. Confirm with `ls` after any
case-sensitive-looking cleanup.

## Third-party window managers can still resize this window

`resizable: false` in `tauri.conf.json` only disables the native
resize-handle drag. A window manager that resizes through the
Accessibility API's size attribute goes straight to setting the window's
frame, bypassing that flag entirely. `lib.rs`'s window-event handler
snaps the size back to the fixed logical size whenever it drifts, in
either the docked or detached state, fighting only a resize and never a
move. The incoming `PhysicalSize` is compared and reasserted in logical
units, the point size times the window's own scale factor, since
converting with that same factor is the only comparison that means
anything on a mixed-DPI setup.

## Self-correcting the docked position

Third-party relocation is not limited to resizes: `WindowEvent::Moved`
nudges the window back to `AppState.docked_target` whenever AppKit
relocates it away from where it should be docked, but only once
relocating has gone quiet for `MOVE_SETTLE_MS`, not on every single
`Moved` event. A generation counter tracks whether a given correction is
still the most recently scheduled one, so a burst of AppKit-internal
relocations finishes settling before anything reasserts. The comparison
against the target uses `SETTLE_TOLERANCE_POINTS`, a tolerance rather
than exact equality, since AppKit settles the window a point or so off
whatever was requested, and correcting a sub-point gap produced an
endless correct-drift-correct loop. `Moved` itself reports the frame
origin in points multiplied by the window's current backing scale
factor, undone before comparison so everything downstream compares in
the one coordinate space that exists; see "Coordinate spaces and panel
placement" in architecture.md.

## Packaging

`npm run tauri build`'s DMG target fails in headless or sandboxed
environments, since the underlying `bundle_dmg.sh` script needs real
disk-image arbitration. `tauri.conf.json`'s bundle targets are `["app"]`
only for that reason; add `"dmg"` back only where DMG creation is
actually needed and has been confirmed to work.

A bare `cargo build` binary, run without the `tauri` CLI, resolves to the
dev configuration and tries to load the Vite dev server rather than the
bundled frontend. With no dev server running, the window shows nothing
at all. Use `npm run tauri dev` or a real bundle to see the app render.

## `flock` and forked children share a test binary

`single_instance`'s claim-and-release test used to fail intermittently
under a full `cargo test` run, never in isolation. BSD `flock` locks
belong to the open file description, so a test elsewhere in the same
binary that forks a process can inherit a duplicate of this test's own
file descriptor before its child's `exec` closes it, keeping the lock
alive after this test's file has already been dropped. The actual
forking culprit was `status_item_render::render`'s tests: `render`
used to call `is_dark_mode`, which shells out to `defaults read`, on
every one of its own test invocations. `render` now takes `dark: bool`
as a parameter instead, and only `shell.rs`'s real repaint path calls
`is_dark_mode` itself, so the pure layout and color tests no longer
fork at all. The shipped app still claims its lock exactly once, at
startup, long before anything else forks; this was always a test-binary
concurrency artifact, never a production defect. Any new test that
spawns a real child process is a reintroduction of this hazard and
should take its environment reading as a parameter the same way.

## `SMAppService` needs an explicit framework link

No other dependency in this crate links `ServiceManagement`, so without
`launch_at_login.rs`'s `#[link(name = "ServiceManagement", kind =
"framework")]`, nothing forces dyld to load that framework and
`AnyClass::get(c"SMAppService")` finds no class to look up. The link
directive has to stay even though no symbol from it is called directly by
name.

## Pinning `objc2` feature flags

`Cargo.toml`'s `[target.'cfg(target_os = "macos")'.dependencies]` block
only exists at all because Core Text gives sharper status item digits
than a hand-rolled bitmap font, and that whole family of crates is
macOS-only since the crate only ever runs as a menu bar app there. Every
feature list under it is pinned to exactly what `tauri`'s own transitive
use of the `objc2` family already resolves in `Cargo.lock`, so `cargo
check` needs no network access to add a new crate version: default
features on `objc2-app-kit` pull in `objc2-core-video`, and default
features on `objc2-core-graphics` pull in `objc2-metal`, neither of
which this crate's actual `NSFont`, `NSFontDescriptor`, and
`CGBitmapContext`-based text rendering needs, and neither of which is
already resolved. `objc2-app-kit`'s `NSStatusBar`, `NSStatusBarButton`,
and `NSStatusItem` features, reached through
`tray_icon::TrayIcon::ns_status_item()`, add no new resolution either,
since the `tray-icon` crate itself already requests those same three for
the same `objc2-app-kit` version.

## The `_lib` crate name suffix

`[lib]`'s `name = "quotos_app_lib"` looks redundant next to the binary's
own `quotos_app`, but dropping the suffix conflicts with the bin name on
Windows; see [cargo#8519](https://github.com/rust-lang/cargo/issues/8519).
