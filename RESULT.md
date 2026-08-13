# Quotos round 3 — visual precision pass (branch `fm/quotos-polish-p1`) — result

The captain tested `Quotos 3.app` himself and found the panel "not neat":
the beak drifts off the glyph, a row's `Fable` tag looked broken, and
detaching produced a titled system window that wouldn't drag. This file
describes the fix for each, how it was verified, and what is still honestly
unverified — not a diff against the previous round's `RESULT.md` (rewritten,
per convention, not appended to).

**99 automated tests pass** (63 vitest, 36 `cargo test`), `tsc --noEmit` and
`cargo check` are both clean.

## 1. Beak under the glyph — two independent bugs found and fixed

Three of the captain's screenshots (glyph only, glyph + `29%`, unpinned
again) each showed the beak in a different place, never under the glyph.
Investigation found **two separate, unrelated bugs**, both now fixed and
proven with real screen measurements, not just code review.

**Bug A — the glyph-offset constant was never actually measured.** The old
code assumed the glyph sits `6px button padding + 9px (half its 18px width)`
= 15pt inside the tray item, a number carried from a design-system mockup.
A pixel-precise screenshot of the real bare glyph (`screencapture` +
centroid analysis, cross-checked against the item's own Accessibility rect
and `CGWindowListCopyWindowInfo`) found the true center sitting at ~18pt in,
not 15 — because `NSStatusItem` centers the *whole* composited image (glyph,
or glyph+digits once pinned) inside a button that's wider than the image by a
small system margin on each side, and that margin isn't a fixed distance
once digits widen the image. Fixed by deriving the margin fresh each time
from two things Quotos actually knows: the item's current measured width
(`tray.rect()`) and the composited image's own known width
(`tray_render`'s own render size, cached in `AppState.last_icon_width_px`) —
see `compute_docked_layout`'s and `glyph_center_offset_from_item_left_physical`'s
doc comments in `lib.rs`.

A second, compounding bug in the same function: the computed value was the
glyph's *center*, but the frontend (`Panel.jsx`) applies it directly as the
12px beak box's CSS `left` (its *left edge*), and never accounted for the
14px inset between the window's own edge and the panel's own edge (the
window is deliberately wider than the panel for the drop-shadow's blur
margin — see `app.css`). Both together put the beak up to ~20pt off from a
provably-correct glyph center. Fixed in the same function.

**Bug B — pinning/unpinning while the panel is open never repositioned it.**
Pin/unpin calls `set_tray_status`, which repaints the tray icon and changes
its width — and on macOS, widening a status item shifts *its own* on-screen
left edge left (items lay out right-to-left), confirmed by a live A/B
measurement: item width 36pt→64pt after pinning one segment, right edge
unchanged either way. Nothing about that resize goes through a `TrayIconEvent`
(no tray click is involved), so the position captured at the last real click
went stale the instant this ran — this is the captain's exact "position
should be stable" complaint. Fixed by re-querying `tray.rect()` and
re-docking whenever the panel is open and still attached (`set_tray_status`
→ `schedule_resync_after_icon_change`). One further wrinkle found only by
testing this live: a single synchronous `tray.rect()` call, made right after
`set_icon()`, reads a **stale** rect — AppKit's own layout pass for the new
width hadn't caught up yet, for one to a few runloop turns. Fixed with a
short-delay retry (30ms + 120ms), the same shape as the existing
`schedule_position_correction` used elsewhere in this file for an unrelated
AppKit-async-layout race.

**Verified live, with numbers**, using the dev-identity build
(`com.quotos.desktop.dev`), a real tray click via `fm-quotos-click.sh`, and
pixel centroid analysis of `screencapture` output (glyph ink) cross-checked
against `CGWindowListCopyWindowInfo` (panel window) and the beak's own
rendered position — the exact table the acceptance criteria asked for:

| State | Glyph center (screen pt) | Beak center (screen pt) | Diff |
|---|---|---|---|
| Docked, no digits (first open) | 1238.75 | 1239.25 | 0.5pt |
| Pinned (`30%`) | 1209.75 | 1210.98 | 1.2pt |
| Unpinned again | 1238.75 | 1239.25 | 0.5pt |

All three states agree to within ~1pt — inside antialiasing/measurement
noise, i.e. "to the pixel" as required. B7 (panel left edge = `icon_left −
6px`) was also confirmed at every state: window X tracked the tray item's
left edge minus 6px throughout, refreshing correctly on pin/unpin instead of
staying frozen at the value from the last click.

Added `glyph_offset_tests` in `tray_render.rs`/`lib.rs` (three unit tests)
covering the bare-glyph case, a wider pinned-digits case, and the
no-rect-available fallback.

## 2. Row layout fault — reproduced, root-caused, fixed

Reproduced against `mockClient.ts`'s `claude:claude-team` account (which has
a `Fable`-scoped weekly window) in a real browser via the `chrome-devtools`
MCP, then confirmed the captain's own description precisely: the `Fable` tag
sat visibly higher than its row's baseline, out of line with the neighboring
`Weekly`/`45%` text.

