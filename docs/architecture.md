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
| `shell.rs` | The interactive shell: the status item's repaint pipeline and the panel's show, hide, dock, detach, and drag lifecycle. See [panel-lifecycle.md](panel-lifecycle.md). |
| `geometry.rs` | Pure placement math and read-only screen queries. No module here moves a window; `shell.rs` does that with these numbers. |
| `panel_window.rs` | Turns the panel into a non-activating `NSPanel`, the AppKit window class that can hold keyboard focus without activating its application. |
| `status_item_render.rs` | Composites the status item's glyph and colored percentage digits into an RGBA bitmap, since `tray-icon` has no colored-title path. See [status-item-rendering.md](status-item-rendering.md). |
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

### Module file names

Rust derives a module's identifier from its file name, and a Rust
identifier cannot contain a hyphen, so a kebab-case file such as
`atomic-write.rs` can only back the `atomic_write` module through a
`#[path = "atomic-write.rs"] mod atomic_write;` declaration. Every module
in this crate would need one, since every multi-word module here is
snake_case: `atomic_write`, `launch_at_login`, `panel_window`,
`single_instance`, `status_item_render`, plus the single-word modules
that already read as kebab-case by coincidence. That `#[path]` line
would then sit on every `mod` declaration in `lib.rs`, permanently
diverging the file name from the module identifier every reader, every
`rustfmt` run, and every `rust-analyzer` jump-to-definition would show
them by, in exchange for no readability gain: a Rust file's own name is
never read as a URL slug or a cross-language import path the way a
TypeScript module's is. `snake_case` file names matching their module
identifiers is also the convention the Rust ecosystem itself uses
throughout the standard library and crates.io, the same kind of
established, tool-recognized spelling this repository already keeps for
`README.md`. This crate's source files stay `snake_case`.

## React frontend

| Module | Owns |
| --- | --- |
| `app.tsx` | Composes the panel shell, the subscription rows, and the Subscriptions screen. |
| `hooks/use-subscriptions.ts` | The state machine: tracked-subscription membership, refresh, the rate-limit interception rule, persistence, and status item segment derivation. |
| `providers/registry.ts` | The frontend half of the provider seam: dispatches normalization and outcome mapping to the right provider by its slug. |
| `providers/claude/` | The Claude adapter: usage and profile normalization, headline selection, outcome-to-state mapping, statusline reconciliation. |
| `lib/persistence.ts` | The frontend seam to the tracked list: the native IPC commands on a real build, `localStorage` as a fallback in the browser harness. |
| `lib/tauri-client.ts` | Swaps between the real Tauri IPC client and a mock client based on whether `__TAURI_INTERNALS__` exists, so every UI state stays reviewable from a plain browser. |
| `lib/row-presentation.ts` | What a row shows besides its numbers: the badge, the one action offered, and the footer note. |
| `lib/status-item-segments.ts` | Turns tracked subscriptions into the status item's digit segments, tooltip, and worst-active-limit percentage. |
| `types/entities.ts` | The provider-agnostic entities. Nothing above its own dividing line, and nothing that renders UI, references a specific provider by name. |
| `src/design-system/` | The component library the app builds against. See its own `index.ts` barrel and [contributing.md](contributing.md) for the module-layout rule that keeps consumers on it. |

## The provider seam

Adding a provider is a new module on each side plus one entry in
`providers/mod.rs` and `providers/registry.ts`. Nothing else references
a provider by name: not the panel, not the hooks, not the state
machine.

Two things stay provider-owned rather than living in the generic shell,
because they are genuinely provider-specific policy:

- **Headline selection.** `normalize-usage.ts`'s `pickAccountWideWeekly`
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
3. `use-subscriptions.ts` applies the result the same way whether it
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
  report. `lib/row-presentation.ts` shows `reason ?? "No limits reported
  yet."` for every state without a number.
- **Health and the rate-limit budget are separate fields on purpose.** A
  self-imposed wait must never overwrite a real diagnosis. `use-subscriptions.ts`
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

