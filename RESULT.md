# Quotos — night build result

Branch: `fm/quotos-build-b1`. Tauri v2 + React + TypeScript + Vite app lives in
`quotos-app/`.

Read this file before judging the night. It's organized as: what works, what
doesn't, how to run it, how it was tested, and every shortcut taken.

## What works

**1. Skeleton.** A Tauri v2 app with an accessory-mode tray icon (no Dock
icon — `LSUIElement` + `ActivationPolicy::Accessory`). Left-click toggles a
borderless, transparent popover positioned at the tray icon via
`tauri-plugin-positioner`; right-click shows a "Quit Quotos" menu item. The
window hides itself on focus loss and on Escape. It builds and launches
cleanly, both in dev (`npm run tauri dev`) and as a release `.app` bundle
(`npm run tauri build`) — verified by launching the built binary directly and
watching it stay up with an empty stderr/stdout log.

**2 & 3. Real numbers, both accounts.** The Rust backend
(`quotos-app/src-tauri/src/providers/claude.rs`) implements exactly the
mechanism from `data/quotos-source-s1/report.md`:
`GET /api/oauth/usage` + `/api/oauth/profile`, bearer token read from the
macOS Keychain via `security find-generic-password`, per-config-dir service
name (`Claude Code-credentials` for `~/.claude`, `Claude
Code-credentials-<sha256(configDir)[:8]>` otherwise, NFC-normalised), 401 →
`CLAUDE_CONFIG_DIR=<dir> claude mcp list` → retry once, and a 5-per-300s
sliding-window rate limiter shared across both accounts' usage calls
(`ratelimit.rs`). Accounts are discovered by scanning `$HOME` for `.claude`
and `.claude-*` directories — both the personal and team accounts on this
machine are found and read live (see **Proof of real data** below). The
provider adapter seam is real: `src/providers/registry.ts` on the frontend
and `src-tauri/src/providers/mod.rs` on the backend are where a second
provider would plug in; nothing in `App.tsx`, the hooks, or the design-system
components branches on "claude" — they only see generic `Subscription`
entities (`src/types/entities.ts`).

**4. States.** All eight states from the brief
(`idle`/`connecting`/`working`/`reading`/`behind`/`repairing`/`broken`/`waiting`)
are modeled in `SubscriptionState` and driven per-subscription in
`useSubscriptions.ts` — one subscription's fetch failure only ever patches
that subscription's row, never touches the others. `lastReadAt` is carried
and rendered ("Last read 2 min ago") regardless of current state, per the
brief's "age is always visible" rule. See **Honesty about `repairing`** below
for the one state that isn't cleanly distinguishable in practice.

**5. Refresh.** `useSubscriptions.ts` is the single owner of cadence: one
effect listens for a `panel-visibility` event emitted from Rust (on tray
click / window blur) and starts/stops a 2-minute `setInterval` accordingly —
nothing refreshes while the panel is closed. The header refresh `IconButton`
calls the same `refreshAll()` the interval uses. There are no other timers
anywhere in the codebase.

**6. Design system.** `design/system/tokens`, `components`, and fonts are
copied verbatim into `src/design-system/` and consumed as-is —
`Panel`, `SubscriptionRow`, `LimitWindow`, `MenuBarTile`, `Badge`, `Button`,
`IconButton` are all the delivered components, unmodified. `main.tsx` links
`design-system/styles.css` and flips `data-appearance="light"` from
`prefers-color-scheme`. Nothing in the app reinvents a design-system
component; `src/components/icons.tsx` only adds the two small chrome icons
(refresh, plus) the system didn't ship as inline SVG, in the same stroke
style as the ones already inline in `SubscriptionRow.jsx`.

**7. Stretch — pinning beside the tray icon.** Implemented. `SubscriptionRow`'s
existing pin toggle is wired to `useSubscriptions.togglePin`; an effect syncs
pinned subscriptions' headline figures to the tray via a new
`set_tray_title` Tauri command, which calls `TrayIcon::set_title` (macOS
`NSStatusItem` title). A broken pin shows `!` instead of a stale number.
**Not implemented**: this only exercised in the mock/browser harness and via
code review — I could not click the real tray icon in this headless
environment to see it land on screen (see **What I could not verify**).

