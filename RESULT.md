# Quotos — current build state (overnight quality pass)

> This file is rewritten each round, not appended to. This round is a
> maintainer's overnight pass over the whole repo: the Rust backend decomposed
> into focused modules, dead vocabulary removed, and hands-on QA of the browser
> harness that found and fixed real UX bugs. The previous rounds' full
> narratives live in git (`git show 321eec5:RESULT.md` for rounds 3–4,
> `git show 4495bdc:RESULT.md` for the beak-drift measurement round); the few
> measurements still load-bearing are kept in the appendix below.

**250 automated tests pass** (145 vitest, 105 `cargo test`); `tsc --noEmit`
and `cargo check` are clean with zero warnings.

## How to run, build, test

- **Browser mock harness**: `npm run dev` from `quotos-app/`, open
  `http://localhost:1420/`. Every UI state is drivable with no Rust side at
  all — `src/lib/mockClient.ts`'s demo accounts encode the designed edge cases
  (broken/sign-in, stale credential, rate-limited-from-first-read, flaky
  network, no-limits, severity-vs-headline disagreement, statusline
  reconciliation, long window names).
- **Real app**: `npm run tauri dev` from `quotos-app/`. A bare `cargo build`
  binary renders **nothing** — without the tauri CLI it resolves the dev
  config and loads `build.devUrl` with no vite behind it, which looks exactly
  like "the window opened on another Space".
- **Tests**: `npx vitest run` (145) from `quotos-app/`; `cargo test` (105)
  from `quotos-app/src-tauri` (no workspace manifest above it). `cargo clippy`
  is not installed for this machine's stable toolchain.
- **Packaging**: `npx tauri build` from `quotos-app/` (bundle target `"app"`
  only — the DMG step needs disk-image arbitration a sandbox doesn't have).
  The side-by-side v3 identity is
  `npx tauri build --config src-tauri/tauri.v3.conf.json` (`productName`
  "Quotos 3"; `identifier` deliberately still `com.quotos.desktop.v2`, shared
  with v2 so the native-persistence migration meets the captain's real data —
  see `AGENTS.md`'s side-by-side-packaging entry before "fixing" that).

## What the app does today

- **Tray**: a procedurally drawn Q capacity-gauge glyph (no raster asset)
  whose arc fill tracks the worst *active* limit across everything tracked;
  optionally, colored per-window pinned digits composited into the same RGBA
  icon (there is no color path through `set_title`); a translucent
  panel-open highlight drawn into the same bitmap so the glyph never shifts.
- **Panel**: docked under the glyph with the beak centred on it, placed in
  global points (never "physical pixels" — three APIs each mean a different
  display's scale by that); a non-activating `NSPanel`, so opening it never
  steals the frontmost app or throws the captain off a full-screen Space;
  detachable by dragging the header (live-follow; the drag-time reactivation
  is a captain-approved tradeoff), snap-back arrow to redock.
- **Subscriptions**: discovered, never hardcoded; nothing tracked by default;
  the tracked list is a natively persisted, `fsync`'d JSON file with one-shot
  migration from the old localStorage store; one automatic read per account
  per minute from the Rust scheduler (a manual refresh resets that minute);
  a shared 5-per-300s request budget with typed failures — a budget wait
  never overwrites a real diagnosis, and "sign-in expired" is only ever said
  when the sign-in is proven to be what failed.
- **Second source**: the Claude Code statusline feed, opt-in per
  subscription, reconciled freshest-wins into the raw usage shape before
  normalization so every existing rule applies unchanged.
- **Sign-in recovery**: drives Claude Code's own `claude setup-token` on a
  real pty; Quotos never touches a credential — it relays the pasted code and
  proves recovery by re-reading the account.
- **Appearance**: the full token system renders correctly in both light and
  dark, and follows a macOS appearance change live.

## What this round changed

1. **Backend decomposition** — `lib.rs` went 2311 → 461 lines across three
   line-exact, lossless extractions: `geometry.rs` (pure placement and
   coordinate math + its tests), `accounts.rs` (the account-data plane:
   fetch, budget, scheduler, tracked-list/statusline/sign-in commands), and
   `shell.rs` (the mutually-recursive tray-repaint + panel-lifecycle
   cluster). `lib.rs` is now `AppState` + module declarations + `run()`.
   Each move was verified by diffing the moved body against `git show HEAD`
   plus the full suites.
2. **Dead vocabulary removed** — round 1's never-used `"repairing"`
   subscription state is gone from `entities.ts` and both `StatusDot` copies;
   the design-system `prompt.md` docs that still described eight states, a
   `remaining` (%-left) prop, and state-list-driven layout were corrected to
   the shipped six-state, `used` (%-consumed) reality.
3. **Scope-badge truncation actually truncates** — `text-overflow: ellipsis`
   silently never applies to a flex container, so long model names were
   hard-clipped mid-character for two rounds while the test suite asserted
   otherwise; the truncation now lives on an inner block span inside the
   badge (both design-system copies).
4. **A first-ever read that hits the rate budget no longer lies** — the B6
   restore wrote the captured prior state back verbatim, and for a
   just-added account that prior was the seeded "connecting", leaving the row
   claiming "Reading…" forever while masking the designed budget-wait note.
   An in-flight prior now settles to idle (never read) or working (had data).
5. **Keyboard and appearance gaps** — the "N limits" disclosure was an inert
   span, invisible to keyboard and the accessibility tree; it is a real
   `<button>` with `aria-expanded` now. Light/dark switching was a one-shot
   startup sample; it follows `prefers-color-scheme` changes live.
6. **QA walks that found nothing to fix** (recorded so the next round doesn't
   re-walk them blind): the full light-mode surface; the sign-in recovery
   flow end-to-end in the mock (paste → finished event → automatic re-read
   shows the recovered account; Escape cancels back to the action button; the
   form survives an unrelated read-all); and header-drag detach →
   snap-back, both directions clean.
7. **This file** — it was two-plus rounds stale (claimed 176 tests and round
   4 as current, predating the statusline feed, working drag, and per-window
   pinning entirely).

## Honest gaps, still open

- **Where `claude setup-token` writes for the default account** is unverified
  now that Quotos no longer forces `CLAUDE_CONFIG_DIR` (see `AGENTS.md`'s
  R3-4 entry) — completing a real login is the captain's own check.
- **Full-screen Space stay-put** is verified at the mechanism level (the app
  no longer activates on open — measured from a separate process) but not
  end-to-end; an agent here cannot put an app into full screen.
- **A real hand drag** was confirmed live by the captain mid-round-R5;
  synthetic drags still produce no `WindowEvent::Moved` on this machine, so
  drag regressions cannot be caught from here — ask him.

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
narratives are in git history (revisions named at the top).

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
