# Quotos — current build state (panel background flicker + v5.1)

> This file is rewritten each round, not appended to. This round did one
> thing: it root-caused and fixed the captain's *"фон должен оставаться
> прозрачным всегда а не мерцать вот так"* — the panel surface intermittently
> stopping being see-through — and packaged the fix as build v5.1. The
> previous round's narrative (repository layout flatten + v5) is
> `git show c00ce58:RESULT.md`; the measurements still load-bearing are kept
> in the appendix.

**329 automated tests pass** (208 vitest, 121 `cargo test`) from the
repository root; `tsc --noEmit`, `cargo clippy --all-targets -D warnings`,
and `cargo fmt --check` are all clean. `npx tauri build --config
src-tauri/tauri.v5-1.conf.json` produces a runnable `.app`.

## The fix, in one line

`src/design-system/components/shell/Panel.jsx`'s fill layer no longer
carries `backdropFilter` / `WebkitBackdropFilter`
(`var(--blur-vibrancy)` = `saturate(180%) blur(28px)`). Nothing else about
the panel changed: `--bg-panel`'s 0.86 alpha, the colours, the rim, the
shadow and the beak geometry are all untouched.

## Diagnosis

### Visible symptom

The panel's own fill area flips between showing what is behind it **sharply**
and being a flat, featureless wash. The window's transparency is not
involved — the shadow margin outside the rounded panel keeps showing the
desktop in both states, and nothing about geometry, content or app state
changes across a flip.

### Which state is the fault, and what it actually is

Firstmate's framing had the two states the right way round but the mechanism
backwards: the flat state is not the backdrop filter failing, it is **the
backdrop filter succeeding**. Two independent measurements off the captain's
own frames settle it, and they agree.

**1. Sharpness.** A blur cannot be undone, so a blurred backdrop and an
unblurred one are told apart by high-frequency energy, not by brightness.
Mean `|dI/dx|` over a band of the Gmail window behind the panel:

| | good frames (3.00s, 5.25s) | flat frames (4.00s, 8.25s) |
|---|---|---|
| raw backdrop, measured *outside* the panel | 16.92 | 16.90 |
| the same band *inside* the panel | **2.22 / 2.16** | **0.041 / 0.007** |
| predicted inside, if unblurred at 0.14 alpha | 16.92 × 0.14 = **2.37** | — |