## What does not work / was not attempted

- **Stretch — the add-subscription flow.** Not built. The footer has an
  "Add subscription" button per the brief's panel requirements, but it's
  disabled with a "coming soon" title. Accounts are currently discovered
  automatically (scan `$HOME` for `.claude*` dirs) rather than added through
  a UI flow. This was explicitly item 8, lowest priority, and time ran out
  before it.
- **Stretch — background refresh & notifications.** Not built, also item 8.
- **`repairing` is not a distinct observable state.** The brief describes it
  as the 401→refresh→retry cycle. In this implementation that whole cycle
  happens inside one `fetch_snapshot` Rust call — the frontend only sees
  "reading" (or "connecting" on first read) before and "working"/"behind"
  after. Splitting it out would mean Rust emitting a transient event mid-command;
  I chose not to build that tonight since the two-call retry is genuinely a
  few hundred milliseconds and the "reading" state already communicates "in
  progress, don't worry." `StatusDot`/`Badge` support the `repairing` value
  if a future change wants to wire it up.
- **DMG bundling fails in this environment.** `npm run tauri build` was
  producing both `Quotos.app` and a `.dmg`; the `.app` bundle always
  succeeded but `bundle_dmg.sh` failed (`hdiutil`/Finder disk-image
  arbitration doesn't work headlessly here). I restricted
  `tauri.conf.json`'s bundle targets to `["app"]` so the build is clean and
  green. **The build produced is the `.app` bundle, not a signed installer
  DMG.** It's ad-hoc code-signed (`codesign -dv` shows
  `flags=0x20002(adhoc,linker-signed)`), which is enough to run locally but
  not to distribute off this machine without Gatekeeper complaints.

## What I could not verify

**Chrome-driven UI testing** (explicitly asked for in the brief) could not
be completed. Every `mcp__chrome-devtools__*` call failed all session long
with:

```
The browser is already running for /Users/supolka/.cache/chrome-devtools-mcp/chrome-profile.
Use --isolated to run multiple browser instances.
```

`ps aux` shows three separate `chrome-devtools-mcp` server processes on this
machine (different ttys, different launch times), all pointed at the same
shared profile directory, with one already holding the single Chrome
instance the profile allows. This is contention between concurrent sessions
on the box, not something in this worktree — I retried across the whole
build (7 attempts, spread over roughly two hours) rather than fighting it
harder, since the instructions are explicit that Chrome is shared and I
should not kill someone else's browser.

**What I did instead**, so the UI states are still exercised and reviewable:

- `src/lib/mockClient.ts` — a browser-only fallback (swapped in via
  `"__TAURI_INTERNALS__" in window` in `src/lib/tauriClient.ts`) that
  simulates 7 of the 8 states across 7 mock accounts: healthy (teal),
  amber-threshold, red-threshold, `idle`, `broken`, `waiting`, and a flaky
  one that starts `working` then flips to `behind` on the next refresh. This
  is dev/QA-only scaffolding — it never runs inside the real Tauri build,
  gated by the same `__TAURI_INTERNALS__` check.
- To view it: `cd quotos-app && npm run dev`, then open
  `http://localhost:1420/` in **any** browser (the captain's own Chrome —
  don't need the MCP tools for this, it's a plain page). You should see
  7 rows in varying states within about half a second (the mock adds a
  500 ms delay to make `connecting`/`reading` visible).
- `npx tsc --noEmit` is clean, `npm run build` (the real production build,
  same code path the Tauri build uses) succeeds and produces the same
  bundle the shipped `.app` embeds.

I also could not screenshot the actual native tray icon / popover — this
environment has no attached display (`screencapture` fails with `could not
create image from display`), so there was no way to visually confirm the
real window even without Chrome in the picture. The release `.app` does run
without crashing (see **Proof it launches**) and its bundled JS is the exact
same build `tsc && vite build` (and thus `npx tsc --noEmit`) validated.

## Proof of real data

