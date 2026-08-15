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
- **R3-4: `CLAUDE_CONFIG_DIR=~/.claude` is NOT the same as leaving it
  unset, and that difference caused a false "sign-in expired" on the
  captain's live account.** With the variable set, Claude Code reads
  `<dir>/.claude.json` (the default account's real config is `~/.claude.json`,
  one level up) and derives the *hashed* Keychain service for that path —
  which for the default dir does not exist (`Claude Code-credentials-a83c75ce`:
  `errSecItemNotFound`; the real item is the bare `Claude Code-credentials`).
  Verified live, same account, same second: `CLAUDE_CONFIG_DIR=$HOME/.claude
  claude auth status --json` → `"loggedIn": false`; unset → `"loggedIn":
  true`. So every "renew the token via the CLI" call for the default account
  operated on an account the CLI thought was signed out. `providers/claude.rs`'s
  `claude_config_dir_env` is the one place that decides this (`None` = must
  not be set); `cli_invocation` carries it to both the renewal and
  `signin.rs`. Access tokens live ~8h, so this turned an ordinary expiry into
  "The sign-in expired" on an account that was signed in the whole time.
- **A menu bar app cannot reach `claude` through `$PATH`.** An app launched
  from Finder/the Dock inherits no `PATH` (the launchd GUI domain sets only
  `SSH_AUTH_SOCK`), so `Command::new("claude")` falls back to
  `/usr/bin:/bin:/usr/sbin:/sbin` and never finds a per-user install — which
  is why the renewal *and* `claude setup-token` silently did nothing in the
  shipped build while working under `npm run tauri dev` from a terminal.
  `providers/claude.rs`'s `claude_cli_path` layers `$PATH` → the user's own
  shell → Claude Code's documented install locations rebuilt from `$HOME`.
  Both shell forms are needed: measured here, `zsh -lc 'command -v claude'`
  finds nothing (this machine's `PATH` is set in `.zshrc`, which only an
  interactive shell reads) while `zsh -ilc` finds it in 0.6s. Verified with
  the shipped resolver under `env -i` (no `PATH`, no `SHELL`).
  Note when measuring this yourself: `open -a` and `NSWorkspace.openApplication`
  both *forward the calling process's environment*, so neither reproduces a
  Finder launch — confirmed with a marker `PATH`. `ps eww` cannot read
  Finder's own environment either (it prints nothing), so `launchctl print
  gui/$(id -u)` is the ground truth for what launchd hands an app.
- **Never say "sign-in expired" without proving the sign-in is what failed.**
  `FetchError` distinguishes `Unauthorized` (the sign-in itself is finished)
  from `CredentialStale` (the token merely aged out and is still renewable —
  a *local* problem), and the 401 path only retries after re-reading the
  credential and seeing it actually change; a renewal that did nothing must
  never cost a second request nor be reported as an expired sign-in
  (`classify_unrenewable`). An expired token is renewed *before* a request is
  spent on it (`EXPIRY_MARGIN_MS`), so an ordinary ~8-hourly expiry never
  reaches the user as a 401 at all. On the frontend the same distinction is
  `OutcomeResult.needsSignIn` / `Subscription.needsSignIn` — the badge used
  to be inferred from `state === "broken"`, so an offline launch or an HTTP
  403 also accused the account of being signed out.
- **The request budget is per real HTTP request, not per read attempt**
  (`providers::RequestBudget`, `accounts.rs`'s `AccountBudget`). One reservation
  for a read that could quietly make two requests is how Quotos spent the
  shared 5-per-300s allowance twice as fast as its own limiter believed,
  until the provider answered 429 with an hour-long `retry-after`. The
  one-time profile fetch is deliberately *outside* the budget — the captain's
  fixed one-read-per-minute cadence already consumes the whole allowance, so
  charging it would make the limiter refuse a scheduled read every launch.
- **A self-imposed wait must never disable the way out.** `useSubscriptions.ts`
  no longer filters rate-limited subscriptions out of `refreshAll` /
  `refreshAccountById`, and the panel's refresh button is no longer disabled
  by it: skipping them client-side meant a wrong diagnosis could never be
  re-tested, and the wait was itself a consequence of the wrong diagnosis.
  The Rust limiter refuses without spending anything, so always attempting is
  free. `lib/rowPresentation.ts` owns what a row says, because the two rules
  that keep it truthful are rules *between* the pieces: a row that needs
  signing in never also shows the budget wait, and the wait states its time
  once (it used to appear in the footer note *and* the action label).
- **Reading Claude Code's Keychain item cannot raise a prompt, and its ACL
  says so.** Both items' decrypt/export ACL trusts exactly `/usr/bin/security`
  with `promptSelector=0` and the `apple-tool:` partition — so `security
  find-generic-password -w` (what Quotos shells out to) is authorized, while
  any other binary gets `errSecAuthFailed`. To check this kind of thing
  without ever risking a dialog, call `SecKeychainSetUserInteractionAllowed(false)`
  first from a small `xcrun swift` snippet: securityd then returns an error
  instead of prompting. `QUOTOS_DEBUG_READS=1` traces the credential/read
  decisions (never a token) to stderr.
- **Sign-in recovery drives Claude Code's own login, never Quotos's own.**
  `quotos-app/src-tauri/src/signin.rs` spawns `claude setup-token` (via
  `providers::claude::cli_invocation`, see R3-4 above) attached to a real pty via
  `portable-pty` — plain pipes risk the CLI detecting a non-tty stdin and
  changing behavior, confirmed by one careful, throwaway-config-dir
  observation showing it renders an interactive, cursor-positioning prompt.
  The CLI opens the browser and prints the authorization URL *itself*;
  Quotos never parses that output, never opens a browser, and never reads
  or writes a credential — it only starts the process and relays a pasted
  code (from the panel's own field) into the process's stdin. Completion is
  detected by the process exiting, at which point Quotos re-reads the
  account normally; the exit status is informational only; the re-read is
  the real proof either way. **Unverified, and worth checking before relying
  on it:** where `claude setup-token` writes for the *default* account now
  that Quotos no longer forces `CLAUDE_CONFIG_DIR` there. Before R3-4 it
  wrote to a Keychain service nothing reads, so the flow was inert; it now
  points at the real account, and whether `setup-token`'s long-lived token
  lands in (or replaces) the same `claudeAiOauth` blob Quotos reads was
  never tested — completing a real login is the captain's own check.
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
  including the native-path variant). The restore has one exception (R5):
  a captured prior state that is itself in-flight ("reading"/"connecting")
  settles to `working`/`idle` instead of being written back — for an
  account whose *first-ever* read got rate-limited, the captured prior is
  the seeded "connecting", and restoring it left the row claiming
  "Reading…" forever while also masking the "Waiting for the rate budget"
  footer note (the row's `reading` branch wins over `footerNote`).
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
  change. `SubscriptionState`'s vocabulary is exactly the round-2 handoff's
  `working`, `reading`, `behind`, `broken`, `connecting`, `idle` (round 1's
  unused `"repairing"` was removed from the union and both `StatusDot`
  copies); don't introduce new state values without updating both this file
  and the handoff's row-state table.
- The "…" row menu (`SubscriptionRow.jsx`) closes on any interaction outside
  itself via a single `window` `mousedown` listener in `App.tsx`, gated by
  `data-quotos-menu-scope` on both the trigger button and the dropdown. It
  only ever *closes* — never (re)opens — so it can't race the trigger
  button's own toggle-on-click. Keep both elements' `data-quotos-menu-scope`
  attribute if you touch this markup, or the menu will close itself on its
  own click. **R4-5: it is `position: fixed`, placed from the trigger
  button's own viewport rect, and that is load-bearing.** As an
  absolutely-positioned child it lived inside the panel body's
  `overflow-y: auto` box, which both clipped its last item at the panel's
  bottom edge and — because an abspos element counts toward its scroll
  container's scrollable area — made a two-row panel scrollable just by being
  opened. A fixed element is neither clipped by an ancestor's `overflow` nor
  part of any scroll extent. This only works while nothing above the row
  establishes a containing block for fixed descendants: no `transform`,
  `filter`, `backdrop-filter`, `perspective`, `will-change` or `contain` on
  `Panel`'s wrappers (the blurred backdrop layer is a *sibling*). Deliberately
  not a React portal — `design/system/`'s components are consumed through a
  prebuilt `_ds_bundle.js`, so adding a `react-dom` import there would need
  that bundle regenerated.
- **R4-3: "Stop tracking" untracks immediately; the undo window is only a
  slot the panel keeps.** `Subscription.pendingRemoval` marks a row that the
  panel still draws (as the Undo row, in place) while `trackedSubscriptions`
  — what persistence, the tray digits and the Subscriptions screen all read —
  already excludes it. The previous shape deferred the whole removal by five
  seconds inside `App.tsx`, so the Subscriptions screen went on offering
  [Remove] for accounts the captain had already stop-tracked. If you add
  another consumer of the list, take `trackedSubscriptions`, not
  `subscriptions`.
- **R4-4: one account, one name, in both views.** `accountLabel` (the
  id-derived fallback) is title-cased, and `useSubscriptions` keeps a
  session-lifetime map of the best label a real read ever reported per
  account id, exposed as `displayLabelFor` for the Subscriptions screen's
  untracked half. Without it an account silently renamed itself from the
  provider's own "Claude Max" to a bare "Claude" the moment it stopped being
  tracked. Not persisted — it is derived from this session's reads.
- **S2: the Claude Code statusline feed is a second, zero-cost usage
  source, opt-in per subscription, installed from the Subscriptions
  screen.** `quotos-app/src-tauri/src/statusline.rs` owns install/status/
  remove against a config dir's `settings.json` (read-merge-write, atomic
  temp+rename, refuses on unparseable JSON, backs up the previous file plus
  a small metadata record of the previous `statusLine` value under
  `<app-support>/statusline-backups/`, and re-derives any conflict fresh on
  every `install()` call rather than trusting an earlier `status()` — a
  `settings.json` edited between the two is still caught). Confirmed live
  against Claude Code's own docs (code.claude.com/docs/en/statusline, not
  just the scout report): `statusLine: {type:"command", command}` runs in a
  shell and receives the exact JSON documented there on stdin, including
  `rate_limits.five_hour`/`.seven_day` (`used_percentage`, `resets_at` as
  Unix epoch seconds) — absent on a session's first invocation, and a
  script producing no stdout just leaves the statusline blank, which is why
  the helper prints nothing at all. **The "helper" installed into
  `<app-support>/statusline-helper/` is a copy of Quotos's own running
  executable, not a purpose-built sidecar binary** — `main.rs` intercepts
  `statusline::INGEST_FLAG` as `argv[1]` before `quotos_app_lib::run()`
  touches Tauri at all, so the copy (invoked by Claude Code's hook, possibly
  many times a minute) never spins up a second GUI instance; this sidesteps
  needing Tauri's `externalBin` sidecar bundling (target-triple-suffixed
  binaries staged before `tauri build`), which this round couldn't verify
  end-to-end in a sandboxed dev environment. The tradeoff is disk (tens of
  MB) for a local desktop app, not correctness — re-copied on a size
  mismatch so an app update refreshes it. On the frontend,
  `providers/claude/statuslineMerge.ts`'s `reconcileWithStatusline` patches
  the *raw* API usage shape (not the already-normalized window list) before
  `normalizeUsage` ever sees it, so every existing, tested rule (headline
  selection, severity, window naming) applies unchanged — a fresher
  statusline reading looks exactly like a fresher API response would have.
  Freshest-wins is a plain `written_at` vs. the API read's own `fetched_at`
  comparison, and it only ever refreshes a window the API response already
  asserts exists (never synthesizes one from `null`/absent — the write-
  mechanism contract's "never double-count"), which is also what makes "no
  interactive session is feeding it" require zero special-casing: an empty
  or stale feed is just never fresher. `providers/registry.ts`'s
  `Normalizer` signature carries a `NormalizeContext` (`fetchedAt`,
  `statuslineFeed`) for this — a provider with nothing to reconcile just
  ignores it.
- **v4: pinning is per-window, not per-subscription** — `Subscription.pinnedWindowIds:
  string[]` (a persisted set of `LimitWindowEntity.id`s) replaced
  `Subscription.pinned: boolean`. Every window a provider's normalizer
  builds now carries a stable `id` (`normalizeUsage.ts`'s `windowId(kind,
  scope)` — `kind` alone collides whenever two windows share it, e.g. two
  `weekly_scoped` entries for different models, hence the scope
  disambiguation). `NormalizedRead.headlineWindowId` names which window is
  "the headline" for a given read; the "…" menu's "Show/Hide in menu bar"
  toggles *that* window specifically (wording unchanged, target changed).
  Tray segments are built by `lib/traySegments.ts`'s `buildTraySegments`
  (panel order, then each subscription's own `windows` order — the same
  order the expanded row lists them in) and `worstActiveLimitPercent` (max
  `used` over every *active* window across every tracked subscription,
  deliberately narrower than `severity`, which counts inactive windows too)
  — both pure functions, called from `useSubscriptions.ts`'s tray effect.
  **Migration** (firstmate-recorded decision, not re-litigated): a pre-v4
  `pinned: true` record becomes "that subscription's headline window is
  pinned," resolved on the first *successful* read after load (headline
  isn't known until then) via `useSubscriptions.ts`'s
  `pendingPinMigrationRef` — a failed read leaves the migration pending
  rather than abandoning it. This needed a matching fix on the Rust side:
  `persistence.rs`'s `TrackedAccount` struct is a plain serde mirror of
  whatever's on disk, so simply renaming its field to `pinned_window_ids`
  would have silently *dropped* a legacy `pinned` value on load (serde
  ignores unknown fields by default) before the frontend ever got a chance
  to see it and migrate — caught by reasoning through the round-trip before
  the live check, not by a failing test. The fix carries both fields:
  `pinned_window_ids` (wire `pinnedWindowIds`, what's actually used) and a
  migration-only `pinned: Option<bool>` (present only when read from an old
  file; `skip_serializing_if` means a record sheds it from disk the moment
  it's next saved, since the frontend never sends it back). **General
  lesson**: any time a TS entity's on-the-wire shape changes, check whether
  a Rust struct mirrors it field-for-field (`grep` the Rust side for the old
  field name) — a mismatched Rust struct doesn't error, it just silently
  reshapes the JSON in transit.
- **v4: the tray glyph is a Q now, not an O** (design/NOTES.md §3) —
  `tray_render.rs`'s `glyph_coverage` moved the ring's gap from 82.75°
  centred at 90° (bottom) to 62° centred at 45° (lower-right), added a
  second capsule (the tail, r 3.2→8.0 along that same diagonal, same stroke
  weight, always drawn even at 0% used — "empty quota, empty letter"), and
  made the arc's own end angle a function of data (`used_fraction`) instead
  of an implicit constant 100%. The two gap-endpoint angles and the tail's
  exact start/end coordinates were cross-checked against
  `design/assets/tray-glyph-*.svg` directly (vector coordinates, not
  eyeballed) and match to 2 decimal places. The tail's own reach happens to
  stay just inside the ring's own axis-aligned bounding box at this specific
  geometry (checked by hand, not by construction) — `natural_outer_diameter`
  needed no change. `render()`'s glyph fill (`worst_used_percent`, 0-100) is
  sent on *every* `set_tray_status` call regardless of whether anything is
  pinned — the arc reflects the worst active limit across everything
  tracked whether the bar shows digits or not, not just in the empty state.
- **v4: the figure layout's "ink-to-ink" numbers in design/NOTES.md §1 are
  outcomes, not independent constants — measure them, don't derive them
  algebraically.** `tray_render.rs`'s actual structural constants
  (`SIDE_PAD_PX`, `GLYPH_TO_CELL_GAP_PX`, `CELL_WIDTH_PX`,
  `GROUP_GUTTER_PRE_PX`/`HAIRLINE_WIDTH_PX`/`GROUP_GUTTER_POST_PX`) are
  box-model gaps between fixed-width cells; the *ink*-to-ink distances the
  handoff table gives (6px glyph→digit, 8px in-group, 19px across the
  hairline) only fall out once each cell's own text-centring margin is
  added back on top — and that margin depends on the actual rendered digit
  width (MonoLisa or the system fallback, whichever this machine resolves
  to — see `text::load_font`), which isn't known until real text is
  measured. `CELL_WIDTH_PX` (30 CSS-px, the one hard invariant — §5) and
  `GROUP_GUTTER_*` landed correctly on a first guess; `GLYPH_TO_CELL_GAP_PX`
  needed one iteration (6 physical px measured 8px ink-to-ink live; 2
  physical px measured exactly 6px) — verified by screenshotting a live
  Retina tray capture (`screencapture -R`, `sips -s format bmp`, a small
  pure-Python BMP column-brightness scan) rather than trusting the algebra.
  See `data/quotos-v4-q1/verification.md` (firstmate data dir) for the full
  transcript and numbers.
- **v4: no SVG-to-raster CLI tool exists on this machine** (`rsvg-convert`,
  `cairosvg`, `inkscape`, ImageMagick's `convert`/`magick` — none
  installed; `sips` doesn't rasterize SVG). The app icon
  (`quotos-app/src-tauri/icons/icon.icns`, from `design/assets/app-icon.svg`)
  was exported by loading the inlined SVG into an `Image` inside an isolated
  headless Chrome page (via the Chrome DevTools MCP) and drawing it onto a
  same-sized `<canvas>` per target resolution (16/32/64/128/256/512/1024,
  each drawn straight from the vector source rather than downsampled from
  one raster, for crisp edges at every size), reading each canvas back via
  `toDataURL('image/png')`. Packed into a standard 10-file `.iconset`
  (`icon_16x16.png` … `icon_512x512@2x.png`) and converted with `iconutil -c
  icns` — no third-party tool needed for that last step, `iconutil` ships
  with Xcode command-line tools.

## Sharp edges

- **Never hardcode anything that describes the running system — read it fresh
  every time, and never bake in a number measured on one particular Mac.**
  The captain's standing rule (2026-08-14): not his subscriptions, not his
  monitor setup. Two cases already paid for ignoring this: a menu-bar-height
  constant that was right on one display and wrong on the other (see the
  menu-bar-height entry below), and `TrayIconEvent`'s rect / `Monitor`'s
  position / `set_position`'s physical coordinates each quietly using a
  *different* display's scale factor (see the coordinate-space entry below).
  The same rule extends to data, not just geometry: subscriptions/accounts
  are discovered (`tauriClient.listAccounts`), never listed in code.
- **A `#[tauri::command]` without `(async)` runs on the main thread, inline
  with the IPC — so it blocks the webview's own rendering.** (`tauri-macros`
  maps a plain command to `ExecutionContext::Blocking`.) That is correct for
  anything touching `NSStatusItem`, and wrong for everything else: three
  commands here forked processes or `fsync`'d on that thread before R4-2
  (`list_accounts`, `load_tracked`, `save_tracked` — all `(async)` now). The
  same reasoning applies to how often the main-thread ones are *called*:
  `set_tray_status` compares its segments and returns early when nothing
  changed, and only re-docks the open panel when the composited icon's width
  actually changed, because it is driven by an effect keyed on the whole
  subscription list.
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
  Always pass `Some("")` to clear it (see `repaint_tray_icon` in `shell.rs`).
  Verified by reading `platform_impl/macos/mod.rs` in the crate source
  directly — don't trust the `Option<S>` signature's apparent symmetry.
