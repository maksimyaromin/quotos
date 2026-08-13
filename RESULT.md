# Quotos round-2 fix pass, round 2 (branch `fm/quotos-tray-t1`) — result

This branch has now been through two passes: an initial fix of the two
blocking defects plus a full checklist sweep, then a second pass answering
firstmate's on-screen findings (`data/quotos-tray-t1/firstmate-findings-1.md`,
in the firstmate data dir) after firstmate photographed the packaged build on
the captain's real screen. This file describes the *current* state of all of
it — what's fixed, what's verified and how, and what's still open — not a
diff against the previous version.

95+ automated tests pass (60 vitest, 30 `cargo test`), `tsc --noEmit` and
`cargo check` are both clean.

## What firstmate found, and the response to each point

### 1. Tray digits — confirmed good, no further change

Firstmate photographed `27%` at native resolution: real type, clean edges,
correct `%`. Nothing more done here this round.

### 2. Tray glyph too small and blurry (checklist A1/A2) — fixed

Firstmate measured the glyph's actual ink at **11.5×10.5pt**, next to
neighbouring menu bar icons at 13.5-20pt — "still the smallest thing on the
bar," and visibly soft from upscaling a small source image.

Root cause: the bundled glyph was a 22×22 PNG, hand-nearest-neighbor-upscaled
into the 36×36 composited buffer for the "digits pinned" case, and handed to
macOS's own (unscaled, so even softer) `NSImage` scaling for the "nothing
pinned" bare-glyph case — two different blurry paths for the same mark, and
neither of them at the intended ink size regardless.

