import React, { useId, useLayoutEffect, useRef, useState } from "react";

// Mirrors --panel-width/--radius-xl (tokens/spacing.css). Duplicated as a
// plain number, the same way the native side duplicates it
// (PANEL_WIDTH_LOGICAL in src-tauri/src/lib.rs) — the SVG path math below
// needs concrete units, not a CSS custom property string.
const PANEL_WIDTH = 332;
export const PANEL_RADIUS = 12;

// The beak's shape. R3-10: the handoff's "12×12" is **superseded by the
// captain's own instruction** after he saw it at real size — *"он во первых
// маленький"*. These numbers are a plain authored shape, free to retune
// visually, but three things elsewhere are keyed to them and must move
// together (there is no build step that syncs them):
//   - BEAK_BASE_HALF * 2 must equal `BEAK_BASE_WIDTH` in
//     src-tauri/src/lib.rs's `docked_layout_in_points`, which converts a
//     glyph centre into this component's `beakLeft`;
//   - BEAK_HEIGHT is what that same function subtracts to put the beak's
//     *tip* just under the menu bar rather than the panel's top edge;
//   - NOTCH_RESERVE must be at least BEAK_HEIGHT plus half the 0.5px stroke,
//     and equal to app.css's `padding-top`, which is what actually keeps the
//     beak inside the transparent window instead of clipped by its edge.
export const BEAK_BASE_HALF = 10;
export const BEAK_HEIGHT = 10;
const BEAK_TIP_ROUND = 3;
// Vertical space this component reserves above the rounded rect's own top
// edge for the beak to render into.
export const NOTCH_RESERVE = 12;

/** Builds one clockwise SVG path for the panel's full outline: a rounded
 *  rect, with the beak (if `beakLeft` is given) fused into the top edge as
 *  a single continuous boundary — no separate shape, no seam. `width`/
 *  `height` are the rounded rect's own dimensions; the returned path is
 *  expressed in a coordinate space whose y=0 is `NOTCH_RESERVE` above the
 *  rect's top edge, matching how the caller positions this shape's own box
 *  (see `Panel`'s `beakBox` style). Coordinates are used directly as SVG
 *  path commands, so this has no other dependencies. */
export function buildPanelOutlinePath(width, height, beakLeft) {
  const rectTop = NOTCH_RESERVE;
  const rectBottom = NOTCH_RESERVE + height;

  // The glyph the beak tracks sits close to the panel's own left edge by
  // design (handoff: "клюв прижат к левому краю") — close enough, in this
  // panel's actual geometry, that the notch's natural base can fall
  // *inside* the top-left corner's own 12px radius zone (confirmed live:
  // the correct, glyph-derived `beakLeft` came out to 3px, well under the
  // corner radius). An earlier version of this function assumed the notch
  // and both top corners were always cleanly separated and would have
  // needed the *caller* to keep them apart — which meant clamping the beak
  // away from its true position to protect the path, moving it visibly off
  // the glyph (a real regression the captain caught live: "центровки снова
  // нет"). Fixed here instead: each of the two top corners' own radius
  // shrinks just enough to stay clear of the notch actually being placed,
  // so the *notch* always gets its true, glyph-correct position and the
  // corner adapts, not the other way around. Only the top-left/top-right
  // corners can ever be affected (the notch never reaches the bottom ones).
  let leftRadius = PANEL_RADIUS;
  let rightRadius = PANEL_RADIUS;
  if (beakLeft != null) {
    leftRadius = Math.max(0, Math.min(PANEL_RADIUS, beakLeft));
    rightRadius = Math.max(0, Math.min(PANEL_RADIUS, width - (beakLeft + BEAK_BASE_HALF * 2)));
  }

  const segments = [`M ${leftRadius} ${rectTop}`];

  if (beakLeft != null) {
    const baseLeftX = beakLeft;
    const baseRightX = beakLeft + BEAK_BASE_HALF * 2;
    const apexX = beakLeft + BEAK_BASE_HALF;
    const apexY = rectTop - BEAK_HEIGHT;

    // Tip rounding: a quadratic Bezier with its control point *at* the
    // sharp apex pulls the curve toward that point without reaching it —
    // the standard way to round a corner without computing a true arc's
    // tangent geometry by hand. `p1`/`p2` are the points, pulled back
    // BEAK_TIP_ROUND along each edge from the apex, where the curve starts
    // and ends.
    const leftLen = Math.hypot(baseLeftX - apexX, rectTop - apexY);
    const p1x = apexX + ((baseLeftX - apexX) / leftLen) * BEAK_TIP_ROUND;
    const p1y = apexY + ((rectTop - apexY) / leftLen) * BEAK_TIP_ROUND;
    const rightLen = Math.hypot(baseRightX - apexX, rectTop - apexY);
    const p2x = apexX + ((baseRightX - apexX) / rightLen) * BEAK_TIP_ROUND;
    const p2y = apexY + ((rectTop - apexY) / rightLen) * BEAK_TIP_ROUND;

    segments.push(
      `L ${baseLeftX} ${rectTop}`,
      `L ${p1x} ${p1y}`,
      `Q ${apexX} ${apexY} ${p2x} ${p2y}`,
      `L ${baseRightX} ${rectTop}`,
    );
  }

  const r = PANEL_RADIUS; // bottom corners are never affected by the beak
  segments.push(
    `L ${width - rightRadius} ${rectTop}`,
    `A ${rightRadius} ${rightRadius} 0 0 1 ${width} ${rectTop + rightRadius}`,
    `L ${width} ${rectBottom - r}`,
    `A ${r} ${r} 0 0 1 ${width - r} ${rectBottom}`,
    `L ${r} ${rectBottom}`,
    `A ${r} ${r} 0 0 1 0 ${rectBottom - r}`,
    `L 0 ${rectTop + leftRadius}`,
    `A ${leftRadius} ${leftRadius} 0 0 1 ${leftRadius} ${rectTop}`,
    "Z",
  );
  return segments.join(" ");
}

