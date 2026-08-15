import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { SubscriptionRow } from "./SubscriptionRow.jsx";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

/** The panel's own window is 360x560 logical; jsdom's default viewport is not,
 *  and the flip-above branch only makes sense against a real one. */
function useWindow(width, height) {
  Object.defineProperty(window, "innerWidth", { value: width, configurable: true });
  Object.defineProperty(window, "innerHeight", { value: height, configurable: true });
}

/** jsdom reports every rect as zero, so the "…" button's position — the only
 *  input the placement math has — has to be supplied. Everything else keeps
 *  jsdom's zeros, including the menu's own measured size, which is why these
 *  tests assert the *anchoring* rules rather than exact pixel offsets. */
function anchorButtonAt({ top, bottom, right }) {
  const original = Element.prototype.getBoundingClientRect;
  vi.spyOn(Element.prototype, "getBoundingClientRect").mockImplementation(function mocked() {
    if (this.getAttribute?.("aria-label") === "More") {
      return { top, bottom, right, left: right - 20, width: 20, height: bottom - top, x: right - 20, y: top };
    }
    return original.call(this);
  });
}

function openMenu() {
  return render(<SubscriptionRow label="Claude Team" provider="Anthropic" state="working" used={40} menuOpen />);
}

// R4-5: the captain's 2026-08-15 screencast — opening the bottom row's "…"
// menu drew it inside the panel body's `overflow-y: auto` box, so "Stop
// tracking" was cut in half by the panel's bottom edge and the two-row panel
// suddenly scrolled. Both consequences come from one property: an absolutely
// positioned element is clipped by, and counts toward the scroll extent of, its
// scroll-container ancestor. A fixed one is neither.
describe("SubscriptionRow's row menu overlays the panel instead of living inside its scroll box (R4-5)", () => {
  beforeEach(() => useWindow(360, 560));

  it("is positioned against the viewport, not against the scrolled row", () => {
    anchorButtonAt({ top: 100, bottom: 120, right: 320 });
    openMenu();
    const menu = screen.getByText("Stop tracking").closest("[data-quotos-menu-scope]");
    expect(menu.style.position).toBe("fixed");
  });

  it("hangs below the '…' button it belongs to", () => {
    anchorButtonAt({ top: 100, bottom: 120, right: 320 });
    openMenu();
    const menu = screen.getByText("Stop tracking").closest("[data-quotos-menu-scope]");
    // Below the button's bottom edge, by a small gap.
    expect(parseFloat(menu.style.top)).toBeGreaterThanOrEqual(120);
    expect(parseFloat(menu.style.top)).toBeLessThan(130);
  });

  it("flips above the button rather than off the bottom of the window", () => {
    // A row near the panel's bottom, which is exactly where the clipping was
    // visible: the menu cannot open downward and stay inside a 560px window.
    anchorButtonAt({ top: 520, bottom: 540, right: 320 });
    vi.spyOn(HTMLElement.prototype, "offsetHeight", "get").mockReturnValue(122);
    vi.spyOn(HTMLElement.prototype, "offsetWidth", "get").mockReturnValue(178);
    openMenu();
    const menu = screen.getByText("Stop tracking").closest("[data-quotos-menu-scope]");
    const top = parseFloat(menu.style.top);
    expect(top).toBeLessThan(520); // above the button
    expect(top).toBeGreaterThanOrEqual(8); // and still inside the window
    expect(top + 122).toBeLessThanOrEqual(560);
  });

  it("stays inside the window horizontally", () => {
    anchorButtonAt({ top: 100, bottom: 120, right: 320 });
    vi.spyOn(HTMLElement.prototype, "offsetWidth", "get").mockReturnValue(178);
    openMenu();
    const menu = screen.getByText("Stop tracking").closest("[data-quotos-menu-scope]");
    const left = parseFloat(menu.style.left);
    expect(left).toBeGreaterThanOrEqual(8);
    expect(left + 178).toBeLessThanOrEqual(360 - 8);
  });

  // App.tsx's click-away handler recognises "inside the menu" by this
  // attribute alone (`closest("[data-quotos-menu-scope]")`), on both the
  // dropdown and its trigger — moving the dropdown out of the row's own box
  // must not cost it that.
  it("keeps the menu-scope marker the click-away handler matches on", () => {
    anchorButtonAt({ top: 100, bottom: 120, right: 320 });
    openMenu();
    expect(screen.getByText("Stop tracking").closest("[data-quotos-menu-scope]")).not.toBeNull();
    expect(screen.getByLabelText("More").getAttribute("data-quotos-menu-scope")).toBe("true");
  });

  it("the trigger reports the menu's open state via aria-expanded", () => {
    const { rerender } = render(<SubscriptionRow label="Claude Team" state="working" used={40} />);
    expect(screen.getByLabelText("More").getAttribute("aria-expanded")).toBe("false");
    rerender(<SubscriptionRow label="Claude Team" state="working" used={40} menuOpen />);
    expect(screen.getByLabelText("More").getAttribute("aria-expanded")).toBe("true");
  });
});

// R6: expanding a row used to be pointer-only — the row div's onClick was the
// sole expand path, and the "N limits" affordance it pointed at was an inert
// span, unreachable by keyboard and invisible to the accessibility tree.
describe("the 'N limits' disclosure is a real, focusable control (R6)", () => {
  const windows = [
    { id: "w1", label: "Session", used: 40 },
    { id: "w2", label: "Weekly", used: 10 },
  ];

  it("is a button carrying aria-expanded", () => {
    const { rerender } = render(
      <SubscriptionRow label="Claude Max" state="working" used={40} windows={windows} />,
    );
    const disclosure = screen.getByRole("button", { name: /2 limits/ });
    expect(disclosure.getAttribute("aria-expanded")).toBe("false");
    rerender(
      <SubscriptionRow label="Claude Max" state="working" used={40} windows={windows} expanded />,
    );
    expect(disclosure.getAttribute("aria-expanded")).toBe("true");
  });

  it("toggles expansion itself, without the row's own click undoing it", () => {
    const onToggleExpand = vi.fn();
    render(
      <SubscriptionRow label="Claude Max" state="working" used={40} windows={windows}
        onToggleExpand={onToggleExpand} />,
    );
    screen.getByRole("button", { name: /2 limits/ }).click();
    // Exactly once: the button's stopPropagation must keep the row div's
    // onClick (the pointer expand path) from firing a second toggle.
    expect(onToggleExpand).toHaveBeenCalledTimes(1);
  });

  it("does not render at all without windows", () => {
    render(<SubscriptionRow label="Claude Max" state="working" used={40} windows={[]} />);
    expect(screen.queryByRole("button", { name: /limits?/ })).toBeNull();
  });
});
