import { useCallback, useEffect, useRef, useState } from "react";
import type { AccountDescriptor, FetchError, RawSnapshot, Subscription, SubscriptionState, TraySegment } from "../types/entities";
import { isFetchError } from "../types/entities";
import { fetchSnapshot, setTrayStatus, onQuotaRefresh, kickScheduler } from "../lib/tauriClient";
import { normalizeFor, providerDisplayName, mapOutcomeFor } from "../providers/registry";
import { loadTracked, saveTracked, type TrackedAccount } from "../lib/persistence";

/** The provider-derived default label for an account before any read has
 * come back (or before a custom rename) — shared with the add-subscription
 * flow so a candidate shows the same name there as it will once tracked. */
export function accountLabel(account: AccountDescriptor): string {
  const slug = account.id.split(":")[1] ?? account.id;
  return slug === "claude" ? "Claude" : slug.replace(/[-_]/g, " ");
}

function initialSubscription(account: AccountDescriptor, labelOverride: string | null, pinned: boolean): Subscription {
  return {
    id: account.id,
    provider: account.provider,
    providerName: providerDisplayName(account.provider),
    label: accountLabel(account),
    labelOverride,
    account: null,
    state: "connecting",
    severity: "healthy",
    used: null,
    resetsAt: null,
    lastReadAt: null,
    windows: [],
    reason: null,
    pinned,
    configDir: account.config_dir,
    rateLimitedUntil: null,
  };
}

function isBlocked(s: Subscription, now: number): boolean {
  return !!s.rateLimitedUntil && new Date(s.rateLimitedUntil).getTime() > now;
}

/** The single owner of tracked-subscription membership and per-subscription
 * state. Every failure is scoped to its own subscription — one broken
 * account is never allowed to blank or block the others.
 *
 * I6: nothing is tracked by default — the list here is exactly what the
 * user added, persisted via lib/persistence.ts. Discovery (tauriClient's
 * listAccounts) only ever feeds the add-subscription flow; it is never
 * rendered directly.
 *
 * R2-4: refresh *cadence* is no longer owned here — it lives in the Rust
 * scheduler (`src-tauri/src/scheduler.rs`), which pushes results via the
 * `quota-refresh` event. This hook's job for automatic reads is just to
 * apply what the scheduler reports, the same way it applies a manual
 * refresh's direct result. B5 is unchanged: opening/closing the panel still
 * spends no budget at all — nothing here is tied to visibility. */
