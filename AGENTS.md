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
  Two things live behind this seam on purpose, both provider-owned rather
  than generic-shell logic: **headline selection** (`normalizeUsage.ts`'s
  `pickAccountWideWeekly` — for Claude the headline is the account-wide
  weekly window, `weekly_all`/`seven_day`, never simply the most-consumed
  window, which let a per-model weekly like Fable outrank the real account
  total) and **outcome→state+reason mapping** (`providers/claude/index.ts`'s
  `mapOutcome`, called via `registry.ts`'s `mapOutcomeFor` — the generic
  hook only ever supplies the outcome and whether a prior good read
  existed). `rate_limited` is deliberately excluded from that mapping — see
  the B5/B6 note below.
- `Subscription.severity` (`"healthy" | "warn" | "critical"`) is
  provider-computed from *every* window, not just the headline one —
  `critical` when any window is >=90% used, `warn` when any is >=75%,
  matching `--cap-critical`/`--cap-warn` exactly, no separate thresholds.
  This is what lets a 20%-headline account with an 85%-used session still
  read amber: the headline number stays 20%, but its color and the tray
  digit's color come from `severity`, not from the headline percentage's
  own magnitude. Computed in `normalizeUsage.ts`, carried on `Subscription`
  and `NormalizedRead`.
- **Colored tray digits have no path through `tray-icon` v0.24.2's
  `set_title`** — verified by reading `platform_impl/macos/mod.rs` (same
  method that found the `set_title(None)` no-op below): it's a plain
  `NSString`, no attributed-string/color channel anywhere in the crate.
  `quotos-app/src-tauri/src/tray_render.rs` composites glyph + colored
  digits into an RGBA bitmap instead (`set_icon`, `icon_as_template(false)`
  when anything is colored), with a tiny embedded 3x5 pixel font — no font
  library needed for digits and `%`/`!`. `set_icon_for_ns_status_item_button`
  always requests an 18pt-tall `NSImage` regardless of the source bitmap's
  pixel size, so rendering at 2x that height is what keeps it crisp on
  Retina without needing a separate `@2x` asset. The tray also never shows
  `"!"` or `"…"` (handoff: digits or nothing) — a pin with no number yet
  contributes no segment at all, and if *any* pinned value is stale every
  digit turns amber, not just that one.
