# Quotos — round 4, panel/Space + subscription-view half (branch `fm/quotos-panel-ux-p1`)

> This file is rewritten each round. Round 4 ships as **Quotos 3.1** and is the
> merge of two branches; the section below covers only this one (panel and
> Space behaviour, view synchronisation, menu rendering). The other half —
> credential reading, sign-in classification, revival — landed separately in
> `5ab9d5a`. Everything under "Round 3" further down is the previous round's
> record, kept because its measurements are still the basis for the tray and
> placement geometry.

**176 automated tests pass** (112 vitest, 64 `cargo test`); `tsc --noEmit` and
`cargo check` are clean.

## The headline bug: a tray click threw the captain off his full-screen Space

His 2026-08-15 screencast catches the Space-transition animation mid-slide at
t=6.6: he clicks the tray icon from a full-screen browser, macOS leaves that
Space, and the panel opens on the desktop behind it.

Rounds 2 and 3 attacked this as *window membership* —
`CanJoinAllSpaces | FullScreenAuxiliary`, `NSStatusWindowLevel`,
`orderFrontRegardless`. All three are necessary, all three are kept, and none
of them is the trigger. The trigger is that every open calls
`WebviewWindow::set_focus()`, whose second half is
`activateIgnoringOtherApps: YES` (`tao`'s `util::set_focus`) — and activating a
*different application* while a full-screen Space is frontmost is precisely
what makes macOS leave it. Round 3 knew about that call and kept it on purpose,
because dropping it left the window non-key, and a non-key window breaks
click-away-to-close, the rename field and the sign-in field.

That trade is real for an `NSWindow` and dissolves for an `NSPanel`:
`src-tauri/src/panel_window.rs` converts the window to a **non-activating
panel**, the AppKit type that can hold keyboard focus while another
application stays active, and the show path stops activating at all.

**Measured here, same-machine A/B against `QUOTOS_PANEL_MODE=window` (the old
path, kept as an escape hatch):**

| | frontmost app after the panel opens | panel `isKeyWindow` |
|---|---|---|
| old path | **quotos-app** — it took activation | true |
| this path | **unchanged** (Arc / iTerm2, whatever was in front) | true |

Read from a separate process, because `-[NSApplication isActive]` inside an
accessory app reports `true` either way and cannot tell them apart.
`windowDidResignKey` still arrives when another app takes focus, so
click-away-to-close is unaffected — observed live, in the same run.

**Honest limit: the full-screen end-to-end is unverified.** An agent here may
not put an app into full screen. What is verified is the mechanism — the
application no longer activates — which is the condition macOS needs in order
to stay put. The captain's own check is the proof.

Also verified on the real build, from a genuine tray click (not a synthetic
one): the panel opens docked at the computed position, `isKeyWindow=true`,
frontmost application unchanged, and it hides on the next focus loss.

## The Subscriptions view kept offering [Remove] for accounts already dropped

"Stop tracking" only *scheduled* a removal that a timer inside `App.tsx`
applied five seconds later, so the panel and the Subscriptions screen watched
two different facts. In his recording: both Undo rows visible at t=24.25, both
accounts still listed as tracked at t=25.0, "Claude Team" flipping at t≈26.0
and "Claude Max" at t≈29.2 — each exactly five seconds after its own click.

Untracking is immediate now, everywhere: persistence, the tray digits and the
Subscriptions screen all read `trackedSubscriptions`, and the undo window is
only a slot the panel keeps (`Subscription.pendingRemoval`). Measured in the
live app: with the panel still showing "Claude Team is no longer tracked /
Undo", the Subscriptions screen already lists that account as **[Add]**.

Names are consistent between the two views as well — the id-derived fallback
is title-cased ("claude team" → "Claude Team"), and a label a real read
reported is remembered for the session, so an account keeps the name the
captain knows it by after it stops being tracked instead of reverting to the
config directory's.

## The row menu was drawn inside the panel's scroll box

Opening a bottom row's "…" menu clipped "Stop tracking" at the panel's bottom
edge *and* made a two-row panel scrollable, so reaching the clipped item meant
scrolling the rows out from under the cursor first. Both come from one
property: an absolutely-positioned element is clipped by, and counts toward the
scroll extent of, its scroll-container ancestor. The menu is `position: fixed`
now, anchored to its own button, flipping above it near the window's bottom.

Measured in the live WKWebView (temporary instrumentation, removed before
commit — jsdom has no layout engine and there is no headless browser here):

| state | panel | body | scrollHeight vs clientHeight | menu |
|---|---|---|---|---|
| 2 rows | 332×354 | 332×274 | 274 == 274 (no scrollbar) | — |
| 2 rows, bottom row's menu open | 332×354 | 332×274 | **274 == 274, unchanged** | fixed, 178×122 at (154,232), fully inside the 360×560 window |
| after Stop tracking (row + Undo row) | 332×269 | 332×190 | 190 == 190 | — |
| that row's menu open | — | 332×190 | 190 == 190 | fixed, inside the window |

`offsetWidth == clientWidth == 332` throughout, i.e. no scrollbar gutter
either. A screenshot of the menu-open state is what the fix looks like: the
full menu, including "Stop tracking", overlaying the panel with the rows
exactly where they were.

## The reported row-removal lag did not reproduce

Frame-stepping the captain's own recording puts the click ripple on "Stop
tracking" at **t=20.80** and the Undo row on screen at **t=20.92** — ~120 ms,
not the ~1–1.5 s in the brief. The perceptible delay before it was the menu
having to be scrolled into view first (above); the earlier timestamp was the
cursor arriving on the item, not the click.

Real main-thread blocking *was* found while measuring, and is fixed anyway: a
`#[tauri::command]` without `(async)` runs inline on the main thread, and
`list_accounts` forks a `security(1)` process per candidate account while
`save_tracked` `fsync`s. Both are `(async)` now, as is `load_tracked`.
`set_tray_status` (which must stay on the main thread) now returns early when
the digits are unchanged and only re-docks the open panel when the icon's width
actually changed; the save effect no longer rewrites the tracked file on every
automatic read.

## Notes for whoever picks this up next

- A bare `cargo build` binary renders **nothing** — it loads `build.devUrl`
  instead of the bundled assets, and a transparent window with no page looks
  exactly like "the panel opened on another Space". Use `npm run tauri dev`.
  With it, screenshots of this app's own windows work fine.
- If the captain reports the panel "not opening" while an agent is testing,
  suspect two Quotos icons in his menu bar before suspecting the code.

---

# Round 3 (previous) — visual precision pass (branch `fm/quotos-polish-p1`)

The captain ran the delivered build and found the panel "not neat": the beak
drifted off the glyph, a row's `Fable` tag looked broken, detaching produced a
titled system window that wouldn't drag — and then, testing live, that the
panel *"мерцает… прыгает по экрану… не появляется где должна"*, with the glitch
appearing on the display he had **not** clicked on.

This file describes each fix, how it was verified, and — importantly — what is
still honestly **unverified**. It is rewritten each round, not appended to.

**124 automated tests pass** (74 vitest, 50 `cargo test`); `tsc --noEmit` and
`cargo check` are clean.

---

## 1. The flicker / jump / wrong-monitor defect — three real causes

Three separate, mechanical bugs. Each is sufficient on its own to produce what
he described; together they explain every symptom, including why it looked
like a two-monitor problem.

### 1a. There is no global "physical pixel" coordinate space, and three APIs each invented a different one

This is the root cause, and it is worth stating precisely because the whole
module was built on the assumption it disproves. macOS has exactly one
coordinate space in which a multi-display layout has a single meaning: the
**global point space** (`CGDisplayBounds` / `NSScreen.frame`). There is no
global *pixel* space at all — "physical pixels" are only defined relative to
one display's own scale factor.

Three APIs this code depends on each hand out a `Physical*` type that is really
"global points × **some** display's scale factor", and they disagree about
which display's:

| source | value | scale used |
|---|---|---|
| `TrayIconEvent`'s `rect` (`tray-icon` 0.24.2 `get_tray_rect`) | tray item frame | the **menu bar display**'s scale |
| `Monitor::position()`/`size()` (`tao` 0.35.3 `monitor.rs`) | `CGDisplayBounds` | **that monitor's own** scale |
| `set_position(Physical)` / `WindowEvent::Moved` (`tao` `window.rs`, `window_delegate.rs`) | window frame | the **window's current** scale |

All three read from the crate sources, not inferred. On a single-display
machine they coincide and the bug is invisible. On his setup — built-in Retina
at point `(0,0,1728,1117)` scale 2, external LG at `(-2560,-855,2560,2880)`
scale 1 (both measured live) — they diverge:

* A tray click on the built-in gives `tray_x = 2366` ("1183 points × 2"). Fed
  to `set_position` while the window happens to sit on the *LG* (scale 1),
  `tao` divides by **1** and places it at point x = 2354 — past the right edge
  of every display. The window is revealed at its stale position first (see 1b)
  and then vanishes: *"панель мерцает только"*, flickering **on the other
  monitor**, which is simply where it was last left.
* The mirror case divides by 2 and lands the panel at half the intended offset
  from the LG's origin — visible, right display, wrong place; the correction
  then re-runs it with the window's now-correct scale and it snaps across:
  *"она прыгает по экрану"*.

Corroboration from the previous round's own logs: the "unrelated AppKit-internal
positions" it recorded were `x=-2106` and `x=4712` — both within rounding of
exactly 2× or ½× a legitimate coordinate here. That is a scale-factor
signature, not an AppKit default placement.

**Fixed** by converting to points at the edges and never leaving them:
`DisplayPoints`, `resolve_tray_point` (which recovers the tray rect's true
point position by trying each display's own scale and keeping the one whose
quotient actually lands inside that display), and placement via
`LogicalPosition` — which `tao`'s `Position::to_logical` passes through
untouched, so no scale factor is consulted on the way out either.

### 1b. The window was revealed before it was moved

`tao`'s `set_outer_position` ends in `util::set_frame_top_left_point_**async**`
(dispatched to the main queue), while `set_visible(true)` ends in
`make_key_and_order_front_**sync**` (inline). Called in either order from the
tray handler — already on the main thread — the window becomes visible at its
stale position and only moves a runloop turn later. A guaranteed one-frame
flash at wherever it last was, which on two displays is routinely the other
one. **Fixed**: `place_window_top_left_sync` sets the frame directly through
`NSWindow` on the main thread, and `show_panel` positions *before* showing.

### 1c. `top_px = 32 * scale` is a hardcoded menu bar height

Firstmate caught this, and the captain's standing rule — *"нигде не
хардкодите мой сетап мониторов"* — is the reason it had to go regardless. The
handoff's own words for 32 are *"6px под меню-баром"*: a menu bar height plus a
gap. Measured live on his two displays: the **notched built-in's menu bar is
33pt**, an unnotched one is 24–30pt. So `32` put the panel's top edge *inside*
the built-in's menu bar, and AppKit clamped it back out to exactly 33 — which
is precisely the unexplained 1pt offset I first measured. One constant, two
different wrong answers on one machine.

**Fixed**: derived per display from `NSScreen.visibleFrame`, with a fallback
that reconstructs the bar from the tray item's own rect (a status item is
centred in its bar). That fallback is not decorative — inside a full-screen
Space the menu bar is auto-hidden and `visibleFrame` reports no bar at all even
while the bar sits revealed on screen. Both paths were checked to land on the
same answer (33) on the built-in.