- **`tauri-plugin-positioner` is gone — its `TrayBottomLeft` anchor math was
  the round-2-to-round-3 regression that made the panel never visibly open.**
  A real click (posted via `fm-quotos-click.sh`, a real CGEvent through the
  HID tap, not synthetic Tauri-internal state) reliably produced a window
  position wildly inconsistent with the tray icon's own rect — e.g. tray at
  physical x=2444 on a 3456px-wide monitor placed the window at x=1368,
  nowhere near anything `move_window_constrained`'s documented clamp-to-monitor
  math could produce. This was confirmed independent of Tauri's own position
  getters (`WebviewWindow::outer_position()` was *also* observed misreporting
  immediately after a window's first-ever `show()` in this Tauri version —
  don't trust it for this kind of diagnosis; read ground truth via a tiny
  `xcrun swift -e` snippet calling `CGWindowListCopyWindowInfo` instead, which
  reports each window's real on-screen bounds independent of what the app
  itself believes). Root cause narrowed to the plugin's own
  `calculate_position`/`get_monitor_for_tray_icon` path, not chased further
  upstream — instead `show_panel` in `shell.rs` now computes the position
  itself, directly from the tray icon's own `Rect` (handed fresh on every
  click via `TrayIconEvent::Click`'s `rect` field, always physical pixels per
  `tray-icon` v0.24.2's own `Rect` type) plus `window.monitor_from_point` on
  that same rect, applying the handoff's left/top math itself with no
  plugin involved. The `tauri-plugin-positioner` dependency is fully removed
  (Cargo.toml, `lib.rs`'s plugin registration, the `on_tray_event` cache
  priming) rather than kept around for some other position mode — nothing
  else in the codebase used it.