Root cause, found by inspecting computed styles rather than guessing:
`LimitWindow.jsx`'s scope-tag `Badge` overrides the base `Badge`'s
`display: inline-flex` with `inline-block`, apparently to make
`text-overflow: ellipsis` truncation work for long model names. Inside a
flex row, a flex item's *outer* display always blockifies regardless of
which of those two values you specify (confirmed live: `getComputedStyle`
reported `"flex"` for the un-overridden base and `"block"` for the override)
— so the override did nothing for truncation, but it did throw away the
badge's own internal `align-items: center`, which is what actually kept its
text vertically centered in the pill. Fix: delete the override; ellipsis
still works fine on the un-modified `inline-flex` base (`flex: 0 1 auto;
min-width: 0; max-width: 120px; overflow: hidden; text-overflow: ellipsis`
alone is sufficient).

**Verified**: `getComputedStyle` before/after showed the badge's own vertical
center moving from off-axis to matching the row exactly; re-checked visually
against every combination the row can produce — tag absent (unaffected),
short tag (`Fable`, 45%), long tag with truncation (`Claude Opus 4.5
(extend…`, 61%, confirmed still ellipsizing correctly), and both expanded
and collapsed row states. Added
`LimitWindow.test.jsx` (3 tests, using the existing but previously-unused
`@testing-library/react` + jsdom setup) as a regression guard — jsdom's CSS
engine doesn't model flex-item blockification the way a real browser does,
so the test asserts the one thing it *can* reliably check: the badge's
literal `display` never regresses back to `inline-block`.

## 3. Detached mode — chrome fixed and verified; drag fixed in code, unverified end-to-end; Magnet answered

**No system chrome — fixed and verified.** `set_detached` used to call
`window.set_decorations(true)` while detached, on the theory that a title
bar was needed to make the window draggable and "unmistakably a window."
With `titleBarStyle: Overlay` in `tauri.conf.json`, turning decorations on
paints real traffic lights over the content and a native title-bar strip
macOS renders regardless of the window's own transparency — exactly the
captain's screenshot: system buttons on a frame visibly bigger than the
332px panel, floating inset inside it. The handoff has no system chrome in
either state; decorations now stay off always. **Verified with a real
screenshot** of the actual detached window (dev build, real click +
synthetic header drag): no titlebar, no traffic lights, no oversized frame —
just the panel with its snap-back arrow, exactly matching the handoff.
Snap-back was also verified live: clicking it re-docks the window at the
exact expected position and restores the beak.