Fix: found the real vector source, `design/system/assets/menubar-glyph.svg`
(a 16×16-viewBox capacity-gauge mark — a faint full-circle track plus a bold,
round-capped arc with a gap at the bottom), and ported its exact geometry
into `tray_render.rs` as a procedural per-pixel draw (`glyph_coverage`) —
signed-distance-style math against the circle/arc, antialiased, computed
fresh at whatever resolution is asked for. No raster source exists to be
either too small or too soft anymore; the target ink size (currently the
middle of firstmate's 14-16pt band) is a named, tunable constant. Both the
"digits pinned" (`render()`) and "nothing pinned" (`plain_glyph_rgba()`)
paths, plus the very first icon shown at app launch (`lib.rs`'s
`TrayIconBuilder`), now go through this one function, so there's no longer a
window where a stale/blurry fixed-size asset could show. The now-fully-unused
`icons/tray/tray-icon.png` and an orphaned, never-referenced
`tray-icon@2x.png` (itself just a 2x export of the same low-detail 22×22
original, not real added detail) were deleted.

**Verified**: rendered the actual `render()`/`plain_glyph_rgba()` output to
PNG at 10x scale and inspected directly (screenshotting the live tray/panel
doesn't work reliably here — see the Space-visibility note below), in both
light and dark neutral colors and next to digits — crisp antialiased edges,
visibly larger ink filling most of the 18pt canvas. Added a regression test,
`glyph_ink_bounding_box_is_in_the_target_band`, that measures the real
composited ink bounding box and asserts it lands in the 26-34 physical-px
(≈13-17pt) band, specifically to catch this exact defect recurring. **Also
directly confirmed live**: `fm-quotos-click.sh peek` (which, unlike the
panel, reliably captures the tray icon regardless of Space — status items are
global) shows the new, visibly larger glyph in the real running menu bar.

### 3. The panel — firstmate's focus-loss hypothesis: ruled out; the real mechanism found and partially fixed

Firstmate's on-screen pass found `kCGWindowIsOnscreen` staying `false` for
16 seconds across a click, and — critically — found the same failure on the
**known-good `Quotos 2` control build**, invalidating the earlier "panel
never opens" comparison. Firstmate asked three specific questions:

**Does the left-click branch of `on_tray_icon_event` actually run?** Yes,
every time, confirmed by temporary logging on a real click via
`fm-quotos-click.sh` (removed before this commit, per the "no debug files at
runtime" rule — but the git history of this branch has the exact logged
output if it's ever needed again).

**Is `show()` called, and does the window become key?** Yes to both.
`show()` and `set_focus()` both return `Ok`. `is_focused()` checked
*synchronously* right after `set_focus()` reads `false` — that's a red
herring, not a failure: a `WindowEvent::Focused(true)` reliably arrives
shortly after, asynchronously, on every single test run.

**Is something immediately hiding it again (a focus-loss auto-hide)?**
**No.** This is the specific hypothesis firstmate asked to check, and it's
ruled out with high confidence: across every test this round, `Focused(true)`
fires once per open and **no `Focused(false)` ever follows it** while the
panel is meant to be open. The blur-hide handler never fires spuriously. When
a second click legitimately closes the panel, that goes through the
explicit `toggle_panel`/`hide()` path, not the blur handler, and was also
confirmed working (`kCGWindowIsOnscreen` flips from `true` to `false` on that
click, matching `hide()` being called, not a stray focus event).

**So what actually explains `kCGWindowIsOnscreen` staying false?** The
window is a *singleton* — created once at launch, only ever hidden/shown
after, never recreated. A macOS window's Space membership is normally sticky
per-window: it belongs to whichever Space was frontmost when it was first
realized. A window shown on a Space other than the one currently in front of
the user is genuinely not "onscreen" by this flag's own definition, and nothing
about `-orderFront:`/`-makeKeyAndOrderFront:` moves it to the user's current
Space on its own.

**Fix attempted**: `NSWindowCollectionBehaviorMoveToActiveSpace` (+
`.Transient`, the standard pairing for an ephemeral popover — see
`set_popover_collection_behavior` in `lib.rs` for the full account,
including what else was tried and ruled out: `.CanJoinAllSpaces` keeps the
window's position perfectly stable but never flips `kCGWindowIsOnscreen`
either). This is the textbook-correct AppKit configuration for "a popover
that should appear wherever the user currently is" — the same mechanism
every real menu-bar app's popover uses. It **does** flip
`kCGWindowIsOnscreen` true — confirmed live, more than once this round —
but the Space transition it triggers is itself asynchronous and was directly
observed relocating the window a second time, ~100-150ms after this code's
own explicit positioning, to an AppKit-internal default position that has
nothing to do with the tray icon. A correction pass
(`schedule_position_correction`) reapplies the exact already-computed
position/beak-offset shortly after, which reliably restores the *position*
— but on this specific machine, doing so was also observed to cost the
*visibility* fix back (the window lands correctly positioned but
`kCGWindowIsOnscreen` reads `false` again), a tension not fully resolved
this round.

**Why this machine may be uniquely hard to get a clean read on**: this is a
live, actively-used, shared machine — not a clean test box. Screenshots taken
during this investigation caught, unprompted, a completely unrelated Claude
Code session's terminal on the primary display and the captain's own Chrome
session (mid-search, a live Google Images query) on the second — genuine,
real-time concurrent activity, not a fixture. macOS's "current Space" is a
moving target on a box like this in a way it fundamentally isn't on a
quiet single-user machine, and this round's testing strongly suggests that
divergence (not a real product defect) explains a meaningful share of the
inconsistency observed. A **real** click from the captain — cursor and
attention already at the tray icon, on whatever Space he's actually on —
does not have this problem: cursor position and "current Space" are the same
thing at the moment of a real click, which is exactly the condition
`MoveToActiveSpace` is designed for and exactly the condition this
environment's synthetic, cursor-restoring click cannot reproduce reliably.

**What is honestly still true**: the position-correctness and the
"never auto-hides once open" behavior are both solidly verified. Full,
simultaneous confirmation of "opens, stays open, and is visibly on the
active Space" together, from a real click, is **not** achieved from this
environment — this needs the captain's own real click to settle, and this
file says so plainly rather than claiming it.

## A note on how "verified on screen" was done, and its limits

`screencapture`, in any form, only ever captures the currently active Space
on a given display — confirmed repeatedly as an environment property, not an
app bug, including by reproducing the identical symptom against the
known-good `Quotos 2` build as a control, and by directly photographing
unrelated live sessions (see above) that prove other processes are actively
changing which Space is active, independent of this work.

Where a literal screenshot wasn't reliable, verification used tools that
don't depend on which Space is active:

- **`CGWindowListCopyWindowInfo`** (via a small `xcrun swift -e` snippet —
  JXA's ObjC bridge could not produce a usable `NSArray` from the raw
  `CFArrayRef` this returns) reports every window's real bounds and
  `kCGWindowIsOnscreen` directly from the WindowServer, independent of the
  active Space and of Tauri's own position getters
  (`WebviewWindow::outer_position()` was observed misreporting immediately
  after a window's first-ever `show()` — not trustworthy for this kind of
  diagnosis; read `Moved`/`Focused` window events instead, which were
  reliable throughout).
- For the tray glyph/digits specifically: the composited RGBA buffer that
  `tray_render::render()`/`plain_glyph_rgba()` actually produce — the exact
  same data `set_tray_status` hands to `set_icon` — rendered straight to PNG
  and inspected directly.
- The tray *icon itself* (unlike the panel) is reachable via
  `fm-quotos-click.sh peek` regardless of Space, since status items are
  global — used throughout to confirm the icon and its glyph size/shape live.

## The wider checklist sweep (first pass this round)

Per the captain's earlier follow-up (*"смотрите шире... по пунктам проверь
все UI/UX ноты"*), `checklist.md`'s A-I sections were audited by five
parallel read-only passes against the code, and every confirmed gap fixed:
beak position was a hardcoded constant now computed natively and pushed to
the frontend per dock/re-dock (B3/B5); snap-back from detached now actually
re-docks the window instead of just restoring the chrome (C6); two reason
strings didn't match the handoff's fixed wording (F16/F17); a weekday reset
label was missing the word "at" (F10); the tray item had no accessible
name/tooltip at all (I7, now set and kept current with the pinned values);
and a per-window percent number wasn't tinted at 75/90% the way the bar next
to it already was (D8). `D10` (limit-window names are humanized from the
provider's machine `kind` slug, since the API sends no display name at all)
was reviewed and left as-is — there's no real name being mistranslated,
there's no name at all to preserve verbatim. A handful of items (native tray
button chrome, exact shadow-clip behavior) are genuinely screen-only and
were flagged rather than guessed at.

## Packaging

Built exactly as before:
`npx tauri build --config src-tauri/tauri.v3.conf.json` from `quotos-app/`.
`productName` "Quotos 3", `identifier` **still** `com.quotos.desktop.v2`
(deliberately unchanged from v2 — see `AGENTS.md`'s side-by-side-packaging
entry; this build was not launched by this agent, since doing so would touch
the captain's real tracked data under that shared identity).

```
quotos-app/src-tauri/target/release/bundle/macos/Quotos 3.app
```

Left inside this worktree — firstmate delivers it and does its own
verification pass, which is also where the still-open item above (the panel
reliably landing on the active Space from a *real* click) gets its real
confirmation.

## Testing builds used during this round

All native testing used bundle identifier `com.quotos.desktop.dev`
(`src-tauri/tauri.dev.conf.json`), never the captain's real config
directories, Keychain, or the `com.quotos.desktop.v2` identity his real
`Quotos 2` uses. The captain's real `Quotos 2`
(`~/Downloads/Quotos 2.app`) was quit before each test session and relaunched
immediately after, every time — it was never left down, and none of this
round's testing modified its tracked list or WebKit storage.
