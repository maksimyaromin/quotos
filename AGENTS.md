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
  upstream — instead `show_panel` in `lib.rs` now computes the position
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
  `glyph_center_offset_from_item_left_physical`, rather than trusting any
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
  shape as `schedule_position_correction`'s unrelated Space-transition fix,
  not a single attempt. Also found and fixed in the same pass: the computed
  glyph *center* was being fed straight into `Panel.jsx`'s `beakLeft` as if
  it were the beak box's own CSS `left` (its *left edge*, off by half the
  12px box), and the 14px inset between the window's own edge and the
  panel's edge (`app.css`'s shadow-blur margin, B9) was never subtracted —
  both silently wrong even when the glyph-center math itself was correct.
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
- **`NSWindowCollectionBehavior` is the lever for "popover shows on the
  user's current Space," but on this machine neither available option is a
  clean win, and the tension is unresolved as of the last round.** The panel
  window is a singleton (created once at launch, only ever hidden/shown
  after, never recreated), and a macOS window's Space membership is normally
  sticky to whichever Space was frontmost when it was first realized —
  `-orderFront:`/`-makeKeyAndOrderFront:` alone don't move it to the user's
  current Space, which is the root cause behind `kCGWindowIsOnscreen` reading
  `false` after an apparently-successful `show()`/`set_focus()` (both
  return `Ok`, a `Focused(true)` event does arrive, no `Focused(false)`
  auto-hide ever follows — all independently confirmed by logging; it is
  not a focus-loss bug). Two behaviors were tried, set once via the raw
  `NSWindow` (`window.ns_window()` cast to `&NSWindow` from the objc2-app-kit
  binding — needs the `NSWindow` feature added to this crate's Cargo.toml,
  see `set_popover_collection_behavior` in `lib.rs`): `.CanJoinAllSpaces`
  keeps the window's own explicit position perfectly stable (no spurious
  `Moved` events) but never flips `kCGWindowIsOnscreen` true, i.e. doesn't
  solve the actual problem. `.MoveToActiveSpace` (paired with `.Transient`,
  the standard flag combo for this kind of ephemeral popover) **does** flip
  `kCGWindowIsOnscreen` true — confirmed live, more than once — but the
  Space transition it triggers is itself asynchronous and was directly
  observed relocating the window a *second* time, ~100-150ms after this
  code's own explicit `set_position` call, to an AppKit-internal default
  position unrelated to the tray icon. `lib.rs`'s
  `schedule_position_correction` reapplies the exact already-computed
  position/beak-offset (not recomputed — recomputing via
  `monitor_from_point` from a window mid-relocation was itself observed
  returning a third, still-wrong answer) a couple of times shortly after,
  which reliably restores the *position* — but doing so was also observed,
  on this same machine, to cost the `kCGWindowIsOnscreen` fix back. Current
  state: `.MoveToActiveSpace | .Transient`, with the position correction, is
  what's shipped, since it's the only option that's ever demonstrated the
  actual visibility fix and the correct-position outcomes independently, even
  though not reliably simultaneously from here. The next person picking this
  up should get the captain to confirm with a real click before assuming
  either outcome — this is likely a testing-environment limitation more than
  a real defect (see the point above), but that has not been proven, only
  argued.
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
