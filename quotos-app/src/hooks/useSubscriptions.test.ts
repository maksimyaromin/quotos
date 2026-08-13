import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const fetchSnapshot = vi.fn();
const setTrayStatus = vi.fn();
const onQuotaRefresh = vi.fn();
const kickScheduler = vi.fn();

vi.mock("../lib/tauriClient", () => ({
  fetchSnapshot: (...args: unknown[]) => fetchSnapshot(...args),
  setTrayStatus: (...args: unknown[]) => setTrayStatus(...args),
  onQuotaRefresh: (...args: unknown[]) => onQuotaRefresh(...args),
  kickScheduler: (...args: unknown[]) => kickScheduler(...args),
}));

const TRACKED = [
  { id: "claude:claude", provider: "claude", config_dir: "~/.claude", label: null, pinned: false },
];

vi.mock("../lib/persistence", () => ({
  loadTracked: () => Promise.resolve(TRACKED),
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

// R2-4: in jsdom (no "__TAURI_INTERNALS__" global), the hook takes its
// browser/mock-harness branch — a one-time initial read via fetchSnapshot,
// exactly like the pre-R2-4 behaviour. The native branch (subscribing to
// the Rust scheduler's quota-refresh push) is exercised separately below by
// setting that global before rendering.
describe("useSubscriptions refresh policy (browser/mock harness path)", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    fetchSnapshot.mockReset();
    setTrayStatus.mockReset();
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

  // B5, still true after R2-4 moved cadence to the Rust side: there is no
  // JS timer left at all, so time passing — however many simulated
  // open/close cycles — can never spend budget on its own.
  it("does not fetch again merely because time passes (no JS timer exists anymore)", async () => {
    const { result } = renderHook(() => useSubscriptions());
    await flush();
    expect(fetchSnapshot).toHaveBeenCalledTimes(1);

    for (let i = 0; i < 5; i++) {
      await act(async () => {
        await vi.advanceTimersByTimeAsync(10 * 60 * 1000);
      });
    }
    expect(fetchSnapshot).toHaveBeenCalledTimes(1);
    expect(result.current.subscriptions[0].state).toBe("working");
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

  // R2-4: "the manual refresh control is debounced" — concurrent calls must
  // collapse into a single in-flight fetch, not double-fire.
  it("concurrent refreshAll calls collapse into a single in-flight fetch", async () => {
    const { result } = renderHook(() => useSubscriptions());
    await flush();
    expect(fetchSnapshot).toHaveBeenCalledTimes(1);

    await act(async () => {
      await Promise.all([result.current.refreshAll(), result.current.refreshAll(), result.current.refreshAll()]);
    });
    // One initial (mount) + one from the three collapsed manual calls.
    expect(fetchSnapshot).toHaveBeenCalledTimes(2);
  });

  it("concurrent refreshAccountById calls for the same id collapse into one fetch", async () => {
    const { result } = renderHook(() => useSubscriptions());
    await flush();
    expect(fetchSnapshot).toHaveBeenCalledTimes(1);

    await act(async () => {
      await Promise.all([
        result.current.refreshAccountById("claude:claude"),
        result.current.refreshAccountById("claude:claude"),
      ]);
    });
    expect(fetchSnapshot).toHaveBeenCalledTimes(2);
  });
});

// R2-4: on the native path, automatic reads arrive as pushed `quota-refresh`
// events from the Rust scheduler, not as JS-initiated fetches — this proves
// the hook applies a pushed event exactly like a direct fetch result, and
// never calls fetchSnapshot itself for it.
describe("useSubscriptions refresh policy (native path)", () => {
  let quotaRefreshCallback: ((event: unknown) => void) | undefined;

  beforeEach(() => {
    vi.useFakeTimers();
    fetchSnapshot.mockReset();
    setTrayStatus.mockReset();
    kickScheduler.mockReset();
    onQuotaRefresh.mockReset();
    quotaRefreshCallback = undefined;
    onQuotaRefresh.mockImplementation((cb: (event: unknown) => void) => {
      quotaRefreshCallback = cb;
      return Promise.resolve(() => {});
    });
    (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = {};
  });

  afterEach(() => {
    vi.useRealTimers();
    delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__;
  });

  it("subscribes to quota-refresh and kicks the scheduler once at mount, without fetching directly", async () => {
    renderHook(() => useSubscriptions());
    await flush();
    expect(onQuotaRefresh).toHaveBeenCalledTimes(1);
    expect(kickScheduler).toHaveBeenCalledTimes(1);
    expect(fetchSnapshot).not.toHaveBeenCalled();
  });

  it("applies a pushed 'ok' event exactly like a direct refresh result", async () => {
    const { result } = renderHook(() => useSubscriptions());
    await flush();
    expect(quotaRefreshCallback).toBeTypeOf("function");

    await act(async () => {
      quotaRefreshCallback?.({
        kind: "ok",
        snapshot: {
          account_id: "claude:claude",
          provider: "claude",
          config_dir: "~/.claude",
          fetched_at: new Date().toISOString(),
          usage: { limits: [{ kind: "weekly_all", percent: 42, is_active: true, resets_at: null, scope: null }] },
          profile: null,
        },
      });
    });

    expect(result.current.subscriptions[0].state).toBe("working");
    expect(result.current.subscriptions[0].used).toBe(42);
    expect(fetchSnapshot).not.toHaveBeenCalled();
  });

  it("a pushed rate_limited event never overwrites prior health state (B5/B6, native path)", async () => {
    const { result } = renderHook(() => useSubscriptions());
    await flush();

    await act(async () => {
      quotaRefreshCallback?.({
        kind: "err",
        account_id: "claude:claude",
        error: { kind: "unauthorized", message: "still unauthorized after refreshing the credential" },
      });
    });
    expect(result.current.subscriptions[0].state).toBe("broken");

    await act(async () => {
      quotaRefreshCallback?.({
        kind: "err",
        account_id: "claude:claude",
        error: { kind: "rate_limited", retry_after_secs: 214 },
      });
    });
    expect(result.current.subscriptions[0].state).toBe("broken");
    expect(result.current.subscriptions[0].reason).toMatch(/sign-in expired/i);
    expect(result.current.subscriptions[0].rateLimitedUntil).not.toBeNull();
  });
});

// B6: a diagnosable failure (e.g. expired login → Broken) must never decay
// into the generic rate-limit wait, even when a retry of that same broken
// account comes back rate-limited. This guards against the exact regression
// found in manual testing: the optimistic "connecting" patch issued at the
// start of every refresh attempt was clobbering the prior Broken state
// before the rate-limited response even came back.
describe("useSubscriptions health vs. rate-limit precedence (B6, manual refresh path)", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    setTrayStatus.mockReset();
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