export function useSubscriptions() {
  const [subscriptions, setSubscriptions] = useState<Subscription[]>([]);
  const subscriptionsRef = useRef<Subscription[]>(subscriptions);
  subscriptionsRef.current = subscriptions;

  // R2-5: the async-load equivalent of the old "lazy useState initializer"
  // trick — loading the tracked list is now inherently async (a real IPC
  // round-trip to the Rust-owned file, see persistence.ts), so it can no
  // longer be seeded synchronously before first render. This flag is what
  // takes over that trick's job: the persistence-writing effect below must
  // never fire before the loaded value has actually committed to state, or
  // it would clobber real stored data with `[]` on the very first render —
  // exactly the bug the lazy initializer used to prevent.
  const hasLoadedRef = useRef(false);

  const patch = useCallback((id: string, changes: Partial<Subscription>) => {
    setSubscriptions((prev) => prev.map((s) => (s.id === id ? { ...s, ...changes } : s)));
  }, []);

  // R2-3: applies a read outcome (success or failure, from either a manual
  // refresh's direct result or a pushed `quota-refresh` event) the same way
  // regardless of where it came from. `hadGoodRead`/`priorState`/
  // `priorReason` are passed in rather than looked up here because *when*
  // they're captured matters — see refreshOne's B5/B6 comment below.
  const applyRefreshResult = useCallback(
    (
      accountId: string,
      provider: string,
      fallbackLabel: string,
      hadGoodRead: boolean,
      priorState: SubscriptionState,
      priorReason: string | null,
      outcome: { ok: true; raw: RawSnapshot } | { ok: false; error: FetchError | null },
    ) => {
      if (outcome.ok) {
        const raw = outcome.raw;
        const normalized = normalizeFor(provider, raw.usage, raw.profile, fallbackLabel);
        const mapped = mapOutcomeFor(provider, { kind: "ok", normalized }, hadGoodRead);
        patch(accountId, {
          state: mapped.state,
          label: normalized.label,
          account: normalized.account,
          windows: normalized.windows,
          used: normalized.used,
          resetsAt: normalized.resetsAt,
          severity: normalized.severity,
          lastReadAt: raw.fetched_at,
          reason: mapped.reason,
          rateLimitedUntil: null,
        });
        return;
      }

      const err = outcome.error;
      if (err && err.kind === "rate_limited") {
        // B5/B6: a self-imposed wait is a separate fact, not a health
        // state — never routed through the provider's outcome mapper.
        // Restore exactly the health state/reason this account had before
        // this attempt started; a real diagnosis (e.g. Broken/expired
        // login) must never decay into a generic wait.
        const until = new Date(Date.now() + err.retry_after_secs * 1000).toISOString();
        patch(accountId, { state: priorState, reason: priorReason, rateLimitedUntil: until });
        return;
      }
      const mapped = mapOutcomeFor(provider, { kind: "error", error: err }, hadGoodRead);
      patch(accountId, { state: mapped.state, reason: mapped.reason, rateLimitedUntil: null });
    },
    [patch],
  );

  const refreshOne = useCallback(
    async (account: AccountDescriptor) => {
      const prior = subscriptionsRef.current.find((s) => s.id === account.id);
      const hadGoodRead = !!prior?.lastReadAt;
      // Captured *before* the optimistic patch below — otherwise a
      // rate-limited outcome restoring "prior" state would read back its
      // own optimistic "reading"/"connecting" patch instead of the real
      // diagnosis that came before it (the exact B6 regression).
      const priorState = prior?.state ?? "connecting";
      const priorReason = prior?.reason ?? null;
      patch(account.id, { state: hadGoodRead ? "reading" : "connecting" });

      try {
        const raw = await fetchSnapshot(account);
        applyRefreshResult(account.id, account.provider, accountLabel(account), hadGoodRead, priorState, priorReason, {
          ok: true,
          raw,
        });
      } catch (err) {
        applyRefreshResult(account.id, account.provider, accountLabel(account), hadGoodRead, priorState, priorReason, {
          ok: false,
          error: isFetchError(err) ? err : null,
        });
      }
    },
    [patch, applyRefreshResult],
  );

  // R2-4: "the manual refresh control is debounced" — guards refreshAll and
  // refreshAccountById so they can't be double-fired regardless of what UI
  // calls them (the header button already disables itself while its own
  // promise is pending, but that's a courtesy, not the source of truth).
  // Concurrent calls collapse into the single in-flight one instead of
  // starting a second fetch.
  const refreshAllInFlight = useRef<Promise<void> | null>(null);
  const refreshOneInFlight = useRef<Map<string, Promise<void>>>(new Map());

  const refreshAll = useCallback(async () => {
    if (refreshAllInFlight.current) return refreshAllInFlight.current;
    const run = (async () => {
      const now = Date.now();
      const targets = subscriptionsRef.current.filter((s) => !isBlocked(s, now));
      await Promise.allSettled(targets.map((s) => refreshOne({ id: s.id, provider: s.provider, config_dir: s.configDir })));
    })();
    refreshAllInFlight.current = run;
    try {
      await run;
    } finally {
      refreshAllInFlight.current = null;
    }
  }, [refreshOne]);

  const refreshAccountById = useCallback(
    async (id: string) => {
      const inFlight = refreshOneInFlight.current.get(id);
      if (inFlight) return inFlight;
      const sub = subscriptionsRef.current.find((s) => s.id === id);
      if (!sub || isBlocked(sub, Date.now())) return;
      const run = refreshOne({ id: sub.id, provider: sub.provider, config_dir: sub.configDir });
      refreshOneInFlight.current.set(id, run);
      try {
        await run;
      } finally {
        refreshOneInFlight.current.delete(id);
      }
    },
    [refreshOne],
  );

  const togglePin = useCallback((id: string) => {
    setSubscriptions((prev) => prev.map((s) => (s.id === id ? { ...s, pinned: !s.pinned } : s)));
  }, []);

  const renameSubscription = useCallback((id: string, label: string | null) => {
    setSubscriptions((prev) => prev.map((s) => (s.id === id ? { ...s, labelOverride: label } : s)));
  }, []);

  const addSubscription = useCallback(
    (account: AccountDescriptor) => {
      setSubscriptions((prev) => {
        if (prev.some((s) => s.id === account.id)) return prev;
        return [...prev, initialSubscription(account, null, false)];
      });
      // Brief §5.3 step 4: verify by reading once immediately, so the
      // person sees what came back rather than a cold placeholder.
      void refreshOne(account);
    },
    [refreshOne],
  );

  const removeSubscription = useCallback((id: string) => {
    setSubscriptions((prev) => prev.filter((s) => s.id !== id));
  }, []);

  // Loads the tracked list once at mount (async — see hasLoadedRef above),
  // then either:
  //  - native: subscribes to the Rust scheduler's `quota-refresh` push and
  //    kicks it once so the launch read is near-instant (R2-4); or
  //  - browser/mock harness: there's no Rust scheduler to push from, so this
  //    does the one-time initial read itself, exactly as before R2-4 moved
  //    cadence onto the Rust side.
  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    // Read fresh on every mount (not module scope) so tests can toggle it;
    // in the real app Tauri injects this global before any app JS runs, so
    // it's equivalent to a constant in practice.
    const isTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

    void (async () => {
      const tracked = await loadTracked();
      if (cancelled) return;
      const loaded = tracked.map((t) =>
        initialSubscription({ id: t.id, provider: t.provider, config_dir: t.config_dir }, t.label, t.pinned),
      );
      setSubscriptions(loaded);
      hasLoadedRef.current = true;

      if (!isTauri) {
        void Promise.allSettled(
          loaded.map((s) => refreshOne({ id: s.id, provider: s.provider, config_dir: s.configDir })),
        );
        return;
      }

      unlisten = await onQuotaRefresh((event) => {
        const accountId = event.kind === "ok" ? event.snapshot.account_id : event.account_id;
        const prior = subscriptionsRef.current.find((s) => s.id === accountId);
        if (!prior) return; // no longer tracked — a race with removal, ignore it
        const hadGoodRead = !!prior.lastReadAt;
        const fallbackLabel = accountLabel({ id: accountId, provider: prior.provider, config_dir: prior.configDir });
        if (event.kind === "ok") {
          applyRefreshResult(accountId, prior.provider, fallbackLabel, hadGoodRead, prior.state, prior.reason, {
            ok: true,
            raw: event.snapshot,
          });
        } else {
          applyRefreshResult(accountId, prior.provider, fallbackLabel, hadGoodRead, prior.state, prior.reason, {
            ok: false,
            error: event.error,
          });
        }
      });
      if (cancelled) {
        unlisten();
        unlisten = undefined;
        return;
      }
      void kickScheduler();
    })();

    return () => {
      cancelled = true;
      unlisten?.();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Persist the user-owned parts of the list (membership, custom labels,
  // pins) on every change, once the initial load has actually committed.
  // Ephemeral read state (windows, used %, reason) is deliberately not
  // persisted — a stale number must never be shown as current after a
  // restart; every subscription re-verifies on launch.
  useEffect(() => {
    if (!hasLoadedRef.current) return;
    const tracked: TrackedAccount[] = subscriptions.map((s) => ({
      id: s.id,
      provider: s.provider,
      config_dir: s.configDir,
      label: s.labelOverride,
      pinned: s.pinned,
    }));
    void saveTracked(tracked);
  }, [subscriptions]);

  // Mirror pinned subscriptions' headline figures beside the tray glyph
  // (I2: consumed, not remaining). A broken pin shows "!" instead of a
  // stale number, never a number presented as current; a pin with no number
  // yet draws nothing at all (per the handoff: "Нет данных — цифра не
  // рисуется"). Unpinned means gone — this effect is a pure function of
  // current state every time it runs.
  //
  // R2-2: color now comes from severity (any window >=90%/75%), not from
  // the headline percentage's own magnitude, so a 20%-headline account with
  // a near-exhausted session still reads amber in the tray. A stale
  // ("behind") read is always amber regardless of severity — the number
  // itself is no longer trustworthy. See tauriClient.ts for why this is a
  // structured call instead of a plain colored string.
  useEffect(() => {
    const segments: TraySegment[] = subscriptions
      .filter((s) => s.pinned)
      .map((s): TraySegment | null => {
        if (s.state === "broken") return { text: "!", color: "red" };
        if (typeof s.used !== "number") return null;
        if (s.state === "behind") return { text: `${s.used}%`, color: "amber" };
        if (s.severity === "critical") return { text: `${s.used}%`, color: "red" };
        if (s.severity === "warn") return { text: `${s.used}%`, color: "amber" };
        return { text: `${s.used}%`, color: "neutral" };
      })
      .filter((s): s is TraySegment => s !== null);
    setTrayStatus(segments);
  }, [subscriptions]);

  return {
    subscriptions,
    refreshAll,
    refreshAccountById,
    togglePin,
    renameSubscription,
    addSubscription,
    removeSubscription,
  };
}
