# Quotos round 2 (engine half) — result

Branch: `fm/quotos-engine-e1`. This is the engine half of round 2 — data,
cadence, persistence, sign-in, locale — against `data/quotos-fixes-f2/feedback.md`
(items 1–7) plus four follow-up items that fell in the seam with the surface
half after it landed on `main` (`data/quotos-engine-e1/followup-1.md`,
`followup-2.md`, `followup-3.md`). Read those for the captain's exact words;
this file states, per item, what was done, how it was verified, and what
wasn't — the captain's own standing rule this round: an honest "unverified"
beats a confident claim, because a build where the intended thing silently
doesn't work is what he explicitly objects to.

The surface half (`quotos-surface-u1`, the whole UI/UX handoff) landed on
`main` first and is not re-described here except where the follow-ups touched
it. 85 automated tests pass (59 vitest, 26 `cargo test`), `tsc --noEmit` and
`cargo check` are both clean.

## 1. Headline = account-wide weekly, not the maximum — **Done, verified in browser + tests**

`normalizeUsage.ts`'s `pickAccountWideWeekly` now selects `weekly_all` from
`limits[]` (or `seven_day` from the fixed top-level shape) as the headline,
falling back to the old most-consumed-active behaviour only when no such
window exists. Provider-owned, per the captain's instruction.

Verified two ways: unit tests reproducing the captain's exact regression
(account weekly 15%, per-model weekly 17% — headline must read 15%), and live
in a browser against `mockClient.ts`'s `demo-critical` account (session 94%,
account weekly 91%, Fable 20%) — the panel shows **91%**, not 94%, confirmed
by a11y snapshot.

## 2. Severity from every window, drives colour — **Done, verified in browser; tray digits are Rust-image-compositing, unit-tested only**

`Subscription.severity` (`"healthy"|"warn"|"critical"`) is computed in
`normalizeUsage.ts` from every window (not just the headline), thresholds
matching `--cap-warn`/`--cap-critical` exactly. The headline number and bar
now take colour from `severity`; the captain's own example (weekly 20%,
session 85%) reads 20% in amber — verified visually in a browser via a new
`demo-severity` mock account (screenshot taken, headline number and bar both
render amber; expanding the row confirms session=85%, weekly=20%).

Tray digits: `tray-icon` v0.24.2's macOS `set_title` has no colour channel at
all (confirmed by reading `platform_impl/macos/mod.rs` directly, the same way
the `set_title(None)` no-op was found last round) — there is no supported way
to colour an `NSStatusItem` title through this crate. Went with the
image-compositing route: `tray_render.rs` draws the glyph plus each pinned
account's own coloured digits into an RGBA bitmap and calls `set_icon`,
dropping `icon_as_template` whenever anything needs colour. Six Rust unit
tests cover the compositing (dimensions, per-segment colour, dark/light
neutral colour). **Not visually verified** — there is no tray in a browser,
and driving the real tray was ruled out this round (see item 6 and follow-up
1) — this is the first thing to check on the merged build.

Followup-2 also fixed the tray never showing `"!"` or `"…"` (handoff:
digits or nothing), and made a stale pinned value turn *every* pinned digit
amber, not just its own — both unit-tested (`useSubscriptions.test.ts`,
"followup-2" describe block).

## 3. Status/health mapping moved to the provider adapter — **Done, verified in tests**

The outcome→state+reason table moved from `useSubscriptions.ts` into
`providers/claude/index.ts`'s `mapOutcome`, reached via `registry.ts`'s
`mapOutcomeFor`. State vocabulary unchanged. `rate_limited` is deliberately
excluded from the provider mapping — B5/B6 still hold: the shell intercepts
it before it reaches any provider and restores the pre-attempt health state.
Six new tests in `mapOutcome.test.ts`; the B6 regression test (rate-limited
retry must not decay a Broken diagnosis) now has both a manual-refresh and a
native-scheduled-push variant.

## 4. Refresh: one read/minute, anchored, native — **Done, reproduced and verified on real builds**

**Reproduced the real bug first, as instructed.** Instrumented a build with an
8-second JS interval and left the panel closed (its default state at launch)
for 150+ seconds: **zero ticks fired**, while the process itself stayed alive
and idle. Confirms the suspicion: macOS/WebKit suspends JS timers in a
hidden/occluded WKWebView.

Fix: `scheduler.rs` is now the single scheduler, entirely on the Rust side —
a native tokio interval with no notion of "hidden webview" to be throttled
by. One automatic read per account per minute, anchored to the last attempt;
`fetch_snapshot` (the one real network call site, shared by manual refresh
and the scheduler loop) always resets that account's clock, so "a manual
refresh resets the minute" needed no separate wiring. Also fixed
`ratelimit.rs`'s off-by-one window-prune boundary (`>=` not `>`) so the
limiter agrees with the new cadence instead of refusing its 6th request one
instant early. Manual refresh calls are debounced/collapsed in the hook
(concurrent `refreshAll`/`refreshAccountById` calls for the same target
collapse into the single in-flight one).