- **Sign-in recovery drives Claude Code's own login, never Quotos's own.**
  `quotos-app/src-tauri/src/signin.rs` spawns `claude setup-token` (pointed
  at the broken account's `CLAUDE_CONFIG_DIR`) attached to a real pty via
  `portable-pty` — plain pipes risk the CLI detecting a non-tty stdin and
  changing behavior, confirmed by one careful, throwaway-config-dir
  observation showing it renders an interactive, cursor-positioning prompt.
  The CLI opens the browser and prints the authorization URL *itself*;
  Quotos never parses that output, never opens a browser, and never reads
  or writes a credential — it only starts the process and relays a pasted
  code (from the panel's own field) into the process's stdin. Completion is
  detected by the process exiting, at which point Quotos re-reads the
  account normally; the exit status is informational only; the re-read is
  the real proof either way.
- **Refresh cadence lives natively, not in JS.** `quotos-app/src-tauri/src/scheduler.rs`
  is the single scheduler: one automatic read per account per minute,
  anchored to the last attempt (manual or scheduled — both funnel through
  the same `fetch_snapshot` command, which is what makes "a manual refresh
  resets the minute" fall out for free with no extra wiring). This replaced
  a JS `setInterval` in `useSubscriptions.ts` after reproducing, on a real
  build, that macOS suspends timers in a hidden/occluded WKWebView (0 ticks
  in 150s+ of an 8s interval while the panel stayed closed, process alive)
  — a native OS-level timer has no notion of "hidden webview" to be
  throttled by. `useSubscriptions.ts` still owns tracked-subscription
  membership and per-subscription state, and reacts to the scheduler's
  `quota-refresh` push event the same way it applies a manual refresh's
  direct result — don't add a second timer anywhere. Opening/closing the
  panel still triggers no read (unchanged, still correct — that was always
  about panel visibility, never about where the clock lives).
- `ratelimit.rs`'s sliding window prunes with `duration_since(front) >=
  window`, not `>` — at exactly `window` old, a reservation must age out
  or the limiter fights the 1-read-per-minute schedule that expects the
  budget to be exactly full after 5 minutes.
- **Percent fields are "consumed", not "remaining", everywhere** —
  `Subscription.used`, `LimitWindowEntity.used`, `NormalizedUsage.used`. Do
  not reintroduce a "remaining" field; the whole surface (headline, bars,
  tray) was deliberately inverted to one consistent meaning.
- **I6: nothing is tracked by default, and the tracked list is now natively
  owned.** `quotos-app/src-tauri/src/persistence.rs` writes a plain JSON
  file (`tracked.json` in the app's config dir) via temp-file + `fsync` +
  atomic rename, not SQLite (the list is a handful of accounts; a file is
  simpler, reviewable, and survives everything SQLite would here, and this
  round confirmed the write itself is durable — see below). `quotos-app/src/lib/persistence.ts`
  is the frontend seam: on the native path it's a thin wrapper over the
  `load_tracked`/`save_tracked` commands; the browser/mock harness (no Rust
  side to call) keeps using `localStorage` as a fallback. Discovery
  (`tauriClient.listAccounts`) only ever feeds the add-subscription flow, it
  is never rendered directly. Because loading the native store is
  inherently async (a real IPC round-trip), seeding React state can no
  longer use the old "lazy `useState` initializer" trick — instead,
  `useSubscriptions.ts` gates the persistence-writing effect behind a
  `hasLoadedRef` flag that only flips true once the async load has actually
  committed to state, so the effect can't fire on the empty pre-load render
  and clobber real stored data with `[]`.
  **What the round found, and a correction worth recording:** the captain's
  original report ("renamed a subscription, reopened, name was gone") was
  first attributed to `localStorage`'s value write losing a race against
  `app.exit(0)` — a `strings`-based check on WebKit's `localstorage.sqlite3`
  found the *key* on disk but not the *value* shortly after an abrupt quit.
  That specific diagnosis turned out to be a tooling artifact: WebKit stores
  localStorage values UTF-16-encoded, which `strings` (ASCII) can't see at
  all — re-checked with `sqlite3 ... "SELECT value FROM ItemTable"` (or
  `hex(value)` piped through `iconv -f UTF-16LE`), the value *was* present
  even after `kill -9` within ~1s of the write. The move to a native,
  `fsync`'d file is still correct and still what the captain asked for
  (durability doesn't depend on WKWebView's internal flush timing at all
  now, whatever that timing actually is) — but the original bug's true root
  cause was never conclusively identified. If a similar report resurfaces,
  don't assume the localStorage-race explanation without re-verifying with
  `sqlite3`, not `strings`.
- **Migrating his real data across a bundle-identifier change needs the same
  identifier.** `tauri.v3.conf.json` deliberately reuses v2's
  `com.quotos.desktop.v2` rather than minting a new one — see the
  side-by-side-packaging entry below for why, and never clear the
  pre-migration `localStorage` key when writing this kind of migration
  (`persistence.ts`'s `migrateFromLocalStorageIfEmpty`): a bad migration
  must stay recoverable by going back to the previous build.
- Health (`Subscription.state`) and the shared rate budget
  (`Subscription.rateLimitedUntil`) are separate fields on purpose — a
  rate-limited response must never overwrite a real diagnosis (e.g. Broken).
  `rate_limited` is intercepted in `useSubscriptions.ts` before it ever
  reaches a provider's `mapOutcome` (it's not part of the `ReadOutcome`
  union `registry.ts` defines) and instead restores whatever
  `state`/`reason` existed *before* the attempt started. When touching
  `refreshOne` (manual path) or the `quota-refresh` event handler (native
  scheduled path) in `useSubscriptions.ts`, remember the optimistic
  "reading"/"connecting" patch issued before a manual fetch starts is
  itself capable of erasing prior health state if the rate-limited branch
  doesn't explicitly restore `state`/`reason` from what was captured
  *before* that patch — this exact race was a real regression caught in
  manual testing (see the `useSubscriptions.test.ts` "B6" describe blocks,
  including the native-path variant).
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
  beak alignment.** The round-2 handoff moved the beak off-center: it's now
  pinned to the panel's left edge, under the glyph, with the panel opening
  rightward (`Panel.jsx`'s `beakLeft` prop, `App.tsx`'s `BEAK_LEFT`
  constant). `lib.rs`'s `show_panel` uses `Position::TrayBottomLeft`
  (window's left edge = icon's left edge, per the plugin's own
  `calculate_position`) plus two handoff-specified adjustments applied
  afterward: the panel's left edge sits 6px inside the icon's own left edge
  (re-clamped to the monitor's left edge; the plugin's own
  `move_window_constrained` already clamps the right edge using the
  window's real width, and nudging further left can only keep that
  satisfied), and the top is pinned at a fixed 32px from the screen top
  (6px under the menu bar) rather than flush against the icon's bottom.
  **Not empirically pixel-verified this round.** A real tray click was
  ruled unsafe to drive from here: with the captain's real Quotos also
  running, both `System Events`-based accessibility queries (`tell process
  "quotos-app"`, by name *and* by `unix id`) resolved to the identical menu
  bar item rectangle for both processes, so there was no reliable way to
  prove a click would land on the dev build and not his. A follow-up
  attempt to prime `tauri-plugin-positioner`'s cached tray position from
  `TrayIcon::rect()` (no OS event, fully in-process) and call
  `show_panel()` directly produced a window position inconsistent with the
  primed tray coordinates in a way not resolved within this round — logged
  here rather than left silent. What *is* settled: `TrayBottomLeft` is the
  semantically correct anchor (the previous `TrayBottomCenter` was
  confirmed live on the captain's screen to center the window instead —
  see firstmate's `measurements.md`), and the same `on_tray_event` →
  `move_window_constrained` mechanism this relies on was already proven
  correct with *real* click events in the prior round (that's how
  `TrayBottomCenter`'s centering behavior was confirmed working before).
  Confirm the exact pixel alignment on the merged build via a real click,
  not synthetic data.
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
- **`--shadow-popover`'s blur radius must stay inside the transparent
  window's own margin** around the panel (`--panel-width` vs the window
  width in `tauri.conf.json`, currently a 14px margin per side via
  `app.css`'s `padding-top` / the centered flex body). A wider blur gets
  hard-clipped by the window edge instead of fading, which reads as a dark
  halo band against a bright desktop rather than a soft shadow.
- Side-by-side packaging (a second identity for the same product) is done
  at packaging time via a config in `quotos-app/src-tauri/` merged with
  `--config` (e.g. `npx tauri build --config src-tauri/tauri.v3.conf.json`
  from `quotos-app/`) — see that file and `quotos-app/README.md`. Don't put
  `--config` before the `build` subcommand; each subcommand defines its own
  flag. A distinct `identifier` is sufficient to diverge WKWebView storage
  (verified empirically: `~/Library/WebKit/<identifier>/` is a separate
  directory per bundle identifier, confirmed by launching both builds).
  **`identifier` is not always meant to diverge, though** —
  `tauri.v3.conf.json` deliberately reuses v2's `com.quotos.desktop.v2`
  rather than minting a new one, on purpose, reversing round-1's own
  "always diverge" convention: v3 carries the R2-5 native-persistence
  migration, and that migration only has real data to prove itself against
  if the build shares the captain's actual in-use identity (`Quotos 2`'s
  WKWebView storage, where his real tracked list and custom names already
  live). A fresh identifier would boot empty and migrate nothing. The next
  person tidying up packaging configs should not "fix" this back to a
  unique identifier without checking whether the same reasoning still
  applies.

## Maintaining this file

Keep this file for knowledge useful to almost every future agent session in this project.
Do not repeat what the codebase already shows; point to the authoritative file or command instead.
Prefer rewriting or pruning existing entries over appending new ones.
When updating this file, preserve this bar for all agents and keep entries concise.
