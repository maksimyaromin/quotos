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
  **Superseded wherever it disagrees** by the round-2 UI/UX handoff,
  `data/quotos-fixes-f2/design-notes/quotos-handoff.html` (firstmate data
  dir, not this repo) — the brief predates any build, the handoff was
  written after the captain used one. `quotos-prototype.html` in the same
  folder is the interactive reference; where code and prototype differ, the
  prototype wins unless the handoff text says otherwise. Its own JS
  (`renderVals()`) is the authoritative source for exact row-state logic —
  read it directly rather than re-deriving from the prose.
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
- **`idle` and "no limits reported yet" are two different things, both
  reachable via `SubscriptionState`.** `idle` renders a hollow grey ring dot
  (`StatusDot`'s `dotRing`) — a subscription never successfully read. "No
  limits reported yet" is `state: "working"` with `used: null` — a *good*
  read that simply had no windows to show — and renders the calm teal dot,
  same as any other working row. This is derived, not a stored value: the
  row body always shows `reason ?? "No limits reported yet."` whenever
  `used` isn't a number, for every non-data state (`idle`, `connecting`,
  `broken`, or `working` with nothing to report) — see `SubscriptionRow.jsx`
  and mirror `quotos-prototype.html`'s `renderVals()` if this needs to
  change. `SubscriptionState` still has an unused `"repairing"` member from
  round 1; the round-2 handoff's fixed vocabulary is exactly `working`,
  `reading`, `behind`, `broken`, `connecting`, `idle` — don't render
  `"repairing"` and don't introduce new state values without updating both
  this file and the handoff's row-state table.
- The "…" row menu (`SubscriptionRow.jsx`) closes on any interaction outside
  itself via a single `window` `mousedown` listener in `App.tsx`, gated by
  `data-quotos-menu-scope` on both the trigger button and the dropdown. It
  only ever *closes* — never (re)opens — so it can't race the trigger
  button's own toggle-on-click. Keep both elements' `data-quotos-menu-scope`
  attribute if you touch this markup, or the menu will close itself on its
  own click.

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
  beak alignment — as of round 2 they no longer do, and fixing one without
  the other breaks it.** `Position::TrayBottomRight` places the window's
  *left* edge at the icon's right edge; `Position::TrayCenter`/
  `TrayBottomCenter` center the window under the icon. The round-2 handoff
  moved the beak off-center: it's now pinned to the panel's left edge,
  under the glyph, with the panel opening rightward (`Panel.jsx`'s
  `beakLeft` prop, `App.tsx`'s `BEAK_LEFT` constant). `lib.rs` still uses
  `Position::TrayBottomCenter` (round-2 surface work didn't touch
  `src-tauri`, per that task's file ownership) — this anchor now
  contradicts the beak, and the two must change together. Confirmed live on
  the captain's screen (see firstmate's `measurements.md`): "the beak is
  horizontally centred under the icon... this follows from
  `Position::TrayBottomCenter`... so the anchor has to change together with
  the beak." Whoever fixes the native anchor should also revisit
  `BEAK_LEFT` in `App.tsx` (currently a static approximation, documented
  in-place, since there's no in-page tray glyph to measure from).
- I7's `set_detached` IPC command is unchanged, but its trigger moved:
  round 2 removed the detach *button* — dragging the header is now the only
  way to detach (`App.tsx`'s `handleHeaderPointerDown`), matching the
  handoff's "first movement detaches" rule. In the real app this calls
  `getCurrentWindow().startDragging()` from `@tauri-apps/api/window` after
  the first `mousemove` past mousedown (a plain click does nothing); the
  browser/mock harness has no OS window to move, so it simulates the same
  interaction by fixed-positioning the Panel via CSS instead (see
  `position`/`docked` props) — this is why dragging is fully testable via
  `npm run dev` even though `startDragging()` itself isn't.
- **The tray title can still show `"!"` and `"…"`**, which the round-2
  handoff explicitly forbids ("Никаких знаков и многоточий... Либо цифра,
  либо ничего") — found while reading `useSubscriptions.ts`'s tray-title
  effect (`s.state === "broken" ? "!" : ... : "…"`). That file is outside
  round-2 surface work's file ownership (`src/hooks/`); flagged here rather
  than fixed, since acceptance criterion 2 depends on it and the next
  worker touching that effect should know.
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
