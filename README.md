# Quotos

A macOS menu bar utility that shows, at a glance, how much of each Claude
subscription is left.

## Why

Two Claude subscriptions are in daily use — a personal Max account and a work
Team account — each reached through its own shell alias. Checking what is left
today means opening the browser, logging into the right account, and reading a
usage page. That breaks the flow while an agent is working.

Quotos puts that number in the menu bar: one click, both accounts, done.

## Status

Scope is not settled yet. Known so far:

- macOS menu bar application with a compact popover.
- Shows remaining limits for more than one Claude account at the same time.
- Other providers are a later extension, not part of the first version.

Open questions live with the work item, not in this file. See `RESULT.md` at
the repo root for the state of the current build — what works, what doesn't,
how it was tested. It is rewritten each round, not appended to.

## Develop

Tauri v2 + React + TypeScript menu bar app. Everything below runs from the
repository root. Run `npm install` before opening the repository in an
editor, not only before running a command: TypeScript resolves `@types/node`
and the rest of the toolchain from `node_modules`, so an uninstalled tree
shows type errors an install clears.

```bash
npm install
npm run tauri dev   # the real app: tray icon, click for the panel
```

To exercise every UI state without the native shell (mock data, any browser —
this is how the states in `data/quotos-fixes-f1/feedback.md` were verified):

```bash
npm run dev
# open http://localhost:1420/
```

## Test

```bash
npm test              # vitest — normaliser, time formatting, refresh policy
npx tsc --noEmit       # typecheck
(cd src-tauri && cargo test)   # discovery unit tests
```

## Build

```bash
npm run tauri build   # produces src-tauri/target/release/bundle/macos/Quotos.app
```

## Packaging a side-by-side build ("v2")

The product name stays `Quotos` in source — a second, differently-identified
build is derived at packaging time via a config override, not by renaming
the product everywhere:

```bash
npx tauri build --config src-tauri/tauri.v2.conf.json
```

This runs `tauri build` with `src-tauri/tauri.v2.conf.json` merged on top of
`tauri.conf.json` (Tauri's `--config` merge), which:

- overrides `productName` to `Quotos 2` and `identifier` to
  `com.quotos.desktop.v2` — a distinct bundle identifier so it can run
  alongside a `com.quotos.desktop` build at the same time, and so their
  Application Support / WebKit storage never collide (macOS scopes both by
  `CFBundleIdentifier`);
- points `beforeBuildCommand` at `npm run build`, the same frontend build v1
  uses. The panel header always reads "Quotos" — `productName` and
  `identifier` are the only things that distinguish the two builds.

The resulting bundle is `src-tauri/target/release/bundle/macos/Quotos 2.app`.
Copy it to `~/Downloads/Quotos 2.app` to run beside `~/Downloads/Quotos.app`.

## Round-2 build ("v3")

`tauri.v3.conf.json` follows the same pattern (`npx tauri build --config
src-tauri/tauri.v3.conf.json`, `productName` "Quotos 3") but **reuses v2's
`identifier`, `com.quotos.desktop.v2`, on purpose** — the opposite of v1→v2's
"always diverge" choice. This round moved the tracked-subscriptions list from
`localStorage` to a native, Rust-owned file, migrated on first run; sharing
v2's identity is what lets that migration run against the captain's real
in-use data (his actual tracked accounts and custom names, already sitting in
`Quotos 2`'s WKWebView storage) instead of booting empty. See the
corresponding `AGENTS.md` sharp-edges entry before changing this.

## Round-4 build ("v4")

`tauri.v4.conf.json` follows the same pattern (`npx tauri build --config
src-tauri/tauri.v4.conf.json`, `productName` "Quotos v4") but, unlike v3,
**mints a fresh identifier** (`com.quotos.desktop.v4`) rather than reusing
v2/v3's — this round's own changes (per-window pinning) don't need the
captain's real in-use data to verify against, and a fresh identity keeps a
throwaway verification build's own tracked-account list from ever touching
his real one. `productName`'s "v4" only ever reaches the bundle's own
filename/`CFBundleName` — the panel title, tray tooltip, and every in-app
string are hardcoded `"Quotos"` regardless of which config built the app; no
version text is meant to reach the UI itself.

## Round-5 build ("v5")

`tauri.v5.conf.json` follows the same pattern (`npx tauri build --config
src-tauri/tauri.v5.conf.json`, `productName` "Quotos v5") and, like v3,
**reuses the previous build's identifier** (`com.quotos.desktop.v4`) rather
than minting a fresh one — this round is a repository layout flatten with no
change to the account/persistence data shape, so there is nothing to isolate
a fresh identity from, and reusing v4's identity is what proves the flattened
build still reads the captain's real tracked-subscription list
(`~/Library/Application Support/com.quotos.desktop.v4/tracked.json`) instead
of booting empty. See the corresponding `AGENTS.md` sharp-edges entry.

## Round-5.1 build ("v5.1")

`tauri.v5-1.conf.json` (`npx tauri build --config
src-tauri/tauri.v5-1.conf.json`, `productName` "Quotos v5.1") carries the
panel-background flicker fix and **reuses `com.quotos.desktop.v4` again**,
for the same reason v5 did: the fix is a single CSS-property removal on the
panel's fill layer with no change to the persisted data shape, so there is
nothing to isolate a fresh identity from, and sharing v4's identity is what
lets the captain judge the fix against his own real tracked subscriptions
rather than an empty panel. `tauri.v5.conf.json` stays in place so the two
builds can be run side by side — note that, sharing an identifier, they also
share `instance.lock`, so only one of them runs at a time (by design; see
`AGENTS.md` on `single_instance.rs`).
