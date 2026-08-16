# Architecture

Quotos splits along the Tauri process boundary: Rust owns the operating
system, native windows, the Keychain, and every HTTP request; React owns
what gets drawn. The two sides talk through a small set of
`#[tauri::command]`s and `emit`/`listen` events, never anything wider.
Individual modules carry their own reasoning in their own doc comments;
this page is the map between them.

## Rust backend

| Module | Owns |
| --- | --- |
| `lib.rs` | Application state and the Tauri setup: window and menu events, the tray click handler, wiring every command. |
| `shell.rs` | The interactive shell: the status item's repaint pipeline and the panel's show, hide, dock, detach, and drag lifecycle. |
| `geometry.rs` | Pure placement math and read-only screen queries. No module here moves a window; `shell.rs` does that with these numbers. |
| `panel_window.rs` | Turns the panel into a non-activating `NSPanel`, the AppKit window class that can hold keyboard focus without activating its application. |
| `status_item_render.rs` | Composites the status item's glyph and colored percentage digits into an RGBA bitmap, since `tray-icon` has no colored-title path. |
| `accounts.rs` | The account-data plane: discovery, the one real fetch path, the shared per-account rate budget, the native refresh scheduler, and the IPC commands for the tracked list, statusline integration, and sign-in. |
| `ratelimit.rs` | The sliding-window request budget one account's reads share with Claude Code itself. |
| `scheduler.rs` | The native one-read-per-account-per-minute timer. |
| `persistence.rs` / `atomic_write.rs` | The tracked-subscriptions list as a durable JSON file, and the fsync-then-rename write helper it shares with `statusline.rs`. |
| `providers/mod.rs` | The provider trait, the request-budget trait, and the typed `FetchError` every provider returns on failure. |
| `providers/claude.rs` | The Claude Code adapter: Keychain reads, the usage and profile HTTP calls, credential renewal. |
| `signin.rs` | Drives Claude Code's own `claude setup-token` login over a pty. Quotos never touches or writes a credential itself. |
| `statusline.rs` | Installs and reads the opt-in Claude Code statusline feed, a second, zero-cost usage source. |
| `single_instance.rs` | The OS file lock that keeps one Quotos running per machine, since the shared request budget assumes exactly one reader. |
| `launch_at_login.rs` | The Launch at Login toggle, through `SMAppService`. Quotos stores nothing; the OS is the single owner of this state. |

## React frontend

| Module | Owns |
| --- | --- |
| `App.tsx` | Composes the panel shell, the subscription rows, and the Subscriptions screen. |
| `hooks/useSubscriptions.ts` | The state machine: tracked-subscription membership, refresh, the rate-limit interception rule, persistence, and status item segment derivation. |
| `providers/registry.ts` | The frontend half of the provider seam: dispatches normalization and outcome mapping to the right provider by its slug. |
| `providers/claude/` | The Claude adapter: usage and profile normalization, headline selection, outcome-to-state mapping, statusline reconciliation. |
| `lib/persistence.ts` | The frontend seam to the tracked list: the native IPC commands on a real build, `localStorage` as a fallback in the browser harness. |
| `lib/tauriClient.ts` | Swaps between the real Tauri IPC client and a mock client based on whether `__TAURI_INTERNALS__` exists, so every UI state stays reviewable from a plain browser. |
| `lib/rowPresentation.ts` | What a row shows besides its numbers: the badge, the one action offered, and the footer note. |
| `lib/statusItemSegments.ts` | Turns tracked subscriptions into the status item's digit segments, tooltip, and worst-active-limit percentage. |
| `types/entities.ts` | The provider-agnostic entities. Nothing above its own dividing line, and nothing that renders UI, references a specific provider by name. |
| `src/design-system/` | The component library the app builds against. See its own `index.ts` barrel and [contributing.md](contributing.md) for the module-layout rule that keeps consumers on it. |

## The provider seam

Adding a provider is a new module on each side plus one entry in
`providers/mod.rs` and `providers/registry.ts`. Nothing else references
a provider by name: not the panel, not the hooks, not the state
machine.

Two things stay provider-owned rather than living in the generic shell,
because they are genuinely provider-specific policy:

- **Headline selection.** `normalizeUsage.ts`'s `pickAccountWideWeekly`
  picks the account-wide weekly window as the headline number, never
  simply the most-consumed window, so a heavily used per-model window
  can't outrank the real account total.
- **Outcome mapping.** `providers/claude/index.ts`'s `mapOutcome`, called
  through `registry.ts`'s `mapOutcomeFor`, turns a read outcome into a
  `SubscriptionState` and a reason string. The generic hook only ever
  supplies the outcome and whether a prior good read existed.

`rate_limited` is deliberately outside this mapping. See State model
below.

## From a read to a rendered row

1. A read starts from `scheduler.rs`'s native timer or a manual action;
   both call the same `fetch_snapshot` command, so a manual refresh resets
   the scheduled minute for free.
2. `fetch_snapshot` reserves one slot of the account's shared request
   budget, calls the provider, and returns a normalized snapshot or a
   typed `FetchError` over IPC, never a stale number as if it were
   current.
3. `useSubscriptions.ts` applies the result the same way whether it
   arrived from a direct call or from the scheduler's `quota-refresh`
   push event, through `applyRefreshResult`.