**Dragging — a real, provable bug found and fixed in code; not verified
end-to-end with a real cursor.** `App.tsx` used to call
`getCurrentWindow().startDragging()` only after the *first* JS `mousemove`
following mousedown. On macOS, `startDragging()` → tao's `drag_window()` →
`NSWindow.performWindowDragWithEvent` hands AppKit whatever
`NSApp.currentEvent()` happens to be *when the Rust side gets around to
running it* — read straight from `tao`'s own source
(`platform_impl/macos/window.rs`), it only synthesizes a substitute event
for one narrow case (a stale `AppKitDefined` event), not the general one.
That call arrives over an async `invoke()` IPC round-trip, so by the time it
lands, `currentEvent` is essentially never still the real mouseDown Apple's
own docs say to call this from. Fixed by calling `startDragging()`
synchronously, directly on the real mousedown — calling it on a click that
never moves is harmless, since AppKit's own tracking loop treats a
stationary mouseDown-then-mouseUp as a no-op, so "a plain click doesn't
detach" (C3) still holds; only the *native drag start* moved earlier, the
*visible* detach state (losing the beak, gaining the snap-back arrow) still
only flips on the first real movement, unchanged.

This fix could **not** be proven end-to-end here: a synthetic drag (posted
mousedown + a sequence of mouseDragged events + mouseup via `CGEventPost`,
the same mechanism `fm-quotos-click.sh` uses for tray clicks) correctly
triggered the app's own movement tracking (the snap-back arrow appeared,
confirming the "first movement detaches" logic ran) but never produced a
single native `Moved` window event, either before or after this fix —
checked directly with a temporary `WindowEvent::Moved` logger. This mirrors
the project's own already-documented finding that synthetic input doesn't
reliably drive every AppKit interaction on this specific machine (the tray-click
limitation noted in `AGENTS.md`); it's plausible the same limitation applies
to `performWindowDragWithEvent`, but that isn't proven either way from here.
**The captain needs to confirm dragging with a real cursor** — the fix is a
strict correctness improvement per Apple's own documented usage regardless,
but "does it actually drag now" is not something this agent could settle
conclusively.

