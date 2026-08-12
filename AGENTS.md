# Project agent memory

This file is the project's committed home for project-intrinsic agent knowledge: build, test, release, architecture, and sharp-edge notes that should travel with the code.

- Add durable project-specific notes here as they are discovered through real work.

## What this is

Tauri v2 + React/TS + Vite menu bar app in `quotos-app/`. See `RESULT.md` at
the repo root for the state of the current build (what works, what doesn't,
how to run/build/test) — read it before assuming a feature is done. It is
rewritten each round, not appended to.

## Sources of truth (don't duplicate, read these)

- `design/brief.md` — product spec: entity model, every state, every surface.
- `design/system/` — the design system (tokens, React components). Use it,
  don't reinvent it; `design/system/readme.md` explains the visual language.
  `quotos-app/src/design-system/` is a verbatim copy consumed by the app —
  when you change a component, edit both copies identically (there is no
  build step that syncs them).
- `data/quotos-source-s1/report.md` (in the firstmate data dir, not this repo)
  — how the Claude usage-reading mechanism was verified: endpoint, headers,
  Keychain service naming, rate limits, the 401 refresh trick.

## Architecture

- Provider adapter seam: `quotos-app/src/providers/registry.ts` (frontend)
  and `quotos-app/src-tauri/src/providers/mod.rs` (backend). A second
  provider is a new module on each side plus one registry entry — nothing
  else (panel, hooks, state machine) may reference a provider by name.
- `quotos-app/src/hooks/useSubscriptions.ts` is the single owner of refresh
  cadence, tracked-subscription membership, and per-subscription state;
  don't add a second timer elsewhere. The background refresh interval runs
  for the app's lifetime regardless of panel visibility — opening/closing
  the panel must never itself trigger a fetch (that was a real bug: five
  open/close cycles alone exhausted the shared rate budget).
- **Percent fields are "consumed", not "remaining", everywhere** —
  `Subscription.used`, `LimitWindowEntity.used`, `NormalizedUsage.used`. Do
  not reintroduce a "remaining" field; the whole surface (headline, bars,
  tray) was deliberately inverted to one consistent meaning.
- I6: nothing is tracked by default. `quotos-app/src/lib/persistence.ts`
  (localStorage, key `quotos.tracked.v1`) is the user-owned list; discovery
  (`tauriClient.listAccounts`) only ever feeds the add-subscription flow, it
  is never rendered directly. When seeding this list into React state, do it
  via a lazy `useState(() => ...)` initializer, not a `useEffect` — an
  effect-based load leaves one render where the list is genuinely `[]`, and
  a persistence-writing effect watching that state will clobber real stored
  data with `[]` before the loaded value ever commits.
- Health (`Subscription.state`) and the shared rate budget
  (`Subscription.rateLimitedUntil`) are separate fields on purpose — a
  rate-limited response must never overwrite a real diagnosis (e.g. Broken).
  When touching `refreshOne` in `useSubscriptions.ts`, remember the
  optimistic "reading"/"connecting" patch issued before the fetch starts is
  itself capable of erasing prior health state if a subsequent rate-limited
  response doesn't explicitly restore `state`/`reason` from what was
  captured before that patch — this exact race was a real regression caught
  in manual testing (see the `useSubscriptions.test.ts` "B6" describe block).
- The Rust side never returns a stale number as current: on read failure it
  returns a typed `FetchError` (see `providers/mod.rs`), and the frontend
  decides `behind` vs `broken` based on whether prior good data exists.
- `quotos-app/src/lib/tauriClient.ts` swaps between the real Tauri IPC client
  and `mockClient.ts` (browser-only demo data) based on
  `"__TAURI_INTERNALS__" in window`. The mock client exists purely so the UI
  states are reviewable from a plain browser (`npm run dev`) — never let it
  leak into a real-build code path. Extend it (not `App.tsx`) when a new
  state needs to be reachable for browser-only QA.

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
- **`tray-icon` v0.24.2's macOS `set_title(None)` is a silent no-op** — it
  only calls `NSStatusItem`'s `setTitle` when given `Some(..)`, so passing
  `None` to "clear" a tray title leaves whatever was last set stuck forever.
  Always pass `Some("")` to clear it (see `set_tray_title` in `lib.rs`).
  Verified by reading `platform_impl/macos/mod.rs` in the crate source
  directly — don't trust the `Option<S>` signature's apparent symmetry.
- **`tauri-plugin-positioner`'s tray anchor mode must match the popover's
  beak alignment.** `Position::TrayBottomRight` places the window's *left*
  edge at the icon's right edge (the whole popover unfolds to one side of
  the icon); `Position::TrayCenter`/`TrayBottomCenter` center it under the
  icon. `design/system/components/shell/Panel.jsx`'s beak is horizontally
  centered, so the anchor must be `TrayBottomCenter` — any other tray
  position will visually point the beak at the wrong menu-bar item.
- **`--shadow-popover`'s blur radius must stay inside the transparent
  window's own margin** around the panel (`--panel-width` vs the window
  width in `tauri.conf.json`, currently a 14px margin per side via
  `app.css`'s `padding-top` / the centered flex body). A wider blur gets
  hard-clipped by the window edge instead of fading, which reads as a dark
  halo band against a bright desktop rather than a soft shadow.
- Side-by-side packaging (a second identity for the same product) is done
  at packaging time via `quotos-app/src-tauri/tauri.v2.conf.json` merged
  with `--config` (`npx tauri build --config src-tauri/tauri.v2.conf.json`
  from `quotos-app/`) — see that file and `quotos-app/README.md`. Don't put
  `--config` before the `build` subcommand; each subcommand defines its own
  flag. A distinct `identifier` is sufficient to diverge WKWebView storage
  (verified empirically: `~/Library/WebKit/<identifier>/` is a separate
  directory per bundle identifier, confirmed by launching both builds).

## Maintaining this file

Keep this file for knowledge useful to almost every future agent session in this project.
Do not repeat what the codebase already shows; point to the authoritative file or command instead.
Prefer rewriting or pruning existing entries over appending new ones.
When updating this file, preserve this bar for all agents and keep entries concise.