- **R3-1: the glyph's on-screen offset inside the tray item must be derived
  fresh each time from measured numbers, never hardcoded — and even a
  single `tray.rect()` call right after `set_icon()` reads a stale rect.**
  Two independent bugs, both found only by pixel-measuring a real tray item
  (`screencapture` + centroid analysis, cross-checked against
  `CGWindowListCopyWindowInfo` and the item's own Accessibility rect — see
  `RESULT.md`'s round-3 entry for the exact numbers), caused the beak to
  drift off the glyph on pin/unpin exactly as the captain described. First:
  a prior round's `GLYPH_CENTER_FROM_ICON_LEFT_LOGICAL = 6px padding + 9px`
  constant was never actually checked against a real tray item — the true
  center sits at ~18pt in, not 15, because `NSStatusItem` centers the
  *whole* composited image (glyph, or glyph+digits) inside a button wider
  than the image by a system margin that isn't fixed once digits widen the
  image. `compute_docked_layout` now derives that margin fresh every call
  from two things Quotos actually knows — the item's *current* measured
  width (`tray.rect()`) and the composited image's own known width
  (`AppState.last_icon_width_px`, set by `set_tray_status` right before
  `set_icon`, always at the buffer's fixed "2x of an 18pt image" convention
  so `/2` gives real points regardless of monitor scale) — via
  `glyph_center_offset_from_item_left_points` (points, not pixels — see
  the coordinate-space entry below), rather than trusting any
  single constant again. Second: `set_tray_status` (pin/unpin) never
  re-docked the panel at all — no `TrayIconEvent` fires for a same-app icon
  resize, so the position captured at the last real tray click just went
  stale the instant the item's width (and therefore its on-screen left
  edge — status items lay out right-to-left, confirmed live: 36pt→64pt
  after pinning one segment, right edge unchanged) changed. Fixed by
  re-querying and re-docking on every `set_tray_status` call
  (`schedule_resync_after_icon_change`), but a *single* synchronous
  `tray.rect()` call made immediately after `set_icon()` was itself found
  to read a stale rect (AppKit's own layout pass for the new width hadn't
  caught up for one to a few runloop turns) — needs the same short-delay-retry
  shape as the `WindowEvent::Moved` correction's own retry,
  not a single attempt. Also found and fixed in the same pass: the computed
  glyph *center* was being fed straight into `Panel.jsx`'s `beakLeft` as if
  it were the beak box's own CSS `left` (its *left edge*, off by half the
  12px box), and the 14px inset between the window's own edge and the
  panel's edge (`app.css`'s shadow-blur margin, B9) was never subtracted —
  both silently wrong even when the glyph-center math itself was correct.