**The Magnet question — answered by reasoning about the window's own
configuration, not by driving Magnet (which is out of bounds here).**
`resizable: false` in `tauri.conf.json` only disables the *native*
resize-handle drag; it does not stop a third-party window manager like
Magnet, which resizes windows via the Accessibility API's
`kAXSizeAttribute` setter — that goes straight to `NSWindow`'s `-setFrame:`,
which does not consult the style-mask resizable bit at all. So a Magnet
snap action landing on the detached panel could, in principle, resize a
window whose entire layout (panel width, shadow margin, beak offset) assumes
exactly 360×560 logical and has no responsive fallback. Added a
`WindowEvent::Resized` guard (`lib.rs`, in the same handler as the existing
blur-hide logic) that snaps the size straight back to 360×560 whenever it
drifts, in either docked or detached state — it does not fight a Magnet
*move* (only a *resize*), so repositioning still works as Magnet intends,
but a resize can't leave the popover in a broken, half-reflowed state. This
was reasoned from the window's configuration and `tao`'s/AppKit's documented
behavior, not observed against a live Magnet action (out of bounds per the
task's own constraints) — worth the captain confirming if he uses a Magnet
snap on the detached panel.

## 4. Checklist sweep — two more real bugs found

Walked the unmarked sections of `checklist.md` (A–I) against the running dev
build and `mockClient.ts` in the browser. Most items were already correct
and are now positively confirmed rather than just assumed (B1/B2 panel
geometry via computed styles; B6/B7 position via `CGWindowListCopyWindowInfo`;
D2 nothing shifts on hover; D3/F20 the "…" menu's exact four items; D11
rename in-place-field/Enter/Esc/empty-restores; D13/F-string exact "is no
longer tracked … Undo" wording; D14–D19 all six row states' dot colors,
badges, and footer text against real mock accounts including the
rate-limited overlay never overwriting a real diagnosis (I6); E1–E3/E6/E7
the Subscriptions screen's list, button styling, and restart hint;
F3/F14/F15/F17/F19/F22/F24 exact string matches; G5 pulse/shimmer timing;
I2 — incidentally exercised for real mid-session when this machine's active
menu bar genuinely moved to a second, negative-coordinate display, and the
panel still docked correctly; I8/I9 no build markers, no stray debug
writes). Two real defects were found and fixed:

- **A11 (tray "panel open" highlight) was entirely unimplemented** — no
  code anywhere set any highlighted/selected state on the tray button. Since
  the icon is already a custom composited bitmap (not a native image), the
  fix draws the handoff's own translucent rounded-rect
  (`rgba(255,255,255,0.20)` dark / `rgba(0,0,0,0.14)` light, 5px radius)
  behind the glyph+digits itself (`tray_render.rs`'s `draw_highlight_background`,
  composited with real alpha blending via a new `blend_pixel` — the old
  `put_pixel` did a flat overwrite, which would have silently discarded the
  highlight everywhere the glyph or a digit covers it). Wired through a new
  `AppState.tray_highlighted` flag and cached `last_tray_segments` (so
  toggling the highlight from native show/hide can repaint without the
  frontend resending pinned data), called from `show_panel`/`hide_panel`/the
  click-away and detached-toggle hide paths. **Verified live**: photographed
  the same tray icon closed vs. open (`fm-quotos-click.sh peek`, real
  screen), pixel-diffed the two — zero difference anywhere except an 18×18
  region exactly over the glyph, closed color `(15,15,15)` vs. open
  `(89,89,89)`, and a side-by-side crop confirms it visually reads as the
  intended soft highlight, not an artifact. Three new `tray_render.rs` unit
  tests guard the blend behavior itself (translucent when highlighted, fully
  transparent when not, digit color unmodified on top).
- **Escape while renaming (or entering a sign-in code) also closed the whole
  panel.** Neither input's `Escape` handler called `stopPropagation()`, so
  the keydown bubbled past the row up to `App.tsx`'s global
  `window`-level Escape listener, which hides the panel — a user meaning to
  cancel a rename would instead lose the whole panel. Fixed by stopping
  propagation in both handlers (`SubscriptionRow.jsx`). Verified live in the
  browser: Escape during a rename now reverts the field and leaves the panel
  open (previously reproduced with the bug present, confirmed by list
  content persisting after the fix).

Not independently re-verified this round (unchanged from the prior round's
already-documented state, or genuinely native-screen-only and out of this
round's reproduction budget): A3/A4/A6 (native NSStatusItem button chrome
proper, distinct from the glyph itself), I3 (menu bar hidden by a
full-screen app — would require launching another app, out of bounds), I7
(tray accessibility name — implemented in a prior round, not re-tested).

## Native verification method

All native claims above (beak/glyph geometry, window chrome, drag, Magnet
reasoning, the A11 highlight) were tested against a `com.quotos.desktop.dev`
build — never the captain's real `com.quotos.desktop.v2` identity or its
config directory. One real Claude account
(`~/.claude`, personal) was added and pinned briefly under that dev identity
purely to get a real, non-empty tray item to measure and photograph; nothing
was written to any real tracked list. Ground truth came from
`CGWindowListCopyWindowInfo` (via a small `xcrun swift -e` snippet) for
window bounds, `screencapture` + pixel centroid/diff analysis (Python +
Pillow) for the tray glyph, beak, and A11 highlight, and
`fm-quotos-click.sh`/plain Accessibility queries (`osascript`/System Events)
to read real UI element positions and drive real clicks and header drags —
never a coordinate guessed in advance. No Quotos process of the captain's
was running at any point this round, so nothing needed to be quit or
relaunched.

## Packaging

```
npx tauri build --config src-tauri/tauri.v3.conf.json   # from quotos-app/
```

`productName` "Quotos 3", `identifier` still `com.quotos.desktop.v2`
(unchanged from v2 on purpose — see `AGENTS.md`'s side-by-side-packaging
entry). This build was not launched by this agent, since doing so would
touch the captain's real tracked data under that shared identity.

```
quotos-app/src-tauri/target/release/bundle/macos/Quotos 3.app
```

`npm run build`'s plain production `dist/` (requested mid-task, so it can be
served independently of this agent/worktree) is also left in place:

```
quotos-app/dist/
```

Both are left inside this worktree for firstmate to deliver.