### Verified, with numbers

Dev-identity build (`com.quotos.desktop.dev`), one real tray click via
`fm-quotos-click.sh`, window-server geometry sampled every 15ms across the
whole open (`CGWindowListCopyWindowInfo` in a Swift sampler), cross-checked
against the app's own instrumented trace (`QUOTOS_DEBUG_POS=1`):

| | value |
|---|---|
| tray rect as `tray-icon` reports it | `(2444, 0)` physical-ish |
| resolved to | display 0 (built-in), point `(1222, 0)` |
| computed layout | `x=1216, y=39, beak_left=3` (pre-R3-10 geometry) |
| `NSWindow` frame after show | `(1216, 39, 360, 560)` |
| window-server bounds | `(1216.0, 39.0, 360.0, 560.0)` |
| position changes during the entire open | **1** |

Computed = AppKit = window server, exactly, and the window moves **once**. The
earlier behaviour was four relocations per click, two to coordinates nowhere
near a real display. A later run with pinned digits gave beak centre
`1160.25 + 14 + 20 + 10 = 1204.25` against a glyph centre of `1204.25` —
identical.

---

## 2. The beak — fused into the panel, then resized on the captain's own orders

**Fused (firstmate's finding).** The beak used to be a second translucent
element stacked on the panel: the same `rgba(19,21,24,0.86)` painted twice at
the overlap, and `backdrop-filter: none` against the panel's blur — a visible
seam over a bright desktop. It is now **one shape**: a single SVG path
(`buildPanelOutlinePath`) used both as a `clip-path` for one fill+blur layer
and as one 0.5px stroke tracing the whole outline, beak edges included, with no
border at the join. Verified in a real browser over a hard-edged
white/red/yellow/blue backdrop at 8× magnification: one material, one blur, one
continuous hairline, no seam, no double-darkening.

**Then resized, three ways, on his explicit instruction — each one overriding
the handoff:**

1. *"он маленький"* → base 12 → **20pt**, height 7 → **10pt**.
2. *"слишком близко к левому краю"* → the notch now sits **20pt** inside the
   panel's own left edge, well clear of the 12pt corner radius. Since its
   centre must still be exactly the glyph's centre, this is achieved by moving
   the **panel** further left, not the beak: the window x is now *derived from*
   where the beak needs to be, replacing the handoff's `icon_left − 6` rule.
   Both constraints therefore hold by construction at whatever glyph offset the
   tray reports.
3. *"слишком низко от топбара… почти в иконке"* → the layout now solves for the
   beak's **tip** rather than the panel's top edge, and `app.css`'s
   `padding-top` (the transparent window's own internal inset, the other half
   of the gap) came down 14 → 12 to match the beak's reserve exactly. The tip
   now lands 2pt under the menu bar. The shadow margin is untouched at the
   sides, and `--shadow-popover`'s top bleed is zero by construction
   (`0 6px 16px -4px` reaches −2px above the box), so nothing was traded away
   to close the gap.

Checklist items **B3, B6 and B7 are therefore deliberately superseded** by his
instruction and no longer match the written handoff.

## 3. The tray "panel open" highlight now has horizontal air

Firstmate: it hugged the glyph and digits instead of reading as a pressed menu
bar button. The composited image now carries 6pt of padding per side
(`tray_render`'s `SIDE_PAD_PX`, the handoff's own button padding), applied
**unconditionally** — highlighted or not — so the glyph cannot shift when the
highlight toggles, which would move the beak. `lib.rs` reads that inset from
`tray_render` rather than duplicating it. **Verified**: photographed the real
menu bar with the panel open (`fm-quotos-click.sh peek`) — the highlight reads
as a proper button with air on both sides, in proportion with the neighbouring
system items.

## 4. Row layout, detached chrome, Magnet — from the earlier pass, unchanged

Already landed and confirmed (firstmate re-verified the row independently):
the `Fable` badge's `display: inline-block` override was discarding the base
`Badge`'s `align-items: center` (a flex item's outer display blockifies either
way, so the override never helped the truncation it was added for);
`set_decorations(true)` while detached was what painted traffic lights on an
oversized frame, and decorations now stay off in both states; `startDragging()`
is called synchronously on the real mousedown, per Apple's documented use of
`performWindowDragWithEvent`; and a `WindowEvent::Resized` guard snaps the
fixed-size popover back to 360×560 so a Magnet snap cannot leave it
half-reflowed (Magnet resizes via the Accessibility API's `kAXSizeAttribute`,
which bypasses `resizable: false` entirely). A Magnet *move* is deliberately
left alone.