/** The popover shell: a floating macOS vibrancy surface with a top beak, a
 *  header (title + toolbar actions), a scrollable body, and an optional footer.
 *  Depth is a single float — a soft shadow + hairline rim.
 *
 *  The beak is pinned to the panel's left edge, not centered — the panel
 *  opens rightward from the tray icon, and `beakLeft` places the beak under
 *  wherever the glyph actually sits (mirrors the interactive prototype's own
 *  anchor math: `glyphCenterX - panelLeft`, clamped to stay inside the
 *  panel). Docked (attached under the tray) shows the beak; detached hides
 *  it and the header carries a snap-back affordance instead (see App.tsx).
 *
 *  Docked, the beak is fused into the panel's own outline as one shape —
 *  a single fill, a single `backdrop-filter` pass, and one 0.5px border
 *  tracing the whole boundary (rect *and* beak edges, no seam where they
 *  meet) — via one SVG path (`buildPanelOutlinePath`) used both as a
 *  `clip-path` for a background layer and as the stroke itself. Two
 *  translucent layers stacked on top of each other (the beak's old
 *  rotated-square div, painted over the panel body wherever they
 *  overlapped) is exactly what read as a visible seam over a real desktop —
 *  0.86 alpha over 0.86 alpha is not 0.86, and the old beak had no
 *  `backdrop-filter` of its own, so it also never blurred what was behind
 *  it the way the panel did. See AGENTS.md's R3-4 note for the measurements
 *  that caught this and why a two-element approach (even a seamless
 *  butt-join) isn't the fix: backdrop-filter's blur kernel doesn't sample
 *  across a real element boundary, so nothing short of one shape actually
 *  removes the seam.
 *
 *  The header is always grab/grabbing — dragging it is how the panel
 *  detaches (there is no detach button). `onHeaderPointerDown` is wired by
 *  the caller, which owns the drag-vs-click distinction and the actual
 *  window move (native Tauri drag, or a simulated position in the browser
 *  harness — see App.tsx). */
