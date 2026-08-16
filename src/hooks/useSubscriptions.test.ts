import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

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
import { accountLabel, STOP_TRACKING_UNDO_MS, useSubscriptions } from "./useSubscriptions";

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

// F4: the header's refresh-everything and a row's own "Read now" are two UI
// paths to the same account — they must share one per-account in-flight
// guard, or pressing both spends two of the shared 5-per-300s budget slots
// on a single account and the next scheduled read gets refused early.
describe("useSubscriptions shared per-account in-flight guard (F4)", () => {
  const SNAPSHOT = {
    account_id: "claude:claude",
    provider: "claude",
    config_dir: "~/.claude",
    fetched_at: new Date().toISOString(),
    usage: { limits: [{ kind: "session", percent: 10, is_active: true, resets_at: null, scope: null }] },
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

  it("refreshAll joins an account's in-flight read instead of double-fetching it", async () => {
    const { result } = renderHook(() => useSubscriptions());
    await flush();
    expect(fetchSnapshot).toHaveBeenCalledTimes(1);

    let release!: (value: unknown) => void;
    fetchSnapshot.mockImplementationOnce(() => new Promise((resolve) => (release = resolve)));

    await act(async () => {
      const readNow = result.current.refreshAccountById("claude:claude");
      const readAll = result.current.refreshAll();
      // The slow row read is the only fetch in flight — refreshAll joined it.
      expect(fetchSnapshot).toHaveBeenCalledTimes(2);
      release(SNAPSHOT);
      await Promise.all([readNow, readAll]);
    });
    expect(fetchSnapshot).toHaveBeenCalledTimes(2);
  });

  it("a row's read during a slow refreshAll joins the in-flight read", async () => {
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

  it("joining one in-flight account never skips the other accounts", async () => {
    loadTracked.mockResolvedValue([
      { id: "claude:claude", provider: "claude", config_dir: "~/.claude", label: null, pinned: false },
      { id: "claude:claude-team", provider: "claude", config_dir: "~/.claude-team", label: null, pinned: false },
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
    // Mount (2) + the row read (1) + refreshAll fetching only the *other*
    // account (1): the joined account is not refetched, the rest still are.
    expect(fetchSnapshot).toHaveBeenCalledTimes(4);
    expect(fetchSnapshot.mock.calls[3]?.[0]).toEqual(expect.objectContaining({ id: "claude:claude-team" }));
  });

  // Adding a subscription reads it once immediately (brief §5.3 step 4), and
  // that read spends a real budget slot like any other — so it belongs to the
  // same guard. It used to call `refreshOne` directly, which never registered
  // the read as in-flight, so a refresh landing during it started a *second*
  // request for the same account. The window is widest exactly where it hurts:
  // adding an account whose token has aged out runs a bounded-20s CLI renewal
  // first, and any refresh in those 20s doubled the spend.
  it("the add-subscription read joins the guard instead of starting a second request", async () => {
    const { result } = renderHook(() => useSubscriptions());
    await flush();
    fetchSnapshot.mockClear();

    let release!: (value: unknown) => void;
    fetchSnapshot.mockImplementationOnce(() => new Promise((resolve) => (release = resolve)));

    await act(async () => {
      result.current.addSubscription({ id: "claude:other", provider: "claude", config_dir: "~/.claude-other" });
    });
    expect(fetchSnapshot).toHaveBeenCalledTimes(1);

    await act(async () => {
      const readNow = result.current.refreshAccountById("claude:other");
      // The add's own read is the only fetch in flight — this joined it.
      expect(fetchSnapshot).toHaveBeenCalledTimes(1);
      release({ ...SNAPSHOT, account_id: "claude:other", config_dir: "~/.claude-other" });
      await readNow;
    });
    expect(fetchSnapshot).toHaveBeenCalledTimes(1);
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

  // R5: the restore rule's blind spot. For an account that had never been
  // read at all, the "prior" state captured before the attempt is the seeded
  // in-flight "connecting" — writing that back left the row claiming
  // "Reading…" forever (nothing was in flight), and the footer's reading
  // branch masked the "Waiting for the rate budget" note entirely. Found by
  // driving the mock harness's demo-waiting account in a real browser.
  it("a first-ever read that is rate-limited settles to idle — never a permanent 'Reading…'", async () => {
    fetchSnapshot.mockRejectedValueOnce({ kind: "rate_limited", retry_after_secs: 214 });

    const { result } = renderHook(() => useSubscriptions());
    await flush();

    const sub = result.current.subscriptions[0];
    expect(sub.state).toBe("idle");
    expect(sub.rateLimitedUntil).not.toBeNull();
    expect(sub.needsSignIn).toBe(false);
  });
});

// R3-4: revival. A wrong diagnosis is survivable if the user can re-test
// it; the captain's build made that impossible, because both manual paths
// silently returned without doing anything while a provider-issued wait was
// pending — and that wait was itself a consequence of the wrong diagnosis.
describe("useSubscriptions revival while a rate-limit wait is pending (R3-4)", () => {
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
    usage: { limits: [{ kind: "weekly_all", percent: 7, is_active: true, resets_at: null, scope: null }] },
    profile: null,
  };

  it("'Read now' still attempts a read while a wait is pending, and revives the row when it succeeds", async () => {
    fetchSnapshot
      // Launch: an expired-looking credential reads as broken…
      .mockRejectedValueOnce({ kind: "unauthorized", message: "the stored sign-in is no longer accepted" })
      // …then the account is throttled for an hour.
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

  it("the panel's refresh-all also still attempts while a wait is pending", async () => {
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

  it("adding a subscription reads it once immediately, as the Subscriptions view promises", async () => {
    fetchSnapshot.mockResolvedValue(goodRead);
    const { result } = renderHook(() => useSubscriptions());
    await flush();
    fetchSnapshot.mockClear();

    await act(async () => {
      result.current.removeSubscription("claude:claude");
    });
    await act(async () => {
      result.current.addSubscription({ id: "claude:claude", provider: "claude", config_dir: "~/.claude" });
    });
    await flush();

    expect(fetchSnapshot).toHaveBeenCalledTimes(1);
    expect(result.current.subscriptions[0].state).toBe("working");
    expect(result.current.subscriptions[0].lastReadAt).not.toBeNull();
  });

  it("a sign-in warning is dropped the moment a read succeeds — no relaunch, no remove-and-re-add", async () => {
    // The captain signs in externally while Quotos is running: the very next
    // read must clear the warning by itself.
    fetchSnapshot
      .mockRejectedValueOnce({ kind: "unauthorized", message: "the stored sign-in is no longer accepted" })
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

  it("a credential Quotos couldn't renew is reported as such, never as a sign-in problem", async () => {
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

// Followup-2: the handoff forbids any tray glyph but a digit ("Никаких
// знаков и многоточий... Либо цифра, либо ничего") — a broken pin must
// never show "!" and a numberless pin must never show "…", only be absent.
// It also restates the stale-taints-everything rule: if *any* pinned value
// is stale, every digit in the tray turns amber, not just that account's.
describe("useSubscriptions tray segments (followup-2)", () => {
  const TWO_PINNED = [
    { id: "claude:claude", provider: "claude", config_dir: "~/.claude", label: null, pinned: true },
    { id: "claude:team", provider: "claude", config_dir: "~/.claude-team", label: null, pinned: true },
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

  it("a broken pin with no number contributes no segment at all — never '!'", async () => {
    fetchSnapshot.mockImplementation(async (account: { id: string }) => {
      if (account.id === "claude:claude") {
        throw { kind: "unauthorized", message: "still unauthorized after refreshing the credential" };
      }
      return {
        account_id: account.id,
        provider: "claude",
        config_dir: "~/.claude-team",
        fetched_at: new Date().toISOString(),
        usage: { limits: [{ kind: "weekly_all", percent: 40, is_active: true, resets_at: null, scope: null }] },
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

  it("one stale pinned account turns every pinned digit amber, not just its own", async () => {
    fetchSnapshot.mockImplementation(async (account: { id: string }) => {
      const base = {
        account_id: account.id,
        provider: "claude",
        config_dir: account.id === "claude:claude" ? "~/.claude" : "~/.claude-team",
        fetched_at: new Date().toISOString(),
        profile: null,
      };
      if (account.id === "claude:claude") {
        return { ...base, usage: { limits: [{ kind: "weekly_all", percent: 10, is_active: true, resets_at: null, scope: null }] } };
      }
      return { ...base, usage: { limits: [{ kind: "weekly_all", percent: 20, is_active: true, resets_at: null, scope: null }] } };
    });

    const { result } = renderHook(() => useSubscriptions());
    await flush();
    expect(setTrayStatus.mock.calls[setTrayStatus.mock.calls.length - 1]?.[0]).toEqual(
      expect.arrayContaining([
        { text: "10%", color: "neutral", groupStart: false },
        { text: "20%", color: "neutral", groupStart: true },
      ]),
    );

    // Now the team account goes stale (a failed retry after real data).
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

    // I7: the same call carries the tooltip that names those bare digits —
    // and the amber-everywhere rule stays digits-only: only the stale
    // account's own tooltip line says "not current".
    const tooltip = trayCalls[trayCalls.length - 1]?.[2];
    expect(tooltip).toMatch(/^Quotos\n/);
    expect(tooltip).toContain("Weekly 10%");
    expect(tooltip).toContain("Weekly 20% — not current");
    expect(tooltip).not.toContain("Weekly 10% — not current");
  });
});

// v4 migration (firstmate-recorded decision, docs/design/NOTES.md §2): a pre-v4
// tracked record's `pinned: true` becomes "that subscription's headline
// window is pinned" — known only once a read reveals `headlineWindowId`.
describe("useSubscriptions v4 pin migration", () => {
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

  it("migrates a legacy pinned:true record to pinning its headline window, once a read reveals it", async () => {
    loadTracked.mockResolvedValue([
      { id: "claude:claude", provider: "claude", config_dir: "~/.claude", label: null, pinned: true },
    ]);
    fetchSnapshot.mockResolvedValue({
      account_id: "claude:claude",
      provider: "claude",
      config_dir: "~/.claude",
      fetched_at: new Date().toISOString(),
      usage: { limits: [{ kind: "weekly_all", percent: 33, is_active: true, resets_at: null, scope: null }] },
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

  it("a legacy pinned:false record migrates to nothing pinned, contributing no segment", async () => {
    loadTracked.mockResolvedValue([
      { id: "claude:claude", provider: "claude", config_dir: "~/.claude", label: null, pinned: false },
    ]);
    fetchSnapshot.mockResolvedValue({
      account_id: "claude:claude",
      provider: "claude",
      config_dir: "~/.claude",
      fetched_at: new Date().toISOString(),
      usage: { limits: [{ kind: "weekly_all", percent: 33, is_active: true, resets_at: null, scope: null }] },
      profile: null,
    });

    const { result } = renderHook(() => useSubscriptions());
    await flush();

    expect(result.current.subscriptions[0].pinnedWindowIds).toEqual([]);
    expect(setTrayStatus.mock.calls[setTrayStatus.mock.calls.length - 1]?.[0]).toEqual([]);
  });

  it("a pending migration survives a failed first read and completes on the next successful one", async () => {
    loadTracked.mockResolvedValue([
      { id: "claude:claude", provider: "claude", config_dir: "~/.claude", label: null, pinned: true },
    ]);
    fetchSnapshot
      .mockRejectedValueOnce({ kind: "network", message: "timed out" })
      .mockResolvedValueOnce({
        account_id: "claude:claude",
        provider: "claude",
        config_dir: "~/.claude",
        fetched_at: new Date().toISOString(),
        usage: { limits: [{ kind: "weekly_all", percent: 8, is_active: true, resets_at: null, scope: null }] },
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

// R2-6: startSignIn/submitSignInCode/cancelSignIn and the sign-in-finished
// reaction — Quotos never inspects a credential itself, so a finished
// session (success or failure) always triggers a real re-read rather than
// trusting the process's exit status alone.
describe("useSubscriptions sign-in flow", () => {
  let signInFinishedCallback: ((event: { account_id: string; success: boolean }) => void) | undefined;

  beforeEach(() => {
    vi.useFakeTimers();
    fetchSnapshot.mockReset();
    setTrayStatus.mockReset();
    startSignIn.mockReset();
    submitSignInCode.mockReset();
    cancelSignIn.mockReset();
    forgetSignIn.mockReset();
    signInFinishedCallback = undefined;
    onSignInFinished.mockImplementation((cb: (event: { account_id: string; success: boolean }) => void) => {
      signInFinishedCallback = cb;
      return Promise.resolve(() => {});
    });
    startSignIn.mockResolvedValue(undefined);
    fetchSnapshot.mockRejectedValue({ kind: "unauthorized", message: "still unauthorized after refreshing the credential" });
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("startSignIn marks the row in-progress and calls the IPC with its config dir", async () => {
    const { result } = renderHook(() => useSubscriptions());
    await flush();

    await act(async () => {
      await result.current.startSignIn("claude:claude");
    });

    expect(startSignIn).toHaveBeenCalledWith("claude:claude", "~/.claude");
    expect(result.current.subscriptions[0].signInInProgress).toBe(true);
  });

  it("a finished sign-in clears signInInProgress and triggers a re-read", async () => {
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
      usage: { limits: [{ kind: "weekly_all", percent: 5, is_active: true, resets_at: null, scope: null }] },
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

  it("cancelSignIn calls the IPC and clears the in-progress flag", async () => {
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

// R4-3: the captain's 2026-08-15 screencast — with both panel rows already
// stop-tracked and showing their Undo rows, the Subscriptions screen went on
// listing both accounts as tracked ([Remove]) for a further five seconds,
// because "Stop tracking" only scheduled a removal that a timer inside App.tsx
// would apply later. These pin the one-source-of-truth shape that replaced it:
// stop-tracking untracks *now* everywhere, and the undo window is nothing but a
// slot the panel keeps.
describe("useSubscriptions stop-tracking is immediate everywhere but the panel's own slot (R4-3)", () => {
  const TWO = [
    { id: "claude:claude", provider: "claude", config_dir: "~/.claude", label: null, pinned: true },
    { id: "claude:claude-team", provider: "claude", config_dir: "~/.claude-team", label: null, pinned: true },
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
      usage: { limits: [{ kind: "weekly_all", percent: 40, is_active: true, resets_at: null, scope: null }] },
      profile: null,
    }));
  });

  afterEach(() => {
    vi.useRealTimers();
    loadTracked.mockResolvedValue(TRACKED);
  });

  it("drops the account from the tracked list the moment it is pressed, while the panel keeps its slot", async () => {
    const { result } = renderHook(() => useSubscriptions());
    await flush();

    act(() => result.current.stopTracking("claude:claude-team"));

    // What the Subscriptions screen sees: gone, immediately.
    expect(result.current.trackedSubscriptions.map((s) => s.id)).toEqual(["claude:claude"]);
    // What the panel sees: still there, in its own slot, flagged for the Undo row.
    expect(result.current.subscriptions.map((s) => s.id)).toEqual(["claude:claude", "claude:claude-team"]);
    expect(result.current.subscriptions[1].pendingRemoval).toBe(true);
  });

  it("persists the removal immediately, not when the undo window expires", async () => {
    const { result } = renderHook(() => useSubscriptions());
    await flush();
    saveTracked.mockClear();

    act(() => result.current.stopTracking("claude:claude-team"));
    await flush();

    const saved = saveTracked.mock.calls[saveTracked.mock.calls.length - 1]?.[0];
    expect(saved.map((t: { id: string }) => t.id)).toEqual(["claude:claude"]);
  });

  it("drops a pinned account's tray digits immediately", async () => {
    const { result } = renderHook(() => useSubscriptions());
    await flush();
    expect(setTrayStatus.mock.calls[setTrayStatus.mock.calls.length - 1]?.[0]).toHaveLength(2);

    act(() => result.current.stopTracking("claude:claude-team"));
    await flush();

    expect(setTrayStatus.mock.calls[setTrayStatus.mock.calls.length - 1]?.[0]).toHaveLength(1);
  });

  it("undo restores it in its original slot, with its data intact", async () => {
    const { result } = renderHook(() => useSubscriptions());
    await flush();
    const before = result.current.subscriptions[1];

    act(() => result.current.stopTracking("claude:claude-team"));
    act(() => result.current.undoStopTracking("claude:claude-team"));

    expect(result.current.trackedSubscriptions.map((s) => s.id)).toEqual(["claude:claude", "claude:claude-team"]);
    expect(result.current.subscriptions[1].pendingRemoval).toBe(false);
    expect(result.current.subscriptions[1].used).toBe(before.used);
  });

  it("keeps the slot for exactly the undo window, then gives it up", async () => {
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

  it("an undone row is never removed by its own expired timer", async () => {
    const { result } = renderHook(() => useSubscriptions());
    await flush();

    act(() => result.current.stopTracking("claude:claude-team"));
    act(() => result.current.undoStopTracking("claude:claude-team"));
    await act(async () => {
      await vi.advanceTimersByTimeAsync(STOP_TRACKING_UNDO_MS * 2);
    });

    expect(result.current.subscriptions.map((s) => s.id)).toEqual(["claude:claude", "claude:claude-team"]);
  });

  it("adding an account back during its undo window is the same as undoing it", async () => {
    const { result } = renderHook(() => useSubscriptions());
    await flush();

    act(() => result.current.stopTracking("claude:claude-team"));
    act(() =>
      result.current.addSubscription({ id: "claude:claude-team", provider: "claude", config_dir: "~/.claude-team" }),
    );
    await act(async () => {
      await vi.advanceTimersByTimeAsync(STOP_TRACKING_UNDO_MS * 2);
    });

    expect(result.current.subscriptions.map((s) => s.id)).toEqual(["claude:claude", "claude:claude-team"]);
    expect(result.current.subscriptions[1].pendingRemoval).toBe(false);
  });

  // R4-2: the save effect is keyed on the whole subscription list, so before it
  // learned to compare, every automatic read wrote the tracked file again —
  // each write a temp file plus an `fsync` plus a rename, on the main thread.
  it("does not re-persist when only read state changed", async () => {
    const { result } = renderHook(() => useSubscriptions());
    await flush();
    saveTracked.mockClear();

    await act(async () => {
      await result.current.refreshAll();
    });

    expect(saveTracked).not.toHaveBeenCalled();
  });

  // R2: the save effect records what it handed to `saveTracked` before the
  // write settles (the R4-2 dedupe above), so a write that then failed used
  // to be remembered as saved — never retried, and the rename died with the
  // process. A failed save now clears that record, which turns the very
  // next effect run — even one where only read state changed — into the
  // retry.
  it("retries a failed save on the next change instead of remembering it as saved", async () => {
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

// v5: "Move up"/"Move down" in the row menu. Panel order is the one order
// everywhere — the persisted list and the tray's digit order both derive
// from `subscriptions`' own array order — so a swap must show up in all
// three, and an impossible move must change nothing (not even a save).
describe("useSubscriptions reordering (v5)", () => {
  const TWO = [
    { id: "claude:claude", provider: "claude", config_dir: "~/.claude", label: null, pinned: true },
    { id: "claude:claude-team", provider: "claude", config_dir: "~/.claude-team", label: null, pinned: true },
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

  it("swaps the row with its neighbor and persists the new order", async () => {
    const { result } = renderHook(() => useSubscriptions());
    await flush();
    saveTracked.mockClear();

    act(() => result.current.moveSubscription("claude:claude", "down"));
    await flush();

    expect(result.current.subscriptions.map((s) => s.id)).toEqual(["claude:claude-team", "claude:claude"]);
    const saved = saveTracked.mock.calls[saveTracked.mock.calls.length - 1]?.[0];
    expect(saved.map((t: { id: string }) => t.id)).toEqual(["claude:claude-team", "claude:claude"]);
  });

  it("moving up then down lands back where it started", async () => {
    const { result } = renderHook(() => useSubscriptions());
    await flush();

    act(() => result.current.moveSubscription("claude:claude-team", "up"));
    expect(result.current.subscriptions.map((s) => s.id)).toEqual(["claude:claude-team", "claude:claude"]);

    act(() => result.current.moveSubscription("claude:claude-team", "down"));
    expect(result.current.subscriptions.map((s) => s.id)).toEqual(["claude:claude", "claude:claude-team"]);
  });

  it("is a no-op at the edges — no reorder, no save", async () => {
    const { result } = renderHook(() => useSubscriptions());
    await flush();
    saveTracked.mockClear();

    act(() => result.current.moveSubscription("claude:claude", "up"));
    act(() => result.current.moveSubscription("claude:claude-team", "down"));
    await flush();

    expect(result.current.subscriptions.map((s) => s.id)).toEqual(["claude:claude", "claude:claude-team"]);
    expect(saveTracked).not.toHaveBeenCalled();
  });

  it("reorders the tray digits with it", async () => {
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

// R4-4: one account, one spelling. The same screencast showed "Claude Max" in
// the panel and "Claude" in the Subscriptions screen for one account, and
// "Claude Team" / "claude team" for the other.
describe("useSubscriptions display names are consistent between the panel and the Subscriptions screen (R4-4)", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    fetchSnapshot.mockReset();
    setTrayStatus.mockReset();
    loadTracked.mockResolvedValue(TRACKED);
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("titles the id-derived fallback the way every other name in the panel is titled", () => {
    expect(accountLabel({ id: "claude:claude", provider: "claude", config_dir: "~/.claude" })).toBe("Claude");
    expect(accountLabel({ id: "claude:claude-team", provider: "claude", config_dir: "~/.claude-team" })).toBe(
      "Claude Team",
    );
    expect(accountLabel({ id: "claude:work_eu", provider: "claude", config_dir: "~/.claude-work_eu" })).toBe("Work Eu");
  });

  it("keeps the provider's own name for an account after it stops being tracked", async () => {
    fetchSnapshot.mockResolvedValue({
      account_id: "claude:claude",
      provider: "claude",
      config_dir: "~/.claude",
      fetched_at: new Date().toISOString(),
      usage: { limits: [{ kind: "weekly_all", percent: 12, is_active: true, resets_at: null, scope: null }] },
      profile: { organization: { name: "Claude Max", organization_type: "claude_max" } },
    });

    const { result } = renderHook(() => useSubscriptions());
    await flush();
    expect(result.current.subscriptions[0].label).toBe("Claude Max");

    act(() => result.current.removeSubscription("claude:claude"));

    // The Subscriptions screen asks for this account by descriptor now that
    // there is no subscription to read a label off — it must still be the name
    // the captain knows it by, not the config directory's.
    expect(
      result.current.displayLabelFor({ id: "claude:claude", provider: "claude", config_dir: "~/.claude" }),
    ).toBe("Claude Max");
  });

  it("falls back to the id-derived title for an account never read", async () => {
    const { result } = renderHook(() => useSubscriptions());
    await flush();
    expect(
      result.current.displayLabelFor({ id: "claude:claude-team", provider: "claude", config_dir: "~/.claude-team" }),
    ).toBe("Claude Team");
  });
});
