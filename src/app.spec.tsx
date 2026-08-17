import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";

afterEach(cleanup);

class ResizeObserverStub {
  observe() {}
  unobserve() {}
  disconnect() {}
}
globalThis.ResizeObserver ??= ResizeObserverStub as unknown as typeof ResizeObserver;

const hidePanel = vi.fn();
const fetchSnapshotSpy = vi.fn();
let visibilityCallback: ((visible: boolean) => void) | null = null;

vi.mock("./lib/tauri-client", () => ({
  listAccounts: () => Promise.resolve([]),
  fetchSnapshot: () => {
    fetchSnapshotSpy();
    return Promise.resolve({
      account_id: "claude:claude",
      provider: "claude",
      config_dir: "~/.claude",
      fetched_at: new Date().toISOString(),
      usage: {
        limits: [{ kind: "session", percent: 10, is_active: true, resets_at: null, scope: null }],
      },
      profile: null,
    });
  },
  hidePanel: () => hidePanel(),
  onPanelVisibility: (callback: (visible: boolean) => void) => {
    visibilityCallback = callback;
    return Promise.resolve(() => {});
  },
  setDetached: () => Promise.resolve(),
  dragWindowStep: () => Promise.resolve(),
  endWindowDrag: () => Promise.resolve(),
  debugRateLimitSnapshot: () => Promise.resolve(null),
  startSignIn: () => Promise.resolve(),
  submitSignInCode: () => Promise.resolve(),
  cancelSignIn: () => Promise.resolve(),
  forgetSignIn: () => Promise.resolve(),
  onSignInFinished: () => Promise.resolve(() => {}),
  renderStatusItem: () => Promise.resolve(),
  onQuotaRefresh: () => Promise.resolve(() => {}),
  kickScheduler: () => Promise.resolve(),
  statuslineStatus: () => Promise.resolve({ kind: "not_installed" }),
  statuslineInstall: () => Promise.resolve(),
  statuslineRemove: () => Promise.resolve(),
  onPanelBeakOffset: () => Promise.resolve(() => {}),
}));

vi.mock("./lib/persistence", () => ({
  loadTracked: () =>
    Promise.resolve([
      {
        id: "claude:claude",
        provider: "claude",
        config_dir: "~/.claude",
        label: null,
        pinnedWindowIds: [],
      },
    ]),
  saveTracked: () => Promise.resolve(),
}));

import App from "./app";

async function renderAppWithRow() {
  render(<App />);
  return await screen.findByLabelText("More");
}

describe("Escape dismissal layering", () => {
  beforeEach(() => {
    hidePanel.mockReset();
    visibilityCallback = null;
  });

  test("a bare Escape hides the panel", async () => {
    await renderAppWithRow();
    fireEvent.keyDown(window, { key: "Escape" });
    expect(hidePanel).toHaveBeenCalledTimes(1);
  });

  test("Escape closes an open row menu and leaves the panel up; the next Escape hides", async () => {
    const trigger = await renderAppWithRow();
    fireEvent.click(trigger);
    expect(screen.getByText("Stop tracking")).toBeTruthy();

    fireEvent.keyDown(window, { key: "Escape" });
    expect(screen.queryByText("Stop tracking")).toBeNull();
    expect(hidePanel).not.toHaveBeenCalled();

    fireEvent.keyDown(window, { key: "Escape" });
    expect(hidePanel).toHaveBeenCalledTimes(1);
  });

  test("Escape in the rename field cancels the rename without hiding the panel", async () => {
    const trigger = await renderAppWithRow();
    fireEvent.click(trigger);
    fireEvent.click(screen.getByText("Rename"));
    const input = screen.getByRole("textbox");

    fireEvent.keyDown(input, { key: "Escape" });
    expect(screen.queryByRole("textbox")).toBeNull();
    expect(hidePanel).not.toHaveBeenCalled();
  });

  test("the row menu does not survive a panel hide", async () => {
    const trigger = await renderAppWithRow();
    fireEvent.click(trigger);
    expect(screen.getByText("Stop tracking")).toBeTruthy();

    act(() => visibilityCallback!(false));
    expect(screen.queryByText("Stop tracking")).toBeNull();
  });
});

describe("pointer dismissal consumes the dismissing click", () => {
  function dismissByClicking(target: Element) {
    fireEvent.mouseDown(target);
    fireEvent.click(target);
  }

  test("a click on the row body only dismisses the menu, the row does not expand", async () => {
    const trigger = await renderAppWithRow();
    fireEvent.click(trigger);
    expect(screen.getByText("Stop tracking")).toBeTruthy();
    const expandToggle = screen.getByRole("button", { name: /1 limit/ });
    expect(expandToggle.getAttribute("aria-expanded")).toBe("false");

    dismissByClicking(screen.getByText("Claude"));
    expect(screen.queryByText("Stop tracking")).toBeNull();
    expect(expandToggle.getAttribute("aria-expanded")).toBe("false");
  });

  test("only the dismissing click is consumed, the next click acts normally", async () => {
    const trigger = await renderAppWithRow();
    fireEvent.click(trigger);

    dismissByClicking(screen.getByText("Claude"));
    dismissByClicking(screen.getByText("Claude"));
    expect(screen.getByRole("button", { name: /1 limit/ }).getAttribute("aria-expanded")).toBe(
      "true",
    );
  });

  test("a dismissing click on a button does not press it", async () => {
    const trigger = await renderAppWithRow();
    const callsBefore = fetchSnapshotSpy.mock.calls.length;
    fireEvent.click(trigger);
    expect(screen.getByText("Stop tracking")).toBeTruthy();

    const refresh = screen.getByRole("button", { name: "Read all now" });
    await act(async () => dismissByClicking(refresh));
    expect(screen.queryByText("Stop tracking")).toBeNull();
    expect(fetchSnapshotSpy.mock.calls.length).toBe(callsBefore);

    await act(async () => dismissByClicking(refresh));
    expect(fetchSnapshotSpy.mock.calls.length).toBe(callsBefore + 1);
  });
});

describe("panel reopen refreshes the presentation clock", () => {
  beforeEach(() => {
    visibilityCallback = null;
  });

  test("re-reads `now` on visible=true so relative times are not hours stale", async () => {
    await renderAppWithRow();
    expect(screen.getByText("Last read just now")).toBeTruthy();

    const twoHoursLater = Date.now() + 2 * 60 * 60 * 1000;
    const nowSpy = vi.spyOn(Date, "now").mockReturnValue(twoHoursLater);
    act(() => visibilityCallback!(true));
    nowSpy.mockRestore();

    expect(screen.getByText("Last read 2h ago")).toBeTruthy();
  });
});
