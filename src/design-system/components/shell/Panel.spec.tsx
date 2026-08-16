import { cleanup, render } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";
import {
  BEAK_BASE_HALF,
  BEAK_HEIGHT,
  buildPanelOutlinePath,
  NOTCH_RESERVE,
  PANEL_RADIUS,
  Panel,
  type PanelProps,
} from "./Panel";
import panelStylesheet from "./Panel.module.css?raw";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

// jsdom has no real layout engine, getBoundingClientRect always reports 0,
// and it does not implement ResizeObserver at all, so both are stubbed for
// the render-based describe block below. The pure buildPanelOutlinePath
// tests above need neither.
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

// These tests assert the path geometry directly, since jsdom has no
// renderer to compare against a screenshot.
describe("buildPanelOutlinePath", () => {
  test("returns one single path (one M...Z run) whether or not a beak is present", () => {
    const withBeak = buildPanelOutlinePath(332, 500, 24);
    const withoutBeak = buildPanelOutlinePath(332, 500, null);
    for (const d of [withBeak, withoutBeak]) {
      expect(d.match(/M /g)).toHaveLength(1);
      expect(d.trim().endsWith("Z")).toBe(true);
    }
  });

  test("the notch's base spans exactly BEAK_BASE_HALF * 2, anchored at beakLeft", () => {
    const beakLeft = 24;
    const d = buildPanelOutlinePath(332, 500, beakLeft);
    // The base-left point sits at x = beakLeft on the rect's top edge, and
    // the base-right point is the full base width further along that edge.
    expect(d).toContain(`L ${beakLeft} ${NOTCH_RESERVE}`);
    expect(d).toContain(`L ${beakLeft + BEAK_BASE_HALF * 2} ${NOTCH_RESERVE}`);
  });

  test("reserves enough headroom above the rect for the beak it draws", () => {
    expect(NOTCH_RESERVE).toBeGreaterThanOrEqual(BEAK_HEIGHT + 1);
    const d = buildPanelOutlinePath(332, 500, 20);
    const yValues = [...d.matchAll(/(?:^|[A-Z]\s*)(-?\d+(?:\.\d+)?)\s+(-?\d+(?:\.\d+)?)/g)].map(
      (m) => Number(m[2]),
    );
    // y = 0 is the transparent window's own top edge once app.css's
    // padding-top matches NOTCH_RESERVE, so nothing may reach above it.
    expect(Math.min(...yValues)).toBeGreaterThanOrEqual(0);
    expect(Math.min(...yValues)).toBeCloseTo(NOTCH_RESERVE - BEAK_HEIGHT, 5);
  });

  test("keeps the beak noticeably larger than a 12px minimum", () => {
    expect(BEAK_BASE_HALF * 2).toBeGreaterThan(12);
    expect(BEAK_HEIGHT).toBeGreaterThan(7);
  });

  test("omits the beak segment entirely when beakLeft is null", () => {
    const d = buildPanelOutlinePath(332, 500, null);
    const yValues = [...d.matchAll(/(?:^|[A-Z]\s*)(-?\d+(?:\.\d+)?)\s+(-?\d+(?:\.\d+)?)/g)].map(
      (m) => Number(m[2]),
    );
    expect(Math.min(...yValues)).toBeGreaterThanOrEqual(NOTCH_RESERVE - 0.01);
  });

  test("the apex sits centered over the notch's own base", () => {
    const beakLeft = 40;
    const d = buildPanelOutlinePath(332, 500, beakLeft);
    const apexX = beakLeft + BEAK_BASE_HALF;
    expect(d).toMatch(new RegExp(`Q ${apexX} -?\\d+(?:\\.\\d+)? `));
  });

  test("shrinks the top-left corner radius to fit a notch placed inside test, rather than requiring the notch to move", () => {
    const beakLeft = 3; // Inside PANEL_RADIUS, the common case.
    const d = buildPanelOutlinePath(332, 500, beakLeft);
    expect(d.startsWith(`M ${beakLeft} ${NOTCH_RESERVE}`)).toBe(true);
    expect(d).toContain(`A ${beakLeft} ${beakLeft} 0 0 1 ${beakLeft} ${NOTCH_RESERVE}`);
    expect(d).toContain(`L ${beakLeft} ${NOTCH_RESERVE}`);
  });

  test("keeps the full 12px corner radius when the notch is nowhere near it", () => {
    const d = buildPanelOutlinePath(332, 500, 100);
    expect(d.startsWith(`M ${PANEL_RADIUS} ${NOTCH_RESERVE}`)).toBe(true);
  });
});