`titleBarStyle: "Overlay"`/`hiddenTitle` are now removed from
`tauri.conf.json` as well — with decorations permanently off they were inert,
and removing them makes "no traffic lights, ever" structural rather than
conditional.

Also fixed this round: `App.tsx`'s `BEAK_LEFT_FALLBACK` no longer applies in
the native build. It starts at `null` there, so a missed `panel-beak-offset`
event draws **no** beak rather than one confidently placed where it is right
for nobody. The mock harness keeps the fallback.

---

## What is NOT verified, and why

### The full-screen Space — the captain's live reproduction, still open

He isolated it after the coordinate fix landed:

> Оно открывается на основном столе (которого я не вижу) и закрывается когда я
> на него перехожу.

From an ordinary desktop it works on both displays. From inside another
application's **full-screen Space** the panel is ordered onto the default
desktop Space instead, where he never sees it.

What I changed for it: collection behaviour is now
`CanJoinAllSpaces | FullScreenAuxiliary | Transient | IgnoresCycle` (the
`FullScreenAuxiliary` half was never set by any earlier round), the window
level is `NSStatusWindowLevel` rather than floating, and `show_panel` calls
`orderFrontRegardless` in addition to the existing focus path.

**None of it is confirmed.** A real full-screen Space happened to be active on
this machine, so I could test it directly — and `-[NSWindow isOnActiveSpace]`
read `false` under *every* variant:

