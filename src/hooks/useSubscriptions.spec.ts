import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";

const fetchSnapshot = vi.fn();
const setTrayStatus = vi.fn();
const onQuotaRefresh = vi.fn();
const kickScheduler = vi.fn();
const startSignIn = vi.fn();
const submitSignInCode = vi.fn();
const cancelSignIn = vi.fn();
const forgetSignIn = vi.fn();
const onSignInFinished = vi.fn((_callback: (event: unknown) => void) => Promise.resolve(() => {}));

vi.mock("../lib/tauriClient", () => ({
  fetchSnapshot: (...args: unknown[]) => fetchSnapshot(...args),
  setTrayStatus: (...args: unknown[]) => setTrayStatus(...args),
  onQuotaRefresh: (...args: unknown[]) => onQuotaRefresh(...args),
  kickScheduler: (...args: unknown[]) => kickScheduler(...args),
  startSignIn: (...args: unknown[]) => startSignIn(...args),
  submitSignInCode: (...args: unknown[]) => submitSignInCode(...args),
  cancelSignIn: (...args: unknown[]) => cancelSignIn(...args),
  forgetSignIn: (...args: unknown[]) => forgetSignIn(...args),
  onSignInFinished: (callback: (event: unknown) => void) => onSignInFinished(callback),
}));

const TRACKED = [
  { id: "claude:claude", provider: "claude", config_dir: "~/.claude", label: null, pinned: false },
];

const loadTracked = vi.fn(() => Promise.resolve(TRACKED));
const saveTracked = vi.fn();

vi.mock("../lib/persistence", () => ({
  loadTracked: () => loadTracked(),
  saveTracked: (...args: unknown[]) => saveTracked(...args),
}));

// vi.mock calls above are hoisted by Vitest, so this static import safely
// resolves against the mocked modules.
import { deriveAccountLabel, STOP_TRACKING_UNDO_MS, useSubscriptions } from "./useSubscriptions";

async function flush() {
  await act(async () => {
    await vi.advanceTimersByTimeAsync(0);
  });
}