The good frames measure within 6–9% of an unblurred pass-through (the
shortfall is the screencast's own H.264 loss on low-contrast detail). **The
good state has no blur contribution at all.** The flat frames have none of
the backdrop's detail left.

**2. Colour.** Un-composite the flat surface through the 0.86 fill —
`B = (P − 0.86 × (19,21,24)) / 0.14` — and it resolves to a uniform
backdrop of **229**. The real backdrop's own mean, measured outside the
panel in the same frames, is **230.2**. That is what `blur(28px)` of that
window produces: the mean survives, the structure does not. (It also rules
out the alternatives: an *empty* backdrop root would leave the surface
identical to the good state, and compositing over opaque black would give
~18, not 49.)

So the flat surface is `rgba(19,21,24,0.86)` over a 28px blur of the real,
below-window content.

### Initiating trigger, masking condition, symptom — separated

- **Symptom:** the backdrop-filter pass replacing everything behind the panel
  with its own average.
- **Masking condition (why it is intermittent and mostly invisible):**
  Quotos's window is `transparent: true` and its page paints nothing behind
  the panel. Most of the time the backdrop pass therefore resolves against
  nothing, and the surface degenerates to a plain alpha composite of the
  0.86 fill over the desktop — which is exactly the see-through look the
  design wants. The flicker is not the filter turning on and off on top of a
  working design; it is a filter that is *usually* a no-op occasionally
  ceasing to be one.
- **Initiating trigger:** what makes a given frame's pass resolve against the
  below-window content instead of against nothing is a WebKit/window-server
  compositing decision, and **I did not identify it.** It is not any app
  event: his own frames show no state change, no re-render, no resize and no
  geometry change across a flip, and it happens docked and not while
  dragging. I could not reproduce the *transition* (see below), so anything
  more specific than "the backdrop pass sometimes has the below-window
  content available to it" would be invention.

The fix does not depend on knowing that trigger. With no backdrop-filter
there is no backdrop pass at all, so neither resolution can occur and the
surface is *always* the plain alpha composite — the exact state the good
frames show.

## Reproduction, on the real screen

Done in the real app on this machine, not in a browser. A dev-identity build
(`com.quotos.desktop`, so it could never touch the captain's tracked list or
spend his shared read budget — its config dir did not exist, so it started
with nothing tracked and made zero HTTP requests) was launched with
`QUOTOS_DEBUG_AUTO_OPEN=1 QUOTOS_DEBUG_KEEP_OPEN=1`, its window bounds read
from `CGWindowListCopyWindowInfo`, and the panel burst-captured with
`screencapture -x -o -R` in a shell loop (~14 fps, no sleep needed). Each
frame reduces to one number: the per-pixel brightness standard deviation of
a band inside the panel.

The backdrop was made observable by giving the *page* a striped background
(`html { background: repeating-linear-gradient(90deg, #fff 0 6px, #000 6px
12px) }`) — the root element's background is the document canvas, so it
needs no extra element and no new stacking context, the smallest disturbance
to the layer tree that still puts known content in the backdrop root. A 12px
stripe period against a 28px blur makes the metric binary.

| run | frames | band sd inside the panel | reading |
|---|---|---|---|
| filter present, panel idle | 90 over 6.2s | 2.291 — **every frame** | blurred |
| filter present, backdrop animated (continuous repaint) | 300 over ~20s | 2.236 – 2.306 | blurred |
| **filter removed** | 200 | **18.072 – 18.074** | sharp |
| control: same band *outside* the panel | — | 127.4 | raw stripes |

The stripes vanish completely under the panel with the filter and come
through sharp without it. The sharp values are the predicted composite
exactly: minimum 18 = `0.86 × 21` over a black stripe.

**What this did and did not reproduce.** It reproduces the mechanism — that
this element's backdrop-filter really does run in this window and really does
erase what is behind it — and it proves the counterfactual. It does **not**
reproduce the intermittency: with a non-empty backdrop root the filter was
applied in 390/390 frames, including 300 under continuous repaint pressure.
Telling the two states apart needs a bright, detailed backdrop *behind the
window*, and the only thing behind this window is a black terminal — a
blurred black backdrop and a sharp one are the same pixels. Rearranging what
is on the captain's screen is outside what an agent may do here, so **the
flip itself is unverified from an agent seat.** What is verified is that the
fix removes the only thing that can produce the flat state.

## Counterfactual and disconfirming checks

**Smallest counterfactual** (the one that should flip the outcome if the
explanation is right): delete the two style properties, change nothing else,
re-run the same burst. Result: sd 2.29 → 18.07, in 200/200 frames. Run.

**What would falsify the explanation, and the checks for it:**

1. *"The good state is also blurred, just less — so removing the filter will
   change how the panel looks."* Falsified by the sharpness table above: the
   good frames measure 2.2 against 2.37 predicted for a completely unblurred
   pass-through. There is no blur to lose.
2. *"Something behind the window changed across the flip, and the panel is
   innocent."* Falsified in the same frames: the raw backdrop measured
   *outside* the panel is 16.92 vs 16.90 and mean 230.25 vs 230.23 across a
   good/flat pair — identical. Only the panel's own fill area changed.
3. *"Removing the blur reopens the beak/panel seam"* — `AGENTS.md` argued
   the single-shape outline partly from the blur kernel not sampling across
   an element boundary, so that argument dies with the filter. Checked live
   over a bright backdrop (`#f2f3f5`, the harshest case; a dark one hides
   seams): beak and panel measure one uniform `(51,52,55)`, matching
   `0.86 × (19,21,24) + 0.14 × (242,243,245)` exactly, with no discontinuity
   at the join. No seam — the double-alpha half of the argument holds on its
   own, and `AGENTS.md` is corrected to say so.
4. *"Some other surface in the app still blurs."* `MenuBarTile.jsx` and
   `docs/design/system/ui_kits/quotos/index.html` also use a backdrop-filter,
   but neither is on any app render path (the app imports `Panel`,
   `IconButton`, `Button`, `SubscriptionRow` only) — they are design-system
   demo surfaces with real in-page content behind them, where a
   backdrop-filter behaves normally. Left alone deliberately. The built
   frontend bundle contains **zero** occurrences of `backdrop`.

## Regression test

`Panel.test.jsx` previously asserted the fill layer's `backdropFilter` was
non-empty. That assertion is **inverted, not deleted**, with the reason
written into the test file. Three new cases, all proven to fail against the
pre-fix component and pass after:

- the fill layer ships with no backdrop filter, under either vendor
  spelling, docked **and** detached;
- no element anywhere in the panel's subtree carries one — which also guards
  R4-5, since a non-none `backdrop-filter` establishes a containing block for
  fixed-position descendants and the row's `position: fixed` "…" dropdown
  depends on no ancestor doing that.

The real invariant — flat vs see-through — lives in a live WKWebView on a
transparent `NSWindow` and is not observable in jsdom (no layout, no
compositor, no window). The test pins the DOM/style contract that decides it
and says so in its own comment rather than pretending to test the pixels; the
pixel half is the burst-capture evidence above.

## Build 5.1

`src-tauri/tauri.v5-1.conf.json`, `productName` "Quotos v5.1", built from
the repo root with `npx tauri build --config src-tauri/tauri.v5-1.conf.json`
(bundle target `["app"]` only — the DMG step needs disk-image arbitration).
It **reuses `com.quotos.desktop.v4`** rather than minting a fresh identifier,
on the same reasoning as v5: the fix changes no persisted data shape, so
there is nothing to isolate a fresh identity from, and sharing v4's identity
is what lets the captain judge it against his own real tracked subscriptions
at `~/Library/Application Support/com.quotos.desktop.v4/tracked.json`
instead of an empty panel. `tauri.v5.conf.json` stays in place.

Produced at:

```
/Users/supolka/.treehouse/quotos-2ea1f5/2/quotos/src-tauri/target/release/bundle/macos/Quotos v5.1.app
```

**Packaged build verified — indirectly, and here is why.** v5.1 deliberately
shares the captain's identifier, and therefore his `instance.lock`; his own
Quotos was running the whole time and must not be quit, so launching v5.1
itself would have exited quietly by design. Instead the same commit was built
a second time under a throwaway identifier
(`com.quotos.desktop.w1probe`), launched, and photographed: the packaged,
CSP-enforced build renders the panel, beak, header and empty state correctly,
with the `import.meta.env.DEV`-gated debug icon absent as expected for a
production bundle. That probe app and its config/WebKit directories were then
deleted. `pgrep -il quotos` before and after this round shows the same single
pre-existing pid (99208, his `~/Downloads/Quotos v5.app`), untouched.

## Honest gaps, still open

- **The initiating trigger is unidentified**, and the flip itself was not
  reproduced from an agent seat — see the reproduction section for exactly
  why. The fix is trigger-independent, but if the captain ever sees the
  surface go flat again on v5.1, that would be new information and this
  explanation would be wrong.
- Unchanged from earlier rounds (`git show c00ce58:RESULT.md` for detail):
  where `claude setup-token` writes for the default account is unverified;
  full-screen Space stay-put is checked at the mechanism level only;
  synthetic drags don't reproduce on this machine, so drag regressions need
  the captain's own check; Launch at Login's registration happy path needs
  his own click in System Settings.

## How to run, build, test

- **Browser mock harness**: `npm run dev`, open `http://localhost:1420/`.
  Every UI state is drivable with no Rust side — `src/lib/mockClient.ts`'s
  demo accounts encode the designed edge cases. It cannot say anything about
  this round's fault: a transparent-window compositing behaviour has no
  equivalent in Chrome.
- **Real app**: `npm run tauri dev`. A bare `cargo build` binary renders
  **nothing** — without the tauri CLI it resolves the dev config and loads
  `build.devUrl` with no vite behind it, which looks exactly like "the window
  opened on another Space".
- **Tests**: `npx vitest run` (208); `cargo test` from `src-tauri` (121).
  Standing bars: `cargo clippy --all-targets -- -D warnings` and
  `cargo fmt --check`.
- **Packaging**: `npx tauri build`. Side-by-side builds: v3/v4/v5/v5.1
  configs live in `src-tauri/tauri.v{3,4,5,5-1}.conf.json` — read
  `README.md`'s packaging sections and `AGENTS.md`'s side-by-side-packaging
  entry before changing an `identifier`.

## Diagnostics left in the build (all opt-in, all silent by default)

`QUOTOS_DEBUG_POS` (placement trace), `QUOTOS_DEBUG_AUTO_OPEN` (open the
panel from `tray.rect()` with no click), `QUOTOS_DEBUG_KEEP_OPEN` (suppress
hide-on-blur), `QUOTOS_DEBUG_READS` (credential/read decisions, never a
token), `QUOTOS_DEBUG_SPACE_BEHAVIOR` / `QUOTOS_DEBUG_WINDOW_LEVEL`
(collection-behaviour/level sweeps), `QUOTOS_PANEL_MODE=window` (restore the
pre-NSPanel activating path for A/B).

---

## Appendix: earlier rounds' measurements still load-bearing

Kept because the tray and placement geometry is built on them; the full
narratives are in git history (`git show 321eec5:RESULT.md` for rounds 3–4,
`git show 4495bdc:RESULT.md` for the beak-drift measurement round).

**Placement, verified end-to-end (round 3)** — dev-identity build, one real
tray click, window-server geometry sampled every 15ms, cross-checked against
the app's own `QUOTOS_DEBUG_POS` trace:

| | value |
|---|---|
| tray rect as `tray-icon` reports it | `(2444, 0)` physical-ish |
| resolved to | display 0 (built-in), point `(1222, 0)` |
| computed layout | `x=1216, y=39, beak_left=3` (pre-R3-10 geometry) |
| `NSWindow` frame after show | `(1216, 39, 360, 560)` |
| window-server bounds | `(1216.0, 39.0, 360.0, 560.0)` |
| position changes during the entire open | **1** |

Computed = AppKit = window server, and the window moves once (previously four
relocations per click). With pinned digits: beak centre
`1160.25 + 14 + 20 + 10 = 1204.25` against glyph centre `1204.25` — identical.
The beak-drift *pixel* measurements (true glyph centre ~18pt in, not the
hardcoded 15) are in `git show 4495bdc:RESULT.md`.

**The row menu vs. the scroll box (round 4)** — measured in the live
WKWebView; the menu is `position: fixed` so it neither clips at the panel's
edge nor adds scroll extent:

| state | body | scrollHeight vs clientHeight | menu |
|---|---|---|---|
| 2 rows | 332×274 | 274 == 274 (no scrollbar) | — |
| 2 rows, bottom menu open | 332×274 | **unchanged** | fixed, 178×122, fully inside the window |

**Menu bar heights on this one machine (round 3)**: notched built-in 33pt,
unnotched external 24–30pt — the reason nothing derives placement from a
hardcoded bar height.
