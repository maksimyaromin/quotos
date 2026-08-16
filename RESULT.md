# Quotos — current build state (overnight quality pass)

> This file is rewritten each round, not appended to. This round is a
> maintainer's overnight pass over the whole repo: the Rust backend decomposed
> into focused modules, dead vocabulary removed, and hands-on QA of the browser
> harness that found and fixed real UX bugs. The previous rounds' full
> narratives live in git (`git show 321eec5:RESULT.md` for rounds 3–4,
> `git show 4495bdc:RESULT.md` for the beak-drift measurement round); the few
> measurements still load-bearing are kept in the appendix below.

**322 automated tests pass** (201 vitest, 121 `cargo test`); `tsc --noEmit`,
`cargo check`, `cargo clippy --all-targets`, and `cargo fmt --check` are all
clean.

## How to run, build, test

- **Browser mock harness**: `npm run dev` from `quotos-app/`, open
  `http://localhost:1420/`. Every UI state is drivable with no Rust side at
  all — `src/lib/mockClient.ts`'s demo accounts encode the designed edge cases
  (broken/sign-in, stale credential, rate-limited-from-first-read, flaky
  network, no-limits, severity-vs-headline disagreement, statusline
  reconciliation, long window names).
- **Real app**: `npm run tauri dev` from `quotos-app/`. A bare `cargo build`
  binary renders **nothing** — without the tauri CLI it resolves the dev
  config and loads `build.devUrl` with no vite behind it, which looks exactly
  like "the window opened on another Space".
- **Tests**: `npx vitest run` (195) from `quotos-app/`; `cargo test` (121)
  from `quotos-app/src-tauri` (no workspace manifest above it). Standing
  lint/format bars: `cargo clippy --all-targets` and `cargo fmt --check`,
  both clean (neither component was installed before this round).
