import { act, cleanup, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";
import type { AccountDescriptor } from "@/types/entities";

afterEach(cleanup);

const listAccounts = vi.fn<() => Promise<AccountDescriptor[]>>();
const stopListening = vi.fn();
let visibilityCallback: ((visible: boolean) => void) | null = null;
const onPanelVisibility = vi.fn((callback: (visible: boolean) => void) => {
  visibilityCallback = callback;
  return Promise.resolve(stopListening);
});

vi.mock("../lib/tauriClient", () => ({
  listAccounts: () => listAccounts(),
  onPanelVisibility: (callback: (visible: boolean) => void) => onPanelVisibility(callback),
  // StatuslineControl imports these by name, so the mocked module must
  // still export them even though it renders only for tracked rows, none
  // of which appear in these tests.
  statuslineStatus: () => Promise.resolve({ kind: "not_installed" }),
  statuslineInstall: () => Promise.resolve(),
  statuslineRemove: () => Promise.resolve(),
}));

// vi.mock is hoisted, so this static import safely resolves against it.
import { SubscriptionsScreen } from "./subscriptions-screen";

const ACCOUNT_A: AccountDescriptor = {
  id: "claude:claude",
  provider: "claude",
  config_dir: "/Users/x/.claude",
};
const ACCOUNT_B: AccountDescriptor = {
  id: "claude:claude-work",
  provider: "claude",
  config_dir: "/Users/x/.claude-work",
};

function renderScreen() {
  return render(
    <SubscriptionsScreen
      tracked={[]}
      onAdd={() => {}}
      onRemove={() => {}}
      displayLabelFor={(a) => a.id}
    />,
  );
}

async function flush() {
  await act(async () => {});
}

describe("SubscriptionsScreen discovery refresh", () => {
  beforeEach(() => {
    listAccounts.mockReset();
    stopListening.mockReset();
    onPanelVisibility.mockClear();
    visibilityCallback = null;
  });

  test("scans on mount and lists what it finds", async () => {
    listAccounts.mockResolvedValue([ACCOUNT_A]);
    renderScreen();
    expect(await screen.findByText("/Users/x/.claude")).toBeTruthy();
    expect(listAccounts).toHaveBeenCalledTimes(1);
  });

  test("rescans when the panel comes back on screen, so a fresh sign-in just appears", async () => {
    listAccounts.mockResolvedValueOnce([ACCOUNT_A]).mockResolvedValueOnce([ACCOUNT_A, ACCOUNT_B]);
    renderScreen();
    await screen.findByText("/Users/x/.claude");
    expect(screen.queryByText("/Users/x/.claude-work")).toBeNull();

    // The panel hides because the terminal where the sign-in happens takes
    // focus.
    act(() => visibilityCallback!(false));
    // The new account is discoverable by the time the panel reopens.
    await act(async () => visibilityCallback!(true));

    expect(await screen.findByText("/Users/x/.claude-work")).toBeTruthy();
    expect(listAccounts).toHaveBeenCalledTimes(2);
  });

  test("does not rescan on a visible signal with no hide before it, the mock harness's subscribe-time signal", async () => {
    listAccounts.mockResolvedValue([ACCOUNT_A]);
    renderScreen();
    await screen.findByText("/Users/x/.claude");

    await act(async () => visibilityCallback!(true));

    expect(listAccounts).toHaveBeenCalledTimes(1);
  });

  test("stops listening on unmount", async () => {
    listAccounts.mockResolvedValue([]);
    const { unmount } = renderScreen();
    await flush();
    unmount();
    expect(stopListening).toHaveBeenCalledTimes(1);
  });
});
