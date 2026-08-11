# Project agent memory

This file is the project's committed home for project-intrinsic agent knowledge: build, test, release, architecture, and sharp-edge notes that should travel with the code.

- Add durable project-specific notes here as they are discovered through real work.

## What this is

Tauri v2 + React/TS + Vite menu bar app in `quotos-app/`. See `RESULT.md` at
the repo root for the state of the first build (what works, what doesn't,
how to run/build/test) — read it before assuming a feature is done.

## Sources of truth (don't duplicate, read these)

- `design/brief.md` — product spec: entity model, every state, every surface.
- `design/system/` — the design system (tokens, React components). Use it,
  don't reinvent it; `design/system/readme.md` explains the visual language.
- `data/quotos-source-s1/report.md` (in the firstmate data dir, not this repo)
  — how the Claude usage-reading mechanism was verified: endpoint, headers,
  Keychain service naming, rate limits, the 401 refresh trick.

## Architecture

- Provider adapter seam: `quotos-app/src/providers/registry.ts` (frontend)
  and `quotos-app/src-tauri/src/providers/mod.rs` (backend). A second
  provider is a new module on each side plus one registry entry — nothing
  else (panel, hooks, state machine) may reference a provider by name.
- `quotos-app/src/hooks/useSubscriptions.ts` is the single owner of refresh
  cadence and per-subscription state; don't add a second timer elsewhere.
- `quotos-app/src/lib/tauriClient.ts` swaps between the real Tauri IPC client
  and `mockClient.ts` (browser-only demo data) based on
  `"__TAURI_INTERNALS__" in window`. The mock client exists purely so the UI
  states are reviewable from a plain browser (`npm run dev`) — never let it
  leak into a real-build code path.
- The Rust side never returns a stale number as current: on read failure it
  returns a typed `FetchError` (see `providers/mod.rs`), and the frontend
  decides `behind` vs `broken` based on whether prior good data exists.

## Sharp edges

- macOS (APFS) is case-insensitive by default: `rm -f src/App.css` will
  silently delete `src/app.css` too if both exist. Confirm with `ls` after
  any case-sensitive-looking cleanup.
- The `/api/oauth/usage` endpoint is rate-limited at 5 requests/300s **per
  account**, shared with Claude Code itself — see `ratelimit.rs`. Don't add
  a second poller; go through `fetch_snapshot`.
- `npm run tauri build`'s DMG target fails in headless/sandboxed
  environments (`bundle_dmg.sh` needs real disk-image arbitration).
  `tauri.conf.json` bundle targets are `["app"]` only for that reason — add
  `"dmg"` back only where DMG creation is actually needed and works.

## Maintaining this file

Keep this file for knowledge useful to almost every future agent session in this project.
Do not repeat what the codebase already shows; point to the authoritative file or command instead.
Prefer rewriting or pruning existing entries over appending new ones.
When updating this file, preserve this bar for all agents and keep entries concise.
