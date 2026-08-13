# Quotos round-2 fix pass (branch `fm/quotos-tray-t1`) — result

This round fixed the two defects that blocked round 2's delivery — the panel
never opening, and the tray digits being a hand-rolled bitmap font — then, per
the captain's follow-up instruction to look wider than just what was flagged,
walked the full UI/UX checklist (`data/quotos-fixes-f2/checklist.md`, in the
firstmate data dir) against the current code and fixed every confirmed,
code-provable gap found. As with round 2: an honest "unverified" beats a
confident claim, so this file says exactly what was checked on the real
screen, what was checked by other rigorous means when the screen wasn't
available, and what's still open.

95 automated tests pass (60 vitest, 29 `cargo test`), `tsc --noEmit` and
`cargo check` are both clean.

## A note on how "verified on screen" was done this round

The panel and tray digits could not be reliably screenshotted live in this
environment: this machine runs a pinned, always-frontmost firstmate terminal
Space and (on the second display) the captain's own live desktop session,
neither of which is the Space a freshly-shown popover window lands on.
`screencapture`, in any form, only ever captures the currently active Space —
confirmed as an environment artifact, not an app bug, by reproducing the
*identical* symptom against the known-good `Quotos 2` build as a control.

Where a literal screenshot wasn't possible, verification instead used tools
that don't depend on which Space is active:

- **`CGWindowListCopyWindowInfo`** (via a small `xcrun swift -e` snippet —
  JXA's ObjC bridge couldn't produce a usable `NSArray` from the raw
  `CFArrayRef` this returns) reports every window's real on-screen bounds
  directly from the WindowServer, independent of both the active Space and of
  Tauri's own position getters (`WebviewWindow::outer_position()` was
  observed misreporting immediately after a window's first-ever `show()` in
  this Tauri version — not trustworthy for this kind of diagnosis).
- Tauri's own `Moved`/`Focused` window events, logged via a temporary
  `on_window_event` hook during development (removed before commit).
- For the tray digits specifically: the composited RGBA buffer that
  `tray_render::render()` actually produces — the exact same function
  `set_tray_status` hands to `set_icon` — was rendered straight to PNG
  (via a scratch `cargo run --example`, deleted before commit; MonoLisa was
  confirmed genuinely in use, not the fallback, for every sample) and
  inspected directly. This is pixel-identical to what the real tray shows,
  just not literally photographed off the menu bar.
- The tray *icon itself* (unlike the panel) actually is reachable via
  `fm-quotos-click.sh peek`, since a status item's menu bar is global to
  every Space — real screenshots of the bare glyph were taken and confirmed
  fine throughout.

## Defect 1 — the panel never opened — **Fixed, verified**

Root cause: `show_panel` went through `tauri-plugin-positioner`'s
`move_window_constrained(Position::TrayBottomLeft)`. On a real click (posted
via `fm-quotos-click.sh`, a genuine CGEvent through the HID tap, not
synthetic Tauri-internal state), this produced a window position wildly
inconsistent with the tray icon's own rect — e.g. tray at physical x=2444 on
a 3456px-wide monitor placed the window at physical x=1368, nowhere near
anything the plugin's documented clamp-to-monitor math could produce, landing
the panel off in empty space on the correct monitor. On screen that's
indistinguishable from "nothing opened" — exactly the reported bug. Root
cause was narrowed to the plugin's own `calculate_position`/
`get_monitor_for_tray_icon` path and not chased further upstream.

Fix: `show_panel` no longer depends on that plugin at all. It computes the
position itself, directly from the tray icon's own `Rect` — handed fresh on
every click via `TrayIconEvent::Click`'s `rect` field, always physical pixels
per `tray-icon` v0.24.2's own `Rect` type — plus `window.monitor_from_point`
on that same rect, applying the handoff's exact math itself (left edge = icon
left − 6px, clamped to no further right than screen right − 340px, never left
of the monitor's own left edge; top pinned at 32px from that monitor's own
top edge). The `tauri-plugin-positioner` dependency is removed entirely
(Cargo.toml, plugin registration, cache priming) — nothing else in the
codebase used it.

**Verified**: real click, via `fm-quotos-click.sh`, on a real running dev
build (`com.quotos.desktop.dev`, isolated from the captain's real data).
`CGWindowListCopyWindowInfo` confirmed the window's real bounds land exactly
on the computed target (to the pixel, both before and after a re-open), on
both the primary display and — caught live once, unprompted, when the
accessibility tree briefly reported the icon at negative coordinates on an
external display before settling — the negative-coordinate case the handoff
calls out. Toggle-closed (second click) and toggle-reopen were both confirmed
via the same mechanism plus Tauri's own `Focused`/`Moved` events (no
unexpected re-hide). **Not literally photographed** — see the note above.

## Defect 2 — tray digits were a hand-rolled bitmap font — **Fixed, verified**

