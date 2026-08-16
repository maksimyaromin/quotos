import { cleanup, render } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { BEAK_BASE_HALF, BEAK_HEIGHT, NOTCH_RESERVE, PANEL_RADIUS, Panel, buildPanelOutlinePath } from "./Panel.jsx";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

// jsdom has no real layout engine (`getBoundingClientRect` always reports
// 0) and doesn't implement `ResizeObserver` at all — both are needed by
// Panel's own height-measurement effect (see its `useLayoutEffect`) to
// pick a non-zero content height, without which it never builds a path at
// all. Stubbed only for the render-based describe block below; the pure
// `buildPanelOutlinePath` tests above don't touch either.
const STUB_CONTENT_HEIGHT = 500;
beforeEach(() => {
  vi.stubGlobal(
    "ResizeObserver",
    class {
      observe() {}
      disconnect() {}
    },
  );
  vi.spyOn(Element.prototype, "getBoundingClientRect").mockReturnValue({
    width: 332,
    height: STUB_CONTENT_HEIGHT,
    top: 0,
    left: 0,
    right: 332,
    bottom: STUB_CONTENT_HEIGHT,
    x: 0,
    y: 0,
    toJSON() {},
  });
});

// R3-4 regression guard: the beak used to be a second, separately-painted
// element — its own translucent fill with no `backdrop-filter` of its own,
// stacked on top of the panel wherever the two overlapped. Over a real
// (bright) desktop that read as a visible seam: 0.86 alpha over 0.86 alpha
// is darker than either alone, and the beak's own unblurred fill sat next
// to the panel's blurred one — two different materials. The fix fuses both
// into one SVG path used both as a single element's `clip-path` (one fill,
// one blur pass) and as a border stroke (one line tracing the whole
// outline, beak edges included, with no seam at the join). These tests
// guard the path geometry itself, since that's what a screenshot can't
// assert precisely.
describe("buildPanelOutlinePath", () => {
  it("returns one single path (one M...Z run) whether or not a beak is present", () => {
    const withBeak = buildPanelOutlinePath(332, 500, 24);
    const withoutBeak = buildPanelOutlinePath(332, 500, null);
    for (const d of [withBeak, withoutBeak]) {
      expect(d.match(/M /g)).toHaveLength(1);
      expect(d.trim().endsWith("Z")).toBe(true);
    }
  });

  it("the notch's base spans exactly BEAK_BASE_HALF * 2, anchored at beakLeft", () => {
    const beakLeft = 24;
    const d = buildPanelOutlinePath(332, 500, beakLeft);
    // The base-left point sits directly on the rect's top edge (y =
    // NOTCH_RESERVE) at x = beakLeft; the base-right point is the full base
    // width further along the same edge. Both must appear as literal line-to
    // coordinates for the notch to actually be that wide.
    expect(d).toContain(`L ${beakLeft} ${NOTCH_RESERVE}`);
    expect(d).toContain(`L ${beakLeft + BEAK_BASE_HALF * 2} ${NOTCH_RESERVE}`);
  });

  // R3-10: the beak's size is the captain's own call, overriding the
  // handoff's 12×12 ("он маленький"), and three other places are keyed to
  // these numbers with no build step to sync them — geometry.rs's BEAK_BASE_WIDTH
  // and BEAK_HEIGHT, and app.css's `padding-top`. This pins the contract that
  // matters most: the reserve must actually contain the beak, or the window's
  // own top edge clips its tip.
  it("reserves enough headroom above the rect for the beak it draws", () => {
    expect(NOTCH_RESERVE).toBeGreaterThanOrEqual(BEAK_HEIGHT + 1);
    const d = buildPanelOutlinePath(332, 500, 20);
    const yValues = [...d.matchAll(/(?:^|[A-Z]\s*)(-?\d+(?:\.\d+)?)\s+(-?\d+(?:\.\d+)?)/g)].map((m) => Number(m[2]));
    // Nothing may reach above y = 0, which is the transparent window's own
    // top edge once app.css's padding-top matches NOTCH_RESERVE.
    expect(Math.min(...yValues)).toBeGreaterThanOrEqual(0);
    // ...and the tip really is BEAK_HEIGHT above the rect's top edge.
    expect(Math.min(...yValues)).toBeCloseTo(NOTCH_RESERVE - BEAK_HEIGHT, 5);
  });

  it("is noticeably bigger than the superseded 12x12 handoff figure", () => {
    expect(BEAK_BASE_HALF * 2).toBeGreaterThan(12);
    expect(BEAK_HEIGHT).toBeGreaterThan(7);
  });

  it("omits the beak segment entirely when beakLeft is null (detached)", () => {
    const d = buildPanelOutlinePath(332, 500, null);
    // No line segment should touch a y coordinate above the rect's own top
    // edge — the whole path stays within [NOTCH_RESERVE, NOTCH_RESERVE+height].
    const yValues = [...d.matchAll(/(?:^|[A-Z]\s*)(-?\d+(?:\.\d+)?)\s+(-?\d+(?:\.\d+)?)/g)].map((m) => Number(m[2]));
    expect(Math.min(...yValues)).toBeGreaterThanOrEqual(NOTCH_RESERVE - 0.01);
  });

  it("the apex sits centered over the notch's own base (beakLeft + half-width)", () => {
    const beakLeft = 40;
    const d = buildPanelOutlinePath(332, 500, beakLeft);
    const apexX = beakLeft + BEAK_BASE_HALF;
    // The quadratic curve command's control point is the sharp (pre-rounding)
    // apex itself — assert it's present at the expected x.
    expect(d).toMatch(new RegExp(`Q ${apexX} -?\\d+(?:\\.\\d+)? `));
  });

  // R3-6 regression guard: the glyph the beak tracks sits close enough to
  // the panel's own left edge, in this panel's real geometry, that the
  // glyph-correct `beakLeft` comes out *inside* the top-left corner's own
  // 12px radius (confirmed live: routinely as small as 3px). The path must
  // shrink that corner's own radius to fit the notch's true position, not
  // require the caller to push the notch away from its correct place to
  // protect the path — the latter is exactly the regression the captain
  // caught live ("центровки снова нет"): a native-side clamp did that, and
  // it visibly moved the beak off the glyph on every normal open.
  it("shrinks the top-left corner radius to fit a notch placed inside it, rather than requiring the notch to move", () => {
    const beakLeft = 3; // inside PANEL_RADIUS (12) — the real, common case
    const d = buildPanelOutlinePath(332, 500, beakLeft);
    // The path must still start (and the top-left arc must still land) at
    // x = beakLeft exactly — a shrunk radius, not a relocated notch.
    expect(d.startsWith(`M ${beakLeft} ${NOTCH_RESERVE}`)).toBe(true);
    expect(d).toContain(`A ${beakLeft} ${beakLeft} 0 0 1 ${beakLeft} ${NOTCH_RESERVE}`);
    // And the notch base itself is untouched — still exactly at beakLeft,
    // not clamped away from the corner.
    expect(d).toContain(`L ${beakLeft} ${NOTCH_RESERVE}`);
  });

  it("keeps the full 12px corner radius when the notch is nowhere near it", () => {
    const d = buildPanelOutlinePath(332, 500, 100);
    expect(d.startsWith(`M ${PANEL_RADIUS} ${NOTCH_RESERVE}`)).toBe(true);
  });
});