- **Packaging**: `npx tauri build` from `quotos-app/` (bundle target `"app"`
  only — the DMG step needs disk-image arbitration a sandbox doesn't have).
  The side-by-side v3 identity is
  `npx tauri build --config src-tauri/tauri.v3.conf.json` (`productName`
  "Quotos 3"; `identifier` deliberately still `com.quotos.desktop.v2`, shared
  with v2 so the native-persistence migration meets the captain's real data —
  see `AGENTS.md`'s side-by-side-packaging entry before "fixing" that).

## What the app does today

- **Tray**: a procedurally drawn Q capacity-gauge glyph (no raster asset)
  whose arc fill tracks the worst *active* limit across everything tracked;
  optionally, colored per-window pinned digits composited into the same RGBA
  icon (there is no color path through `set_title`); a translucent
  panel-open highlight drawn into the same bitmap so the glyph never shifts.
- **Panel**: docked under the glyph with the beak centred on it, placed in
  global points (never "physical pixels" — three APIs each mean a different
  display's scale by that); a non-activating `NSPanel`, so opening it never
  steals the frontmost app or throws the captain off a full-screen Space;
  detachable by dragging the header (live-follow; the drag-time reactivation
  is a captain-approved tradeoff), snap-back arrow to redock.
- **Subscriptions**: discovered, never hardcoded; nothing tracked by default;
  the tracked list is a natively persisted, `fsync`'d JSON file with one-shot
  migration from the old localStorage store; rows reorder from the "…" menu
  (Move up / Move down — panel order drives the persisted list and the tray's
  digit order alike); one automatic read per account
  per minute from the Rust scheduler (a manual refresh resets that minute);
  a shared 5-per-300s request budget with typed failures — a budget wait
  never overwrites a real diagnosis, and "sign-in expired" is only ever said
  when the sign-in is proven to be what failed.
- **Second source**: the Claude Code statusline feed, opt-in per
  subscription, reconciled freshest-wins into the raw usage shape before
  normalization so every existing rule applies unchanged.
- **Sign-in recovery**: drives Claude Code's own `claude setup-token` on a
  real pty; Quotos never touches a credential — it relays the pasted code and
  proves recovery by re-reading the account.
- **Appearance**: the full token system renders correctly in both light and
  dark, and follows a macOS appearance change live.
- **Launch at Login**: a check item in the tray's right-click menu, backed by
  the OS's own login-item registry (`SMAppService`, macOS 13+) — Quotos
  stores nothing and always re-reads what the OS says.

## What this round changed

1. **Backend decomposition** — `lib.rs` went 2311 → 461 lines across three
   line-exact, lossless extractions: `geometry.rs` (pure placement and
   coordinate math + its tests), `accounts.rs` (the account-data plane:
   fetch, budget, scheduler, tracked-list/statusline/sign-in commands), and
   `shell.rs` (the mutually-recursive tray-repaint + panel-lifecycle
   cluster). `lib.rs` is now `AppState` + module declarations + `run()`.
   Each move was verified by diffing the moved body against `git show HEAD`
   plus the full suites.
2. **Dead vocabulary removed** — round 1's never-used `"repairing"`
   subscription state is gone from `entities.ts` and both `StatusDot` copies;
   the design-system `prompt.md` docs that still described eight states, a
   `remaining` (%-left) prop, and state-list-driven layout were corrected to
   the shipped six-state, `used` (%-consumed) reality.
3. **Scope-badge truncation actually truncates** — `text-overflow: ellipsis`
   silently never applies to a flex container, so long model names were
   hard-clipped mid-character for two rounds while the test suite asserted
   otherwise; the truncation now lives on an inner block span inside the
   badge (both design-system copies).
4. **A first-ever read that hits the rate budget no longer lies** — the B6
   restore wrote the captured prior state back verbatim, and for a
   just-added account that prior was the seeded "connecting", leaving the row
   claiming "Reading…" forever while masking the designed budget-wait note.
   An in-flight prior now settles to idle (never read) or working (had data).
5. **Keyboard and appearance gaps** — the "N limits" disclosure was an inert
   span, invisible to keyboard and the accessibility tree; it is a real
   `<button>` with `aria-expanded` now. Light/dark switching was a one-shot
   startup sample; it follows `prefers-color-scheme` changes live.
6. **QA walks that found nothing to fix** (recorded so the next round doesn't
   re-walk them blind): the full light-mode surface; the sign-in recovery
   flow end-to-end in the mock (paste → finished event → automatic re-read
   shows the recovered account; Escape cancels back to the action button; the
   form survives an unrelated read-all); and header-drag detach →
   snap-back, both directions clean.
7. **First-ever lint and format passes** — neither clippy nor rustfmt was
   installed for this machine's toolchain; both are now. The three
   default-level clippy findings are fixed (most notably a named `DragAnchor`
   struct replacing `AppState`'s naked tuple-of-tuples drag anchor), and the
   whole crate is rustfmt-normalized (deliberately no `rustfmt.toml`; the bar
   is the community default, not the crate's accidental ~120-column habit).
8. **The design-system two-copy rule is a test now** —
   `src/design-system/sync.test.js` fails naming the exact offending file
   whenever the app's verbatim copy diverges byte-wise from `design/system/`.
9. **The Subscriptions screen picks up a new sign-in by itself** — it rescans
   discovery whenever the panel comes back after a hide (sign in via
   terminal, reopen the panel, the account is just there), and the footer's
   factually-false "Quit and reopen Quotos to pick it up" copy now tells the
   truth ("Reopen this screen to pick it up").
10. **Transient layers dismiss in order** — Escape closes an open "…" row
    menu first and only a bare Escape hides the panel; a menu left open no
    longer survives a panel hide; the menu trigger carries `aria-expanded`.
11. **Cross-references survived the decomposition** — every comment that
    still pointed at `lib.rs` for code that moved in (1) — frontend comments,
    `app.css`, both `Panel.jsx` copies, Rust doc comments — now names the
    module that actually owns it.
12. **This file** — it was two-plus rounds stale (claimed 176 tests and round
    4 as current, predating the statusline feed, working drag, and per-window
    pinning entirely).
13. **Subscriptions are reorderable** — "Move up" / "Move down" in the "…"
    row menu swap the row with its neighbor; panel order is the one order
    everywhere, so the persisted list and the tray's digit order follow the
    same swap. An edge row's impossible direction renders disabled
    (macOS-style) rather than hidden, keeping the menu's fixed item set.
14. **Reopening the panel no longer shows an hours-stale clock** — the same
    WKWebView timer suspension that moved the refresh cadence native also
    freezes the panel's 30-second presentation tick, so a panel reopened
    after hours painted "Last read 2 min ago", stale "Resets today at …"
    copy, and wrong rate-budget waits until the first post-resume tick. The
    clock now re-reads on the panel-visibility show event.
15. **macOS "Reduce Motion" is honored** — `tokens/elevation.css` now answers
    `prefers-reduced-motion` in one place: the `--dur-*` tokens (which every
    transition takes its duration from) zero out, the inline-styled keyframe
    animations (pulse, spin, shimmer) stop via `!important`, and the shimmer
    overlay is hidden outright (stopped, its gradient reads as a static white
    stripe). Verified in a real browser engine with
    `--force-prefers-reduced-motion`; `reducedMotion.test.jsx` pins the token
    coverage and the no-literal-duration-transitions invariant that makes it
    complete.
16. **The "…" row menu is fully keyboard-operable** — with the menu open,
    ArrowUp/ArrowDown walk the enabled items (wrapping, disabled items
    skipped), Home/End jump to the edges, and a close that unmounts the
    focused item (Escape, or activating an item) hands focus back to the "…"
    trigger instead of stranding it on `<body>`. Verified live in a real
    engine: Enter opens, arrows navigate, Enter activates, focus returns.
17. **A corrupt `tracked.json` can no longer be silently destroyed** — the
    store already (correctly) started empty on an unparseable file, but it
    left the bytes in place, where the very next save's atomic overwrite
    erased the only copy of the user's tracked list. `Store::load` now moves
    an existing-but-unparseable file aside to `tracked.json.corrupt`
    (best-effort, one slot) before starting empty, so recovery stays
    possible by hand — the same recoverability rule the localStorage
    migration already followed.
18. **The "…" menu says what it is to assistive tech** — the trigger carries
    `aria-haspopup="menu"`, the dropdown is a real `role="menu"` named
    "Subscription actions", every action is a `role="menuitem"` (disabled
    edges included), and the divider is a `role="separator"`. Before this, a
    screen reader heard "More, button" and then six unrelated buttons;
    Chrome's live accessibility tree now reports the standard menu pattern,
    matching the keyboard behavior item 16 already gave it.
19. **Only one Quotos runs at a time** — `single_instance.rs` takes an OS
    file lock (`instance.lock` in the app config dir, pure std
    `File::try_lock`, no plugin crate) at setup; a second instance exits
    quietly, and an uncheckable lock steps aside rather than blocking
    launch. Two live instances each ran their own limiter against the same
    shared 5-per-300s allowance and spent it double-speed — the realistic
    pair being a dev run beside the installed build, which share a bundle
    identifier and therefore the config dir the lock lives in. The
    statusline helper (`main.rs`'s ingest intercept) exits before `run()`
    and never meets the lock.
20. **Two scheduler passes can no longer double-spend the budget** — the
    launch-time `kick_scheduler` and the periodic 5s tick both call
    `run_due_pass`, and an account is only marked attempted *after* its
    fetch completes, so when the first pass ran long (the ordinary case: an
    ~8h-expired token at morning launch means a bounded-20s CLI renewal
    before the request) the tick's pass saw the same account still due and
    fetched it again — two slots of the 5-per-300s budget, plus two
    concurrent CLI renewals, for one read. `Scheduler::begin_pass` (an
    `AtomicBool` gate with an RAII guard, no new dependencies) now makes the
    loser skip; whatever is due is already the running pass's job.
21. **Launch at Login** — a "Launch at Login" check item now sits above "Quit
    Quotos" in the tray's right-click menu (`launch_at_login.rs`), driving
    `SMAppService.mainAppService` directly via the already-present `objc2`
    (an empty `#[link]` extern block links `ServiceManagement`; zero new
    crates). The OS owns the state: the checkmark is read from `status` at
    menu build and re-read after every toggle, so a refused registration
    reads as still-off instead of lying. Measured from an unbundled test
    binary the OS answers `NotFound` — which is the graceful-refusal path;
    the registered happy path needs the packaged `.app` (see gaps below).
22. **Hovering the tray icon now says whose number is whose** — the tooltip
    used to read "Quotos — 40% 70%": bare digits with no owner, useless
    with more than one subscription pinned (and short of its own comment's
    "in words" intent for VoiceOver). It now names every contributing
    figure, one line per subscription in the bar's own order ("Claude Max:
    Weekly 40% · Session 70%"), marks a stale subscription's line "— not
    current" (the row badge's own words), and is composed in one testable
    pure function (`lib/traySegments.ts`'s `buildTrayTooltip`) on the
    frontend, where the labels live — the Rust side applies it verbatim. A
    rename updates it too: the tooltip participates in `set_tray_status`'s
    skip-identical guard, since a renamed subscription changes its tooltip
    line while leaving every digit byte-identical.

23. **The template's opener plugin is gone** — `tauri-plugin-opener` was
    registered at startup, granted to the webview (`opener:default`), and
    shipped as an npm dependency, yet nothing ever called it: the UI has no
    links, and the sign-in flow deliberately lets the Claude CLI open the
    browser itself. Removing it pruned 42 crates from `Cargo.lock` (the
    whole zbus/dbus async ecosystem, compiled on macOS for nothing) and
    revoked the one capability that let the webview ask the OS to open
    arbitrary URLs or paths — the same "remove what nothing uses" rule that
    already took out `tauri-plugin-positioner`.

24. **The webview runs under a real Content-Security-Policy** — the config
    shipped `"csp": null` (template residue, the same class as the opener
    plugin above): any injected markup could have run scripts, talked to any
    host, loaded anything. The policy is now `default-src 'self'; script-src
    'self'; style-src 'self' 'unsafe-inline'; font-src 'self' data:;
    connect-src ipc: http://ipc.localhost; object-src 'none'; base-uri
    'none'; form-action 'none'` — the webview cannot reach the network at
    all (`connect-src` is Tauri's own IPC and nothing else). Each relaxation
    is earned: `style-src 'unsafe-inline'` for React's inline style
    attributes plus the three keyframe `<style>` elements, `font-src data:`
    because vite inlines the two sub-4KB MonoLisa weights into the CSS.
    Verified twice: the built bundle served in a real browser under the same
    policy (zero `securitypolicyviolation` events across the full UI,
    including the data: fonts and a live subscription row), and the packaged
    `.app` itself rendering a seeded probe row end-to-end (the IPC
    round-trip proven under the policy, not assumed).

25. **The webview's ACL is now two permissions, not a bundle** —
    `capabilities/default.json` shipped `core:default` plus four explicit
    `core:window:allow-*` grants, all first-commit residue from before every
    window operation moved behind this app's own commands. The frontend
    reaches Rust exactly two ways: custom `#[tauri::command]`s (not gated by
    the capability system) and `listen`/`unlisten` from
    `@tauri-apps/api/event` — so the grant is now exactly
    `core:event:allow-listen` + `core:event:allow-unlisten`. Revoked along
    with the window grants: `core:default`'s menu, tray, app, path,
    resources, image, and webview bundles (a compromised webview could have
    rewritten the tray icon or created native menus). Verified end-to-end in
    the packaged `.app` with the round-24 probe technique: the seeded row
    renders with its custom label (invoke works) *and* shows the
    needs-sign-in diagnosis, which only reaches the frontend through the
    `quota-refresh` push — proving `listen` survives the tightened grant.

26. **The last of the template's icon residue is gone** — this macOS-only
    app (bundle targets `["app"]`) still shipped twelve Windows icon files
    from the Tauri template: ten Windows Store logos
    (`Square*Logo.png`/`StoreLogo.png`, referenced by nothing) and
    `icon.ico` (listed in the bundle config), all still carrying the stock
    Tauri logo — the round-that-drew-the-Q-mark regeneration only covered
    `icon.icns` and the PNGs. Deleted all twelve, dropped `icon.ico` from
    `tauri.conf.json`'s icon list, and removed `Cargo.toml`'s placeholder
    `authors = ["you"]`. Verified by a passing production bundle whose
    `Contents/Resources` holds exactly the correct Q-mark `icon.icns`,
    byte-identical to source.

27. **The design system's own exhibits tell the truth again** — the card
    pages and the `ui_kits/quotos` interactive demo were a full component
    generation behind the app: the prebuilt `_ds_bundle.js` still carried
    round-1 components (eight states, `remaining` %-left fields, per-
    subscription pinning), and the pages fed them matching stale props. The
    bundle is now regenerable from source (`design/system/_build_bundle.mjs`
    — a small Node generator reusing quotos-app's esbuild, same in-page
    contract: per-module try-wrapped IIFEs over a shared scope, public
    components exposed on the namespace), the demo data and all four card
    pages speak the shipped vocabulary (`used`/`severity`, the six states,
    per-window pinning with `id`s, "Not current"/"Needs sign-in" badges),
    and `MenuBarTile` — the last component still holding %-remaining
    semantics (`<=10` red / `<=25` amber) — was flipped to consumed
    (`>=90`/`>=75`, both copies). Verified in a real browser: all five
    bundle-consuming pages load with zero `__errors`, the severity seam
    renders (38% headline amber because an 82% window exists), and the
    interactive demo's per-window pin toggle adds/removes tray figures in
    subscription order.

28. **Confirming a rename without editing no longer deletes the custom
    name** — the rename field shows the *composed* label (the custom
    override when one exists), and commit treated "draft equals what the
    field showed" as a request to revert to the provider name. So pressing
    Enter on an untouched draft — or merely opening Rename and clicking
    away, since blur commits — silently cleared an existing custom name and
    persisted the loss. Unchanged is now a no-op; only an explicitly
    emptied field clears (both design-system copies, bundle regenerated,
    four regression tests).

29. **The statusline second source can actually win now** — the snapshot's
    `fetched_at` was stamped at snapshot *assembly*, which happens after the
    feed file is read, so the freshest-wins comparison
    (`statuslineMerge.ts`) discarded the feed on every live read: the whole
    S2 feature (helper install, ingest, `read_feed`) shipped inert, and its
    tests never noticed because they fabricate a feed timestamp the Rust
    producer couldn't construct. `fetched_at` is now stamped the moment the
    usage HTTP response arrives (`providers/claude.rs`'s `UsageRead`,
    pinned by a slow-body local-server test). In the same pass the ingest
    helper's write path stopped sharing one fixed temp name per feed file —
    two concurrent Claude Code sessions (one helper process per statusline
    render) could truncate each other's temp between write and rename,
    making the feed intermittently vanish; temp names are now unique per
    writer (pid + counter), pinned by an overlapping-writers test. Still
    open by design: a failed or rate-limited API read delivers no feed at
    all (`perform_fetch` returns the error before reading it) — the case
    where a zero-cost second source would matter most; that needs a wire-
    shape and presentation decision (what a "behind" row says when the feed
    is fresher than the last good read), recorded for a future round.

30. **Two overlapping saves can no longer destroy the tracked list.** The
    native store's `save` ran its whole temp-file + `fsync` + rename on one
    *fixed* temp name and took its mutex only afterwards, to update memory —
    and `save_tracked` is an async command the frontend fires on every
    membership/label/pin change, so two quick panel changes could truncate
    each other's temp mid-write (spliced JSON → `tracked.json.corrupt` on
    the next launch → the whole tracked list gone, the exact loss class the
    native store was built to end) or land on disk in the opposite order to
    memory (the scheduler then polls an account the disk says is
    stop-tracked). The lock is now held across the whole write, and the
    write goes through a new shared `atomic_write` module — the
    unique-temp-name helper item 29 built for the statusline feed,
    extracted rather than copied a third time — pinned by a four-writer
    concurrency test that failed against the old code on its first run
    (concurrent renames of the shared temp name error with ENOENT). On the
    frontend, a failed save is no longer swallowed and remembered as saved:
    `saveTracked` now rejects, and the save effect clears its dedupe record
    on failure so the next effect run — even one where only read state
    changed, i.e. within about a minute — retries; a failed *migration*
    save still shows the legacy list and simply re-fires on the next launch
    (three regression tests).

31. **A 200 with an unreadable body is a failed read now, not a healthy
    empty one.** `get_json` swallowed any body that didn't parse as JSON
    into `Ok(Null)` — so a mid-body connection reset, a timeout during the
    body, or a proxy's HTML error page (served with a 200) flowed through
    normalization as a *successful* read with no windows: "working / No
    limits reported yet", every window and tray digit wiped, `lastReadAt`
    bumped, the honest "behind — these numbers are from the last successful
    read" path never taken. The same swallow let `fetch_profile` cache a
    `Null` profile for the whole run (wrong label until relaunch). An
    unreadable 200 body is now `FetchError::Network` (which the frontend
    already maps to `behind`/`broken` by prior-good-data), while a non-200's
    body may still be unreadable — its status alone classifies it, the body
    is never consumed, and an HTML-bodied 401 must keep reaching the 401
    handling. Both sides pinned by local-server regression tests; the
    failure one failed against the old code on its first run.

32. **Tab no longer disappears into a collapsed row.** The expanded detail
    area is always mounted (the I4 grid-template-rows animation needs it),
    and `0fr` + `overflow: hidden` + `aria-hidden` only *clipped* the
    per-window pin buttons — they stayed in the tab order, so keyboard focus
    vanished into the closed row and Enter toggled a tray digit with nothing
    visible on screen. The collapsed container is now also
    `visibility: hidden`, which is what actually removes clipped content
    from the tab order; visibility transitions on the same `--dur-base`
    token (discretely — content stays visible while the row animates closed,
    then leaves the tab order). Fixed identically in both design-system
    copies, bundle regenerated, pinned by 3 regression tests that fail
    against the old component.

33. **The header refresh and a row's "Read now" can no longer double-spend
    the budget on one account.** `refreshAll` fanned out to `refreshOne`
    directly, bypassing the per-account in-flight map that `refreshAccountById`
    used — so pressing both at once spent two of the shared 5-per-300s
    request slots on the same account, and the next scheduled read got
    refused a minute early. Every fetch-spending path (header refresh,
    row read, launch read) now funnels through one guarded helper: a
    concurrent request for an account already being read joins the in-flight
    read, while other accounts still fetch normally. Pinned by 3 regression
    tests proven to fail against the old hook. (This also removes the main
    route into F5's stale-restore race — the full re-capture fix is still
    open.)

34. **A statusline reading without a reset time no longer blanks the API's.**
    The feed's ingest side only requires `used_percentage` (`statusline.rs`'s
    `extract_window`), so a fresher reading with `resets_at: null` is
    routine — but `reconcileWithStatusline` wrote the feed's `resets_at`
    unconditionally, so that reading erased the API's real reset timestamp
    and the row's "Resets today at…" line flickered out. All four patch
    sites now go through one helper that always refreshes the percentage
    but only overwrites `resets_at` when the feed actually supplies one.
    Pinned by 3 regression tests, the two preservation ones proven to fail
    against the old code.

## Honest gaps, still open

- **Where `claude setup-token` writes for the default account** is unverified
  now that Quotos no longer forces `CLAUDE_CONFIG_DIR` (see `AGENTS.md`'s
  R3-4 entry) — completing a real login is the captain's own check.
- **Full-screen Space stay-put** is verified at the mechanism level (the app
  no longer activates on open — measured from a separate process) but not
  end-to-end; an agent here cannot put an app into full screen.
- **A real hand drag** was confirmed live by the captain mid-round-R5;
  synthetic drags still produce no `WindowEvent::Moved` on this machine, so
  drag regressions cannot be caught from here — ask him.
- **Launch at Login's happy path** (toggling it on from the packaged `.app`
  and seeing Quotos in System Settings → Login Items) is untested end-to-end:
  registering a login item mutates the live machine's settings, which is the
  captain's own click to make. What is verified: the framework links, the
  live `status` call answers, and an unregistrable binary degrades to an
  unchecked box plus a stderr line rather than an error dialog or a lie.

## Diagnostics left in the build (all opt-in, all silent by default)

`QUOTOS_DEBUG_POS` (placement trace), `QUOTOS_DEBUG_AUTO_OPEN` (open the
panel from `tray.rect()` with no click), `QUOTOS_DEBUG_KEEP_OPEN` (suppress
hide-on-blur), `QUOTOS_DEBUG_READS` (credential/read decisions, never a
token), `QUOTOS_DEBUG_SPACE_BEHAVIOR` / `QUOTOS_DEBUG_WINDOW_LEVEL`
(collection-behaviour/level sweeps), `QUOTOS_PANEL_MODE=window` (restore the
pre-NSPanel activating path for A/B).

---

## Appendix: earlier rounds' measurements still load-bearing

Kept because the tray and placement geometry is built on them; the full
narratives are in git history (revisions named at the top).

**Placement, verified end-to-end (round 3)** — dev-identity build, one real
tray click, window-server geometry sampled every 15ms, cross-checked against
the app's own `QUOTOS_DEBUG_POS` trace:

| | value |
|---|---|
| tray rect as `tray-icon` reports it | `(2444, 0)` physical-ish |
| resolved to | display 0 (built-in), point `(1222, 0)` |
| computed layout | `x=1216, y=39, beak_left=3` (pre-R3-10 geometry) |
| `NSWindow` frame after show | `(1216, 39, 360, 560)` |
| window-server bounds | `(1216.0, 39.0, 360.0, 560.0)` |
| position changes during the entire open | **1** |

Computed = AppKit = window server, and the window moves once (previously four
relocations per click). With pinned digits: beak centre
`1160.25 + 14 + 20 + 10 = 1204.25` against glyph centre `1204.25` — identical.
The beak-drift *pixel* measurements (true glyph centre ~18pt in, not the
hardcoded 15) are in `git show 4495bdc:RESULT.md`.

**The row menu vs. the scroll box (round 4)** — measured in the live
WKWebView; the menu is `position: fixed` so it neither clips at the panel's
edge nor adds scroll extent:

| state | body | scrollHeight vs clientHeight | menu |
|---|---|---|---|
| 2 rows | 332×274 | 274 == 274 (no scrollbar) | — |
| 2 rows, bottom menu open | 332×274 | **unchanged** | fixed, 178×122, fully inside the window |

**Menu bar heights on this one machine (round 3)**: notched built-in 33pt,
unnotched external 24–30pt — the reason nothing derives placement from a
hardcoded bar height.
