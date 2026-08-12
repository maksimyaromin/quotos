# Quotos v2 — result

Branch: `fm/quotos-fixes-f1`. Fixes and improvements against the captain's
feedback in `data/quotos-fixes-f1/feedback.md` (bugs B1-B6, improvements
I1-I7). Read that file for his exact words; this file states, per item,
what was done, how it was verified, and what wasn't.

Every state below was driven in a plain browser against `src/lib/mockClient.ts`
via the `chrome-devtools` MCP, per the testing contract — the captain tests the
real, packaged app himself. Where a bug lived on the native side (tray, window
positioning, WebView storage) and couldn't be seen in a browser, the fix is
backed by reading the actual dependency source rather than guessing — noted
per item below.

## Bugs

### B1 — menu bar icon — **Done**

The tray icon PNGs (`src-tauri/icons/tray/tray-icon.png` and `@2x.png`) are
now rasterized from `design/system/assets/menubar-glyph.svg` (the quota-ring
glyph), not a placeholder square. Rendered as black-on-transparent with the
SVG's own two opacity levels preserved (the faint full ring at 0.28, the
bold arc at full opacity) so macOS's template-image tinting recolors it
correctly in both light and dark menu bars — `icon_as_template(true)` was
already set in `lib.rs` and is unchanged. **Not verified**: I cannot see the
real menu bar from this environment; verified instead by inspecting the PNG's
alpha channel (`sips -g hasAlpha` → yes) and compositing it against a dark
background to confirm the silhouette matches the intended ring glyph.

### B2/B3 — unpinning doesn't clear the tray, and the leftover number freezes — **Done, root cause found**

Read the `tray-icon` crate's own source (v0.24.2,
`platform_impl/macos/mod.rs`): its macOS `set_title` only calls
`NSStatusItem`'s `setTitle` when given `Some(..)` — passing `None` to
"clear" it is a **silent no-op**, so whatever title was last set stays on
screen forever. `set_tray_title` (`lib.rs`) previously passed `None` when
the computed title was empty; it now always passes `Some(title.as_str())`,
including `Some("")`. Combined with `useSubscriptions.ts`'s tray-title
effect — already a pure function of current `pinned` state on every render
— unpinning now genuinely clears the tray, and a pinned value stays live
because it's recomputed from live state on every read, not held separately.
**Not verified visually**: no real tray to look at from here; the fix is a
direct, confirmed read of the crate's actual behavior, not a guess.

### B4 — discovery invents a subscription from a plain folder — **Done, unit-tested**

`discover_accounts()` (`providers/claude.rs`) now only returns a config
directory when a Keychain entry actually exists for its derived service
name — `credential_exists_in_keychain()` runs `security find-generic-password`
(existence check only, no secret read) before a directory is allowed to
become an `AccountDescriptor`. The code comment states the captain's own
principle verbatim: *a folder in a certain place is not a subscription*.
Split into a pure `discover_accounts_in(home, has_credential)` so the
qualification logic is unit-tested without touching the real Keychain —
`cargo test`, 4 tests, covering: two credentialed dirs → two subscriptions
and nothing else; a plain folder with no credential → excluded; the default
`~/.claude` alone; an empty home. This is exactly the captain's own ground
truth (two Keychain services on his machine → two subscriptions).

### B5 — "Waiting on limits" badge, and the refresh algorithm — **Done, all three parts**

1. **The badge is gone.** `SubscriptionState` no longer has a `"waiting"`
   value at all — a self-imposed rate-limit wait is tracked as a separate
   fact, `Subscription.rateLimitedUntil`, that never touches the health
   `state`. In its place: the row's normal "Last read …" text is left alone
   (the quiet fact), the row's action (Retry) is visually inert with its
   label showing exactly when it becomes available (`Retry at 4:05 PM`), and
   the header's global refresh control disables itself with a tooltip
   ("Waiting for the rate budget — available 4:05 PM") **only when every
   tracked subscription is currently blocked** — a single blocked account no
   longer prevents refreshing the others. Verified in-browser for both the
   "has data, now blocked" and "never read, blocked from the first attempt"
   cases.
