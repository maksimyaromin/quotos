import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

afterEach(cleanup);

// jsdom has no ResizeObserver; Panel's height animation observes its content
// with one. The tests here never assert on height, so an inert stub is enough.
class ResizeObserverStub {
  observe() {}
  unobserve() {}
  disconnect() {}
}
globalThis.ResizeObserver ??= ResizeObserverStub as unknown as typeof ResizeObserver;

const hidePanel = vi.fn();
let visibilityCallback: ((visible: boolean) => void) | null = null;

vi.mock("./lib/tauriClient", () => ({
  listAccounts: () => Promise.resolve([]),
  fetchSnapshot: () =>
    Promise.resolve({
      account_id: "claude:claude",
      provider: "claude",
      config_dir: "~/.claude",
      fetched_at: new Date().toISOString(),
      usage: { limits: [{ kind: "session", percent: 10, is_active: true, resets_at: null, scope: null }] },
      profile: null,
    }),
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
  setTrayStatus: () => Promise.resolve(),
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
      { id: "claude:claude", provider: "claude", config_dir: "~/.claude", label: null, pinnedWindowIds: [] },
    ]),
  saveTracked: () => Promise.resolve(),
}));

// vi.mock calls above are hoisted by Vitest, so this static import safely
// resolves against the mocked modules.
import App from "./App";

async function renderAppWithRow() {
  render(<App />);
  // The tracked account has loaded and read once when its row menu trigger
  // exists.
  return await screen.findByLabelText("More");
}

describe("Escape dismissal layering", () => {
  beforeEach(() => {
    hidePanel.mockReset();
    visibilityCallback = null;
  });

  it("a bare Escape hides the panel", async () => {
    await renderAppWithRow();
    fireEvent.keyDown(window, { key: "Escape" });
    expect(hidePanel).toHaveBeenCalledTimes(1);
  });

  it("Escape closes an open row menu and leaves the panel up; the next Escape hides", async () => {
    const trigger = await renderAppWithRow();
    fireEvent.click(trigger);
    expect(screen.getByText("Stop tracking")).toBeTruthy();

    fireEvent.keyDown(window, { key: "Escape" });
    expect(screen.queryByText("Stop tracking")).toBeNull();
    expect(hidePanel).not.toHaveBeenCalled();

    fireEvent.keyDown(window, { key: "Escape" });
    expect(hidePanel).toHaveBeenCalledTimes(1);
  });

  it("Escape in the rename field cancels the rename without hiding the panel", async () => {
    const trigger = await renderAppWithRow();
    fireEvent.click(trigger);
    fireEvent.click(screen.getByText("Rename"));
    const input = screen.getByRole("textbox");

    fireEvent.keyDown(input, { key: "Escape" });
    expect(screen.queryByRole("textbox")).toBeNull();
    expect(hidePanel).not.toHaveBeenCalled();
  });

  it("the row menu does not survive a panel hide", async () => {
    const trigger = await renderAppWithRow();
    fireEvent.click(trigger);
    expect(screen.getByText("Stop tracking")).toBeTruthy();

    act(() => visibilityCallback!(false));
    expect(screen.queryByText("Stop tracking")).toBeNull();
  });
});