// The rendered component: confirms there is exactly one filled/blurred
// layer and one stroked border layer, not two independently-painted
// translucent shapes (the actual bug), and that the border traces the
// *same* path the fill was clipped to (so they can never drift apart).
describe("Panel's docked beak rendering", () => {
  it("clips a single background layer to the same path the border strokes", () => {
    const { container } = render(<Panel docked beakLeft={24}><div>content</div></Panel>);
    const clipPath = container.querySelector("clipPath path");
    const strokePath = container.querySelector("path[fill='none']") ?? container.querySelector("path[fill=none]");
    expect(clipPath).toBeTruthy();
    expect(strokePath).toBeTruthy();
    expect(clipPath.getAttribute("d")).toBe(strokePath.getAttribute("d"));
  });

  it("there is exactly one element carrying a background fill (no second translucent layer)", () => {
    const { container } = render(<Panel docked beakLeft={24}><div>content</div></Panel>);
    const filled = Array.from(container.querySelectorAll("div")).filter(
      (el) => el.style.background && el.style.background !== "" && el.style.background !== "transparent",
    );
    expect(filled).toHaveLength(1);
  });

  // W1. This assertion used to be its exact opposite — it required the fill
  // layer to carry a non-empty `backdropFilter` — and it is inverted here on
  // purpose, not deleted. The captain reported the panel's surface
  // intermittently going flat instead of staying see-through ("фон должен
  // оставаться прозрачным всегда а не мерцать вот так"), and the flat state
  // is what `backdrop-filter: saturate(180%) blur(28px)` produces when its
  // pass actually runs against the content behind this transparent window:
  // a 28px blur of a text-bearing backdrop is a flat wash. The see-through
  // state — the one the design intends and the one his good frames show — is
  // a plain alpha composite of --bg-panel over whatever is behind, with the
  // backdrop's own detail arriving *sharp*, i.e. with no blur contribution
  // whatsoever. So the filter was never part of the intended look; it was
  // only ever the flicker. See Panel.jsx's fill-layer comment for the
  // measurements.
  //
  // What this test can and cannot hold: the flat-vs-see-through outcome
  // lives in a real WKWebView on a `transparent: true` NSWindow and cannot
  // be observed in jsdom at all (no layout, no compositor, no window). What
  // *is* testable, and what is pinned here, is the DOM/style contract that
  // decides it — the fill layer must ship with no backdrop filter under
  // either vendor spelling, docked or detached. If someone reintroduces one,
  // this fails; the on-screen behaviour it stands in for was verified
  // separately by burst-capturing the live panel (390 frames blurred with
  // the filter, 200 frames sharp without it) — see RESULT.md.
  it.each([
    ["docked", { docked: true, beakLeft: 24 }],
    ["detached", { docked: false }],
  ])("ships the fill layer with no backdrop filter (%s)", (_label, props) => {
    const { container } = render(
      <Panel {...props}>
        <div>content</div>
      </Panel>,
    );
    const filled = Array.from(container.querySelectorAll("div")).filter(
      (el) => el.style.background && el.style.background !== "" && el.style.background !== "transparent",
    );
    expect(filled).toHaveLength(1);
    expect(filled[0].style.backdropFilter).toBe("");
    expect(filled[0].style.webkitBackdropFilter ?? "").toBe("");
    expect(filled[0].getAttribute("style")).not.toMatch(/backdrop-filter/i);
  });

  // The same rule one level up: nothing in the panel's own subtree may carry
  // a backdrop filter either. Besides bringing the flicker back, a non-none
  // `backdrop-filter` makes an element a containing block for fixed-position
  // descendants, which is exactly what SubscriptionRow's fixed "…" dropdown
  // relies on no ancestor doing (R4-5).
  it("no element anywhere in the panel carries a backdrop filter", () => {
    const { container } = render(
      <Panel docked beakLeft={24}>
        <div>content</div>
      </Panel>,
    );
    const offenders = Array.from(container.querySelectorAll("[style]")).filter((el) =>
      /backdrop-filter/i.test(el.getAttribute("style") ?? ""),
    );
    expect(offenders).toHaveLength(0);
  });

  it("detached (no beak) still renders the same single-layer shape", () => {
    const { container } = render(
      <Panel docked={false}>
        <div>content</div>
      </Panel>,
    );
    const filled = Array.from(container.querySelectorAll("div")).filter(
      (el) => el.style.background && el.style.background !== "" && el.style.background !== "transparent",
    );
    expect(filled).toHaveLength(1);
  });
});