2. **The algorithm.** Opening/closing the panel no longer triggers any
   fetch — the old bug was a refresh tied to a `panel-visibility` event;
   that trigger is gone entirely. The only things that ever call
   `fetch_snapshot` now: the one-time initial read at launch, an explicit
   manual refresh (header button or a row's Retry), and a single background
   interval that runs for the app's lifetime, independent of panel
   visibility, every 5 minutes (matching the report's "poll every 5 minutes"
   guidance — 1 of the 5 available slots per window, leaving headroom for
   manual refreshes). Unit-tested in
   `useSubscriptions.test.ts`: five simulated "open/close cycles" worth of
   elapsed time cause zero extra fetches; the scheduled interval and a
   manual `refreshAll()` each cause exactly one.
3. **Is the limit real?** Yes — settled, not re-tested live per the
   instruction. `data/quotos-source-s1/report.md`, section "Rate limits —
   measured": 12 requests 1 second apart, request 6 onward returns `HTTP 429`
   with `retry-after=300`, measured twice (with and without the
   `claude-code` User-Agent, identical result). I did not re-run this — it
   would spend the captain's live, shared quota — and took the figure as
   authoritative.

Standing requirement ("must be smooth, no basic inconvenience"): a
health-diagnosed row's action is hidden from spam only in the sense that it's
disabled while blocked, never removed — and it re-enables itself the moment
`rateLimitedUntil` passes (checked on a 30-second UI tick, so it doesn't
need a data refresh to notice).

**A real regression found and fixed while building this.** The optimistic
"reading"/"connecting" patch `refreshOne` issues *before* a fetch starts was
itself capable of reproducing B6: a Broken account's retry that came back
`rate_limited` was left showing "Connecting…" with no reason, because the
pre-fetch patch had already overwritten the Broken state before the
rate-limited branch ran (which only patched `rateLimitedUntil`, not
`state`). Caught by manually retrying a broken demo account in the browser
harness, fixed by capturing `state`/`reason` before the optimistic patch and
having the rate-limited branch explicitly restore them. A regression test
now guards this exact sequence (`useSubscriptions.test.ts`, "B6" describe
block).

### B6 — Blocked decaying into "Waiting on limits" — **Done, unit-tested**

`Subscription.state` (health) and `Subscription.rateLimitedUntil` (polling
availability) are now separate fields, and a rate-limited fetch response
never patches `state` or `reason` — it restores whatever health state
existed before the attempt (see the regression above). Verified in-browser:
added a demo account that returns `unauthorized` on the first read (shows
Broken + "Sign-in expired…") then `rate_limited` on every read after
(simulating the shared-budget exhaustion from repeated retries); clicking
Retry leaves the Broken badge and reason exactly in place, with the
rate-limit quiet fact appended alongside it, not replacing it.

## Improvements

### I1 — re-login from inside Quotos — **Not done, cut deliberately**

This is explicit P7 ("if capacity remains") and I made a call not to attempt
it. The captain's own suggestions — open the agent, or launch `claudet` with
the right option — both plausibly mean shelling out to an interactive
terminal/login flow, which is the kind of thing most likely to trigger a
macOS Automation/Accessibility permission prompt (e.g. controlling
Terminal.app). The testing contract explicitly forbids "anything that would
raise a macOS permission prompt" while he's away, and I judged the risk of
building this blind — with no way to verify it doesn't prompt — higher than
the value of a P7 item. Stated loudly here rather than attempted silently.

### I2 — invert to consumed, everywhere — **Done**

`used` replaced `remaining` as the field name throughout the entity types,
the normalizer (`normalizeUsage.ts` — the raw provider `percent` is already
percent-*used*, so this also removed a double inversion that used to exist),
the headline, the expanded window rows, and the tray title text. The
capacity bar's fill and color now key directly off `used` (full + red = most
consumed = worst), so a full bar means the same thing everywhere it appears
— there is no longer a place where "full" means "empty" and another where it
means "nothing used." Verified visually at 2%, 78%, and 94% used.

### I3 — exact reset time — **Done**

New `formatExactReset()` (`lib/time.ts`): "Resets today at 4:05 PM" /
"Resets tomorrow at 10:00 AM" / "Resets Wed at 10:00 AM" (within the week) /
"Resets Aug 24 at 9:00 AM" (further out) — never a bare relative offset.
Applied to every window's reset label, not just the headline. Unit-tested
(`time.test.ts`) for all four bands plus the missing/unparseable cases.
Locale-respecting (uses `Intl.DateTimeFormat` with no forced locale), which
is why the mock screenshots taken during development show Cyrillic weekday
abbreviations — that's the correct behavior for a system set to Russian, not
a bug.