* five collection behaviours, including `CanJoinAllSpaces` alone and `empty()`
* window levels 3, 25 and 101
* with and without `activateIgnoringOtherApps`
* launched by direct exec and through LaunchServices

Identical results for `CanJoinAllSpaces` and `empty()` mean the measurement is
not discriminating here — it is evidence about this environment, not about the
fix. I will not claim it works. `QUOTOS_DEBUG_SPACE_BEHAVIOR` and
`QUOTOS_DEBUG_WINDOW_LEVEL` are left in the build so the next attempt can sweep
variants against a real click without rebuilding.

Dropping `set_focus()` in favour of `orderFrontRegardless` alone was tried and
measured to leave the window non-key, which would break click-away-to-close
(it rides on `Focused(false)`) and the rename/sign-in text fields — a certain
regression for an unproven fix, so both calls are kept.

### Other honest gaps

- **The docked panel could not be photographed beside the menu bar.** It is not
  on the active Space here (same issue), so `screencapture` cannot see it. Its
  geometry is proven from the window server instead; its *appearance* was
  judged in a real browser at magnification, over a bright backdrop.
- **Native dragging** still has not been confirmed with a real cursor —
  synthetic drags produce no `WindowEvent::Moved` on this machine, with or
  without the fix.
- **Magnet** was reasoned about from the window's configuration, never driven
  (out of bounds).

## Diagnostics left in the build (all opt-in, all off by default)

`QUOTOS_DEBUG_POS` (full placement trace + Space diagnostics),
`QUOTOS_DEBUG_AUTO_OPEN` (opens the panel from the tray rect a few seconds
after launch, with **no click** — this is what made the Space question
iterable without touching the captain's menu bar),
`QUOTOS_DEBUG_KEEP_OPEN` (suppresses hide-on-blur so the panel can be
photographed), `QUOTOS_DEBUG_SPACE_BEHAVIOR`, `QUOTOS_DEBUG_WINDOW_LEVEL`.
No shipped run writes anything to stderr on its own.

## Packaging

```
npx tauri build --config src-tauri/tauri.v3.conf.json   # from quotos-app/
```

`productName` "Quotos 3", `identifier` still `com.quotos.desktop.v2` (shared
with v2 on purpose — see `AGENTS.md`'s side-by-side-packaging entry). Not
launched by this agent: doing so would touch his real tracked data under that
shared identity. Every native measurement above used a
`com.quotos.desktop.dev` build.

```
quotos-app/src-tauri/target/release/bundle/macos/Quotos 3.app
```