A file that fails to parse is moved aside to `tracked.json.corrupt`
first, best effort, since `Store::load` starting empty never depends on
the move succeeding; leaving the unread bytes in place would let the
very next save destroy the only copy of whatever the file held. The
backup keeps exactly one slot, so a second corruption overwrites the
first. `save_tracked` is an async command the frontend fires on every
membership, label, or pin change, so two saves can overlap; `Store::save`
holds its mutex across the whole write, disk and memory updating
together, since a writer that renamed its file last but locked the
mutex first would leave disk and memory telling different stories, and
the scheduler polls from memory while the next launch loads from disk.
The save runs on a blocking thread, never across an `await`, so holding
the lock through file I/O blocks only sibling saves.

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
`normalize-usage.ts`'s `windowId(kind, scope)`. `persistence.rs`'s
`TrackedAccount` struct mirrors the wire shape field for field, including
a migration-only `pinned` field present only when a record was loaded
from a file that predates `pinnedWindowIds` and wrote `pinned: true` or
`pinned: false` instead. `use-subscriptions.ts`'s `pendingPinMigrationRef`
reads that field to detect and migrate such a record, and never sends it
back on `save_tracked`, so `skip_serializing_if` sheds it from disk on
the very next save; it appears only on the one load that still has the
old shape to read. Any time an entity's wire shape changes, check
whether this struct still mirrors it, since a mismatched Rust struct
does not error; it silently reshapes the JSON in transit.

## Refresh scheduling and the shared request budget

`scheduler.rs` runs natively rather than as a JavaScript interval,
because macOS suspends timers in a hidden or occluded `WKWebView`: a
measured 8-second `setInterval` produced zero ticks over 150 seconds
while the window stayed hidden and the process itself stayed alive and
idle. An OS-level timer has no notion of "hidden" at all.
`ratelimit.rs`'s sliding window is the account-level budget this
scheduler's cadence assumes stays full; the request budget itself is
reserved per real HTTP request, not per read attempt, since a read that
quietly makes two requests would otherwise spend the shared allowance
twice as fast as the limiter believes. `RequestBudget::reserve` is
passed into the provider rather than taken once by the caller, because
only the provider knows how many requests one read actually costs: a
single reservation per read attempt would undercount the 401
refresh-and-retry path by a factor of two, leaving Quotos hard-throttled
by the provider's own 429 while the limiter still believes it is under
budget.

Each account is due for an automatic read once a minute, anchored to
its last attempt rather than a free-running timer: `mark_attempted`
anchors the next automatic read 60 seconds out from whenever the
attempt actually happened, whether that attempt came from the
scheduler's own periodic pass or from a manual refresh, so a manual
refresh resets the minute for free with nothing extra to wire up. A
rate-limited `retry_after` under one minute is floored at one minute
anyway, since retrying earlier would only spend another slot on a
guaranteed second failure.

Two independent entrants can call the scheduler's due-pass: its own
periodic loop and the frontend's launch-time kick. An account is only
marked attempted after its fetch completes, and a fetch may first run a
bounded credential renewal ahead of the ordinary token expiry, so the
kicked pass can still be mid-fetch when the loop's own tick arrives.
Without a gate, both entrants would see the same account as due and
fetch it twice, spending two slots of the shared budget on one read;
`begin_pass` lets only one entrant run at a time and the loser skips
outright, since whatever is due is already the running pass's job and
anything that becomes due later is at most one tick away.

`spawn_scheduler`'s native timer ticks every 5 seconds, cheap since each
tick is just a due-time comparison per tracked account with no network
call unless something is actually due, and runs for the app's lifetime
regardless of panel visibility. Its first periodic tick is deliberately
delayed by those same 5 seconds rather than firing immediately: every
tracked account is due the moment the app starts, and an immediate tick
could fire and emit before the frontend has mounted and subscribed to
`quota-refresh`, silently losing that first read, since events are not
queued for late subscribers. `kick_scheduler`, called once the frontend
has actually subscribed, covers the real read-at-launch case; the
loop's own first tick is only a safety net for if that kick is somehow
skipped, and both go through the same `is_due` and `mark_attempted`
bookkeeping, so whichever reaches a given account first makes the other
a no-op rather than a second scheduler.

Because that budget is shared per account and not per process,
`single_instance.rs` keeps exactly one Quotos running per machine with an
OS file lock: two live instances would each spend the whole allowance at
double speed until the provider answers 429. Double-clicking the bundle
never produces two instances, since Launch Services activates the
running copy instead; a dev run alongside an installed build, or a
duplicated `.app`, does, since both share one bundle identifier and
therefore one config dir, where the lock file lives. `File::try_lock`
calls `flock`, which the kernel releases whenever the owning process
exits, so there is no stale lock file to detect or repair and no pid to
misread after reuse, unlike a pid file; the standard library has had
file locking since Rust 1.89, so a dedicated single-instance plugin,
built around forwarding argv to a window to focus, would be a dependency
pulled in for one syscall this windowless app has no use for.