### I4 — expand jump — **Done**

Two changes: `Panel.jsx`'s scrollable body now has `scrollbarGutter: "stable"`
so a scrollbar's appearance/disappearance never changes the available width
(the actual cause of the reflow); and a subscription's expanded detail is now
always mounted (not conditionally rendered) inside a `display: grid;
grid-template-rows: 0fr → 1fr` wrapper that transitions on expand/collapse,
so the growth itself is an animated, deliberate motion rather than an
instant pop. Verified visually — expand/collapse now grows the card
smoothly with no visible jolt.

### I5 — halo around the panel — **Done, reasoned from measurements, not visually verified**

Root cause: `--shadow-popover`'s blur radius (44px) was more than triple the
transparent window's actual margin around the panel (14px per side —
`--panel-width: 332px` inside a 360px window, centered). A blur that wide
gets hard-clipped by the window's edge instead of fading out, which is
exactly what would read as a dark band hugging the panel rather than a soft
shadow. Reduced the shadow (`design/system/tokens/elevation.css`) to a blur
radius that comfortably fits inside the existing margin, in both dark and
light appearance variants. **Not verified visually**: this is a native
transparent-window artifact against the real desktop, which a browser tab
can't reproduce — the fix follows directly from the CSS blur-radius vs.
window-margin arithmetic, not a screenshot comparison.

### I6 — subscription management — **Done**

Nothing is tracked by default (`useSubscriptions.ts` seeds from
`lib/persistence.ts`, which starts empty). A new "Add subscription" flow
(`components/AddSubscriptionSheet.tsx`) re-scans discovery on open and lists
only *untracked* candidates; adding one performs the verifying read
immediately per the brief's add-flow step 4. Each row gained inline rename
(pencil → editable text, Enter/blur commits, Escape cancels, empty reverts
to the provider-derived name) and delete (trash → confirm-to-remove, one
click arms it, a second confirms, moving the mouse away disarms it). The
flow is provider-shaped, not Claude-specific — it lists whatever
`tauriClient.listAccounts()` returns, keyed off `AccountDescriptor`, so a
second provider slots in without touching this flow. Persisted to
`localStorage` (see the divergence note under I7/packaging below).
Verified end-to-end in the browser: empty state → add two → rename one →
reload the page → both survive with the rename intact → delete one →
reload → one remains.

**A real bug found and fixed while building this**: the first version of
the persistence-writing effect ran on the very first render (before the
async-loaded list was applied) and unconditionally saved the still-empty
`[]` state, clobbering anything already in storage — every reload silently
wiped the tracked list. Fixed by seeding React state directly from storage
via a lazy `useState` initializer instead of a `useEffect`, which removes
the empty-first-render window entirely. Caught by reloading the browser tab
mid-testing and noticing the list didn't survive; there is no automated
regression test for this specific ordering bug (the fix is structural —
state is no longer loaded in an effect at all — so the failure mode can't
recur the same way), but the persisted round-trip is exercised manually
above.

### I7 — detach + anchor — **Done**

**Anchor.** `lib.rs` used `Position::TrayBottomRight`, which places the
window's own *left* edge at the tray icon's right edge — with a 332px-wide
panel and a ~20px icon, that puts the panel's beak (horizontally centered in
the window, per `Panel.jsx`) roughly 150px to the right of the icon it's
supposed to point at, landing under whatever else sits there. Confirmed by
reading `tauri-plugin-positioner` v2.3.3's own position-calculation source
(`ext.rs`) rather than guessing. Switched to `Position::TrayBottomCenter`,
which centers the window under the icon — matching the beak's own alignment
— and to `move_window_constrained` so it can't be pushed off-screen near a
display edge.

**Detach.** New `set_detached` Tauri command + `AppState.detached` flag.
Detaching: gives the window real title-bar decorations (so it's draggable
and unmistakably a window, not a popover), removes it from
`skip_taskbar` (so it shows in Cmd+Tab), stays always-on-top, and — the
actual fix — the window's focus-loss handler now checks the detached flag
before hiding, so a detached window survives losing focus, which is the
entire point (park it next to the agent and watch both). A left-click on the
tray while detached brings it forward instead of toggling it closed, so a
casual click can't accidentally lose a deliberately parked window. In the
UI, a header icon toggles detach/attach — obvious and one click, per the
brief's "make getting into that mode obvious." **Not verified beyond code
review + the click not crashing in the mock harness**: real window
decorations/always-on-top/focus behavior can only be seen in the native app,
which was off-limits beyond the one final smoke launch.

