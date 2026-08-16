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
  width on every repaint, so this app's own highlight painting and
  AppKit's native click highlight share one frame.

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
move.

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

## A known flaky Rust test

`single_instance`'s claim-and-release test is flaky only under a full
`cargo test` run, never in isolation. BSD `flock` locks belong to the
open file description, and a test elsewhere in the same binary that
forks a process, several here shell out or spawn a pty, can inherit a
duplicate of this test's own file descriptor before its child's `exec`
closes it, keeping the lock alive after this test's file has already
been dropped. This can only happen when file locking and process
spawning run concurrently inside one test binary. The shipped app claims
its lock exactly once, at startup, long before anything else forks.