// The rendered component: confirms there is exactly one filled layer and
// one stroked border layer, tracing the same path, so they can never
// drift apart.
describe("Panel's docked beak rendering", () => {
  test("clips a single background layer to the same path the border strokes", () => {
    const { container } = render(
      <Panel docked beakLeft={24}>
        <div>content</div>
      </Panel>,
    );
    const clipPath = container.querySelector("clipPath path");
    const strokePath =
      container.querySelector("path[fill='none']") ?? container.querySelector("path[fill=none]");
    expect(clipPath).toBeTruthy();
    expect(strokePath).toBeTruthy();
    expect(clipPath!.getAttribute("d")).toBe(strokePath!.getAttribute("d"));
  });

  test("there is exactly one element carrying a background fill (no second translucent layer)", () => {
    const { container } = render(
      <Panel docked beakLeft={24}>
        <div>content</div>
      </Panel>,
    );
    const filled = Array.from(container.querySelectorAll("div")).filter(
      (el) =>
        el.style.background && el.style.background !== "" && el.style.background !== "transparent",
    );
    expect(filled).toHaveLength(1);
  });

  // The flat-versus-see-through outcome lives in a real WKWebView on a
  // transparent NSWindow and cannot be observed in jsdom at all. What is
  // testable, and what is pinned here, is the DOM contract that decides
  // it: the fill layer must ship with no backdrop filter under either
  // vendor spelling, docked or detached.
  test.each<[string, Pick<PanelProps, "docked" | "beakLeft">]>([
    ["docked", { docked: true, beakLeft: 24 }],
    ["detached", { docked: false }],
  ])("ships the fill layer with no backdrop filter (%s)", (_label, props) => {
    const { container } = render(
      <Panel {...props}>
        <div>content</div>
      </Panel>,
    );
    const filled = Array.from(container.querySelectorAll("div")).filter(
      (el) =>
        el.style.background && el.style.background !== "" && el.style.background !== "transparent",
    );
    expect(filled).toHaveLength(1);
    expect(filled[0].style.backdropFilter).toBe("");
    expect(filled[0].style.getPropertyValue("-webkit-backdrop-filter")).toBe("");
    expect(filled[0].getAttribute("style")).not.toMatch(/backdrop-filter/i);
  });

  // A non-none backdrop-filter anywhere in the subtree makes that element a
  // containing block for fixed-position descendants, which is exactly what
  // SubscriptionRow's fixed "…" dropdown relies on no ancestor doing.
  test("no element anywhere in the panel carries a backdrop filter", () => {
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

  // The fill layer's own background and box-shadow stay inline, since they
  // ride along with its measured, per-render geometry, but the rest of the
  // panel's chrome, header, body and footer, moved to Panel.module.css.
  // This guards that stylesheet the same way the inline scan above guards
  // the fill layer, so the invariant holds regardless of which layer a
  // future change touches.
  test("Panel.module.css never declares a backdrop filter", () => {
    expect(panelStylesheet).not.toMatch(/backdrop-filter/i);
  });

  test("detached (no beak) still renders the same single-layer shape", () => {
    const { container } = render(
      <Panel docked={false}>
        <div>content</div>
      </Panel>,
    );
    const filled = Array.from(container.querySelectorAll("div")).filter(
      (el) =>
        el.style.background && el.style.background !== "" && el.style.background !== "transparent",
    );
    expect(filled).toHaveLength(1);
  });
});
