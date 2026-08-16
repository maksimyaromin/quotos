# Quotos — current build state (tray frame unification)

> This file is rewritten each round, not appended to. This round fixed the
> two visual defects the captain reported 2026-08-16 from screenshots of his
> real menu bar: an oversized gap to the next menu bar extra, and two
> differently-sized rounded frames for the click-highlight vs. the
> panel-open state. Previous round's narrative (panel background flicker +
> v5.1) is `git show 6367755:RESULT.md`.

**330 automated tests pass** (208 vitest, 122 `cargo test` — one new,
pinning this round's own invariant) from the repository root; `tsc
--noEmit`, `cargo clippy --all-targets -D warnings`, and `cargo fmt --check`
are all clean. This round produced no new packaged `.app` — the task was
scoped local-only (branch only, no packaging requested); verification was
done against a real, native `npm run tauri dev` build (dev identity
`com.quotos.desktop`, distinct from the captain's live `com.quotos.desktop.v4`
install — never touched his tracked accounts or shared read budget).

## The fix, in one line

`src-tauri/src/shell.rs`'s new `sync_status_item_length`, called on every
tray repaint, pins the underlying `NSStatusItem`'s `length` to exactly match
the just-composited image's own width, instead of leaving it on
`tray-icon`'s default `NSVariableStatusItemLength`.

## Diagnosis

The brief's working hypothesis was verified, not assumed: `tray-icon`
v0.24.2 creates the status item with `NSVariableStatusItemLength` and never
touches its length again (`TrayIcon::create` in the crate source). A
variable-length item's *button* is measurably wider than its own composited
image. Measured live via Accessibility (`System Events`, `position`/`size`
of the menu bar item) against a real running build, comparing the reported
item rect to the image width `tray_render::render`/`plain_glyph_rgba`
actually produce:

| state | item rect (reported) | image width (computed) | extra |
|---|---|---|---|
| bare glyph, nothing pinned | 46pt | 28pt | 18pt (9pt/side) |
| 5 digit segments pinned (`51% 67% 96% \| 0% 0%`) | 208pt | 190pt | 18pt (9pt/side) |

The same ~9pt/side margin both times, independent of content — a fixed
AppKit behaviour, not something proportional to the image. That single
extra margin was **both** reported defects at once:

- it is extra dead space beyond the item's own drawn content, directly
  widening the gap to whatever sits next in the menu bar;
- it is why a plain click's native highlight — which AppKit paints across
  the *button's* bounds via `-[NSStatusBarButton highlight:]` — read wider
  than the panel-open pill this app draws itself, into the *image's* bounds
  (`tray_render::draw_highlight_background`). The button and the image were
  simply never the same rectangle.

## The fix

`shell.rs`'s `sync_status_item_length(tray, icon_width_px)` reaches the raw
`NSStatusItem` via `tauri::tray::TrayIcon::with_inner_tray_icon` →
`tray_icon::TrayIcon::ns_status_item()` (both already-public escape hatches,
no fork needed) and calls `setLength` with the composited image's own width
in points (`icon_width_px as f64 / 2.0` — `tray_render`'s buffer is always
2x an 18pt-tall image, the same convention `geometry.rs`'s
`glyph_center_offset_from_item_left_points` already relies on, so this is
correct on any display regardless of its own backing scale). Called from
`repaint_tray_icon` on every repaint (a fixed-length item never resizes
itself when a new image is set, unlike a variable-length one — leaving it
stale would clip or under-fill the button the next time the digit count
changes) and once more right after the tray is built in `lib.rs`'s `setup`,
so there is no window at launch where the item is still
`NSVariableStatusItemLength`.

`objc2-app-kit`'s `NSStatusBar`/`NSStatusBarButton`/`NSStatusItem` features
were added to this crate's own `Cargo.toml` declaration for this (already
resolved in `Cargo.lock` — the `tray-icon` crate itself requests the same
three for the same crate version, so this added no new dependency
resolution, just made the existing symbols nameable from this crate's own
code too).

**Nothing inside the composited image changed.** Glyph, digits, the `|`
separator, colours, `CELL_WIDTH_PX`'s per-segment reserve, `SIDE_PAD_PX` —
all untouched, per the brief's explicit constraint and the captain's own
direct instruction mid-round not to touch anything inside Quotos's own
drawn content.

## Verification, measured live

Real `npm run tauri dev` build, one Quotos process at a time (the captain's
own live `Quotos v5.1.app` was quit before each test pass and relaunched
immediately after — `pgrep -il quotos` confirms only his original process
is running now, matching the pre-task baseline). Item geometry via System
Events (`position`/`size` of the menu bar item); screenshots via
`screencapture -x -o -R` in global points; ink bounding boxes via `sips -s
format bmp` + a pure-Python 32bpp BMP column scan (no Pillow available).

**Defect 2 (two frames) — fully fixed, confirmed directly, not inferred.**
The same five-segment state's item rect read identical in all three states:

| state | item rect |
|---|---|
| idle | `1018,4 192x24` |
| panel-open (own drawn pill) | `1018,4 192x24` |
| click (native highlight) | same button, size unchanged by highlighting (AppKit toggles a paint state, never the frame — confirmed from the `tray-icon` crate source: `-highlight:` is called on the identical `NSStatusBarButton` whose `.frame()` is what this rect reports) |

Pinned in code: `tray_render::tests::click_highlight_and_panel_open_share_one_frame_width_with_segments_pinned`
asserts `render`'s width never depends on `highlighted`, for a real
(non-empty) segment set — the property `sync_status_item_length` depends on
to keep every state's button length in sync with one image width.

The live, sub-150ms native-highlight frame itself could not be
photographed: ten burst-capture attempts (`screencapture` in a tight loop,
~14fps, the same technique the previous round's W1 investigation used)
across varied timing offsets each showed a single clean step between idle
and highlighted brightness, never an intermediate frame distinguishable as
"native flash, not yet the panel-open pill" — consistent with either
missing the transient at this sampling rate, or the two highlights simply
reading as visually similar at the one background sample point checked.
Given the geometry is proven directly (same button, same frame, in every
state, matching AppKit's own documented per-button highlight behaviour),
this gap in photographic evidence doesn't weaken the conclusion, and is
recorded here rather than glossed over.

**Defect 1 (oversized gap) — the AppKit-margin portion is fixed and
confirmed; the visible gap did not fully close, and the reason is
understood, not a loose end.** Item rect before/after, same five segments:

| | item rect | right edge |
|---|---|---|
| before | `1010,4 208x24` | 1218 |
| after | `1018,4 192x24` | 1210 |

Image width is 190pt either way (unchanged, since the image itself never
changed) — so the button now tracks the image to within ~1-2pt/side, down
from ~9pt/side. But the *visual* ink-to-ink gap to the next menu bar extra
(a different, unrelated app's icon) measured **32pt both before and after**
— unchanged. Reasoned and cross-checked: the neighbour's own ink position
didn't move at all between the two captures (AppKit doesn't repack sibling
items just because Quotos's own length changed), and within Quotos's own
image, the last segment's ink ("0%") sits well inside its reserved 30pt
cell (`CELL_WIDTH_PX`), leaving roughly 13.5pt of intentional, tested,
content-layout slack after it — explicitly off-limits for this task. A
baseline check between two *other*, unrelated menu bar extras on the same
bar (not Quotos) measured 19-21pt apart, confirming some of the 32pt was
never AppKit-button-margin or Quotos's own drawn spacing at all — it's
however much room a *different* app's own icon leaves.

So: this round removed the one thing in Quotos's own control that was
provably excess (the button-vs-image margin, 18pt gone) and fully unified
the two frames. What's left of the gap is either this app's own intentional
`CELL_WIDTH_PX` reserve (content layout, out of scope per the brief and the
captain's own words) or another app's status item margin (not Quotos's to
change). If the gap still reads as too large, that's a real product
question — whether to revisit `CELL_WIDTH_PX`'s reserve — for a scoped
follow-up, not evidence this fix is wrong or incomplete.

## Honest gaps, still open

- The sub-150ms native click-highlight frame was not directly photographed
  (see above) — geometry is proven by measurement + mechanism instead.
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
`AGENTS.md`'s "Run only ONE Quotos build at a time" note, and the new
quotos-tray-frame-t1 entry for this round's own measurement method).