Every path that spends a real fetch on an account, a header refresh, a
row's manual refresh, a launch read, or a newly added subscription's
first read, funnels through `use-subscriptions.ts`'s
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

macOS has exactly one coordinate space in which a multi-display layout
has a single, consistent meaning: global points (`CGDisplayBounds` /
`NSScreen.frame`), top-left origin at the main display's top-left. There
is no global pixel space; "physical pixels" are only ever defined
relative to one display's own backing scale factor. The three APIs
`geometry.rs` depends on each hand out a `Physical*` type that is really
"global points times some display's scale factor", and they disagree on
which display's: `TrayIconEvent`'s `rect` uses the menu bar display's
own scale factor, `tao`'s `Monitor::position()`/`size()` use that
monitor's own scale factor, and `set_position(Physical)` /
`WindowEvent::Moved` use the window's current scale factor. On a
single-display machine all three coincide; on displays at different
scale factors they diverge, so `resolve_status_item_point` tries each
display's own scale factor against the status item's rect and keeps the
one whose quotient actually lands inside that display, tie-breaking on
whichever candidate sits closest to its own display's top edge, since a
menu bar always hugs it. On the way out, `apply_docked_position` places
the window with a `LogicalPosition`, which `tao`'s `Position::to_logical`
passes through untouched, so no scale factor is consulted there either;
a `PhysicalPosition` would be reinterpreted through whatever display the
window happens to currently sit on.

`displays_in_points` reads `NSScreen` directly on macOS rather than
going through `tao`'s `Monitor`, since it is the only API that also
reports `visibleFrame`, where the menu bar height comes from; it falls
back to `tao`'s monitor list anywhere else, losing only that height.
AppKit's global space is y-up from the first screen's bottom-left,
while the rest of `geometry.rs` is y-down from its top-left, so the
first screen's own height is the flip constant, its origin being
`(0,0)` by definition. `visibleFrame` also excludes the Dock, but the
Dock never sits at the top, so the difference at the top edge is the
menu bar and nothing else; that height must never be hardcoded, since a
notched built-in display's menu bar is noticeably taller than an
unnotched external display's.

The status item glyph's own center, as an offset from the item's left
edge, is not the whole button's center: `NSStatusItem` centers the
whole composited image inside a button wider than the image by a system
margin that grows once pinned digits widen the image.
`glyph_center_offset_from_item_left_points` derives that margin fresh
from the item's current width and the composited image's own known
width, and falls back to treating the glyph as flush with the item's
left edge if the item's width is unavailable, a plausible worst case
rather than a crash.

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
`docked_layout_in_points`, `panel.tsx`'s matching constants, and
`app.css`'s panel padding. The beak's center stays on the status item
glyph's own center; the layout solves for the beak's tip, not the
panel's top edge.

`docked_layout_in_points` derives the window's x from where the beak
wants to sit rather than positioning the panel first and fitting the
beak to it afterward: the beak's center must stay exactly on the
glyph's center, so moving the beak away from the panel's corner can
only be done by moving the whole panel left, and deriving x from the
beak's wanted position makes both hold by construction at whatever
glyph offset the status item reports. The panel is inset inside its own
window, which is deliberately wider and taller at the top, for the drop
shadow's blur to fade into rather than be clipped by, and for headroom
above the beak; both insets live in `app.css` and are load-bearing here
because the window is what gets positioned while the panel is what is
actually visible. The x candidate is clamped to the display before the
beak offset is recomputed from the clamped x, not the wanted one, so a
screen-edge clamp moves the beak across the panel instead of dragging
it off the glyph; the clamp only keeps the notch on the panel, since
`buildPanelOutlinePath` shrinks whichever top corner the notch
encroaches on rather than letting the notch move off the glyph.

The window's y solves for where the beak's tip should land: the tip
sits a fixed clearance below the menu bar's bottom edge, read live from
`NSScreen.visibleFrame` rather than a constant, since the notched
built-in's menu bar is noticeably taller than an external display's.
Inside a full-screen Space the menu bar is auto-hidden and
`visibleFrame` reports no bar height even while the bar sits revealed
under the cursor, so the fallback reconstructs the bar's bottom from
the status item itself: macOS centers a status item vertically in its
bar, so the bar's bottom is the item's own bottom plus the same inset
that sits above it.