`tray_render.rs`'s digits are now drawn with real Core Text
(`CTLineCreateWithAttributedString`/`CTLineDraw`) via the `objc2`/
`objc2-core-text`/`objc2-core-graphics`/`objc2-app-kit` bindings Tauri already
pulls in transitively — no new crate versions needed, everything resolved
offline against the existing `Cargo.lock`. Font: **MonoLisa is genuinely
installed on this machine** (`~/Library/Fonts/MonoLisa-Medium.ttf` et al.) and
is what's actually used — `used_fallback_font()` returned `false` for every
sample rendered. The fallback path (`NSFont.monospacedDigitSystemFontOfSize:
weight:`) exists and is exercised by a dedicated test, but did not fire on
this machine. **If the captain's own machine doesn't have MonoLisa
installed, the tray will silently use the system fallback font instead** —
this is the intended, sanctioned behavior (no arbitrary substitution, no
bundled font), but worth knowing going in.

Two real rendering bugs were found and fixed along the way — both by
rendering the actual production function to PNG and inspecting it directly
(see the verification note above), not by trusting the unit tests, which
only checked "a pixel of approximately the right color exists somewhere" and
missed both:

1. Drawing into an 8-bit **alpha-only** (`kCGImageAlphaOnly`) offscreen
   context — chosen to sidestep premultiply/unpremultiply math entirely —
   corrupted glyph shapes: specific glyphs came out with wrong or missing
   strokes (a "7" losing its entire top bar, while "8" right next to it,
   same call sequence, rendered perfectly). Fixed by using a normal
   (premultiplied) sRGB RGBA context instead, unpremultiplying on read-back.