- **The beak's size and placement override the handoff, and the numbers live
  in four files with no build step syncing them.** The captain rejected the
  handoff's 12x12 beak on sight ("маленький… слишком близко к левому краю…
  слишком низко от топбара"), so: `Panel.jsx`'s `BEAK_BASE_HALF`/`BEAK_HEIGHT`/
  `NOTCH_RESERVE` (both copies), `geometry.rs`'s `BEAK_BASE_WIDTH`/`BEAK_HEIGHT`/
  `BEAK_INSET_IN_PANEL`/`BEAK_TIP_CLEARANCE` in `docked_layout_in_points`, and
  `app.css`'s `body { padding-top }` all have to move together — each one's own
  comment names the others. Two invariants make the whole thing work and are
  easy to break separately: the beak's centre must stay exactly on the glyph's
  centre (so "move the beak away from the corner" is implemented by moving the
  **panel** further left — the window x is derived from where the beak needs to
  be, replacing the handoff's `icon_left - 6`), and the layout solves for the
  beak's **tip** rather than the panel's top edge, since the window's own
  `padding-top` inset is what was putting a visible gap under the menu bar.
  `--shadow-popover`'s top bleed is zero by construction (`0 6px 16px -4px`
  reaches -2px above the box), so closing that gap costs nothing at the top;
  the left/right 14px margin is still load-bearing for the shadow (B9).