export function Panel({
  title = "Quotos",
  docked = true,
  beakLeft = 24,
  dragging = false,
  onHeaderPointerDown,
  leading = null,
  headerActions = null,
  footer = null,
  maxBodyHeight = 452,
  position = null,
  children,
  style,
}) {
  const detachedFixed = !docked && position ? { position: "fixed", left: position.x, top: position.y, margin: 0 } : null;
  const clipId = useId();

  // The unified beak+rect shape needs the content's *real* rendered height
  // (rows expand/collapse, the body scrolls past a variable number of
  // subscriptions) — there is no way to express "however tall the content
  // turns out to be" as a static SVG path, so this measures it directly
  // rather than guessing or hardcoding a max.
  const contentRef = useRef(null);
  const [contentHeight, setContentHeight] = useState(0);
  useLayoutEffect(() => {
    const el = contentRef.current;
    if (!el) return undefined;
    // Measured synchronously here (not only via the observer's own,
    // inherently-async first callback) so the very first paint already has
    // a real height — `useLayoutEffect` flushes its own state update
    // before the browser paints, so there is no flash of an unstyled,
    // backgroundless panel on open.
    setContentHeight(el.getBoundingClientRect().height);
    const observer = new ResizeObserver((entries) => {
      const next = entries[0]?.contentRect.height;
      if (typeof next === "number") setContentHeight(next);
    });
    observer.observe(el);
    return () => observer.disconnect();
  }, []);

  const showBeak = docked && !detachedFixed;
  const pathD = contentHeight > 0 ? buildPanelOutlinePath(PANEL_WIDTH, contentHeight, showBeak ? beakLeft : null) : null;
  const beakBoxHeight = contentHeight + NOTCH_RESERVE;

  return (
    <div data-quotos-panel="true" style={{ position: "relative", width: "var(--panel-width)", ...detachedFixed, ...style }}>
      {pathD ? (
        <>
          {/* The fill + blur layer — clipped to the exact same path the
              stroke (below, painted after the content so it's never
              partly covered by the content box's own edge) traces, so
              there is nothing for a second translucent layer to double up
              against. The clipPath def itself has no visual footprint of
              its own — just referenced by `clip-path` below. */}
          <svg width="0" height="0" style={{ position: "absolute" }}>
            <defs>
              <clipPath id={clipId}>
                <path d={pathD} />
              </clipPath>
            </defs>
          </svg>
          <div
            style={{
              position: "absolute", top: -NOTCH_RESERVE, left: 0, width: PANEL_WIDTH, height: beakBoxHeight,
              background: "var(--bg-panel)",
              backdropFilter: "var(--blur-vibrancy)",
              WebkitBackdropFilter: "var(--blur-vibrancy)",
              boxShadow: "var(--shadow-popover)",
              clipPath: `url(#${clipId})`,
              WebkitClipPath: `url(#${clipId})`,
            }}
          />
        </>
      ) : null}
      <div
        ref={contentRef}
        style={{
          display: "flex", flexDirection: "column",
          position: "relative",
          borderRadius: "var(--radius-xl)",
          overflow: "hidden",
        }}
      >
        {/* header — always the drag handle; the first movement detaches
            (see App.tsx's pointer handling), so the cursor always reads
            grab/grabbing even while docked. */}
        <div
          onMouseDown={onHeaderPointerDown}
          style={{
            display: "flex", alignItems: "center", gap: "var(--space-1)",
            padding: "var(--space-2-5) var(--space-2) var(--space-2-5) var(--space-3)",
            borderBottom: "0.5px solid var(--border-subtle)",
            cursor: dragging ? "grabbing" : "grab",
            userSelect: "none",
          }}
        >
          {leading}
          <span style={{
            flex: 1, fontFamily: "var(--font-sans)", fontSize: "var(--text-base)",
            fontWeight: "var(--weight-semibold)", color: "var(--text-primary)",
            letterSpacing: "var(--tracking-tight)",
          }}>{title}</span>
          {headerActions}
        </div>
        {/* body — no reserved scrollbar gutter; the track is hidden outright
            (scrollbar-width: none / ::-webkit-scrollbar{width:0} in app.css)
            so there is nothing to reserve space for in the first place. */}
        <div className="quotos-scroll" style={{
          display: "flex", flexDirection: "column", gap: "var(--space-0-5)",
          padding: "var(--space-1-5)",
          maxHeight: maxBodyHeight, overflowY: "auto",
        }}>
          {children}
        </div>
        {/* footer */}
        {footer ? (
          <div style={{
            display: "flex", alignItems: "center", gap: "var(--space-2)",
            padding: "var(--space-1-5) var(--space-2)",
            borderTop: "0.5px solid var(--border-subtle)",
          }}>{footer}</div>
        ) : null}
      </div>
      {pathD ? (
        // The border — one stroke along the same path the fill was clipped
        // to, painted *after* (on top of) the content box so the content's
        // own edge — which sits almost exactly on the rect portion of this
        // same boundary — never covers half its width. Traces the beak's
        // two exposed edges and the rect's corners as a single continuous
        // line, with no seam at the join between them.
        <svg
          aria-hidden="true"
          width={PANEL_WIDTH}
          height={beakBoxHeight}
          viewBox={`0 0 ${PANEL_WIDTH} ${beakBoxHeight}`}
          style={{ position: "absolute", top: -NOTCH_RESERVE, left: 0, overflow: "visible", pointerEvents: "none" }}
        >
          <path d={pathD} fill="none" style={{ stroke: "var(--border-strong)" }} strokeWidth={0.5} />
        </svg>
      ) : null}
    </div>
  );
}