4. The same state feeds three places at once: the panel row, the
   persisted tracked list, and the status item's digits.

## State model

- **Percent fields mean consumed, not remaining**, across
  `Subscription.used`, `LimitWindowEntity.used`, and
  `NormalizedUsage.used`.
- **`severity`** is computed from every window a subscription has, not
  just its headline one: `critical` when any window is at least 90%
  used, `warn` at 75%. A subscription with a low headline percentage can
  still read amber, because color comes from `severity`, not from the
  headline number's own size.
- **`idle` and "no limits reported yet" are different states.** `idle`
  means the subscription has never been read successfully. `state:
  "working"` with `used: null` means a good read that had nothing to
  report. `lib/rowPresentation.ts` shows `reason ?? "No limits reported
  yet."` for every state without a number.
- **Health and the rate-limit budget are separate fields on purpose.** A
  self-imposed wait must never overwrite a real diagnosis. `useSubscriptions.ts`
  intercepts a `rate_limited` outcome before it reaches a provider's
  outcome mapper and restores the state and reason captured before the
  attempt started, except when that captured state was itself in
  flight, in which case it settles to `working` or `idle` instead of
  being written back verbatim as "reading forever."
- **A subscription's refresh is never skipped client-side for being
  rate-limited.** The Rust limiter refuses without spending anything, so
  attempting is free, and skipping would make a wrong diagnosis
  impossible to retest.
- **`FetchError` distinguishes `Unauthorized`, meaning the sign-in itself
  ended, from `CredentialStale`, meaning the access token merely aged
  out and is still renewable.** Conflating the two reports a working
  account as needing to sign in again. See
  [claude-provider.md](claude-provider.md) for the renewal mechanics
  that produce this distinction.

## Persistence

`persistence.rs` writes the tracked list, membership, custom names, and
pins, as a plain JSON file rather than trusting the webview's
`localStorage`, through `atomic_write.rs`'s temp-file-then-rename helper.
`lib/persistence.ts` wraps the same data on the frontend: the native
`load_tracked` / `save_tracked` commands on a real build, `localStorage`
in the browser harness, which has no Rust side to call.

A `localStorage` write returns as soon as the in-memory page state
updates, and WebKit flushes its backing store to disk on its own
schedule; whether an abrupt quit can race that flush was never
conclusively pinned down, so the Rust side owns the durable copy instead,
synchronously `fsync`'d before `save` returns. A plain JSON file rather
than SQLite for the same reason as every data store this small in the
app: a handful of accounts is simpler to read, review, and hand-edit as a
file than as a database, and a file serves every query pattern this data
needs.

Pinning is per limit window, not per subscription:
`Subscription.pinnedWindowIds` is a persisted set of window ids, since
every window a provider's normalizer builds carries a stable id from
`normalizeUsage.ts`'s `windowId(kind, scope)`. `persistence.rs`'s
`TrackedAccount` struct mirrors the wire shape field for field, including
a migration-only field for a tracked list written before per-window
pinning existed. Any time an entity's wire shape changes, check whether
this struct still mirrors it, since a mismatched Rust struct does not
error; it silently reshapes the JSON in transit.

## Refresh scheduling and the shared request budget

`scheduler.rs` runs natively rather than as a JavaScript interval,
because macOS suspends timers in a hidden or occluded `WKWebView`; a
native OS-level timer has no notion of a hidden webview to throttle.
`ratelimit.rs`'s sliding window is the account-level budget this
scheduler's cadence assumes stays full; the request budget itself is
reserved per real HTTP request, not per read attempt, since a read that
quietly makes two requests would otherwise spend the shared allowance
twice as fast as the limiter believes.

Because that budget is shared per account and not per process,
`single_instance.rs` keeps exactly one Quotos running per machine with an
OS file lock. Every path that spends a real fetch on an account, a
header refresh, a row's manual refresh, a launch read, or a newly added
subscription's first read, funnels through `useSubscriptions.ts`'s
`refreshOneGuarded`, the one per-account in-flight guard, so a concurrent
request joins the in-flight read instead of spending a second slot.

## Coordinate spaces and panel placement

No single "physical pixel" coordinate space exists across the APIs this
app depends on: a tray click's rect, a monitor's position, and a
window's own position each carry a different display's scale factor.
`geometry.rs`'s `DisplayPoints` documents the full conversion table and
converts everything to points at the edges; nothing else in the codebase
stores or compares a physical value. `shell.rs` computes the panel's
position itself from the tray icon's rect, handed fresh on every click,
rather than through any positioning plugin.

The header's `startDragging()` call fires synchronously on `mousedown`,
not after the first `mousemove`, since AppKit's window-drag API uses
whatever the current event is at the moment Rust actually runs it.
Relocating the window's frame while a mouse button is held over it
reactivates the app regardless of which API moves it; Quotos accepts
that as the cost of a working drag rather than fighting it.

## The beak

The panel's beak is one shape with the panel itself, not a second
layer, drawn from a single outline path so no two translucent layers
overlap and double their alpha at the seam. Its size and position live
in three files with nothing syncing them automatically: `geometry.rs`'s
`docked_layout_in_points`, `Panel.tsx`'s matching constants, and
`app.css`'s panel padding. The beak's center stays on the status item
glyph's own center; the layout solves for the beak's tip, not the
panel's top edge.