- **The beak is one shape with the panel, not a second layer.** Two stacked
  translucent elements paint `rgba(19,21,24,0.86)` twice at the overlap and the
  beak had no `backdrop-filter` of its own — a seam the captain could see over
  a bright desktop. `Panel.jsx`'s `buildPanelOutlinePath` emits a single path
  used both as the `clip-path` for one fill+blur layer and as one 0.5px stroke
  round the whole outline. Don't try to fix a seam by tuning opacities:
  backdrop-filter's blur kernel doesn't sample across an element boundary, so
  only one shape actually removes it. Judge it over a *bright* backdrop — a
  dark one hides both faults.
- **The tray image carries fixed side padding, always.** `tray_render`'s
  `SIDE_PAD_PX` (6pt per side) exists so A11's "panel open" highlight reads as
  a pressed menu bar button rather than a box hugging the ink, and it is
  applied whether highlighted or not so the glyph — and therefore the beak —
  cannot shift when the highlight toggles. `plain_glyph_rgba` and `render`
  must keep producing the same width (there's a test). `geometry.rs` reads
  `GLYPH_LEFT_INSET_POINTS` from `tray_render` rather than assuming the glyph
  is the image's leftmost 18pt.
- **This machine is live and shared — not a clean test box — and
  screenshotting this app's own windows only ever shows whatever's on the
  *currently active macOS Space*, which is frequently not where a tray
  click's resulting window actually is.** Confirmed more than once, directly:
  screenshots taken mid-investigation caught an unrelated Claude Code
  session's terminal on the primary display and the captain's own live
  Chrome session (mid Google-Images search) on the second — genuine
  concurrent activity, not a fixture, actively changing which Space is
  "active" independent of anything this work does. `screencapture` (any
  `-D`/`-l` variant) only ever captures the active Space; a window that
  gained focus and reports `is_visible=true` internally can still be
  invisible to every `screencapture` invocation for this reason alone —
  confirmed as an environment property, not an app bug, by reproducing the
  *identical* symptom against the known-good `Quotos 2` build as a control.
  A **real** click from the captain does not have this problem (his cursor
  and "current Space" are the same thing at the moment he clicks); it is
  specifically synthetic, cursor-restoring clicks fired from here, on a box
  with other concurrent Space-changing activity, that can't reliably land on
  a Space anyone is currently looking at. Ground truth that *is*
  Space-independent: `CGWindowListCopyWindowInfo` (via a small `xcrun swift
  -e` snippet — JXA's ObjC bridge could not get a usable `NSArray` out of the
  raw `CFArrayRef` this returns, don't waste time on that route) reports each
  window's real bounds and `kCGWindowIsOnscreen` regardless of the active
  Space, and Tauri's own `Moved`/`Focused` window events (logged via a
  temporary `on_window_event` eprintln, removed before commit each time —
  don't leave these in) confirm positioning and focus independent of any
  screenshot. `screencapture -l <windowID>` does not help either — it
  renders blank/white for a window on an inactive Space (no real compositing
  happened to sample from), even though the window and its content genuinely
  exist.
  **R4 correction, and check this first before blaming the environment: a
  bare `cargo build` binary renders nothing at all.** Without the `tauri` CLI
  driving it, `generate_context!` resolves to the *dev* configuration and the
  webview loads `build.devUrl` (`http://localhost:1420`) instead of the
  bundled `dist` — with no vite running, the page never loads, and a
  `transparent: true` window with no page is invisible. That is
  indistinguishable from "the window is on another Space": correct bounds,
  `kCGWindowIsOnscreen=true`, blank capture. Hours were spent on the Space
  theory before this turned out to be the whole explanation. Use `npm run
  tauri dev` (or a real bundle); with it, `screencapture -R <the window's own
  bounds from CGWindowListCopyWindowInfo>` captured the panel reliably, dozens
  of times, including one opened by a real tray click. Ordering evidence that
  *is* trustworthy either way: `CGWindowListCopyWindowInfo(.optionOnScreenOnly)`
  returns front-to-back, so where the panel sits relative to a full-screen
  window is readable without capturing anything.
- **Reading the live DOM beats reasoning about it: a temporary
  `debug_report(String)` command plus a small measurement module is the way to
  get real layout numbers out of the WKWebView.** jsdom has no layout engine
  and there is no headless browser in this repo, so "does the panel scroll?"
  is otherwise unanswerable. Have the module drive the UI itself
  (`element.click()`), not synthetic CGEvents — no aiming, no risk of a click
  landing outside Quotos. Two things that cost a cycle each: React
  `StrictMode` invokes the effect twice, so an unguarded measurement script
  fires every click twice (a menu opens and instantly closes); and vite HMR
  does *not* re-run a `[]`-dependency effect, so iterate by restarting, not by
  editing. Remove the scaffolding before committing.
- **There is no global "physical pixel" coordinate space on macOS, and three
  APIs this app depends on each invent a different one.** This was the root
  cause of the captain's "панель мерцает, прыгает по экрану, появляется не на
  том мониторе", and the arithmetic is only safe in **global points**
  (`CGDisplayBounds`/`NSScreen.frame`). Each of these hands out a `Physical*`
  type that is really "points x *some* display's scale factor", and they
  disagree about which: `TrayIconEvent`'s `rect` uses the **menu bar
  display's** scale (`tray-icon` 0.24.2 `get_tray_rect`), `Monitor::position()`/
  `size()` use **that monitor's own** (`tao` `platform_impl/macos/monitor.rs`),
  and `set_position(Physical)`/`WindowEvent::Moved` use **the window's
  current** one (`tao` `window.rs`/`window_delegate.rs` — all read from the
  crate sources, not inferred). They coincide on a single-display machine and
  diverge on a mixed-DPI multi-display one. `geometry.rs`'s `DisplayPoints` doc
  comment carries the full table and the worked examples; the rules that fall
  out: convert at the edges via `resolve_tray_point` (it recovers the tray
  rect's true point position by trying each display's own scale and keeping
  the quotient that lands inside that display — the scale cannot be known in
  advance), never store or compare a `Physical*` value, and place the window
  with a `LogicalPosition`, which `tao`'s `Position::to_logical` passes through
  untouched so no scale factor is consulted on the way out.
- **`set_position` lands asynchronously but `show()` runs inline — position
  before showing, and place the frame synchronously yourself.** `tao`'s
  `set_outer_position` ends in `util::set_frame_top_left_point_async`
  (dispatched to the main queue) while `set_visible(true)` ends in
  `make_key_and_order_front_sync` (inline). Called in either order from the
  tray handler, which is already on the main thread, the window becomes
  visible at its stale position and moves a runloop turn later — a guaranteed
  one-frame flash wherever it last was. `place_window_top_left_sync` in
  `shell.rs` sets the `NSWindow` frame directly instead.
- **Menu bar height differs per display on one machine — don't hardcode it**
  (see the do-not-hardcode-the-environment rule at the top of this section).
  The handoff's "32px from the top" is a bar height plus a 6px gap, and this
  machine's notched built-in measures **33pt** against an unnotched display's
  24-30pt — so the constant put the panel *inside* the bar on one display and
  8pt too low on the other, and AppKit silently clamped the overlap back out
  (which reads as a mysterious 1pt offset). Derive it from
  `NSScreen.visibleFrame` per display, and keep the tray-item fallback in
  `docked_layout_in_points`: inside a full-screen Space the bar is auto-hidden
  and `visibleFrame` reports no bar at all *even while the bar is on screen
  under the cursor*. Both paths were checked to agree.
- **R4-1: the panel must never activate the application — that, not any
  collection behaviour, is what threw the captain out of a full-screen
  Space.** Rounds 2 and 3 chased this as window *membership*
  (`CanJoinAllSpaces | FullScreenAuxiliary | Transient | IgnoresCycle`,
  `NSStatusWindowLevel`, `orderFrontRegardless`); all of it is necessary and
  all of it is kept, and none of it was the trigger. `set_focus()` ends in
  `activateIgnoringOtherApps: YES` (`tao`'s `util::set_focus`), and activating
  another application while a full-screen Space is frontmost is exactly what
  makes macOS leave it — the same visible slide Cmd-Tab produces. The fix is
  `src-tauri/src/panel_window.rs`: the window becomes a **non-activating
  `NSPanel`**, the one AppKit type that can hold keyboard focus while another
  app stays active, so the show path never activates at all. Read that module
  before touching any of it — it carries the class-swap safety argument, the
  measured A/B, and two traps (the non-activating style bit on a plain
  `NSWindow` *aborts the process*; the swap displaces a pre-existing KVO
  isa-swizzle). `QUOTOS_PANEL_MODE=window` restores the old path for a
  same-session comparison. **Unverified end-to-end**: an agent here cannot put
  an app into full screen; what is measured is that the frontmost application
  no longer changes while the panel is key, which is the condition macOS needs
  to stay put.
- **`-[NSApplication isActive]` is useless for "did we steal focus" in an
  accessory app** — it read `true` both with and without
  `activateIgnoringOtherApps`, so the `app_active` field in the
  `QUOTOS_DEBUG_POS` trace cannot discriminate. Read
  `NSWorkspace.frontmostApplication` / `NSRunningApplication.isActive` from a
  *separate* process (`xcrun swift <file>.swift`) instead; that told the two
  cases apart immediately.
- **R5: relocating this window's frame while the mouse is held down over it
  reactivates the app, on this non-activating panel and on the old activating
  `NSWindow` alike — R4-1's fix does not cover dragging, and no repositioning
  API sidesteps it.** Found chasing "detached panel doesn't drag by hand"
  (`4495bdc` claimed the fix and didn't close it): the header's
  `startDragging()` call was first found to do nothing at all because
  `capabilities/default.json` never granted `core:window:allow-start-dragging`
  (missing from `core:window:default`/`core:default`, unlike `allow-show`/
  `allow-hide`/etc. which are listed explicitly for the same reason) — a
  denied-ACL promise, never awaited or caught, so silently invisible. Granting
  it made `performWindowDragWithEvent:` actually move the window, which
  surfaced the real, deeper finding: doing so measurably reactivates the app
  (`NSWorkspace.frontmostApplication`, read from a separate process, same as
  R4-1's own method) — and replacing that call with a hand-rolled drag
  (`drag_window_step` in `shell.rs`, moving the frame directly via the same
  `place_window_top_left_sync` the docking path already uses, on every
  `mousemove`) has the **identical** problem. Isolated precisely: a stationary
  click-and-hold never activates, a full click-drag-release gesture over
  non-header content that never touches the window's frame never activates,
  and a *single* frame-set mid-gesture is enough to activate regardless of
  which API performs it — so the trigger is "the frame changed while a
  mouse-down is live over this window," not anything specific to
  `performWindowDragWithEvent:`'s documented Space-participation. Escalated to
  the captain rather than picked silently; his call (2026-08-15): **ship as
  live-follow, reactivation and all** — dragging working smoothly, at the cost
  of reactivating only for the physical gesture's duration (never on show, and
  never on a Magnet/AX move), beat the two alternatives offered (a live
  "hand focus back to the previous frontmost app" correction, rejected as an
  unverifiable hack this sandbox cannot check against a real full-screen
  Space; commit-only-on-mouseup, rejected for feeling like repositioning
  rather than dragging). If a future round needs to revisit this, both
  rejected alternatives are still open, just not the current answer.
- **A second, previously-unreachable bug the above uncovered: `set_detached`'s
  snap-back path never restored `NSStatusWindowLevel`, because `tao`'s
  `set_always_on_top` is asynchronous.** It ends in `util::set_level_async`
  (`DispatchQueue.main.async`), so a call still queued from the *detach* that
  preceded a snap-back could fire *after* the snap-back's own synchronous
  `setLevel(25)` restore and silently drop it back to
  `NSFloatingWindowLevel` — confirmed live with a temporary trace showing the
  restore call run and return, then the window still read back at the lower
  level moments later. `set_detached` now only calls `set_always_on_top` in
  the `detached=true` branch; snap-back relies solely on the synchronous
  `set_popover_collection_behavior`. Unreachable, and so unseen, until
  dragging itself worked at all — the captain caught this live, mid-round, by
  watching a real detach/reattach happen for the first time.
- **Iterate on tray/panel behaviour without clicking the captain's menu bar.**
  He watches it while he works and repeated test-clicking drew a complaint.
  `QUOTOS_DEBUG_AUTO_OPEN=1` opens the panel from `tray.rect()` a few seconds
  after launch with no click at all (only the *event* needs a click; the rect
  does not); `QUOTOS_DEBUG_KEEP_OPEN=1` suppresses hide-on-blur so the panel
  survives long enough to photograph; `QUOTOS_DEBUG_POS=1` traces every input
  and output of the placement arithmetic plus `isOnActiveSpace`. All opt-in,
  all silent by default.
- **Drawing real text (Core Text) into an offscreen `CGBitmapContext` has two
  non-obvious failure modes, both found by rendering `tray_render.rs`'s
  actual `render()` output to PNG and inspecting it directly (screenshotting
  the live tray/panel doesn't work here — see above), not by trusting
  passing unit tests (the original tests only checked "a pixel of
  approximately the right color exists somewhere," which survived both bugs
  below undetected).** (1) An 8-bit **alpha-only** (`kCGImageAlphaOnly`)
  context — chosen to sidestep premultiply/unpremultiply math entirely,
  since only a coverage mask was thought to be needed — corrupts glyph
  shapes when used with `CTFontDrawGlyphs`/`CTLineDraw`; specific glyphs come
  out with wrong or missing strokes (e.g. a "7" losing its entire top bar)
  while others render fine, which reads exactly like "some other bug" until
  you render several test strings and compare glyph-by-glyph. Use a normal
  (premultiplied) RGBA context instead and unpremultiply on read-back. (2)
  The standard flip for turning a `CGBitmapContext`'s native bottom-left/
  y-up coordinate system into top-left/y-down (`CGContextTranslateCTM(0,
  h)` + `CGContextScaleCTM(1,-1)`) works fine for shapes/fills but **mirrors
  glyphs drawn via CoreText**, because `CTFontDrawGlyphs`/`CTLineDraw`
  orient glyph outlines relative to the CTM's handedness rather than
  compensating for it — the textbook fix is to *also* set a matching
  flipped `CGContextSetTextMatrix`, but it's simpler to just draw in the
  context's native (unflipped) convention and adjust the baseline-position
  formula to account for that instead (`tray_render.rs`'s `draw_text_impl`
  does this — see its comment for the exact math). A third, unrelated bug
  found the same way: pairing a `CGColorCreateGenericRGB` fill color with a
  `CGColorSpaceCreateDeviceRGB` bitmap context shifts even fully-opaque
  pixels well off the requested color (a real ~20-point-per-channel gamma
  mismatch, not rounding noise) — use sRGB consistently for both the fill
  color (`CGColorCreateSRGB`) and the context's color space
  (`CGColorSpaceCreateWithName(kCGColorSpaceSRGB)`), since design-token
  colors are plain CSS hex values, i.e. already sRGB by convention.
- **The tray glyph is drawn procedurally now, not from a raster asset —
  there is no `icons/tray/tray-icon.png` to update if the mark ever
  changes.** `tray_render.rs`'s `glyph_coverage` ports the exact geometry of
  `design/system/assets/menubar-glyph.svg` (a 16×16-viewBox capacity-gauge
  mark: a faint full-circle track, `r=5.4` stroke `1.4` opacity `0.28`, plus
  a bold round-capped arc on the same circle, stroke `1.9`, with a gap at
  the bottom — the gap's two angles were derived by hand from the SVG path's
  endpoints via `atan2`, see the function's own doc comment for the exact
  numbers) into a per-pixel signed-distance-style coverage test, computed
  fresh at whatever resolution is asked for. This replaced a 22×22 bundled
  PNG that firstmate's on-screen pass measured as both undersized (ink
  11.5×10.5pt against neighbouring menu bar icons' 13.5-20pt) and visibly
  soft (nearest-neighbor-upscaled for the "digits pinned" path, and hand off
  entirely to macOS's own unscaled `NSImage` scaling for the "nothing
  pinned" path — two different blurry paths for the same mark). If the
  design system's SVG ever changes, update the constants at the top of
  `glyph_coverage` to match rather than reaching for a new raster export —
  that's the mistake this replaced. The target ink size
  (`TARGET_INK_DIAMETER_CSS_PX`) is independently guarded by a test,
  `glyph_ink_bounding_box_is_in_the_target_band`, that measures the real
  composited output rather than trusting the constant alone.
- **A11 (tray "panel open" highlight) is drawn into the same composited
  bitmap as the glyph/digits, not reached via any native `NSStatusItem`
  highlighted state** — `tray_render.rs`'s `draw_highlight_background`
  paints the handoff's exact translucent rounded rect
  (`rgba(255,255,255,0.20)` dark / `rgba(0,0,0,0.14)` light) behind
  everything else, composited with real "src-over" alpha blending
  (`blend_pixel`, which replaced the old `put_pixel`'s flat overwrite —
  needed the moment two translucent layers can occupy the same pixel, since
  an overwrite would silently discard the highlight everywhere the glyph or
  a digit covers it). Driven by `AppState.tray_highlighted` (set from
  `show_panel`/`hide_panel`/the click-away and detached-toggle hide paths —
  never from the frontend) plus a cached `AppState.last_tray_segments` (so
  toggling the highlight alone, with no new pinned data, can still repaint
  with the *same* digits) — both read fresh by `repaint_tray_icon`, the one
  place `set_icon` is actually called now, shared by `set_tray_status` and
  `set_tray_highlighted`. Verified live by pixel-diffing the same tray icon
  closed vs. open (`fm-quotos-click.sh peek`): zero difference anywhere
  except an 18×18 region exactly over the glyph.
- I7's `set_detached` IPC command has no detach *button* — dragging the
  header is the only way to detach (`App.tsx`'s `handleHeaderPointerDown`),
  matching the handoff's "first movement detaches" rule. The browser/mock
  harness has no OS window to move, so it simulates the same interaction by
  fixed-positioning the Panel via CSS instead (see `position`/`docked`
  props) — this is why dragging is testable via `npm run dev` even though
  the native path below isn't, from here.
- **R3-3: `startDragging()` must be called synchronously on the real
  mousedown, not after waiting for the first `mousemove` — the latter is
  what silently broke native dragging entirely (the captain: "the window
  doesn't drag at all").** `getCurrentWindow().startDragging()` →
  `tauri-runtime-wry`'s `WindowMessage::DragWindow` → `tao`'s
  `drag_window()` → `NSWindow.performWindowDragWithEvent`, which hands
  AppKit whatever `NSApp.currentEvent()` is *at the moment the Rust side
  actually runs it* (`tao`'s `platform_impl/macos/window.rs` only
  synthesizes a substitute event for one narrow stale-event case, not the
  general one). Waiting for a `mousemove` before calling `invoke()` adds a
  full webview→IPC→main-thread round trip on top of the movement itself, so
  by the time it lands, `currentEvent` is essentially never still the
  mouseDown Apple's docs say to call this from. Fixed in `App.tsx`'s
  `handleHeaderPointerDown`: `startDragging()` is now called immediately,
  unconditionally, on `mousedown` itself (harmless on a click that never
  moves — AppKit's own tracking loop treats a stationary mouseDown-then-up
  as a no-op) — the *visible* detach (beak gone, snap-back arrow, no
  close-on-click-away) still only flips on the first real `mousemove`,
  separately, so C3 ("a plain click does not detach") is unaffected. Also
  removed `set_detached`'s `window.set_decorations(true)` while detached —
  with `titleBarStyle: Overlay`, turning decorations on for *any* reason
  paints real traffic lights plus a native title-bar strip regardless of
  the window's own transparency, which was the captain's other complaint
  (system chrome, oversized frame) — decorations must stay off in both
  states, always; dragging needs no native title bar. **Not provable
  end-to-end from here**: a synthetic mousedown+drag (`CGEventPost`, same
  mechanism as `fm-quotos-click.sh`) correctly drove this component's own
  movement tracking (confirmed the snap-back arrow appears) but produced
  zero native `WindowEvent::Moved` events, with or without this fix —
  checked directly with a temporary logger. Consistent with, but not proof
  of, the same synthetic-input limitation already documented above for tray
  clicks; get the captain to confirm with a real drag before assuming
  either way.
- **Third-party window managers (Magnet) can resize this window despite
  `resizable: false`.** That flag only disables the *native* resize-handle
  drag; a window manager that resizes via the Accessibility API's
  `kAXSizeAttribute` setter goes straight to `-setFrame:`, which doesn't
  consult the style-mask resizable bit at all. `lib.rs`'s
  `on_window_event` now also matches `WindowEvent::Resized` and snaps the
  size back to the fixed 360×560 logical whenever it drifts (in either
  docked or detached state) — deliberately only fights a *resize*, not a
  *move*, so a Magnet move/snap-position action still works. Reasoned from
  `tao`/AppKit's documented behavior, not observed against a live Magnet
  action (out of bounds for an agent here — Magnet must never be driven
  directly).
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
