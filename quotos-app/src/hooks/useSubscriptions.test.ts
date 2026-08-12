import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const fetchSnapshot = vi.fn();
const setTrayTitle = vi.fn();

vi.mock("../lib/tauriClient", () => ({
  fetchSnapshot: (...args: unknown[]) => fetchSnapshot(...args),
  setTrayTitle: (...args: unknown[]) => setTrayTitle(...args),
}));

const TRACKED = [
  { id: "claude:claude", provider: "claude", config_dir: "~/.claude", label: null, pinned: false },
];

vi.mock("../lib/persistence", () => ({
  loadTracked: () => TRACKED,
  saveTracked: vi.fn(),
}));

// vi.mock calls above are hoisted by Vitest, so this static import safely
// resolves against the mocked modules.
import { useSubscriptions } from "./useSubscriptions";

async function flush() {
  await act(async () => {
    await vi.advanceTimersByTimeAsync(0);
  });
}

// B5: opening/closing the panel must never spend the shared rate budget —
// only an explicit manual refresh and the scheduled background refresh may.
// The panel's own show/hide no longer triggers any fetch at all (the old
// bug was a refresh tied to a `panel-visibility` event); this test proves
// that repeatedly letting time pass short of the background interval never
// calls fetchSnapshot again, while the scheduled interval and a manual
// refreshAll() both do.
describe("useSubscriptions refresh policy", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    fetchSnapshot.mockReset();
    setTrayTitle.mockReset();
    fetchSnapshot.mockResolvedValue({
      account_id: "claude:claude",
      provider: "claude",
      config_dir: "~/.claude",
      fetched_at: new Date().toISOString(),
      usage: { limits: [{ kind: "session", percent: 10, is_active: true, resets_at: null, scope: null }] },
      profile: null,
    });
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("does not fetch again merely because time passes short of the schedule (simulating repeated opens)", async () => {
    const { result } = renderHook(() => useSubscriptions());
    await flush();
    expect(fetchSnapshot).toHaveBeenCalledTimes(1);

    // Five "open/close cycles" worth of elapsed time, each well short of
    // the 5-minute background interval — none of this may spend budget.
    for (let i = 0; i < 5; i++) {
      await act(async () => {
        await vi.advanceTimersByTimeAsync(10_000);
      });
    }
    expect(fetchSnapshot).toHaveBeenCalledTimes(1);
    expect(result.current.subscriptions[0].state).toBe("working");
  });

  it("the scheduled background refresh does spend budget once its interval elapses", async () => {
    renderHook(() => useSubscriptions());
    await flush();
    expect(fetchSnapshot).toHaveBeenCalledTimes(1);

    await act(async () => {
      await vi.advanceTimersByTimeAsync(5 * 60 * 1000 + 1000);
    });
    expect(fetchSnapshot).toHaveBeenCalledTimes(2);
  });

  it("an explicit manual refresh spends budget on demand", async () => {
    const { result } = renderHook(() => useSubscriptions());
    await flush();
    expect(fetchSnapshot).toHaveBeenCalledTimes(1);

    await act(async () => {
      await result.current.refreshAll();
    });
    expect(fetchSnapshot).toHaveBeenCalledTimes(2);
  });
});

// B6: a diagnosable failure (e.g. expired login → Broken) must never decay
// into the generic rate-limit wait, even when a retry of that same broken
// account comes back rate-limited. This guards against the exact regression
// found in manual testing: the optimistic "connecting" patch issued at the
// start of every refresh attempt was clobbering the prior Broken state
// before the rate-limited response even came back.
describe("useSubscriptions health vs. rate-limit precedence (B6)", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    setTrayTitle.mockReset();
    fetchSnapshot.mockReset();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("a rate-limited retry restores the prior Broken state instead of leaving it 'connecting'", async () => {
    fetchSnapshot
      .mockRejectedValueOnce({ kind: "unauthorized", message: "still unauthorized after refreshing the credential" })
      .mockRejectedValueOnce({ kind: "rate_limited", retry_after_secs: 214 });

    const { result } = renderHook(() => useSubscriptions());
    await flush();
    expect(result.current.subscriptions[0].state).toBe("broken");
    expect(result.current.subscriptions[0].reason).toMatch(/sign-in expired/i);

    await act(async () => {
      await result.current.refreshAccountById("claude:claude");
    });

    expect(result.current.subscriptions[0].state).toBe("broken");
    expect(result.current.subscriptions[0].reason).toMatch(/sign-in expired/i);
    expect(result.current.subscriptions[0].rateLimitedUntil).not.toBeNull();
  });
});
