# Quotos — current build state (tray frame unification, both defects)

> This file is rewritten each round, not appended to. This round fixed the
> two visual defects the captain reported 2026-08-16 from screenshots of his
> real menu bar: an oversized gap to the next menu bar extra, and two
> differently-sized rounded frames for the click-highlight vs. the
> panel-open state. Defect 2 (two frames) was accepted on first pass; a
> followup reopened defect 1 (the gap) because the first pass's own evidence
> contained an unresolved contradiction — see "Resolving the contradiction"
> below for how that was settled before any further code changed. Previous
> round's narrative (panel background flicker + v5.1) is
> `git show 6367755:RESULT.md`.

**333 automated tests pass** (208 vitest, 125 `cargo test` — four net new
across this round, one from the defect-2 fix and three from the followup)
from the repository root; `tsc
--noEmit`, `cargo clippy --all-targets -D warnings`, and `cargo fmt --check`
are all clean. This round produced no new packaged `.app` — the task was
scoped local-only (branch only, no packaging requested); verification was
done against a real, native `npm run tauri dev` build (dev identity
`com.quotos.desktop`, distinct from the captain's live `com.quotos.desktop.v4`
install — never touched his tracked accounts or shared read budget).

## The fix, in two lines

`src-tauri/src/shell.rs`'s `sync_status_item_length`, called on every tray
repaint, pins the underlying `NSStatusItem`'s `length` to exactly match the
just-composited image's own width, instead of leaving it on `tray-icon`'s
default `NSVariableStatusItemLength` — this alone fixed defect 2.
`src-tauri/src/tray_render.rs`'s `render()` now sizes only the *trailing*
segment's cell to that one segment's own measured text width, instead of
the fixed `CELL_WIDTH_PX` reserve every other segment still gets — this is
what closed most of what was left of defect 1.

## Diagnosis, part 1: the button-vs-image margin (defect 2, and part of defect 1)

The brief's working hypothesis was verified, not assumed: `tray-icon`
v0.24.2 creates the status item with `NSVariableStatusItemLength` and never
touches its length again (`TrayIcon::create` in the crate source). A
variable-length item's *button* is measurably wider than its own composited
image. Measured live via Accessibility (`System Events`, `position`/`size`
of the menu bar item), comparing the reported item rect to the image width
`tray_render::render`/`plain_glyph_rgba` actually produce:

| state | item rect (reported) | image width (computed) | extra |
|---|---|---|---|
| bare glyph, nothing pinned | 46pt | 28pt | 18pt (9pt/side) |
| 5 digit segments pinned (`51% 67% 96% \| 0% 0%`) | 208pt | 190pt | 18pt (9pt/side) |

The same ~9pt/side margin both times, independent of content. That single
extra margin was **both** reported defects at once: extra dead space
widening the gap to the next menu bar extra, and why a plain click's native
highlight — painted across the *button's* bounds via
`-[NSStatusBarButton highlight:]` — read wider than the panel-open pill this
app draws itself, into the *image's* bounds.

`sync_status_item_length(tray, icon_width_px)` reaches the raw
`NSStatusItem` via `tauri::tray::TrayIcon::with_inner_tray_icon` →
`tray_icon::TrayIcon::ns_status_item()` (both already-public escape hatches)
and calls `setLength` with the composited image's own width in points.
Called from `repaint_tray_icon` on every repaint, and once more right after
the tray is built in `lib.rs`'s `setup`, so there is no window at launch
where the item is still `NSVariableStatusItemLength`. `objc2-app-kit`'s
`NSStatusBar`/`NSStatusBarButton`/`NSStatusItem` features were added to this
crate's own `Cargo.toml` for the escape hatch (already resolved in
`Cargo.lock` via `tray-icon`'s own request for them, so no new dependency
resolution).

**This fully fixed defect 2** — the captain confirmed it. Item rect read
identical across idle, panel-open, and (by construction of AppKit's own
per-button highlighting, which never resizes the button) native
click-highlight.

