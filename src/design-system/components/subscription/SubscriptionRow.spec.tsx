import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";
import { SubscriptionRow } from "./SubscriptionRow";
import rowStylesheet from "./SubscriptionRow.module.css?raw";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

/** The panel's own window is 360x560 logical. jsdom's default viewport is
 *  not, and the flip-above branch only makes sense against a real one. */
function useWindow(width: number, height: number) {
  Object.defineProperty(window, "innerWidth", { value: width, configurable: true });
  Object.defineProperty(window, "innerHeight", { value: height, configurable: true });
}

/** jsdom reports every rect as zero, so the "…" button's position, the only
 *  input the placement math has, must be supplied. These tests assert the
 *  anchoring rules rather than exact pixel offsets. */
function anchorButtonAt({ top, bottom, right }: { top: number; bottom: number; right: number }) {
  const original = Element.prototype.getBoundingClientRect;
  vi.spyOn(Element.prototype, "getBoundingClientRect").mockImplementation(function (this: Element) {
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
        toJSON() {},
      } satisfies DOMRect;
    }
    return original.call(this);
  });
}

function openMenu() {
  return render(
    <SubscriptionRow label="Claude Team" provider="Anthropic" state="working" used={40} menuOpen />,
  );
}

// An absolutely positioned element is clipped by, and counts toward the
// scroll extent of, its scroll-container ancestor. A fixed one is neither.
describe("SubscriptionRow's row menu overlays the panel instead of living inside its scroll box", () => {
  beforeEach(() => useWindow(360, 560));

  test("is positioned against the viewport, not against the scrolled row", () => {
    anchorButtonAt({ top: 100, bottom: 120, right: 320 });
    openMenu();
    const menu = screen
      .getByText("Stop tracking")
      .closest<HTMLElement>("[data-quotos-menu-scope]")!;
    expect(getComputedStyle(menu).position).toBe("fixed");
  });

  test("hangs below the '…' button it belongs to", () => {
    anchorButtonAt({ top: 100, bottom: 120, right: 320 });
    openMenu();
    const menu = screen
      .getByText("Stop tracking")
      .closest<HTMLElement>("[data-quotos-menu-scope]")!;
    // Below the button's bottom edge, by a small gap.
    expect(parseFloat(menu.style.top)).toBeGreaterThanOrEqual(120);
    expect(parseFloat(menu.style.top)).toBeLessThan(130);
  });

  test("flips above the button rather than off the bottom of the window", () => {
    // A row near the panel's bottom, which is exactly where the clipping was
    // visible: the menu cannot open downward and stay inside a 560px window.
    anchorButtonAt({ top: 520, bottom: 540, right: 320 });
    vi.spyOn(HTMLElement.prototype, "offsetHeight", "get").mockReturnValue(122);
    vi.spyOn(HTMLElement.prototype, "offsetWidth", "get").mockReturnValue(178);
    openMenu();
    const menu = screen
      .getByText("Stop tracking")
      .closest<HTMLElement>("[data-quotos-menu-scope]")!;
    const top = parseFloat(menu.style.top);
    expect(top).toBeLessThan(520); // above the button
    expect(top).toBeGreaterThanOrEqual(8); // and still inside the window
    expect(top + 122).toBeLessThanOrEqual(560);
  });

  test("stays inside the window horizontally", () => {
    anchorButtonAt({ top: 100, bottom: 120, right: 320 });
    vi.spyOn(HTMLElement.prototype, "offsetWidth", "get").mockReturnValue(178);
    openMenu();
    const menu = screen
      .getByText("Stop tracking")
      .closest<HTMLElement>("[data-quotos-menu-scope]")!;
    const left = parseFloat(menu.style.left);
    expect(left).toBeGreaterThanOrEqual(8);
    expect(left + 178).toBeLessThanOrEqual(360 - 8);
  });

  // App.tsx's click-away handler recognises "inside the menu" by this
  // attribute alone on both the dropdown and its trigger. Moving the
  // dropdown out of the row's own box must not cost it that.
  test("keeps the menu-scope marker the click-away handler matches on", () => {
    anchorButtonAt({ top: 100, bottom: 120, right: 320 });
    openMenu();
    expect(screen.getByText("Stop tracking").closest("[data-quotos-menu-scope]")).not.toBeNull();
    expect(screen.getByLabelText("More").getAttribute("data-quotos-menu-scope")).toBe("true");
  });

  test("the trigger reports the menu's open state via aria-expanded", () => {
    const { rerender } = render(<SubscriptionRow label="Claude Team" state="working" used={40} />);
    expect(screen.getByLabelText("More").getAttribute("aria-expanded")).toBe("false");
    rerender(<SubscriptionRow label="Claude Team" state="working" used={40} menuOpen />);
    expect(screen.getByLabelText("More").getAttribute("aria-expanded")).toBe("true");
  });
});