### A developer state dump (P7) — **Done**

A dev-only header icon (`import.meta.env.DEV`-gated, so it never appears in
a production build) dumps the current subscriptions array, the tracked
persisted list's effect on it, and a new read-only Rust command
(`debug_rate_limit_snapshot`, exposing `RateLimiter::snapshot()`) showing
each account's current request count against the 5/300s budget and time
until it frees up — to the console and the clipboard as JSON.

## Packaging: v2 beside v1

`~/Downloads/Quotos.app` (v1) was not touched — verified by comparing its
mtime and a recursive file checksum before and after this work; both are
byte-identical to the start of the session.

`~/Downloads/Quotos 2.app` is the v2 deliverable, built via
`npx tauri build --config src-tauri/tauri.v2.conf.json` from `quotos-app/`.
That config (new file, `src-tauri/tauri.v2.conf.json`) overrides only
`productName` ("Quotos 2"), `identifier` (`com.quotos.desktop.v2`, distinct
from v1's `com.quotos.desktop`), and points `beforeBuildCommand` at
`npm run build:v2`, which sets `VITE_APP_LABEL="Quotos 2"` for the frontend
build — the one place the product name changes in source is a fallback
constant in `App.tsx` (`APP_LABEL`), read at build time; nothing else in the
repo was renamed, per the instruction to keep the repo's own product name
clean.

**State-path divergence, verified empirically, not just assumed**: I6's
persistence uses `localStorage`, which WKWebView scopes by the host app's
own `CFBundleIdentifier` (confirmed by reading `wry` v0.55.1's source —
`wkwebview/mod.rs` — the webview uses `WKWebsiteDataStore::defaultDataStore`,
which macOS keys per bundle identifier on disk). After the required smoke
launch of the packaged v2 app, `~/Library/WebKit/` shows both
`com.quotos.desktop` (from the captain's prior use of v1) and
`com.quotos.desktop.v2` as separate directories — the two builds' stored
subscription lists genuinely cannot collide.

**The one permitted native launch**: `open "~/Downloads/Quotos 2.app"`,
waited ~4s, confirmed the process was alive via `ps`
(`.../Quotos 2.app/Contents/MacOS/quotos-app`, low CPU, not crash-looping),
then `kill`ed it and confirmed it exited. Nothing was clicked, no screen was
captured. `Info.plist` was also inspected (a file read, not app interaction)
to confirm `CFBundleIdentifier` = `com.quotos.desktop.v2` and
`CFBundleDisplayName` = `Quotos 2`.

## Tests left behind

- `src-tauri/src/providers/claude.rs` — 4 tests: B4's discovery-qualification
  logic (`cargo test`, all passing).
- `quotos-app/src/hooks/useSubscriptions.test.ts` — 4 tests: B5's refresh
  policy (open/close spends nothing; scheduled and manual refreshes do) and
  B6's health/rate-limit precedence regression.
- `quotos-app/src/providers/claude/normalizeUsage.test.ts` — 16 tests,
  updated for I2 (asserts `used`, not `remaining`) plus the original
  coverage (arbitrary window counts, unknown kinds, missing percentages,
  scoped windows, fixed-window fallback, malformed entries, clamping,
  headline selection).
- `quotos-app/src/lib/time.test.ts` — 9 tests: I3's exact-reset formatting
  across today/tomorrow/this-week/further-out, plus the existing relative-past
  formatter.
- 29 vitest tests total, all passing; `npx tsc --noEmit` clean; `cargo test`
  and `cargo check` clean; `npm run build` and the v2 release build both
  succeed.

## What I could not verify (real-app testing was off limits)

Everything native — the actual menu bar icon rendering (B1), tray title
clearing/updating (B2/B3), the detach window's real decorations/always-on-top/
focus behavior (I7), and the panel's shadow against a real desktop (I5) —
was fixed by reading the relevant dependency source or doing exact CSS/window
measurements, not by looking at it, because the testing contract reserved
real-app interaction for the captain. The mock-driven browser harness
(`npm run dev`) covers every reachable *application logic* state; it cannot
show native chrome. These are the first things worth checking when he tests
the packaged build.

Nothing was cut silently. I1 (re-login from inside Quotos) is the one item
not attempted, for the permission-prompt-risk reason stated above under I1.
