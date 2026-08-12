# Quotos

Tauri v2 + React + TypeScript menu bar app. See `RESULT.md` at the repo root
for the state of the build — what works, what doesn't, how it was tested.

## Develop

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