Ran directly against both live accounts tonight (percentages only, no token
recorded anywhere — see reproduction commands below, same shape as
`data/quotos-source-s1/report.md`'s):

**Personal (`~/.claude`, keychain service `Claude Code-credentials`):**

```
HTTP 200
 - session       percent_used=15  is_active=true   scope=None
 - weekly_all    percent_used=6   is_active=false  scope=None
 - weekly_scoped percent_used=3   is_active=false  scope=Fable
```

**Team (`~/.claude-team`, keychain service
`Claude Code-credentials-67d45c83`):**

```
HTTP 200
 - session       percent_used=2  is_active=false  scope=None
 - weekly_all    percent_used=6  is_active=false  scope=None
 - weekly_scoped percent_used=9  is_active=true   scope=Fable
HTTP 200 (profile) — organization.name="Scompler", organization_type="claude_team"
```

The team account's token happened to be fresh tonight, so the 401→`claude
mcp list`→retry path was exercised in code review only, not live — I didn't
find a stale token to trigger it naturally and wasn't going to fabricate one
by, say, editing the Keychain.

**The actual TypeScript normaliser was run against a live capture**, not
just synthetic fixtures — I fetched the personal account's response, fed the
raw JSON through `normalizeUsage()` via a throwaway vitest file (deleted
immediately after, along with the temp JSON — nothing from this lives in the
repo), and it produced:

```json
{
  "windows": [
    { "name": "Session", "scope": null, "remaining": 84, "isActive": true, ... },
    { "name": "Weekly", "scope": null, "remaining": 94, "isActive": false, ... },
    { "name": "Weekly", "scope": "Fable", "remaining": 97, "isActive": false, ... }
  ],
  "remaining": 84,
  "resetsAt": "2026-08-11T23:19:59.207817+00:00"
}
```
84% left = 100 − 16% used (a slightly higher session usage than the first
capture a few minutes earlier — expected, this machine was in active use
tonight). Headline correctly picked the only `is_active: true` window.

Reproduction (never prints a token):

```bash
TOKEN=$(security find-generic-password -s "Claude Code-credentials" -a "$USER" -w \
  | python3 -c 'import sys,json;print(json.load(sys.stdin)["claudeAiOauth"]["accessToken"])')
curl -sS -H "authorization: Bearer $TOKEN" -H "anthropic-beta: oauth-2025-04-20" \
  -H "User-Agent: claude-code/2.1.227" https://api.anthropic.com/api/oauth/usage
```

## Proof it launches

```
$ npm run tauri build
   ...
   Finished `release` profile [optimized] target(s) in 11.44s
   Built application at: .../src-tauri/target/release/quotos-app
   Bundling Quotos.app (.../src-tauri/target/release/bundle/macos/Quotos.app)
   Finished 1 bundle at: .../src-tauri/target/release/bundle/macos/Quotos.app

$ codesign -dv Quotos.app
   Format=app bundle with Mach-O thin (arm64)
   CodeDirectory ... flags=0x20002(adhoc,linker-signed)

$ ./Quotos.app/Contents/MacOS/quotos-app &   # ran directly, 4s, no output, still running
STILL RUNNING
```

`npm run tauri dev` was also run to completion earlier in the session (debug
profile) with no runtime errors in the log once the `app.css` import bug
(below) and the `macOSPrivateApi` warning were fixed.

## Tests

`cd quotos-app && npm test` (vitest) — 16 passing tests in
`src/providers/claude/normalizeUsage.test.ts`, covering every case the brief
asked for: preferring `limits[]` over fixed top-level windows, 1/3/8-length
arbitrary window lists, a never-seen-before `kind` (humanized, not dropped),
a window with a name but `percent: null` (stays `null`, never coerced to
`0`), a model `scope` carried separately from the window name, falling back
to the fixed top-level windows when `limits[]` is absent or `[]`, a `null`
fixed window treated as absent (not zero), `extra_usage` only surfaced when
`is_enabled`, a fully empty `{}` and `null`/`undefined` response, a
subscription with genuinely zero windows, headline = most-consumed *active*
window (falling back to any window with a percentage if none are marked
active), malformed entries inside `limits[]` (non-object, `null`) skipped
without throwing, and out-of-range percentages clamped.

`npx tsc --noEmit` — clean, no errors, strict mode
(`noUnusedLocals`/`noUnusedParameters` on).

`cargo check` in `src-tauri/` — clean, no warnings.

## Honest shortcuts and things a reviewer should double-check

- **Window kind → display name is a best-effort humanizer**, not a lookup
  table sourced from the provider. The API ships no human-readable label for
  `limits[].kind`; I hand-map the handful of known kinds (`session`,
  `weekly_all`, `weekly_scoped`, …) to "Session"/"Weekly" and fall back to
  title-casing the raw `kind` string for anything unrecognized (e.g.
  `nimbus_quill_experimental` → "Nimbus Quill Experimental"). This satisfies
  "the provider's own wording" in spirit but it's not literally verbatim —
  there's no verbatim string to use.
- **Profile is cached in memory per account for the app's lifetime**
  (`profile_cache` in `lib.rs`), fetched once on first successful read. This
  was a deliberate call to conserve the rate budget (profile is a "nice to
  have" for the label; the report notes it's on a separate rate-limit bucket
  from `/usage`, but there's no reason to hit it every 2 minutes when an
  org's name/type essentially never changes mid-session).
- **Account discovery is a directory scan**, not the full "scan the
  machine → report what it found" add-subscription flow from brief §5.3. It
  runs silently on launch; there's no "found two subscriptions, add them?"
  confirmation step. Given item 8 (the real add flow) was out of scope
  tonight, I judged silent auto-discovery of `.claude`/`.claude-*` a
  reasonable stand-in that still gets both real accounts on screen without
  inventing a fake settings UI.
- **The rate limiter is conservative on purpose**: it reserves a slot
  *before* the network call, so a failed request still spends its budget.
  That's the safer failure mode (never accidentally exceed 5/300s) at the
  cost of occasionally under-using the budget if a request errors out for a
  reason unrelated to rate limiting.
- **I hit and fixed one dumb bug worth flagging**: `rm -f src/App.css` (run
  to clean up the Tauri scaffold's unused default stylesheet) silently
  deleted my own `src/app.css` too, because APFS is case-insensitive by
  default. Caught it because `npm run tauri dev` errored on the missing
  import; if you add new files with names that only differ by case from
  something else in the tree, double-check `ls` afterward.
- **The DMG target is off** (see above) — only `.app` is produced.
- Never printed, logged, or committed a token anywhere, including in this
  file, in mock data (all mock data is synthetic), and in the throwaway
  real-data verification (deleted after use, and it only ever held
  percentages/timestamps, which is usage data, not a credential).
- Nothing was installed globally; `node_modules` and `src-tauri/target`
  stay local to `quotos-app/` (both gitignored). No files were touched
  outside this worktree except reads from
  `/Users/supolka/dev/firstmate/projects/quotos/design/` and
  `/Users/supolka/dev/firstmate/data/quotos-source-s1/report.md`, and the
  macOS Keychain reads (read-only, standard `security` CLI, both accounts
  ended the night with valid unmodified tokens).

## How to run it

```bash
cd quotos-app
npm install        # first time only
npm run tauri dev  # opens the real app: tray icon appears, click it for the panel
```

Or just the packaged build already produced on this branch:

```bash
open quotos-app/src-tauri/target/release/bundle/macos/Quotos.app
```

(Gatekeeper may ask to confirm opening an unsigned-by-Apple app the first
time — right-click → Open, or `xattr -dr com.apple.quarantine
Quotos.app` if it was copied from elsewhere.)

To see every UI state without the real Tauri shell (mock data, any browser):

```bash
cd quotos-app
npm run dev
# open http://localhost:1420/ in a browser
```

To rebuild the release bundle from scratch:

```bash
cd quotos-app
npm run tauri build
```

To run the tests:

```bash
cd quotos-app
npm test          # normaliser unit tests (vitest)
npx tsc --noEmit  # typecheck
cd src-tauri && cargo check  # Rust typecheck
```