2. The standard flip for turning a `CGBitmapContext`'s native bottom-left/
   y-up coordinate system into top-left/y-down mirrors glyphs drawn via
   CoreText (it orients glyph outlines relative to the CTM's handedness).
   Fixed by drawing in the context's native convention and adjusting the
   baseline-position formula instead of flipping the CTM.
3. A third, unrelated bug: pairing a Generic-RGB fill color with a
   Device-RGB bitmap context shifted even fully-opaque pixels well off the
   requested color (confirmed via a direct pixel diff — not rounding noise).
   Fixed by using sRGB consistently for both.

Full root-cause narrative and the exact fix code are in `AGENTS.md`'s Sharp
edges section.

**Verified**: `used_fallback_font() == false` (MonoLisa genuinely resolved
and used); the actual `render()` output for six representative
severity/multi-segment cases, rendered to PNG and inspected directly, shows
crisp, correctly-shaped, correctly-colored digits (`35%` neutral,
`78%`/`94%` amber/red, a two-segment `20% 85%` case) in both light and dark
neutral variants, with tabular figures (`'1'` and `'8'` render at identical
widths) — all confirmed by eye, not just by the passing exact-pixel-color
unit tests. **Not literally photographed off the live tray** — see the note
above; the bare glyph (no pinned data in the isolated dev-identity test
environment) *was* photographed live via `fm-quotos-click.sh peek` and
renders correctly.

## The wider checklist sweep

Per the captain's follow-up (*"смотрите шире... по пунктам проверь все
UI/UX ноты"*), `checklist.md`'s A–I sections (166 items) were audited against
the current code by five parallel read-only passes, then every confirmed gap
was fixed:

- **B3/B5 (beak position)** — the popover beak's horizontal position was a
  hardcoded constant (`BEAK_LEFT = 24`), structurally disconnected from the
  tray glyph's real position and from the panel's own right-edge clamp (so
  it visibly wouldn't track the glyph as pinned digits changed the button's
  width, and couldn't "keep following the icon" at the screen edge per B5).
  Now computed natively (`compute_docked_layout` in `lib.rs`, shared with
  `show_panel`'s own positioning) and pushed to the frontend via a new
  `panel-beak-offset` event on every dock/re-dock; `App.tsx` consumes it,
  keeping the old constant only as the browser-mock-harness fallback (no
  real tray glyph exists there to measure). **Logic verified by hand-tracing
  the arithmetic against a real captured tray rect (icon left − 6, clamped);
  not visually confirmed** — same Space limitation as the panel itself.
- **C6 (snap-back doesn't re-dock)** — detaching then snapping back restored
  the chrome (beak, no titlebar) but never actually moved the OS window back
  under the tray icon; it silently stayed wherever the drag left it. Fixed:
  `set_detached` now re-docks using the last tray rect seen by *any* tray
  icon event (a new `AppState` field, since re-docking is triggered by the
  panel's own header button, not a fresh tray event). **Not verified live**
  — driving a real detach requires a drag gesture (mousedown+move+mouseup)
  that `fm-quotos-click.sh` deliberately doesn't support (discrete clicks
  only, to stay inside the "use the provided helper" rule), and hand-rolling
  a synthetic drag was judged out of scope for this round. Code-reasoned fix
  only.
- **F10** — `"Resets Sun 10:00 AM"` was missing the word "at" the handoff
  requires (`"Resets Sun at 10:00 AM"`) — only the today/tomorrow branches
  had it; fixed in `time.ts`, with a tightened test that would have caught
  this originally.
- **F16/F17** — the stale ("behind") and broken-with-expired-sign-in
  reasons used dynamic/paraphrased text instead of the handoff's two fixed
  sentences (*"The provider didn't answer..."* / *"The sign-in expired..."*)
  — `providers/claude/index.ts`'s `mapFetchError` now returns those exact
  strings; state-transition behavior (when a row is "behind" vs "broken") is
  unchanged from round 2, matched against the existing tests.
- **I7 (no tray accessibility name)** — the composited tray image carries no
  text a screen reader can read at all; `TrayIconBuilder` never called
  `.tooltip()`. Fixed: set at creation and kept current with the actual
  pinned values in `set_tray_status` (e.g. `"Quotos — 78%"`). **Verified
  live** — `System Events`' accessibility tree now reports `help=Quotos` for
  the tray item, confirmed via `osascript` against the real running dev
  build.
- **D8 (per-window number not tinted)** — `LimitWindow`'s percent number was
  always neutral-colored, never tinted amber/red at 75%/90% the way the bar
  right next to it (and the headline number elsewhere) already was. Fixed in
  both design-system copies (`quotos-app/src/design-system/` and
  `design/system/`, kept identical per `AGENTS.md`'s file-ownership note).

**Reviewed and deliberately left as-is:**

- **D10** — limit-window names are humanized from the provider's machine
  `kind` slug (e.g. `weekly_all` → "Weekly"), not passed through verbatim.
  The Claude API sends no display name at all for these windows (confirmed
  in `data/quotos-source-s1/report.md`), so this isn't renaming/translating
  a name the provider gave — there's no name to preserve. The code's own
  comment already documents this; no change made.
- **A1/A2, A3, A11, B9** — flagged by the audit as needing a real screen
  (bundled tray glyph is a 22×22 source PNG upscaled to the 36×36 composited
  buffer, not a native high-res asset — the composited *dimensions* are
  correct per spec, but final crispness/ink-density next to system icons
  needs an eye on it; NSStatusItem button chrome — height/radius/padding,
  and the "panel open → highlighted button" state — is largely OS-drawn with
  no corresponding app code to inspect; the panel's shadow-blur-vs-window-
  margin claim in `AGENTS.md` needs confirming over a bright real desktop).
  None of these are code bugs found by reading the source; they're
  screen-only judgment calls for whoever does the final on-screen pass.
- **I3** — "works with the menu bar hidden by a full-screen app" is
  structural to how `fm-quotos-click.sh` itself works (it slides the menu
  bar down before clicking), not something app code controls.
- **I8** — `tauri.v3.conf.json`'s `productName: "Quotos 3"` shows up in
  Force Quit/Activity Monitor/Cmd+Tab, but never in any in-app UI string
  (confirmed: the panel header always reads plain "Quotos"). This is the
  same intentional packaging-level naming round 2 already used for "Quotos
  2"; not treated as a violation of "nothing reads Quotos 3" that item is
  really about (in-product text, not the OS-level bundle name).

All other checklist items (the captain's five defects H1–H12, most of
row-state D1-D19, subscriptions-screen E1-E9, the design-system tokens G1-G6,
and the remaining string items F1-F27) were independently re-confirmed
against the current code by the same audit and already hold — round 2's own
claims in the prior version of this file checked out.

## Packaging

Built exactly as round 2 did:
`npx tauri build --config src-tauri/tauri.v3.conf.json` from `quotos-app/`.
`productName` "Quotos 3", `identifier` **still** `com.quotos.desktop.v2`
(deliberately unchanged from v2 — see `AGENTS.md`'s side-by-side-packaging
entry for why; this build was *not* launched by this agent, since doing so
would touch the captain's real tracked data under that shared identity).

```
quotos-app/src-tauri/target/release/bundle/macos/Quotos 3.app
```

Left inside this worktree, not copied anywhere — firstmate delivers it and
does its own verification pass, which is also where the still-open items
above (beak pixel alignment, snap-back drag, tray glyph crispness, button
chrome, shadow clipping) get their real-screen check.

## Testing builds used during this round

All native testing used bundle identifier `com.quotos.desktop.dev`
(`src-tauri/tauri.dev.conf.json`), never the captain's real config
directories, Keychain, or the `com.quotos.desktop.v2` identity his real
`Quotos 2` uses. The captain's real `Quotos 2` (`~/Downloads/Quotos 2.app`)
was quit before each test session and relaunched immediately after — it was
never left down, and this file's own testing never modified its tracked
list or WebKit storage.