## Resolving the contradiction (before any further code changed)

The captain's followup flagged a real contradiction in the first pass's own
numbers: removing ~9pt of margin per side should have moved the neighbouring
extra ~9pt closer, but the measured ink-to-ink gap held at 32pt both before
and after `sync_status_item_length`. Investigated with more measurement, not
asserted away:

| | item rect | right edge | last "0%" ink ends | neighbour ink starts |
|---|---|---|---|---|
| before the length pin | `1010,4 208x24` | 1218pt | 1195.5pt | 1227.5pt |
| after the length pin | `1018,4 192x24` | 1210pt | 1195.5pt | 1227.5pt |

The digit ink itself never moved (nothing inside the image changed) and the
neighbour's ink never moved either (a different app, and AppKit does not
repack sibling status items when Quotos's own length changes) — so the
32pt gap splits into two components that must sum to 32pt regardless:
*(last ink → Quotos's own right edge)* + *(Quotos's own right edge →
neighbour's ink)*.

| component | before any fix | after the length pin alone | after the trailing-cell trim too |
|---|---|---|---|
| last ink → Quotos's right edge | 22.5pt | 14.5pt | 6.5pt |
| Quotos's right edge → neighbour | 9.5pt | 17.5pt | 18.5pt |
| **sum (the measured ink-to-ink gap)** | **32pt** | **32pt** | **25pt** |

`setLength` re-centres the button about its own midpoint — confirmed live,
the item's *left* edge moved right by the same ~8pt its right edge moved
left — so only about half the removed margin ever reached the trailing
edge; the freed trailing space just enlarged the gap between Quotos's new
right edge and the neighbour instead of pulling anything closer, because
nothing pulls neighbours closer. **No evidence of a macOS-enforced minimum
inter-item spacing floor was found**: the button-edge-to-neighbour
component (17.5-18.5pt across this investigation) was already at or under
the 19-21pt baseline gap measured between two *unrelated* neighbours on the
same bar in the same session (see below) — nothing was "absorbing" freed
margin beyond ordinary per-icon variation.

That resolved which of the followup's three offered explanations was real:
partial-reach (real, ~half), not a spacing floor (no evidence), and the two
measurements were fully comparable — the contradiction was just an
undecomposed sum.

## Diagnosis, part 2: the trailing cell reserve (rest of defect 1)

With the button-margin component understood and already minimal, the
remaining ~14.5pt of "last ink → Quotos's right edge" was `tray_render`'s
own drawn content: the trailing segment's ink sits well inside its fixed
30pt `CELL_WIDTH_PX` reserve (~8.5pt of unused space after "0%"'s own
~15pt), plus the intentional 5pt `SIDE_PAD_PX` trailing pad A11's
highlight needs.

The followup lifted the content-layout freeze for exactly this one piece —
not `CELL_WIDTH_PX` in general, only the *trailing* segment's own cell,
since nothing ever sits after it to be moved by its width changing.
`render()` now sizes the trailing cell to that segment's own measured text
width; every earlier segment keeps the fixed reserve, so an earlier
segment's own digit count still can never move anything after it
(`a_non_trailing_figures_digit_count_never_moves_what_follows_it` pins
this). `SIDE_PAD_PX` was kept in full — still needed, unchanged, for the
same reason it always was (A11's highlight pill needs to read as a pressed
button, not hug the last digit).

## Verification, measured live (both rounds)

Real `npm run tauri dev` build, one Quotos process at a time — the
captain's own live `Quotos v5.1.app` was quit before each test pass and
relaunched immediately after; `pgrep -il quotos` confirms only his original
process is running now, matching the pre-task baseline. Item geometry via
System Events; screenshots via `screencapture -x -o -R` in global points;
ink bounding boxes via `sips -s format bmp` + a pure-Python 32bpp BMP
column scan.

**Before any fix, same five segments (`51% 67% 96% | 0% 0%`):**

| | value |
|---|---|
| item rect | `1010,4 208x24` |
| image width | 190pt |
| ink-to-ink gap to neighbour | 32pt |
| baseline gap, two unrelated neighbours (same session) | 19pt and 21pt |

**After `sync_status_item_length` alone (defect 2 fixed, defect 1 not yet):**

| | value |
|---|---|
| item rect | `1018,4 192x24` (idle, panel-open, and click-highlight — identical in all three) |
| image width | 190pt |
| ink-to-ink gap to neighbour | 32pt (unchanged — see "Resolving the contradiction") |

**After the trailing-cell trim (this followup, both defects addressed):**

| | value |
|---|---|
| item rect | `1032,4 177x24` (idle, panel-open, and click-highlight — still identical in all three) |
| image width | 175pt (190 − 15, exactly the removed unused reserve) |
| ink-to-ink gap to neighbour | **25pt**, down from 32pt |
| baseline gap, two unrelated neighbours (same session) | 19pt and 21pt (unchanged, different apps) |

25pt is `SIDE_PAD_PX` (5pt, kept intentionally) plus a couple of points of
CoreText advance-width/antialiasing rounding above the 19-21pt baseline —
not a further leftover reserve. Photographed in all three states both
before and after the trailing-cell trim; the panel-open pill now hugs the
"0%" with the same proportioned breathing room the glyph side has, instead
of a visibly long tail.

Pinned in code:
`tray_render::tests::click_highlight_and_panel_open_share_one_frame_width_with_segments_pinned`
and its followup extension,
`click_highlight_and_panel_open_share_one_frame_width_even_as_the_trailing_digit_count_varies`,
assert `render`'s width never depends on `highlighted` — including across
several trailing digit counts, since the trailing cell's width is the new
axis this round introduced.
`nothing_but_the_side_pad_survives_after_the_trailing_segments_own_ink`
pins how much dead space is allowed after the trailing ink now (measured
11 physical px against a 10px `SIDE_PAD_PX`, bound set a few px looser for
font-fallback safety).

The live, sub-150ms native-highlight frame itself could not be
photographed, in either round: repeated burst-capture attempts (`screencapture`
in a tight loop, ~14fps, the same technique the previous round's W1
investigation used) across varied timing offsets each showed a single clean
step between idle and highlighted brightness, never an intermediate frame
distinguishable as "native flash, not yet the panel-open pill." Given the
geometry is proven directly (same button rect measured via Accessibility in
every state, matching AppKit's own documented per-button highlight
behaviour, which never resizes the button), this gap in photographic
evidence doesn't weaken the conclusion, and is recorded rather than glossed
over.

## Honest gaps, still open

- The sub-150ms native click-highlight frame was not directly photographed
  (see above) — geometry is proven by measurement + mechanism instead.
- The residual ~4-6pt above the 19-21pt baseline (`SIDE_PAD_PX` plus
  CoreText rounding) is believed final, but wasn't independently
  re-litigated against a *different* trailing string than "0%" beyond the
  digit-count sweep the pinned tests already cover.
- Unchanged from earlier rounds (`git show 6367755:RESULT.md` for detail
  and further history): the initiating trigger for the (now-fixed) panel
  background flicker was never identified, though the fix is
  trigger-independent; where `claude setup-token` writes for the default
  account is unverified; full-screen Space stay-put is checked at the
  mechanism level only; synthetic drags don't reproduce on this machine;
  Launch at Login's registration happy path needs the captain's own click.

## How to run, build, test

Unchanged from previous rounds — see `README.md`'s Develop/Test sections
and `AGENTS.md`'s "How to run, build, test" precedent. `npm run tauri dev`
for the real app (dev identity, safe to run alongside the captain's own
installed build as long as only one Quotos process runs at a time — see
`AGENTS.md`'s "Run only ONE Quotos build at a time" note, and the
quotos-tray-frame-t1 entry for this round's own measurement method).
