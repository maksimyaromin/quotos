# Quotos — current build state (repository layout flatten + v5)

> This file is rewritten each round, not appended to. This round did no
> product work: it flattened the repository layout (`quotos-app/`'s contents
> moved to the repo root, `design/` moved to `docs/design/`) and packaged and
> verified the resulting v5 build. The previous round's full narrative (the
> overnight quality pass — Rust decomposition, F1–F6/R1–R7 fixes, a11y,
> security hardening) is `git show 84c5df0:RESULT.md`; the few measurements
> still load-bearing are kept in the appendix below.

**325 automated tests pass** (204 vitest, 121 `cargo test`) from the
repository root; `tsc --noEmit`, `cargo clippy --all-targets`, and
`cargo fmt --check` are all clean. A production `npx tauri build` produces a
runnable `.app`.

## How to run, build, test

Everything below runs from the repository root — `quotos-app/` no longer
exists.

- **Browser mock harness**: `npm run dev`, open `http://localhost:1420/`.
  Every UI state is drivable with no Rust side at all —
  `src/lib/mockClient.ts`'s demo accounts encode the designed edge cases
  (broken/sign-in, stale credential, rate-limited-from-first-read, flaky
  network, no-limits, severity-vs-headline disagreement, statusline
  reconciliation, long window names).
- **Real app**: `npm run tauri dev`. A bare `cargo build` binary renders
  **nothing** — without the tauri CLI it resolves the dev config and loads
  `build.devUrl` with no vite behind it, which looks exactly like "the window
  opened on another Space".
- **Tests**: `npx vitest run` (204); `cargo test` from `src-tauri` (no
  workspace manifest above it, 121). Standing lint/format bars:
  `cargo clippy --all-targets` and `cargo fmt --check`, both clean.
- **Packaging**: `npx tauri build` (bundle target `"app"` only — the DMG step
  needs disk-image arbitration a sandbox doesn't have). Side-by-side builds:
  v3/v4/v5 configs live in `src-tauri/tauri.v{3,4,5}.conf.json` — see
  `README.md`'s packaging sections and `AGENTS.md`'s side-by-side-packaging
  entry before changing an `identifier`.

## What this round changed

1. **`quotos-app/` no longer exists.** `src/`, `src-tauri/`, `index.html`,
   `package.json`, `package-lock.json`, `vite.config.ts`, `tsconfig.json`,
   `tsconfig.node.json`, and everything else it held now live at the repo
   root — moved with `git mv` so history follows. `npm`, `vitest`, `tsc`, and
   `tauri` all run from here now.
2. **`design/` is `docs/design/`.** Same content, moved with `git mv`.
3. **Two collisions resolved.** The root and app-level `README.md` are now
   one authoritative root `README.md` (why/status from the root file, plus
   develop/test/build/every packaging round from the app file). The two
   `.gitignore` files are merged the same way (the app-level file's fuller
   ignore list, kept at the root).
4. **Every reference the move broke is fixed** — not just the files, the
   paths inside them: `src/design-system/sync.test.js`'s relative resolution
   of `docs/design/system/`, `docs/design/system/_build_bundle.mjs`'s esbuild
   lookup (now resolves the root's `node_modules` via `package.json` three
   levels up instead of a sibling `quotos-app/`), and every doc/comment that
   spelled `quotos-app/` or `design/` as a path — `AGENTS.md`, `README.md`,
   `src-tauri/src/tray_render.rs`'s design-token/asset comments, and the
   scattered `design/NOTES.md` §-references across the tray/frontend source
   (that file doesn't exist in this repo — likely a firstmate-data-dir
   artifact — but the stale `design/` prefix is now consistently
   `docs/design/` rather than a dangling pre-move path). The two
   `tauri*.conf.json` `frontendDist`/build-command paths needed no change:
   `src-tauri` and `dist` moved up one level together, so their relative
   path to each other was preserved.
5. **v5 packaging config.** `src-tauri/tauri.v5.conf.json` (`productName`
   "Quotos v5") reuses v4's `com.quotos.desktop.v4` identifier rather than
   minting a fresh one — this round changed no persisted-data shape, so
   there's nothing to isolate a fresh identity from, and reusing v4's
   identity is what let the flattened build prove itself against the
   captain's real tracked-subscription list instead of booting empty. See
   `AGENTS.md`'s side-by-side-packaging entry for the full reasoning (it now
   also records the v3-not-v4 precedent this follows). No version text
   reaches any product surface — `productName` only affects the bundle
   filename/`CFBundleName`; the panel title, tray tooltip, and every in-app
   string stay the hardcoded `"Quotos"` regardless of which config built it.
6. **Verified live, on the captain's machine, under the standing
   look-only/Quotos-only rules.** His running `~/Downloads/Quotos v4.app`
   (pid confirmed via `pgrep`) was quit, `Quotos v5.app` was launched in its
   place, and (via `fm-quotos-click.sh peek` then a real click — never a
   hand-rolled click) the tray glyph rendered real data (two subscription
   groups, 3 and 2 pinned digits, matching his real `tracked.json` exactly),
   the panel opened and showed both real tracked subscriptions
   (`yaromin.m.y@gmail.com's Or...` and `Scompler`) with a completed read
   (66% / 0% used, "Read 36s ago"), and no version text appeared anywhere.
   `tracked.json` was backed up before any of this and diffed
   byte-identical afterward. v5 was then quit and his exact
   `~/Downloads/Quotos v4.app` relaunched; `pgrep -il quotos` confirmed
   exactly one instance running afterward, his. Full numbers and screenshots
   are in `data/quotos-layout-l1/verification.md` (firstmate data dir, not
   this repo).

## Honest gaps, still open

Unchanged from the previous round (`git show 84c5df0:RESULT.md` for detail):
where `claude setup-token` writes for the default account is unverified,
full-screen Space stay-put is checked at the mechanism level only, synthetic
drags don't reproduce on this machine so drag regressions need the captain's
own check, and Launch at Login's registration happy-path needs his own click
in System Settings.

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