describe("the 'N limits' disclosure is a real, focusable control", () => {
  const windows = [
    { id: "w1", name: "Session", used: 40 },
    { id: "w2", name: "Weekly", used: 10 },
  ];

  test("is a button carrying aria-expanded", () => {
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

  test("toggles expansion itself, without the row's own click undoing it", () => {
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

  test("does not render at all without windows", () => {
    render(<SubscriptionRow label="Claude Max" state="working" used={40} windows={[]} />);
    expect(screen.queryByRole("button", { name: /limits?/ })).toBeNull();
  });
});

describe("the row menu's Move up and Move down", () => {
  test("fires the move callback and closes the menu, once each", () => {
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

  test("renders an impossible direction disabled, and clicking it does nothing", () => {
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
    const moveUp = screen.getByText<HTMLButtonElement>("Move up");
    expect(moveUp.disabled).toBe(true);
    expect(screen.getByText<HTMLButtonElement>("Move down").disabled).toBe(false);
    moveUp.click();
    expect(onMoveUp).not.toHaveBeenCalled();
    expect(onToggleMenu).not.toHaveBeenCalled();
  });

  test("keeps both items in the menu even when neither direction is possible", () => {
    render(<SubscriptionRow label="Claude Max" state="working" used={40} menuOpen />);
    expect(screen.getByText<HTMLButtonElement>("Move up").disabled).toBe(true);
    expect(screen.getByText<HTMLButtonElement>("Move down").disabled).toBe(true);
  });
});

describe("the row menu is keyboard-operable", () => {
  // Keydowns bubble from wherever focus is to the row div's own handler, so
  // firing on the trigger models "Enter opened the menu, focus still on the
  // '…' button".
  const arrow = (key: string) =>
    fireEvent.keyDown(
      document.activeElement === document.body
        ? screen.getByLabelText("More")
        : (document.activeElement ?? document.body),
      { key },
    );

  test("ArrowDown walks the items top-to-bottom and wraps past the end", () => {
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
      expect(document.activeElement?.textContent).toBe(label);
    }
    arrow("ArrowDown");
    expect(document.activeElement?.textContent).toBe("Read now");
  });

  test("ArrowUp enters at the last item and walks backwards", () => {
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
    expect(document.activeElement?.textContent).toBe("Stop tracking");
    arrow("ArrowUp");
    expect(document.activeElement?.textContent).toBe("Move down");
  });

  test("skips disabled items, exactly as the pointer path does", () => {
    render(<SubscriptionRow label="Claude Max" state="working" used={40} menuOpen canMoveDown />);
    arrow("ArrowDown"); // Read now
    arrow("ArrowDown"); // Rename
    arrow("ArrowDown"); // Show in menu bar
    arrow("ArrowDown"); // Move up is disabled, so this lands on Move down.
    expect(document.activeElement?.textContent).toBe("Move down");
  });

  test("Home and End jump to the edges", () => {
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
    expect(document.activeElement?.textContent).toBe("Stop tracking");
    arrow("Home");
    expect(document.activeElement?.textContent).toBe("Read now");
  });

  test("leaves the arrow keys alone while the menu is closed", () => {
    render(<SubscriptionRow label="Claude Max" state="working" used={40} />);
    fireEvent.keyDown(screen.getByLabelText("More"), { key: "ArrowDown" });
    expect(document.activeElement).toBe(document.body);
  });

  test("leaves the sign-in code field's caret alone even with the menu open", () => {
    render(<SubscriptionRow label="Claude Max" state="broken" menuOpen signInInProgress />);
    const code = screen.getByPlaceholderText("Paste code");
    code.focus();
    fireEvent.keyDown(code, { key: "ArrowDown" });
    expect(document.activeElement).toBe(code);
  });

  test("hands focus back to the trigger when closing unmounts the focused item", () => {
    const { rerender } = render(
      <SubscriptionRow label="Claude Max" state="working" used={40} menuOpen />,
    );
    arrow("ArrowDown");
    expect(document.activeElement?.textContent).toBe("Read now");
    rerender(<SubscriptionRow label="Claude Max" state="working" used={40} />);
    expect(document.activeElement).toBe(screen.getByLabelText("More"));
  });

  test("does not steal focus when the close left it somewhere real", () => {
    const windows = [{ id: "w1", name: "Session", used: 40 }];
    const { rerender } = render(
      <SubscriptionRow label="Claude Max" state="working" used={40} windows={windows} menuOpen />,
    );
    const disclosure = screen.getByRole("button", { name: /1 limit/ });
    disclosure.focus();
    rerender(<SubscriptionRow label="Claude Max" state="working" used={40} windows={windows} />);
    expect(document.activeElement).toBe(disclosure);
  });
});

describe("the row menu exposes WAI-ARIA menu semantics", () => {
  test("the trigger declares it opens a menu", () => {
    render(<SubscriptionRow label="Claude Max" state="working" used={40} />);
    expect(screen.getByLabelText("More").getAttribute("aria-haspopup")).toBe("menu");
  });

  test("the open dropdown is a named menu", () => {
    render(<SubscriptionRow label="Claude Max" state="working" used={40} menuOpen />);
    const menu = screen.getByRole("menu");
    expect(menu.getAttribute("aria-label")).toBe("Subscription actions");
    expect(menu.getAttribute("data-quotos-menu-scope")).toBe("true");
  });

  test("every action is a menuitem, in the menu's one fixed order", () => {
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

  test("the divider before Stop tracking is a separator", () => {
    render(<SubscriptionRow label="Claude Max" state="working" used={40} menuOpen />);
    expect(screen.getByRole("separator")).toBeTruthy();
  });
});

describe("committing a rename without editing keeps an existing custom name", () => {
  function openRenameField(onRename: (nextLabel: string | null) => void) {
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
    return screen.getByRole<HTMLInputElement>("textbox");
  }

  test("Enter with an untouched draft is a no-op, not a clear", () => {
    const onRename = vi.fn();
    const input = openRenameField(onRename);
    expect(input.value).toBe("My Max");
    fireEvent.keyDown(input, { key: "Enter" });
    expect(onRename).not.toHaveBeenCalled();
    expect(screen.queryByRole("textbox")).toBeNull();
  });

  test("clicking away (blur) with an untouched draft is a no-op too", () => {
    const onRename = vi.fn();
    const input = openRenameField(onRename);
    fireEvent.blur(input);
    expect(onRename).not.toHaveBeenCalled();
  });

  test("an edited draft commits the trimmed new name", () => {
    const onRename = vi.fn();
    const input = openRenameField(onRename);
    fireEvent.change(input, { target: { value: "  Team account " } });
    fireEvent.keyDown(input, { key: "Enter" });
    expect(onRename).toHaveBeenCalledWith("Team account");
  });

  test("an explicitly emptied field clears the custom name", () => {
    const onRename = vi.fn();
    const input = openRenameField(onRename);
    fireEvent.change(input, { target: { value: "   " } });
    fireEvent.keyDown(input, { key: "Enter" });
    expect(onRename).toHaveBeenCalledWith(null);
  });
});

// visibility: hidden is what removes clipped content from the tab order.
// jsdom computes no focusability from style, so these pin the computed
// style itself rather than simulating Tab.
describe("a collapsed row's pin buttons are out of reach, not just out of sight", () => {
  const windows = [
    { id: "w1", name: "Session", used: 40 },
    { id: "w2", name: "Weekly", used: 10 },
  ];

  function detailContainer(): HTMLElement {
    const [pin] = screen.getAllByLabelText("Show in menu bar");
    return pin.closest<HTMLElement>("[aria-hidden]")!;
  }

  test("hides the collapsed detail area with visibility, not just clipping", () => {
    render(<SubscriptionRow label="Claude Max" state="working" used={40} windows={windows} />);
    const detail = detailContainer();
    expect(detail.getAttribute("aria-hidden")).toBe("true");
    expect(getComputedStyle(detail).visibility).toBe("hidden");
  });

  test("shows it again when expanded", () => {
    render(
      <SubscriptionRow label="Claude Max" state="working" used={40} windows={windows} expanded />,
    );
    const detail = detailContainer();
    expect(detail.getAttribute("aria-hidden")).toBe("false");
    expect(getComputedStyle(detail).visibility).toBe("visible");
  });

  // jsdom's CSS engine cannot parse a multi-value transition shorthand back
  // into its longhand computed properties, so this reads the rule's actual
  // text instead of getComputedStyle, the same way reducedMotion.spec.tsx
  // reads a stylesheet's real text for a fact jsdom cannot compute.
  test("transitions visibility on the same duration token as grid-template-rows, so content stays visible while the row closes", () => {
    expect(rowStylesheet).toContain("grid-template-rows var(--dur-base)");
    expect(rowStylesheet).toContain("visibility var(--dur-base)");
  });
});