// In jsdom, with no "__TAURI_INTERNALS__" global, the hook takes its
// browser/mock-harness branch: a one-time initial read via fetchSnapshot.
// The native branch, subscribing to the Rust scheduler's quota-refresh
// push, is exercised separately below by setting that global before
// rendering.
describe("useSubscriptions refresh policy in the browser mock harness path", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    fetchSnapshot.mockReset();
    setTrayStatus.mockReset();
    fetchSnapshot.mockResolvedValue({
      account_id: "claude:claude",
      provider: "claude",
      config_dir: "~/.claude",
      fetched_at: new Date().toISOString(),
      usage: {
        limits: [{ kind: "session", percent: 10, is_active: true, resets_at: null, scope: null }],
      },
      profile: null,
    });
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  // There is no JS timer left at all, so time passing, however many
  // simulated open/close cycles, can never spend budget on its own.
  test("does not fetch again merely because time passes, since no JS timer exists anymore", async () => {
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

  test("an explicit manual refresh spends budget on demand", async () => {
    const { result } = renderHook(() => useSubscriptions());
    await flush();
    expect(fetchSnapshot).toHaveBeenCalledTimes(1);

    await act(async () => {
      await result.current.refreshAll();
    });
    expect(fetchSnapshot).toHaveBeenCalledTimes(2);
  });

  // The manual refresh control is debounced: concurrent calls must
  // collapse into a single in-flight fetch, not double-fire.
  test("concurrent refreshAll calls collapse into a single in-flight fetch", async () => {
    const { result } = renderHook(() => useSubscriptions());
    await flush();
    expect(fetchSnapshot).toHaveBeenCalledTimes(1);

    await act(async () => {
      await Promise.all([
        result.current.refreshAll(),
        result.current.refreshAll(),
        result.current.refreshAll(),
      ]);
    });
    // One initial mount fetch plus one from the three collapsed manual calls.
    expect(fetchSnapshot).toHaveBeenCalledTimes(2);
  });

  test("concurrent refreshAccountById calls for the same id collapse into one fetch", async () => {
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

// The header's refresh-everything and a row's own "Read now" are two UI
// paths to the same account. They must share one per-account in-flight
// guard, or pressing both spends two of the shared 5-per-300s budget slots
// on a single account and the next scheduled read gets refused early.
describe("useSubscriptions shared per-account in-flight guard", () => {
  const SNAPSHOT = {
    account_id: "claude:claude",
    provider: "claude",
    config_dir: "~/.claude",
    fetched_at: new Date().toISOString(),
    usage: {
      limits: [{ kind: "session", percent: 10, is_active: true, resets_at: null, scope: null }],
    },
    profile: null,
  };

  beforeEach(() => {
    vi.useFakeTimers();
    fetchSnapshot.mockReset();
    setTrayStatus.mockReset();
    loadTracked.mockResolvedValue(TRACKED);
    fetchSnapshot.mockResolvedValue(SNAPSHOT);
  });

  afterEach(() => {
    vi.useRealTimers();
    loadTracked.mockResolvedValue(TRACKED);
  });

  test("refreshAll joins an account's in-flight read instead of double-fetching it", async () => {
    const { result } = renderHook(() => useSubscriptions());
    await flush();
    expect(fetchSnapshot).toHaveBeenCalledTimes(1);

    let release!: (value: unknown) => void;
    fetchSnapshot.mockImplementationOnce(() => new Promise((resolve) => (release = resolve)));

    await act(async () => {
      const readNow = result.current.refreshAccountById("claude:claude");
      const readAll = result.current.refreshAll();
      // The slow row read is the only fetch in flight. refreshAll joined it.
      expect(fetchSnapshot).toHaveBeenCalledTimes(2);
      release(SNAPSHOT);
      await Promise.all([readNow, readAll]);
    });
    expect(fetchSnapshot).toHaveBeenCalledTimes(2);
  });

  test("a row's read during a slow refreshAll joins the in-flight read", async () => {
    const { result } = renderHook(() => useSubscriptions());
    await flush();
    expect(fetchSnapshot).toHaveBeenCalledTimes(1);

    let release!: (value: unknown) => void;
    fetchSnapshot.mockImplementationOnce(() => new Promise((resolve) => (release = resolve)));

    await act(async () => {
      const readAll = result.current.refreshAll();
      const readNow = result.current.refreshAccountById("claude:claude");
      expect(fetchSnapshot).toHaveBeenCalledTimes(2);
      release(SNAPSHOT);
      await Promise.all([readNow, readAll]);
    });
    expect(fetchSnapshot).toHaveBeenCalledTimes(2);
  });

  test("joining one in-flight account never skips the other accounts", async () => {
    loadTracked.mockResolvedValue([
      {
        id: "claude:claude",
        provider: "claude",
        config_dir: "~/.claude",
        label: null,
        pinned: false,
      },
      {
        id: "claude:claude-team",
        provider: "claude",
        config_dir: "~/.claude-team",
        label: null,
        pinned: false,
      },
    ]);
    const { result } = renderHook(() => useSubscriptions());
    await flush();
    expect(fetchSnapshot).toHaveBeenCalledTimes(2);

    let release!: (value: unknown) => void;
    fetchSnapshot.mockImplementationOnce(() => new Promise((resolve) => (release = resolve)));

    await act(async () => {
      const readNow = result.current.refreshAccountById("claude:claude");
      const readAll = result.current.refreshAll();
      release(SNAPSHOT);
      await Promise.all([readNow, readAll]);
    });
    // Two mount fetches plus the row read plus refreshAll fetching only the
    // other account: the joined account is not refetched, the rest still
    // are.
    expect(fetchSnapshot).toHaveBeenCalledTimes(4);
    expect(fetchSnapshot.mock.calls[3]?.[0]).toEqual(
      expect.objectContaining({ id: "claude:claude-team" }),
    );
  });

  // Adding a subscription reads it once immediately, per
  // docs/design/brief.md section 5.3 step 4, and that read spends a real
  // budget slot like any other, so it belongs to the same guard. A refresh
  // landing during that read must join it rather than starting a second
  // request for the same account. The window is widest exactly where it
  // hurts: adding an account whose token has aged out runs a bounded-20s
  // CLI renewal first, and any refresh in those 20 seconds would otherwise
  // double the spend.
  test("the add-subscription read joins the guard instead of starting a second request", async () => {
    const { result } = renderHook(() => useSubscriptions());
    await flush();
    fetchSnapshot.mockClear();

    let release!: (value: unknown) => void;
    fetchSnapshot.mockImplementationOnce(() => new Promise((resolve) => (release = resolve)));

    await act(async () => {
      result.current.addSubscription({
        id: "claude:other",
        provider: "claude",
        config_dir: "~/.claude-other",
      });
    });
    expect(fetchSnapshot).toHaveBeenCalledTimes(1);

    await act(async () => {
      const readNow = result.current.refreshAccountById("claude:other");
      // The add's own read is the only fetch in flight. This joined it.
      expect(fetchSnapshot).toHaveBeenCalledTimes(1);
      release({ ...SNAPSHOT, account_id: "claude:other", config_dir: "~/.claude-other" });
      await readNow;
    });
    expect(fetchSnapshot).toHaveBeenCalledTimes(1);
  });
});

// On the native path, automatic reads arrive as pushed `quota-refresh`
// events from the Rust scheduler, not as JS-initiated fetches. This proves
// the hook applies a pushed event exactly like a direct fetch result, and
// never calls fetchSnapshot itself for it.
describe("useSubscriptions refresh policy on the native path", () => {
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

  test("subscribes to quota-refresh and kicks the scheduler once at mount, without fetching directly", async () => {
    renderHook(() => useSubscriptions());
    await flush();
    expect(onQuotaRefresh).toHaveBeenCalledTimes(1);
    expect(kickScheduler).toHaveBeenCalledTimes(1);
    expect(fetchSnapshot).not.toHaveBeenCalled();
  });

  test("applies a pushed 'ok' event exactly like a direct refresh result", async () => {
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
          usage: {
            limits: [
              { kind: "weekly_all", percent: 42, is_active: true, resets_at: null, scope: null },
            ],
          },
          profile: null,
        },
      });
    });

    expect(result.current.subscriptions[0].state).toBe("working");
    expect(result.current.subscriptions[0].used).toBe(42);
    expect(fetchSnapshot).not.toHaveBeenCalled();
  });

  test("a pushed rate_limited event never overwrites prior health state on the native path", async () => {
    const { result } = renderHook(() => useSubscriptions());
    await flush();

    await act(async () => {
      quotaRefreshCallback?.({
        kind: "err",
        account_id: "claude:claude",
        error: {
          kind: "unauthorized",
          message: "still unauthorized after refreshing the credential",
        },
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

// A diagnosable failure such as an expired login becoming broken must
// never decay into the generic rate-limit wait, even when a retry of that
// same broken account comes back rate-limited.
describe("useSubscriptions health vs. rate-limit precedence on the manual refresh path", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    setTrayStatus.mockReset();
    fetchSnapshot.mockReset();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  test("a rate-limited retry restores the prior Broken state instead of leaving it 'connecting'", async () => {
    fetchSnapshot
      .mockRejectedValueOnce({
        kind: "unauthorized",
        message: "still unauthorized after refreshing the credential",
      })
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

  // The restore rule's blind spot: for an account that had never been read
  // at all, the prior state captured before the attempt is the seeded
  // in-flight "connecting". Writing that back would leave the row claiming
  // "Reading…" forever with nothing in flight, and the footer's reading
  // branch would mask the "Waiting for the rate budget" note entirely.
  test("a first-ever read that is rate-limited settles to idle, never a permanent 'Reading…'", async () => {
    fetchSnapshot.mockRejectedValueOnce({ kind: "rate_limited", retry_after_secs: 214 });

    const { result } = renderHook(() => useSubscriptions());
    await flush();

    const sub = result.current.subscriptions[0];
    expect(sub.state).toBe("idle");
    expect(sub.rateLimitedUntil).not.toBeNull();
    expect(sub.needsSignIn).toBe(false);
  });
});

// A wrong diagnosis is survivable only if the user can re-test it, so both
// manual refresh paths must still attempt a real read while a
// provider-issued wait is pending, even though that wait may itself be a
// consequence of the wrong diagnosis.
describe("useSubscriptions revival while a rate-limit wait is pending", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    setTrayStatus.mockReset();
    fetchSnapshot.mockReset();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  const goodRead = {
    account_id: "claude:claude",
    provider: "claude",
    config_dir: "~/.claude",
    fetched_at: new Date().toISOString(),
    usage: {
      limits: [{ kind: "weekly_all", percent: 7, is_active: true, resets_at: null, scope: null }],
    },
    profile: null,
  };

  test("'Read now' still attempts a read while a wait is pending, and revives the row when it succeeds", async () => {
    fetchSnapshot
      // Launch: an expired-looking credential reads as broken.
      .mockRejectedValueOnce({
        kind: "unauthorized",
        message: "the stored sign-in is no longer accepted",
      })
      // The account is then throttled for an hour.
      .mockRejectedValueOnce({ kind: "rate_limited", retry_after_secs: 3540 })
      // The next explicit press must still reach the provider.
      .mockResolvedValueOnce(goodRead);

    const { result } = renderHook(() => useSubscriptions());
    await flush();
    expect(result.current.subscriptions[0].state).toBe("broken");

    await act(async () => {
      await result.current.refreshAccountById("claude:claude");
    });
    expect(fetchSnapshot).toHaveBeenCalledTimes(2);
    expect(result.current.subscriptions[0].rateLimitedUntil).not.toBeNull();

    await act(async () => {
      await result.current.refreshAccountById("claude:claude");
    });
    expect(fetchSnapshot).toHaveBeenCalledTimes(3);
    expect(result.current.subscriptions[0].state).toBe("working");
    expect(result.current.subscriptions[0].used).toBe(7);
    expect(result.current.subscriptions[0].needsSignIn).toBe(false);
    expect(result.current.subscriptions[0].rateLimitedUntil).toBeNull();
  });

  test("the panel's refresh-all also still attempts while a wait is pending", async () => {
    fetchSnapshot
      .mockRejectedValueOnce({ kind: "rate_limited", retry_after_secs: 3540 })
      .mockResolvedValueOnce(goodRead);

    const { result } = renderHook(() => useSubscriptions());
    await flush();
    expect(result.current.subscriptions[0].rateLimitedUntil).not.toBeNull();

    await act(async () => {
      await result.current.refreshAll();
    });
    expect(fetchSnapshot).toHaveBeenCalledTimes(2);
    expect(result.current.subscriptions[0].state).toBe("working");
  });

  test("adding a subscription reads it once immediately, as the Subscriptions view promises", async () => {
    fetchSnapshot.mockResolvedValue(goodRead);
    const { result } = renderHook(() => useSubscriptions());
    await flush();
    fetchSnapshot.mockClear();

    await act(async () => {
      result.current.removeSubscription("claude:claude");
    });
    await act(async () => {
      result.current.addSubscription({
        id: "claude:claude",
        provider: "claude",
        config_dir: "~/.claude",
      });
    });
    await flush();

    expect(fetchSnapshot).toHaveBeenCalledTimes(1);
    expect(result.current.subscriptions[0].state).toBe("working");
    expect(result.current.subscriptions[0].lastReadAt).not.toBeNull();
  });

  test("a sign-in warning is dropped the moment a read succeeds, with no relaunch and no remove-and-re-add", async () => {
    // Signing in externally while Quotos is running must clear the warning
    // on the very next read by itself.
    fetchSnapshot
      .mockRejectedValueOnce({
        kind: "unauthorized",
        message: "the stored sign-in is no longer accepted",
      })
      .mockResolvedValueOnce(goodRead);

    const { result } = renderHook(() => useSubscriptions());
    await flush();
    expect(result.current.subscriptions[0].needsSignIn).toBe(true);

    await act(async () => {
      await result.current.refreshAccountById("claude:claude");
    });
    expect(result.current.subscriptions[0].needsSignIn).toBe(false);
    expect(result.current.subscriptions[0].state).toBe("working");
  });

  test("a credential Quotos couldn't renew is reported as such, never as a sign-in problem", async () => {
    fetchSnapshot.mockRejectedValueOnce({
      kind: "credential_stale",
      message: "This account's access token has expired and Quotos couldn't renew it here.",
    });

    const { result } = renderHook(() => useSubscriptions());
    await flush();

    expect(result.current.subscriptions[0].needsSignIn).toBe(false);
    expect(result.current.subscriptions[0].reason).not.toMatch(/sign-in expired/i);
  });
});

// A broken pin must never show "!" and a numberless pin must never show
// "…", only be absent. If any pinned value is stale, every digit in the
// tray turns amber, not just that account's.
describe("useSubscriptions tray segments", () => {
  const TWO_PINNED = [
    { id: "claude:claude", provider: "claude", config_dir: "~/.claude", label: null, pinned: true },
    {
      id: "claude:team",
      provider: "claude",
      config_dir: "~/.claude-team",
      label: null,
      pinned: true,
    },
  ];

  beforeEach(() => {
    vi.useFakeTimers();
    fetchSnapshot.mockReset();
    setTrayStatus.mockReset();
    loadTracked.mockResolvedValue(TWO_PINNED);
  });

  afterEach(() => {
    vi.useRealTimers();
    loadTracked.mockResolvedValue(TRACKED);
  });

  test("a broken pin with no number contributes no segment at all, never '!'", async () => {
    fetchSnapshot.mockImplementation(async (account: { id: string }) => {
      if (account.id === "claude:claude") {
        throw {
          kind: "unauthorized",
          message: "still unauthorized after refreshing the credential",
        };
      }
      return {
        account_id: account.id,
        provider: "claude",
        config_dir: "~/.claude-team",
        fetched_at: new Date().toISOString(),
        usage: {
          limits: [
            { kind: "weekly_all", percent: 40, is_active: true, resets_at: null, scope: null },
          ],
        },
        profile: null,
      };
    });

    renderHook(() => useSubscriptions());
    await flush();

    const calls = setTrayStatus.mock.calls;
    const lastCall = calls[calls.length - 1]?.[0];
    expect(lastCall).toEqual([{ text: "40%", color: "neutral", groupStart: false }]);
    for (const call of setTrayStatus.mock.calls) {
      for (const segment of call[0]) {
        expect(segment.text).not.toBe("!");
        expect(segment.text).not.toContain("…");
      }
    }
  });

  test("one stale pinned account turns every pinned digit amber, not just its own", async () => {
    fetchSnapshot.mockImplementation(async (account: { id: string }) => {
      const base = {
        account_id: account.id,
        provider: "claude",
        config_dir: account.id === "claude:claude" ? "~/.claude" : "~/.claude-team",
        fetched_at: new Date().toISOString(),
        profile: null,
      };
      if (account.id === "claude:claude") {
        return {
          ...base,
          usage: {
            limits: [
              { kind: "weekly_all", percent: 10, is_active: true, resets_at: null, scope: null },
            ],
          },
        };
      }
      return {
        ...base,
        usage: {
          limits: [
            { kind: "weekly_all", percent: 20, is_active: true, resets_at: null, scope: null },
          ],
        },
      };
    });

    const { result } = renderHook(() => useSubscriptions());
    await flush();
    expect(setTrayStatus.mock.calls[setTrayStatus.mock.calls.length - 1]?.[0]).toEqual(
      expect.arrayContaining([
        { text: "10%", color: "neutral", groupStart: false },
        { text: "20%", color: "neutral", groupStart: true },
      ]),
    );

    // The team account now goes stale: a failed retry after real data.
    fetchSnapshot.mockImplementationOnce(async () => {
      throw { kind: "network", message: "the connection timed out" };
    });
    await act(async () => {
      await result.current.refreshAccountById("claude:team");
    });

    const trayCalls = setTrayStatus.mock.calls;
    const segments = trayCalls[trayCalls.length - 1]?.[0];
    expect(segments).toEqual(
      expect.arrayContaining([
        { text: "10%", color: "amber", groupStart: false },
        { text: "20%", color: "amber", groupStart: true },
      ]),
    );

    // The same call carries the tooltip that names those bare digits, and
    // the amber-everywhere rule stays digits-only: only the stale account's
    // own tooltip line says "not current".
    const tooltip = trayCalls[trayCalls.length - 1]?.[2];
    expect(tooltip).toMatch(/^Quotos\n/);
    expect(tooltip).toContain("Weekly 10%");
    expect(tooltip).toContain("Weekly 20% — not current");
    expect(tooltip).not.toContain("Weekly 10% — not current");
  });
});

// An older tracked record's `pinned: true` becomes "that subscription's
// headline window is pinned", known only once a read reveals
// `headlineWindowId`.
describe("useSubscriptions pin migration from a legacy pinned boolean", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    fetchSnapshot.mockReset();
    setTrayStatus.mockReset();
    saveTracked.mockReset();
  });

  afterEach(() => {
    vi.useRealTimers();
    loadTracked.mockResolvedValue(TRACKED);
  });

  test("migrates a legacy pinned:true record to pinning its headline window, once a read reveals it", async () => {
    loadTracked.mockResolvedValue([
      {
        id: "claude:claude",
        provider: "claude",
        config_dir: "~/.claude",
        label: null,
        pinned: true,
      },
    ]);
    fetchSnapshot.mockResolvedValue({
      account_id: "claude:claude",
      provider: "claude",
      config_dir: "~/.claude",
      fetched_at: new Date().toISOString(),
      usage: {
        limits: [
          { kind: "weekly_all", percent: 33, is_active: true, resets_at: null, scope: null },
        ],
      },
      profile: null,
    });

    const { result } = renderHook(() => useSubscriptions());
    await flush();

    expect(result.current.subscriptions[0].headlineWindowId).toBe("weekly_all");
    expect(result.current.subscriptions[0].pinnedWindowIds).toEqual(["weekly_all"]);
    expect(setTrayStatus.mock.calls[setTrayStatus.mock.calls.length - 1]?.[0]).toEqual([
      { text: "33%", color: "neutral", groupStart: false },
    ]);
  });

  test("a legacy pinned:false record migrates to nothing pinned, contributing no segment", async () => {
    loadTracked.mockResolvedValue([
      {
        id: "claude:claude",
        provider: "claude",
        config_dir: "~/.claude",
        label: null,
        pinned: false,
      },
    ]);
    fetchSnapshot.mockResolvedValue({
      account_id: "claude:claude",
      provider: "claude",
      config_dir: "~/.claude",
      fetched_at: new Date().toISOString(),
      usage: {
        limits: [
          { kind: "weekly_all", percent: 33, is_active: true, resets_at: null, scope: null },
        ],
      },
      profile: null,
    });

    const { result } = renderHook(() => useSubscriptions());
    await flush();

    expect(result.current.subscriptions[0].pinnedWindowIds).toEqual([]);
    expect(setTrayStatus.mock.calls[setTrayStatus.mock.calls.length - 1]?.[0]).toEqual([]);
  });

  test("a pending migration survives a failed first read and completes on the next successful one", async () => {
    loadTracked.mockResolvedValue([
      {
        id: "claude:claude",
        provider: "claude",
        config_dir: "~/.claude",
        label: null,
        pinned: true,
      },
    ]);
    fetchSnapshot
      .mockRejectedValueOnce({ kind: "network", message: "timed out" })
      .mockResolvedValueOnce({
        account_id: "claude:claude",
        provider: "claude",
        config_dir: "~/.claude",
        fetched_at: new Date().toISOString(),
        usage: {
          limits: [
            { kind: "weekly_all", percent: 8, is_active: true, resets_at: null, scope: null },
          ],
        },
        profile: null,
      });

    const { result } = renderHook(() => useSubscriptions());
    await flush();
    expect(result.current.subscriptions[0].pinnedWindowIds).toEqual([]);

    await act(async () => {
      await result.current.refreshAccountById("claude:claude");
    });
    expect(result.current.subscriptions[0].pinnedWindowIds).toEqual(["weekly_all"]);
  });
});

// startSignIn, submitSignInCode, cancelSignIn and the sign-in-finished
// reaction. Quotos never inspects a credential itself, so a finished
// session, success or failure, always triggers a real re-read rather than
// trusting the process's exit status alone.
describe("useSubscriptions sign-in flow", () => {
  let signInFinishedCallback:
    | ((event: { account_id: string; success: boolean }) => void)
    | undefined;

  beforeEach(() => {
    vi.useFakeTimers();
    fetchSnapshot.mockReset();
    setTrayStatus.mockReset();
    startSignIn.mockReset();
    submitSignInCode.mockReset();
    cancelSignIn.mockReset();
    forgetSignIn.mockReset();
    signInFinishedCallback = undefined;
    onSignInFinished.mockImplementation(
      (cb: (event: { account_id: string; success: boolean }) => void) => {
        signInFinishedCallback = cb;
        return Promise.resolve(() => {});
      },
    );
    startSignIn.mockResolvedValue(undefined);
    fetchSnapshot.mockRejectedValue({
      kind: "unauthorized",
      message: "still unauthorized after refreshing the credential",
    });
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  test("startSignIn marks the row in-progress and calls the IPC with its config dir", async () => {
    const { result } = renderHook(() => useSubscriptions());
    await flush();

    await act(async () => {
      await result.current.startSignIn("claude:claude");
    });

    expect(startSignIn).toHaveBeenCalledWith("claude:claude", "~/.claude");
    expect(result.current.subscriptions[0].signInInProgress).toBe(true);
  });

  test("a finished sign-in clears signInInProgress and triggers a re-read", async () => {
    const { result } = renderHook(() => useSubscriptions());
    await flush();

    await act(async () => {
      await result.current.startSignIn("claude:claude");
    });
    expect(result.current.subscriptions[0].signInInProgress).toBe(true);

    fetchSnapshot.mockResolvedValueOnce({
      account_id: "claude:claude",
      provider: "claude",
      config_dir: "~/.claude",
      fetched_at: new Date().toISOString(),
      usage: {
        limits: [{ kind: "weekly_all", percent: 5, is_active: true, resets_at: null, scope: null }],
      },
      profile: null,
    });

    await act(async () => {
      signInFinishedCallback?.({ account_id: "claude:claude", success: true });
      await vi.advanceTimersByTimeAsync(0);
    });

    expect(result.current.subscriptions[0].signInInProgress).toBe(false);
    expect(forgetSignIn).toHaveBeenCalledWith("claude:claude");
    expect(result.current.subscriptions[0].state).toBe("working");
    expect(result.current.subscriptions[0].used).toBe(5);
  });

  test("cancelSignIn calls the IPC and clears the in-progress flag", async () => {
    const { result } = renderHook(() => useSubscriptions());
    await flush();
    await act(async () => {
      await result.current.startSignIn("claude:claude");
    });

    await act(async () => {
      await result.current.cancelSignIn("claude:claude");
    });

    expect(cancelSignIn).toHaveBeenCalledWith("claude:claude");
    expect(result.current.subscriptions[0].signInInProgress).toBe(false);
  });
});

// Stop-tracking untracks now everywhere. The undo window is nothing but a
// slot the panel keeps.
describe("useSubscriptions stop-tracking is immediate everywhere but the panel's own slot", () => {
  const TWO = [
    { id: "claude:claude", provider: "claude", config_dir: "~/.claude", label: null, pinned: true },
    {
      id: "claude:claude-team",
      provider: "claude",
      config_dir: "~/.claude-team",
      label: null,
      pinned: true,
    },
  ];

  beforeEach(() => {
    vi.useFakeTimers();
    fetchSnapshot.mockReset();
    setTrayStatus.mockReset();
    saveTracked.mockReset();
    cancelSignIn.mockReset();
    loadTracked.mockResolvedValue(TWO);
    fetchSnapshot.mockImplementation(async (account: { id: string; config_dir: string }) => ({
      account_id: account.id,
      provider: "claude",
      config_dir: account.config_dir,
      fetched_at: new Date().toISOString(),
      usage: {
        limits: [
          { kind: "weekly_all", percent: 40, is_active: true, resets_at: null, scope: null },
        ],
      },
      profile: null,
    }));
  });

  afterEach(() => {
    vi.useRealTimers();
    loadTracked.mockResolvedValue(TRACKED);
  });

  test("drops the account from the tracked list the moment it is pressed, while the panel keeps its slot", async () => {
    const { result } = renderHook(() => useSubscriptions());
    await flush();

    act(() => result.current.stopTracking("claude:claude-team"));

    // What the Subscriptions screen sees: gone, immediately.
    expect(result.current.trackedSubscriptions.map((s) => s.id)).toEqual(["claude:claude"]);
    // What the panel sees: still there, in its own slot, flagged for the Undo row.
    expect(result.current.subscriptions.map((s) => s.id)).toEqual([
      "claude:claude",
      "claude:claude-team",
    ]);
    expect(result.current.subscriptions[1].pendingRemoval).toBe(true);
  });

  test("persists the removal immediately, not when the undo window expires", async () => {
    const { result } = renderHook(() => useSubscriptions());
    await flush();
    saveTracked.mockClear();

    act(() => result.current.stopTracking("claude:claude-team"));
    await flush();

    const saved = saveTracked.mock.calls[saveTracked.mock.calls.length - 1]?.[0];
    expect(saved.map((t: { id: string }) => t.id)).toEqual(["claude:claude"]);
  });

  test("drops a pinned account's tray digits immediately", async () => {
    const { result } = renderHook(() => useSubscriptions());
    await flush();
    expect(setTrayStatus.mock.calls[setTrayStatus.mock.calls.length - 1]?.[0]).toHaveLength(2);

    act(() => result.current.stopTracking("claude:claude-team"));
    await flush();

    expect(setTrayStatus.mock.calls[setTrayStatus.mock.calls.length - 1]?.[0]).toHaveLength(1);
  });

  test("undo restores it in its original slot, with its data intact", async () => {
    const { result } = renderHook(() => useSubscriptions());
    await flush();
    const before = result.current.subscriptions[1];

    act(() => result.current.stopTracking("claude:claude-team"));
    act(() => result.current.undoStopTracking("claude:claude-team"));

    expect(result.current.trackedSubscriptions.map((s) => s.id)).toEqual([
      "claude:claude",
      "claude:claude-team",
    ]);
    expect(result.current.subscriptions[1].pendingRemoval).toBe(false);
    expect(result.current.subscriptions[1].used).toBe(before.used);
  });

  test("keeps the slot for exactly the undo window, then gives it up", async () => {
    const { result } = renderHook(() => useSubscriptions());
    await flush();

    act(() => result.current.stopTracking("claude:claude-team"));
    await act(async () => {
      await vi.advanceTimersByTimeAsync(STOP_TRACKING_UNDO_MS - 1);
    });
    expect(result.current.subscriptions).toHaveLength(2);

    await act(async () => {
      await vi.advanceTimersByTimeAsync(2);
    });
    expect(result.current.subscriptions.map((s) => s.id)).toEqual(["claude:claude"]);
  });

  test("an undone row is never removed by its own expired timer", async () => {
    const { result } = renderHook(() => useSubscriptions());
    await flush();

    act(() => result.current.stopTracking("claude:claude-team"));
    act(() => result.current.undoStopTracking("claude:claude-team"));
    await act(async () => {
      await vi.advanceTimersByTimeAsync(STOP_TRACKING_UNDO_MS * 2);
    });

    expect(result.current.subscriptions.map((s) => s.id)).toEqual([
      "claude:claude",
      "claude:claude-team",
    ]);
  });

  test("adding an account back during its undo window is the same as undoing it", async () => {
    const { result } = renderHook(() => useSubscriptions());
    await flush();

    act(() => result.current.stopTracking("claude:claude-team"));
    act(() =>
      result.current.addSubscription({
        id: "claude:claude-team",
        provider: "claude",
        config_dir: "~/.claude-team",
      }),
    );
    await act(async () => {
      await vi.advanceTimersByTimeAsync(STOP_TRACKING_UNDO_MS * 2);
    });

    expect(result.current.subscriptions.map((s) => s.id)).toEqual([
      "claude:claude",
      "claude:claude-team",
    ]);
    expect(result.current.subscriptions[1].pendingRemoval).toBe(false);
  });

  // The save effect is keyed on the whole subscription list. Without a
  // compare against the last written value, every automatic read would
  // write the tracked file again, each write a temp file plus an `fsync`
  // plus a rename, on the main thread.
  test("does not re-persist when only read state changed", async () => {
    const { result } = renderHook(() => useSubscriptions());
    await flush();
    saveTracked.mockClear();

    await act(async () => {
      await result.current.refreshAll();
    });

    expect(saveTracked).not.toHaveBeenCalled();
  });

  // The save effect records what it handed to `saveTracked` before the
  // write settles, the dedupe from the test above, so a write that then
  // fails must not stay recorded as saved, or it would never be retried
  // and the rename would die with the process. A failed save clears that
  // record, which turns the very next effect run, even one where only
  // read state changed, into the retry.
  test("retries a failed save on the next change instead of remembering it as saved", async () => {
    const consoleError = vi.spyOn(console, "error").mockImplementation(() => {});
    try {
      const { result } = renderHook(() => useSubscriptions());
      await flush();
      saveTracked.mockClear();
      saveTracked.mockRejectedValueOnce(new Error("disk full"));

      act(() => result.current.renameSubscription("claude:claude", "Renamed Personal"));
      await flush();
      expect(saveTracked).toHaveBeenCalledTimes(1);
      expect(consoleError).toHaveBeenCalled();

      await act(async () => {
        await result.current.refreshAll();
      });

      expect(saveTracked).toHaveBeenCalledTimes(2);
      expect(saveTracked.mock.calls[1][0]).toEqual(saveTracked.mock.calls[0][0]);
    } finally {
      consoleError.mockRestore();
    }
  });
});

// "Move up" and "Move down" in the row menu. Panel order is the one order
// everywhere, since the persisted list and the tray's digit order both
// derive from `subscriptions`' own array order, so a swap must show up in
// all three, and an impossible move must change nothing, not even a save.
describe("useSubscriptions reordering", () => {
  const TWO = [
    { id: "claude:claude", provider: "claude", config_dir: "~/.claude", label: null, pinned: true },
    {
      id: "claude:claude-team",
      provider: "claude",
      config_dir: "~/.claude-team",
      label: null,
      pinned: true,
    },
  ];

  beforeEach(() => {
    vi.useFakeTimers();
    fetchSnapshot.mockReset();
    setTrayStatus.mockReset();
    saveTracked.mockReset();
    loadTracked.mockResolvedValue(TWO);
    fetchSnapshot.mockImplementation(async (account: { id: string; config_dir: string }) => ({
      account_id: account.id,
      provider: "claude",
      config_dir: account.config_dir,
      fetched_at: new Date().toISOString(),
      usage: {
        limits: [
          {
            kind: "weekly_all",
            percent: account.id === "claude:claude" ? 40 : 70,
            is_active: true,
            resets_at: null,
            scope: null,
          },
        ],
      },
      profile: null,
    }));
  });

  afterEach(() => {
    vi.useRealTimers();
    loadTracked.mockResolvedValue(TRACKED);
  });

  test("swaps the row with its neighbor and persists the new order", async () => {
    const { result } = renderHook(() => useSubscriptions());
    await flush();
    saveTracked.mockClear();

    act(() => result.current.moveSubscription("claude:claude", "down"));
    await flush();

    expect(result.current.subscriptions.map((s) => s.id)).toEqual([
      "claude:claude-team",
      "claude:claude",
    ]);
    const saved = saveTracked.mock.calls[saveTracked.mock.calls.length - 1]?.[0];
    expect(saved.map((t: { id: string }) => t.id)).toEqual(["claude:claude-team", "claude:claude"]);
  });

  test("moving up then down lands back where it started", async () => {
    const { result } = renderHook(() => useSubscriptions());
    await flush();

    act(() => result.current.moveSubscription("claude:claude-team", "up"));
    expect(result.current.subscriptions.map((s) => s.id)).toEqual([
      "claude:claude-team",
      "claude:claude",
    ]);

    act(() => result.current.moveSubscription("claude:claude-team", "down"));
    expect(result.current.subscriptions.map((s) => s.id)).toEqual([
      "claude:claude",
      "claude:claude-team",
    ]);
  });

  test("is a no-op at the edges, with no reorder and no save", async () => {
    const { result } = renderHook(() => useSubscriptions());
    await flush();
    saveTracked.mockClear();

    act(() => result.current.moveSubscription("claude:claude", "up"));
    act(() => result.current.moveSubscription("claude:claude-team", "down"));
    await flush();

    expect(result.current.subscriptions.map((s) => s.id)).toEqual([
      "claude:claude",
      "claude:claude-team",
    ]);
    expect(saveTracked).not.toHaveBeenCalled();
  });

  test("reorders the tray digits with it", async () => {
    const { result } = renderHook(() => useSubscriptions());
    await flush();
    const before = setTrayStatus.mock.calls[setTrayStatus.mock.calls.length - 1]?.[0];
    expect(before.map((s: { text: string }) => s.text)).toEqual(["40%", "70%"]);

    act(() => result.current.moveSubscription("claude:claude", "down"));
    await flush();

    const after = setTrayStatus.mock.calls[setTrayStatus.mock.calls.length - 1]?.[0];
    expect(after.map((s: { text: string }) => s.text)).toEqual(["70%", "40%"]);
  });
});

// One account, one spelling, everywhere.
describe("useSubscriptions display names are consistent between the panel and the Subscriptions screen", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    fetchSnapshot.mockReset();
    setTrayStatus.mockReset();
    loadTracked.mockResolvedValue(TRACKED);
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  test("titles the id-derived fallback the way every other name in the panel is titled", () => {
    expect(
      deriveAccountLabel({ id: "claude:claude", provider: "claude", config_dir: "~/.claude" }),
    ).toBe("Claude");
    expect(
      deriveAccountLabel({
        id: "claude:claude-team",
        provider: "claude",
        config_dir: "~/.claude-team",
      }),
    ).toBe("Claude Team");
    expect(
      deriveAccountLabel({
        id: "claude:work_eu",
        provider: "claude",
        config_dir: "~/.claude-work_eu",
      }),
    ).toBe("Work Eu");
  });

  test("keeps the provider's own name for an account after it stops being tracked", async () => {
    fetchSnapshot.mockResolvedValue({
      account_id: "claude:claude",
      provider: "claude",
      config_dir: "~/.claude",
      fetched_at: new Date().toISOString(),
      usage: {
        limits: [
          { kind: "weekly_all", percent: 12, is_active: true, resets_at: null, scope: null },
        ],
      },
      profile: { organization: { name: "Claude Max", organization_type: "claude_max" } },
    });

    const { result } = renderHook(() => useSubscriptions());
    await flush();
    expect(result.current.subscriptions[0].label).toBe("Claude Max");

    act(() => result.current.removeSubscription("claude:claude"));

    // The Subscriptions screen asks for this account by descriptor now
    // that there is no subscription to read a label off. It must still be
    // the name the provider reported, not the config directory's.
    expect(
      result.current.displayLabelFor({
        id: "claude:claude",
        provider: "claude",
        config_dir: "~/.claude",
      }),
    ).toBe("Claude Max");
  });

  test("falls back to the id-derived title for an account never read", async () => {
    const { result } = renderHook(() => useSubscriptions());
    await flush();
    expect(
      result.current.displayLabelFor({
        id: "claude:claude-team",
        provider: "claude",
        config_dir: "~/.claude-team",
      }),
    ).toBe("Claude Team");
  });
});
