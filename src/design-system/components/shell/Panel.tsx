import type * as React from "react";
import { useId, useLayoutEffect, useRef, useState } from "react";
import { joinClassNames } from "../../joinClassNames";
import styles from "./Panel.module.css";

// Mirrors --panel-width in tokens/spacing.css, duplicated as a plain number
// because the SVG path math below needs concrete units, not a CSS custom
// property string.
const PANEL_WIDTH = 332;
export const PANEL_RADIUS = 12;

// A plain authored shape. Three things elsewhere are keyed to these numbers
// and must move together, since nothing syncs them automatically:
//   - BEAK_BASE_HALF * 2 must equal BEAK_BASE_WIDTH in
//     src-tauri/src/geometry.rs's docked_layout_in_points, which converts a
//     glyph center into this component's beakLeft.
//   - BEAK_HEIGHT is what that function subtracts to put the beak's tip
//     just under the menu bar rather than the panel's top edge.
//   - NOTCH_RESERVE must be at least BEAK_HEIGHT plus half the 0.5px
//     stroke, and equal app.css's padding-top, which keeps the beak inside
//     the transparent window instead of clipped by its edge.
export const BEAK_BASE_HALF = 10;
export const BEAK_HEIGHT = 10;
const BEAK_TIP_ROUND = 3;
// Vertical space this component reserves above the rounded rect's own top
// edge for the beak to render into.
export const NOTCH_RESERVE = 12;

/** Builds one clockwise SVG path for the panel's full outline: a rounded
 *  rect, with the beak fused into the top edge as a single continuous
 *  boundary when `beakLeft` is given. `width` and `height` are the rounded
 *  rect's own dimensions. The returned path is expressed in a coordinate
 *  space whose y=0 is `NOTCH_RESERVE` above the rect's top edge, matching
 *  how the caller positions this shape's own box. */