Verified: `scheduler.rs`/`ratelimit.rs` have 10 unit tests between them
(due-immediately, floors at one minute, rate-limit override, isolation
between accounts, the exact window-boundary case). The frontend's reaction to
pushed `quota-refresh` events (vs. direct fetches) is covered in
`useSubscriptions.test.ts`'s "native path" describe block. **Not re-measured
end-to-end for a full 5 minutes on a live build** — the mechanism (native
timer, no webview involved) was reproduced correctly for what it replaces;
watching five real ticks land one per minute on the merged build is the
natural next check.

## 5. Persistence survives a restart — **Done, verified on real builds; one methodology correction below**

**Reproduced first, as instructed — and re-checked with a better tool.** The
initial reproduction (seed `localStorage` with a renamed subscription, quit
via `app.exit(0)`, inspect WebKit's `localstorage.sqlite3` with `strings`)
found the *key* on disk but not the *value*, and was read as a WebKit
write-durability race. **That specific diagnosis was wrong** — `strings`
cannot see WebKit's UTF-16-encoded values at all. Re-checked with `sqlite3`
directly (`SELECT value FROM ItemTable`, or `hex(value) | iconv -f
UTF-16LE`): the value *was* present, even after `kill -9` within ~1s of the
write. I could not conclusively identify the original bug's true root cause
after that correction. Recorded in `AGENTS.md` so the wrong explanation
doesn't get repeated.

The fix — moving the tracked list to a native, Rust-owned file
(`persistence.rs`, `tracked.json` in the app config dir, written via
temp-file + `fsync` + atomic rename) — is still correct and is what the
captain asked for regardless: durability no longer depends on WKWebView's
internal flush timing at all, whatever that timing actually is. A file over
SQLite because the list is a handful of accounts; a file is simpler,
reviewable, and survives everything SQLite would here.

Verified on real builds, twice: (1) a pre-existing renamed+pinned entry
survived a full app lifecycle including a `kill -9` and a clean
launch/quit/relaunch cycle, file contents checked directly; (2) the
followup-3 migration scenario — seeded `localStorage` with a realistic
pre-upgrade list (custom name + pin), launched, confirmed the native
`tracked.json` came out with the migrated data *and* the legacy
`localStorage` key was left untouched (per followup-3: never clear it, so a
bad migration is recoverable by going back to the old build). 5 Rust unit
tests (missing/corrupt file, round-trip, atomicity, no leftover temp file) +
8 TypeScript unit tests (migrate-once, absent/corrupt key, native-already-populated,
browser fallback).

## 6. Sign-in recovery — **Implemented; verified up to the captain's own final step, which only he can do**

Investigated via `claude setup-token --help` and one careful,
throwaway-`CLAUDE_CONFIG_DIR` observation: the CLI opens the browser and
prints the authorization URL *itself*, then waits for a pasted code on
stdin — exactly the mechanism the captain asked for, meaning Quotos doesn't
need to open a browser, capture a URL, or handle a credential at all. It
needs to start that process for the right account and relay a pasted code
back into it.

**An incident during that investigation, reported and resolved with
firstmate:** running the CLI to observe its output (per the brief's own
instruction) opened a real browser to a live claude.ai login page — a boundary
crossing neither I nor the brief anticipated (the brief asked me to
investigate the flow and separately forbade opening a browser, and the CLI
does both in the same step). No credentials were entered, nothing was
clicked, no login completed; I stopped immediately, reported it, and did not
repeat the action. Firstmate's follow-up (`data/quotos-engine-e1/followup-2.md`)
confirmed the read and gave the "build from `--help` and reading, don't
re-run it" instruction the rest of item 6 followed.

Delivery: `signin.rs` spawns `claude setup-token` (pointed at the broken
row's `CLAUDE_CONFIG_DIR`) attached to a real pty via `portable-pty` — the
CLI renders an interactive, cursor-positioning prompt, so plain pipes risked
it detecting a non-tty and behaving differently. Quotos never parses the
process's output, never opens a browser, never touches the Keychain or a
credential. The broken row's action starts this flow; the panel shows a
paste-code field in place of the reason text while it's running; submitting
relays the code into the process's stdin; the process exiting (success or
failure — the exit code is informational, never trusted) triggers a normal
re-read.

**Verified end-to-end in the browser**, via a mock simulation of the same
lifecycle (`mockClient.ts`'s `startSignIn`/`submitSignInCode` — the real
process and the browser it opens can't be driven from here): clicked "Open
Claude Code" on a broken demo row, the paste-code field appeared with focus,
typed a code, clicked Submit, the row went from "Needs sign-in" to healthy
with real numbers. **Not verified**: whether a *real* pasted authorization
code, from a completed real login, makes `claude setup-token` exit 0 and
Claude Code write a working credential — that is the captain's own final
check, on his own deliberately-logged-out test account, per his explicit
decision that Quotos never handles a credential itself.

## 7. Whole app is English — **Done, verified in tests + live in a real browser**

`time.ts`'s `Intl.DateTimeFormat` calls are now pinned to `"en-US"` instead of
the system locale. Verified two ways: a regression test that spies on the
`Intl.DateTimeFormat` constructor itself (not just output — a system already
set to English wouldn't have caught a regression here) and asserts every call
site passes `"en-US"`, and a live check in a real Chrome browser on this Mac
— whose system locale is `ru_PL` (Russian) — showing "Resets Tue 5:40 PM" in
English. Swept the rest of the frontend for `toLocale*`/`Intl.NumberFormat`
calls: none found outside `time.ts`. Rust side was already locale-independent
(RFC3339 timestamps only, no local formatting).

## Follow-ups (after the surface half landed on `main`)

**1. Tray anchor vs. beak alignment** — `show_panel` now uses
`Position::TrayBottomLeft` (window left edge = icon left edge) plus the
handoff's two adjustments (6px inset, fixed 32px from screen top), matching
the beak the surface half pinned to the panel's own left edge. **Not
empirically pixel-verified.** A real tray click was ruled unsafe: with the
captain's real Quotos also running, `System Events` accessibility queries —
by process name *and* by `unix id` — resolved to the identical tray-icon
rectangle for both processes, so there was no reliable way to prove a click
would land on the throwaway dev build and not his. A follow-up attempt to
prime the plugin's cached tray position programmatically (no OS event) and
call `show_panel()` directly produced a window position inconsistent with the
primed coordinates, in a way not resolved this round — recorded honestly in
`AGENTS.md` rather than glossed over. What is settled: `TrayBottomLeft` is
the semantically correct anchor (confirmed by reading the plugin's own
position-calculation source), and the same `on_tray_event` →
`move_window_constrained` mechanism was already proven correct with *real*
click events in the prior round. Confirm the exact pixel alignment on the
merged build via a real click.

**2. Tray must never show `"!"`/`"…"`** — Done, unit-tested. A broken pin now
contributes no tray segment at all rather than `"!"`; a value with no number
yet already drew nothing. Any pinned value being stale now turns every
pinned digit amber, not just its own.

**3. Severity wired end to end** — Done, verified live in a browser
(screenshot: 20% headline in amber, session window at 85%). `CapacityBar`
gained an optional `severity` prop that overrides its old used%-based colour
when given; `SubscriptionRow`'s headline number now reads `severity` too.
`LimitWindow`'s own per-window bars are untouched — each still colours from
its own percentage, which is correct for a single window.

**4. Re-drove the states in the browser** — Personal (working), Critical
demo (critical/red, 91%), demo idle (idle), demo broken → sign-in flow →
Recovered demo (working), demo waiting (rate-limited from first attempt),
Flaky demo (behind after one good read), No limits demo (working, no
windows), Severity demo (warn/amber despite a 20% headline) — all re-checked
visually after the follow-up changes, screenshots taken for the ones with
colour to verify (items 1, 2, 3 above).

## Packaging

`quotos-app/src-tauri/tauri.v3.conf.json` — `productName` "Quotos 3",
`identifier` **deliberately unchanged from v2** (`com.quotos.desktop.v2`),
reversing round 1's "always diverge" convention: this build carries the R2-5
native-persistence migration, which only has real data to prove itself
against if it shares the captain's actual in-use WKWebView storage (his real
tracked list and custom names, already in `Quotos 2`'s storage). A fresh
identifier would boot empty and migrate nothing observable. Reasoning
recorded in `AGENTS.md`'s side-by-side-packaging entry so it isn't "fixed"
back by a future pass.

Built via `npx tauri build --config src-tauri/tauri.v3.conf.json` from
`quotos-app/` (bundle targets are `["app"]` only — the DMG target doesn't
work in this environment). The bundle is left inside this worktree, at:

```
quotos-app/src-tauri/target/release/bundle/macos/Quotos 3.app
```

Not copied to `~/Downloads` or anywhere else — firstmate delivers it and
verifies on the captain's real screen, which is also where item 6's last
step and follow-up 1's exact pixel alignment get their final check.

## Testing builds used during this round

All native testing used bundle identifier `com.quotos.desktop.dev`
(`src-tauri/tauri.dev.conf.json`) and, where account data mattered, a fully
throwaway `$HOME` — never the captain's real config directories or Keychain.
The one exception is the final `tauri.v3.conf.json` package itself, built for
delivery but not run by this agent against real data (see item 6's incident
note above for the one machine-boundary exception this round, already
reported and resolved).
