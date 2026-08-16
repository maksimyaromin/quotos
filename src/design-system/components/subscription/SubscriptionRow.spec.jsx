import { cleanup, fireEvent, render, screen } from "@testing-library/react";
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
      return {
        top,
        bottom,
        right,
        left: right - 20,
        width: 20,
        height: bottom - top,
        x: right - 20,
        y: top,
      };
    }
    return original.call(this);
  });
}

function openMenu() {
  return render(
    <SubscriptionRow label="Claude Team" provider="Anthropic" state="working" used={40} menuOpen />,
  );
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
      <SubscriptionRow
        label="Claude Max"
        state="working"
        used={40}
        windows={windows}
        onToggleExpand={onToggleExpand}
      />,
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

// v5: "Move up"/"Move down" reorder the panel's rows (and with them the
// tray's digit order). The menu keeps its one fixed set of items — an edge
// row's impossible direction renders disabled, macOS-style, never hidden.
describe("the row menu's Move up / Move down (v5)", () => {
  it("fires the move callback and closes the menu, once each", () => {
    const onMoveDown = vi.fn();
    const onToggleMenu = vi.fn();
    render(
      <SubscriptionRow
        label="Claude Max"
        state="working"
        used={40}
        menuOpen
        canMoveUp
        canMoveDown
        onMoveDown={onMoveDown}
        onToggleMenu={onToggleMenu}
      />,
    );
    screen.getByText("Move down").click();
    expect(onMoveDown).toHaveBeenCalledTimes(1);
    expect(onToggleMenu).toHaveBeenCalledTimes(1);
  });

  it("renders an impossible direction disabled, and clicking it does nothing", () => {
    const onMoveUp = vi.fn();
    const onToggleMenu = vi.fn();
    render(
      <SubscriptionRow
        label="Claude Max"
        state="working"
        used={40}
        menuOpen
        canMoveDown
        onMoveUp={onMoveUp}
        onToggleMenu={onToggleMenu}
      />,
    );
    const moveUp = screen.getByText("Move up");
    expect(moveUp.disabled).toBe(true);
    expect(screen.getByText("Move down").disabled).toBe(false);
    moveUp.click();
    expect(onMoveUp).not.toHaveBeenCalled();
    expect(onToggleMenu).not.toHaveBeenCalled();
  });

  it("keeps both items in the menu even when neither direction is possible", () => {
    render(<SubscriptionRow label="Claude Max" state="working" used={40} menuOpen />);
    expect(screen.getByText("Move up").disabled).toBe(true);
    expect(screen.getByText("Move down").disabled).toBe(true);
  });
});

// v6: before this, the open menu's items were reachable by keyboard only by
// tabbing through the row's other controls in between, arrows did nothing,
// and closing the menu (Escape, or activating an item) unmounted the focused
// button — dropping focus to <body> and stranding a keyboard user mid-panel.
describe("the row menu is keyboard-operable (v6)", () => {
  // Keydowns bubble from wherever focus is to the row div's own handler, so
  // firing on the trigger models "Enter opened the menu, focus still on the
  // '…' button".
  const arrow = (key) =>
    fireEvent.keyDown(
      document.activeElement === document.body
        ? screen.getByLabelText("More")
        : document.activeElement,
      { key },
    );

  it("ArrowDown walks the items top-to-bottom and wraps past the end", () => {
    render(
      <SubscriptionRow
        label="Claude Max"
        state="working"
        used={40}
        menuOpen
        canMoveUp
        canMoveDown
      />,
    );
    const expected = [
      "Read now",
      "Rename",
      "Show in menu bar",
      "Move up",
      "Move down",
      "Stop tracking",
    ];
    for (const label of expected) {
      arrow("ArrowDown");
      expect(document.activeElement.textContent).toBe(label);
    }
    arrow("ArrowDown");
    expect(document.activeElement.textContent).toBe("Read now");
  });

  it("ArrowUp enters at the last item and walks backwards", () => {
    render(
      <SubscriptionRow
        label="Claude Max"
        state="working"
        used={40}
        menuOpen
        canMoveUp
        canMoveDown
      />,
    );
    arrow("ArrowUp");
    expect(document.activeElement.textContent).toBe("Stop tracking");
    arrow("ArrowUp");
    expect(document.activeElement.textContent).toBe("Move down");
  });

  it("skips disabled items, exactly as the pointer path does", () => {
    render(<SubscriptionRow label="Claude Max" state="working" used={40} menuOpen canMoveDown />);
    arrow("ArrowDown"); // Read now
    arrow("ArrowDown"); // Rename
    arrow("ArrowDown"); // Show in menu bar
    arrow("ArrowDown"); // Move up is disabled — lands on Move down
    expect(document.activeElement.textContent).toBe("Move down");
  });

  it("Home and End jump to the edges", () => {
    render(
      <SubscriptionRow
        label="Claude Max"
        state="working"
        used={40}
        menuOpen
        canMoveUp
        canMoveDown
      />,
    );
    arrow("End");
    expect(document.activeElement.textContent).toBe("Stop tracking");
    arrow("Home");
    expect(document.activeElement.textContent).toBe("Read now");
  });

  it("leaves the arrow keys alone while the menu is closed", () => {
    render(<SubscriptionRow label="Claude Max" state="working" used={40} />);
    fireEvent.keyDown(screen.getByLabelText("More"), { key: "ArrowDown" });
    expect(document.activeElement).toBe(document.body);
  });

  it("leaves the sign-in code field's caret alone even with the menu open", () => {
    render(<SubscriptionRow label="Claude Max" state="broken" menuOpen signInInProgress />);
    const code = screen.getByPlaceholderText("Paste code");
    code.focus();
    fireEvent.keyDown(code, { key: "ArrowDown" });
    expect(document.activeElement).toBe(code);
  });

  it("hands focus back to the trigger when closing unmounts the focused item", () => {
    const { rerender } = render(
      <SubscriptionRow label="Claude Max" state="working" used={40} menuOpen />,
    );
    arrow("ArrowDown");
    expect(document.activeElement.textContent).toBe("Read now");
    rerender(<SubscriptionRow label="Claude Max" state="working" used={40} />);
    expect(document.activeElement).toBe(screen.getByLabelText("More"));
  });

  it("does not steal focus when the close left it somewhere real", () => {
    const windows = [{ id: "w1", label: "Session", used: 40 }];
    const { rerender } = render(
      <SubscriptionRow label="Claude Max" state="working" used={40} windows={windows} menuOpen />,
    );
    const disclosure = screen.getByRole("button", { name: /1 limit/ });
    disclosure.focus();
    rerender(<SubscriptionRow label="Claude Max" state="working" used={40} windows={windows} />);
    expect(document.activeElement).toBe(disclosure);
  });
});

// v7: the "…" dropdown carries the standard WAI-ARIA menu semantics. Before
// this, a screen reader saw "More, button, expanded" and then six unrelated
// buttons in the document — nothing announced that a menu had opened, how many
// items it holds, or where it ends. The keyboard behavior (arrows wrap,
// disabled items skipped — v6) already matched the ARIA menu pattern; these
// roles make the markup say what the interaction already does.
describe("the row menu exposes WAI-ARIA menu semantics (v7)", () => {
  it("the trigger declares it opens a menu", () => {
    render(<SubscriptionRow label="Claude Max" state="working" used={40} />);
    expect(screen.getByLabelText("More").getAttribute("aria-haspopup")).toBe("menu");
  });

  it("the open dropdown is a named menu", () => {
    render(<SubscriptionRow label="Claude Max" state="working" used={40} menuOpen />);
    const menu = screen.getByRole("menu");
    expect(menu.getAttribute("aria-label")).toBe("Subscription actions");
    expect(menu.getAttribute("data-quotos-menu-scope")).toBe("true");
  });

  it("every action is a menuitem, in the menu's one fixed order", () => {
    render(<SubscriptionRow label="Claude Max" state="working" used={40} menuOpen />);
    const items = screen.getAllByRole("menuitem");
    expect(items.map((b) => b.textContent)).toEqual([
      "Read now",
      "Rename",
      "Show in menu bar",
      "Move up",
      "Move down",
      "Stop tracking",
    ]);
  });

  it("the divider before Stop tracking is a separator", () => {
    render(<SubscriptionRow label="Claude Max" state="working" used={40} menuOpen />);
    expect(screen.getByRole("separator")).toBeTruthy();
  });
});

// F1: the rename field shows the *composed* label — the custom override when
// one exists — and commit used to send null ("clear the custom name") whenever
// the draft equaled it. So confirming without editing, or just opening Rename
// and clicking away (blur commits), silently deleted an existing custom name.
// Unchanged must be a no-op; only an explicitly emptied field clears.
describe("committing a rename without editing keeps an existing custom name (F1)", () => {
  function openRenameField(onRename) {
    render(
      <SubscriptionRow
        label="My Max"
        provider="Anthropic"
        state="working"
        used={40}
        menuOpen
        onRename={onRename}
      />,
    );
    fireEvent.click(screen.getByText("Rename"));
    return screen.getByRole("textbox");
  }

  it("Enter with an untouched draft is a no-op, not a clear", () => {
    const onRename = vi.fn();
    const input = openRenameField(onRename);
    expect(input.value).toBe("My Max");
    fireEvent.keyDown(input, { key: "Enter" });
    expect(onRename).not.toHaveBeenCalled();
    expect(screen.queryByRole("textbox")).toBeNull();
  });

  it("clicking away (blur) with an untouched draft is a no-op too", () => {
    const onRename = vi.fn();
    const input = openRenameField(onRename);
    fireEvent.blur(input);
    expect(onRename).not.toHaveBeenCalled();
  });

  it("an edited draft commits the trimmed new name", () => {
    const onRename = vi.fn();
    const input = openRenameField(onRename);
    fireEvent.change(input, { target: { value: "  Team account " } });
    fireEvent.keyDown(input, { key: "Enter" });
    expect(onRename).toHaveBeenCalledWith("Team account");
  });

  it("an explicitly emptied field clears the custom name", () => {
    const onRename = vi.fn();
    const input = openRenameField(onRename);
    fireEvent.change(input, { target: { value: "   " } });
    fireEvent.keyDown(input, { key: "Enter" });
    expect(onRename).toHaveBeenCalledWith(null);
  });
});

// F2: the collapsed detail area is always mounted (the I4 animation needs it)
// and used to hide via grid-template-rows: 0fr + overflow: hidden alone —
// which clips the per-window pin buttons but leaves them focusable, so Tab
// disappeared into the closed row and Enter toggled a tray digit with nothing
// visible. visibility: hidden is what actually removes clipped content from
// the tab order; jsdom computes no focusability from style, so these pin the
// style itself rather than simulating Tab.
describe("a collapsed row's pin buttons are out of reach, not just out of sight (F2)", () => {
  const windows = [
    { id: "w1", name: "Session", used: 40 },
    { id: "w2", name: "Weekly", used: 10 },
  ];

  function detailContainer() {
    const [pin] = screen.getAllByLabelText("Show in menu bar");
    return pin.closest("[aria-hidden]");
  }

  it("hides the collapsed detail area with visibility, not just clipping", () => {
    render(<SubscriptionRow label="Claude Max" state="working" used={40} windows={windows} />);
    const detail = detailContainer();
    expect(detail.getAttribute("aria-hidden")).toBe("true");
    expect(detail.style.visibility).toBe("hidden");
  });

  it("shows it again when expanded", () => {
    render(
      <SubscriptionRow label="Claude Max" state="working" used={40} windows={windows} expanded />,
    );
    const detail = detailContainer();
    expect(detail.getAttribute("aria-hidden")).toBe("false");
    expect(detail.style.visibility).toBe("visible");
  });

  it("transitions visibility on the duration token, so content stays visible while the row closes", () => {
    // Without this, the windows vanish the instant a collapse starts and the
    // I4 animation closes an already-empty box.
    render(
      <SubscriptionRow label="Claude Max" state="working" used={40} windows={windows} expanded />,
    );
    expect(detailContainer().style.transition).toContain("visibility var(--dur-base)");
  });
});