export function buildPanelOutlinePath(width: number, height: number, beakLeft: number | null) {
  const rectTop = NOTCH_RESERVE;
  const rectBottom = NOTCH_RESERVE + height;

  // The beak's glyph anchor can sit close enough to the panel's left edge
  // that the notch falls inside a top corner's own radius zone, so each top
  // corner's radius shrinks to stay clear of the notch's actual position.
  // Only the top corners are ever affected.
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

    // A quadratic Bezier with its control point at the sharp apex rounds
    // the tip without computing an arc's tangent geometry directly. p1 and
    // p2 are the curve's start and end points, pulled back BEAK_TIP_ROUND
    // along each edge from the apex.
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

export interface PanelProps {
  /** Popover title. Defaults to "Quotos". */
  title?: string;
  /** Docked under the status item, showing the beak, or torn off into a free-floating window. */
  docked?: boolean;
  /** Left edge in pixels from the panel's own left edge of the beak's 12px-wide base. Tracks the status item glyph's center. */
  beakLeft?: number;
  /** True while the header is being dragged. */
  dragging?: boolean;
  /** Mousedown handler on the header, the drag and detach entry point. There is no detach button. */
  onHeaderPointerDown?: (event: React.MouseEvent) => void;
  /** Optional leading header node, such as the Subscriptions screen's back arrow. */
  leading?: React.ReactNode;
  /** Toolbar nodes on the right of the header. */
  headerActions?: React.ReactNode;
  /** Optional footer content. */
  footer?: React.ReactNode;
  /** Max scroll height of the body before it scrolls. Default 452. */
  maxBodyHeight?: number;
  /** Fixed screen position while detached, for browser-harness dragging. Native Tauri leaves this null: the OS window itself moves. */
  position?: { x: number; y: number } | null;
  /** The subscription rows. */
  children?: React.ReactNode;
  style?: React.CSSProperties;
}

/**
 * The popover shell: a floating macOS vibrancy surface with a top beak, a
 * header, a scrollable body, and an optional footer. Depth is a single
 * float: a soft shadow and hairline rim.
 *
 * The beak is pinned to the panel's left edge, not centered, since the
 * panel opens rightward from the status item and `beakLeft` places the
 * beak under wherever the glyph actually sits. Docked shows the beak.
 * Detached hides it, and the header carries a snap-back affordance
 * instead.
 *
 * Docked, the beak is fused into the panel's own outline as one shape,
 * not two stacked layers: two overlapping translucent fills would double
 * the alpha and show a seam where they meet. One SVG path,
 * `buildPanelOutlinePath`, is used both as a `clip-path` for the
 * background layer and as the border stroke.
 *
 * The header is always grab or grabbing, since dragging it is how the
 * panel detaches. There is no detach button. `onHeaderPointerDown` is
 * wired by the caller, which owns the drag-versus-click distinction and
 * the actual window move.
 */
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
}: PanelProps) {
  const detachedFixed: React.CSSProperties | null =
    !docked && position
      ? { position: "fixed", left: position.x, top: position.y, margin: 0 }
      : null;
  const clipId = useId();

  // The SVG outline needs the content's real rendered height, and there is
  // no way to express that as a static path, so this measures it directly
  // instead of guessing or hardcoding a max.
  const contentRef = useRef<HTMLDivElement>(null);
  const [contentHeight, setContentHeight] = useState(0);
  useLayoutEffect(() => {
    const el = contentRef.current;
    if (!el) return undefined;
    // Measured synchronously here, not only via the observer's async first
    // callback, so the first paint already has a real height.
    // useLayoutEffect flushes its update before the browser paints,
    // avoiding a flash of an unstyled panel.
    setContentHeight(el.getBoundingClientRect().height);
    const observer = new ResizeObserver((entries) => {
      const next = entries[0]?.contentRect.height;
      if (typeof next === "number") setContentHeight(next);
    });
    observer.observe(el);
    return () => observer.disconnect();
  }, []);

  const showBeak = docked && !detachedFixed;
  const pathD =
    contentHeight > 0
      ? buildPanelOutlinePath(PANEL_WIDTH, contentHeight, showBeak ? beakLeft : null)
      : null;
  const beakBoxHeight = contentHeight + NOTCH_RESERVE;

  return (
    <div data-quotos-panel="true" className={styles.root} style={{ ...detachedFixed, ...style }}>
      {pathD ? (
        <>
          {/* The fill layer is clipped to the same path the stroke traces,
              so nothing doubles up visually.

              Deliberately no backdrop-filter here. The window is
              transparent with nothing behind it to blur, so the filter
              only adds flicker with no visual benefit. Never add
              backdrop-filter to any ancestor of the panel body either: a
              non-none value creates a containing block for fixed-position
              descendants, which SubscriptionRow's row menu depends on not
              happening. */}
          <svg width="0" height="0" style={{ position: "absolute" }}>
            <defs>
              <clipPath id={clipId}>
                <path d={pathD} />
              </clipPath>
            </defs>
          </svg>
          <div
            data-quotos-panel-fill="true"
            style={{
              position: "absolute",
              top: -NOTCH_RESERVE,
              left: 0,
              width: PANEL_WIDTH,
              height: beakBoxHeight,
              background: "var(--bg-panel)",
              boxShadow: "var(--shadow-popover)",
              clipPath: `url(#${clipId})`,
              WebkitClipPath: `url(#${clipId})`,
            }}
          />
        </>
      ) : null}
      <div ref={contentRef} className={styles.content}>
        <div
          onMouseDown={onHeaderPointerDown}
          data-dragging={dragging ? "true" : undefined}
          className={styles.header}
        >
          {leading}
          <span className={styles.title}>{title}</span>
          {headerActions}
        </div>
        <div
          className={joinClassNames("quotos-scroll", styles.body)}
          style={{ maxHeight: maxBodyHeight }}
        >
          {children}
        </div>
        {footer ? <div className={styles.footer}>{footer}</div> : null}
      </div>
      {pathD ? (
        // Painted after the content box, so the content's own edge does
        // not cover half the stroke's width. One continuous line traces
        // both the beak's exposed edges and the rect's corners, with no
        // seam at the join.
        <svg
          aria-hidden="true"
          width={PANEL_WIDTH}
          height={beakBoxHeight}
          viewBox={`0 0 ${PANEL_WIDTH} ${beakBoxHeight}`}
          style={{ top: -NOTCH_RESERVE }}
          className={styles.strokeLayer}
        >
          <path d={pathD} fill="none" className={styles.strokePath} strokeWidth={0.5} />
        </svg>
      ) : null}
    </div>
  );
}
