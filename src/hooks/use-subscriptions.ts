import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { loadTracked, saveTracked, type TrackedAccount } from "@/lib/persistence";
import {
  buildStatusItemSegments,
  buildStatusItemTooltip,
  computeWorstActiveLimitPercent,
} from "@/lib/status-item-segments";
import {
  cancelSignIn as cancelSignInIpc,
  fetchSnapshot,
  forgetSignIn,
  kickScheduler,
  onQuotaRefresh,
  onSignInFinished,
  renderStatusItem,
  startSignIn as startSignInIpc,
  submitSignInCode as submitSignInCodeIpc,
} from "@/lib/tauri-client";
import { mapOutcomeFor, normalizeFor, resolveProviderDisplayName } from "@/providers/registry";
import type {
  AccountDescriptor,
  FetchError,
  RawSnapshot,
  Subscription,
  SubscriptionState,
} from "@/types/entities";
import { isFetchError } from "@/types/entities";

/** Id-derived default label before any read or custom rename, shared with
 * the add-subscription flow. Title-cased so the same account never shows
 * two spellings of its own name across the panel and the Subscriptions
 * screen. */
export function deriveAccountLabel(account: AccountDescriptor): string {
  const slug = account.id.split(":")[1] ?? account.id;
  return slug
    .replace(/[-_]/g, " ")
    .replace(/\S+/g, (word) => word.charAt(0).toUpperCase() + word.slice(1));
}

function buildInitialSubscription(
  account: AccountDescriptor,
  labelOverride: string | null,
  pinnedWindowIds: string[],
  label = deriveAccountLabel(account),
): Subscription {
  return {
    id: account.id,
    provider: account.provider,
    providerName: resolveProviderDisplayName(account.provider),
    label,
    labelOverride,
    account: null,
    state: "connecting",
    severity: "healthy",
    used: null,
    resetsAt: null,
    lastReadAt: null,
    windows: [],
    reason: null,
    needsSignIn: false,
    pinnedWindowIds,
    headlineWindowId: null,
    configDir: account.config_dir,
    rateLimitedUntil: null,
    signInInProgress: false,
    pendingRemoval: false,
  };
}

/** The account is untracked from the moment "Stop tracking" is pressed.
 * This is only how long the Undo row keeps its slot in the panel. */
export const STOP_TRACKING_UNDO_MS = 5_000;

/** An older tracked record's `pinned: true` becomes "that subscription's
 * headline window is pinned" once a read reveals which window that is, so
 * a record without the newer `pinnedWindowIds` array but with the older
 * flag set comes back needing that one-shot migration. */
function migrateLegacyTracked(tracked: TrackedAccount[]): {
  subscriptions: Subscription[];
  migratingPinIds: Set<string>;
} {
  const migratingPinIds = new Set<string>();
  const subscriptions = tracked.map((t) => {
    const legacy = t as unknown as { pinnedWindowIds?: unknown; pinned?: unknown };
    const pinnedWindowIds = Array.isArray(legacy.pinnedWindowIds)
      ? (legacy.pinnedWindowIds as string[])
      : [];
    if (!Array.isArray(legacy.pinnedWindowIds) && legacy.pinned === true) {
      migratingPinIds.add(t.id);
    }
    return buildInitialSubscription(
      { id: t.id, provider: t.provider, config_dir: t.config_dir },
      t.label,
      pinnedWindowIds,
    );
  });
  return { subscriptions, migratingPinIds };
}

/** The persisted projection of a subscription: membership, custom name,
 * pin, and nothing that a read produced. */
function toTrackedAccounts(subscriptions: Subscription[]): TrackedAccount[] {
  return subscriptions.map((s) => ({
    id: s.id,
    provider: s.provider,
    config_dir: s.configDir,
    label: s.labelOverride,
    pinnedWindowIds: s.pinnedWindowIds,
  }));
}

/** Captured before the optimistic patch that starts a manual refresh, so a
 * rate-limited answer restores the real prior diagnosis rather than that
 * patch. */
interface PriorRead {
  hadGoodRead: boolean;
  state: SubscriptionState;
  reason: string | null;
  needsSignIn: boolean;
}

function capturePriorRead(sub: Subscription | undefined): PriorRead {
  return {
    hadGoodRead: !!sub?.lastReadAt,
    state: sub?.state ?? "connecting",
    reason: sub?.reason ?? null,
    needsSignIn: sub?.needsSignIn ?? false,
  };
}

/** Owns tracked-subscription membership and per-subscription state. Every
 * failure is scoped to its own subscription, never blanking the others.
 * Nothing is tracked by default: the list here is exactly what the user
 * added, persisted via lib/persistence.ts. Refresh cadence lives in the
 * Rust scheduler, `src-tauri/src/scheduler.rs`, which pushes results via
 * the `quota-refresh` event. This hook applies a pushed result the same
 * way it applies a manual refresh's direct result. */
export function useSubscriptions() {
  const [subscriptions, setSubscriptions] = useState<Subscription[]>([]);
  const subscriptionsRef = useRef<Subscription[]>(subscriptions);
  subscriptionsRef.current = subscriptions;

  // The best name learned for an account this session, kept after it stops
  // being tracked so the Subscriptions screen's untracked half does not
  // fall back to a bare directory-derived label. Not persisted.
  const [knownLabels, setKnownLabels] = useState<Record<string, string>>({});
  const knownLabelsRef = useRef<Record<string, string>>(knownLabels);
  knownLabelsRef.current = knownLabels;

  // Owned here, not in app.tsx, since the state these timers resolve into,
  // membership, is owned here too.
  const removalTimers = useRef<Record<string, ReturnType<typeof setTimeout>>>({});

  /** The exact JSON last handed to `saveTracked`. See the save effect. */
  const lastSavedRef = useRef<string | null>(null);

  // Accounts still needing migrateLegacyTracked's one-shot migration wait
  // here until a read reveals their headline window.
  const pendingPinMigrationRef = useRef<Set<string>>(new Set());

  // Loading the tracked list is an IPC round-trip, so it cannot be seeded
  // synchronously before first render. Guards the persistence-writing
  // effect below from firing before the load has committed to state and
  // clobbering real stored data with `[]`.
  const hasLoadedRef = useRef(false);

  const patch = useCallback((id: string, changes: Partial<Subscription>) => {
    setSubscriptions((prev) => prev.map((s) => (s.id === id ? { ...s, ...changes } : s)));
  }, []);

  // Applies a read outcome the same way regardless of whether it came from
  // a manual refresh or a pushed `quota-refresh` event. `prior` is passed
  // in because when it is captured matters, see `refreshOne` below.
  const applyRefreshResult = useCallback(
    (
      accountId: string,
      provider: string,
      fallbackLabel: string,
      prior: PriorRead,
      outcome: { ok: true; raw: RawSnapshot } | { ok: false; error: FetchError | null },
    ) => {
      if (outcome.ok) {
        const raw = outcome.raw;
        const normalized = normalizeFor(provider, raw.usage, raw.profile, fallbackLabel, {
          fetchedAt: raw.fetched_at,
          statuslineFeed: raw.statusline,
        });
        const mapped = mapOutcomeFor(provider, { kind: "ok", normalized }, prior.hadGoodRead);
        setKnownLabels((prev) =>
          prev[accountId] === normalized.label ? prev : { ...prev, [accountId]: normalized.label },
        );
        const pinnedWindowIds = pendingPinMigrationRef.current.has(accountId)
          ? normalized.headlineWindowId
            ? [normalized.headlineWindowId]
            : []
          : undefined;
        pendingPinMigrationRef.current.delete(accountId);
        patch(accountId, {
          state: mapped.state,
          label: normalized.label,
          account: normalized.account,
          windows: normalized.windows,
          used: normalized.used,
          resetsAt: normalized.resetsAt,
          severity: normalized.severity,
          headlineWindowId: normalized.headlineWindowId,
          lastReadAt: raw.fetched_at,
          reason: mapped.reason,
          needsSignIn: mapped.needsSignIn,
          rateLimitedUntil: null,
          ...(pinnedWindowIds !== undefined ? { pinnedWindowIds } : {}),
        });
        return;
      }

      const err = outcome.error;
      if (err && err.kind === "rate_limited") {
        // A self-imposed wait is never a health state and never reaches the
        // provider's outcome mapper. Restore the health state and reason
        // this account had before the attempt started, unless that prior
        // state was itself in-flight, "connecting" or "reading": writing
        // one of those back would leave the row claiming "Reading…"
        // forever with nothing in flight, masking the wait note.
        const until = new Date(Date.now() + err.retry_after_secs * 1000).toISOString();
        const settledPrior =
          prior.state === "connecting" || prior.state === "reading"
            ? prior.hadGoodRead
              ? "working"
              : "idle"
            : prior.state;
        patch(accountId, {
          state: settledPrior,
          reason: prior.reason,
          needsSignIn: prior.needsSignIn,
          rateLimitedUntil: until,
        });
        return;
      }
      const mapped = mapOutcomeFor(provider, { kind: "error", error: err }, prior.hadGoodRead);
      patch(accountId, {
        state: mapped.state,
        reason: mapped.reason,
        needsSignIn: mapped.needsSignIn,
        rateLimitedUntil: null,
      });
    },
    [patch],
  );

  const refreshOne = useCallback(
    async (account: AccountDescriptor) => {
      const prior = capturePriorRead(subscriptionsRef.current.find((s) => s.id === account.id));
      patch(account.id, { state: prior.hadGoodRead ? "reading" : "connecting" });

      try {
        const raw = await fetchSnapshot(account);
        applyRefreshResult(account.id, account.provider, deriveAccountLabel(account), prior, {
          ok: true,
          raw,
        });
      } catch (err) {
        applyRefreshResult(account.id, account.provider, deriveAccountLabel(account), prior, {
          ok: false,
          error: isFetchError(err) ? err : null,
        });
      }
    },
    [patch, applyRefreshResult],
  );

  const refreshAllInFlight = useRef<Promise<void> | null>(null);
  const refreshOneInFlight = useRef<Map<string, Promise<void>>>(new Map());

  // Every path that spends a real fetch on one account goes through this
  // guard, so two of them hitting the same account at once join one
  // in-flight read instead of spending two of the shared budget slots.
  // `refreshOne` must have exactly one caller, and it is this function.
  const refreshOneGuarded = useCallback(
    async (account: AccountDescriptor) => {
      const inFlight = refreshOneInFlight.current.get(account.id);
      if (inFlight) return inFlight;
      const run = refreshOne(account);
      refreshOneInFlight.current.set(account.id, run);
      try {
        await run;
      } finally {
        refreshOneInFlight.current.delete(account.id);
      }
    },
    [refreshOne],
  );

  // Rate-limited subscriptions are never filtered out client-side: the
  // Rust limiter refuses without spending anything, so always attempting
  // costs nothing and keeps every revival path live.
  const refreshAll = useCallback(async () => {
    if (refreshAllInFlight.current) return refreshAllInFlight.current;
    const run = (async () => {
      const targets = subscriptionsRef.current.filter((s) => !s.pendingRemoval);
      await Promise.allSettled(
        targets.map((s) =>
          refreshOneGuarded({ id: s.id, provider: s.provider, config_dir: s.configDir }),
        ),
      );
    })();
    refreshAllInFlight.current = run;
    try {
      await run;
    } finally {
      refreshAllInFlight.current = null;
    }
  }, [refreshOneGuarded]);

  const refreshAccountById = useCallback(
    async (id: string) => {
      const sub = subscriptionsRef.current.find((s) => s.id === id);
      if (!sub || sub.pendingRemoval) return;
      return refreshOneGuarded({ id: sub.id, provider: sub.provider, config_dir: sub.configDir });
    },
    [refreshOneGuarded],
  );

  // `windowId` is either a specific window's own id, from the per-window
  // pin buttons, or the subscription's current headline window, from the
  // "…" menu's toggle. `null` means no headline yet and is a no-op.
  const togglePin = useCallback((id: string, windowId: string | null) => {
    if (windowId === null) return;
    setSubscriptions((prev) =>
      prev.map((s) => {
        if (s.id !== id) return s;
        const has = s.pinnedWindowIds.includes(windowId);
        return {
          ...s,
          pinnedWindowIds: has
            ? s.pinnedWindowIds.filter((w) => w !== windowId)
            : [...s.pinnedWindowIds, windowId],
        };
      }),
    );
  }, []);

  const renameSubscription = useCallback((id: string, label: string | null) => {
    setSubscriptions((prev) => prev.map((s) => (s.id === id ? { ...s, labelOverride: label } : s)));
  }, []);

  /** Swaps the row with its neighbor in the panel's own slot order, which
   * the persisted list and the status item's digit order both derive
   * from. */
  const moveSubscription = useCallback((id: string, direction: "up" | "down") => {
    setSubscriptions((prev) => {
      const index = prev.findIndex((s) => s.id === id);
      if (index === -1) return prev;
      const neighbor = direction === "up" ? index - 1 : index + 1;
      if (neighbor < 0 || neighbor >= prev.length) return prev;
      const next = [...prev];
      [next[index], next[neighbor]] = [next[neighbor], next[index]];
      return next;
    });
  }, []);

  const clearRemovalTimer = useCallback((id: string) => {
    const timer = removalTimers.current[id];
    if (timer === undefined) return;
    clearTimeout(timer);
    delete removalTimers.current[id];
  }, []);

  const addSubscription = useCallback(
    (account: AccountDescriptor) => {
      clearRemovalTimer(account.id);
      setSubscriptions((prev) => {
        const existing = prev.find((s) => s.id === account.id);
        if (existing) {
          return existing.pendingRemoval
            ? prev.map((s) => (s.id === account.id ? { ...s, pendingRemoval: false } : s))
            : prev;
        }
        return [
          ...prev,
          buildInitialSubscription(account, null, [], knownLabelsRef.current[account.id]),
        ];
      });
      // Goes through the guard, not `refreshOne` directly, so a refresh
      // landing during this read joins it instead of double-fetching.
      void refreshOneGuarded(account);
    },
    [refreshOneGuarded, clearRemovalTimer],
  );

  const removeSubscription = useCallback(
    (id: string) => {
      clearRemovalTimer(id);
      setSubscriptions((prev) => prev.filter((s) => s.id !== id));
      void cancelSignInIpc(id);
    },
    [clearRemovalTimer],
  );

  const stopTracking = useCallback(
    (id: string) => {
      clearRemovalTimer(id);
      setSubscriptions((prev) =>
        prev.map((s) => (s.id === id ? { ...s, pendingRemoval: true } : s)),
      );
      void cancelSignInIpc(id);
      removalTimers.current[id] = setTimeout(() => {
        delete removalTimers.current[id];
        setSubscriptions((prev) => prev.filter((s) => !(s.id === id && s.pendingRemoval)));
      }, STOP_TRACKING_UNDO_MS);
    },
    [clearRemovalTimer],
  );

  const undoStopTracking = useCallback(
    (id: string) => {
      clearRemovalTimer(id);
      setSubscriptions((prev) =>
        prev.map((s) => (s.id === id ? { ...s, pendingRemoval: false } : s)),
      );
    },
    [clearRemovalTimer],
  );

  useEffect(() => {
    const timers = removalTimers;
    return () => {
      Object.values(timers.current).forEach(clearTimeout);
      timers.current = {};
    };
  }, []);

  const startSignIn = useCallback(
    async (id: string) => {
      const sub = subscriptionsRef.current.find((s) => s.id === id);
      if (!sub) return;
      patch(id, { signInInProgress: true });
      try {
        await startSignInIpc(id, sub.configDir);
      } catch (err) {
        const message = typeof err === "string" ? err : err instanceof Error ? err.message : null;
        patch(id, {
          signInInProgress: false,
          reason: message ?? "Quotos couldn't start the Claude Code sign-in.",
        });
      }
    },
    [patch],
  );

  const submitSignInCode = useCallback(async (id: string, code: string) => {
    await submitSignInCodeIpc(id, code);
  }, []);

  const cancelSignIn = useCallback(
    async (id: string) => {
      await cancelSignInIpc(id);
      patch(id, { signInInProgress: false });
    },
    [patch],
  );

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    void onSignInFinished((event) => {
      patch(event.account_id, { signInInProgress: false });
      void forgetSignIn(event.account_id);
      if (subscriptionsRef.current.some((s) => s.id === event.account_id)) {
        void refreshAccountById(event.account_id);
      }
    }).then((fn) => {
      if (cancelled) fn();
      else unlisten = fn;
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [patch, refreshAccountById]);

  // Loads the tracked list once at mount, then either subscribes to the
  // Rust scheduler's push on native, or does a one-time read itself in the
  // browser harness, which has no scheduler to push from. Runs exactly
  // once despite listing applyRefreshResult and refreshOneGuarded below:
  // both bottom out at patch, whose own dependency array is empty, so
  // neither ever gets a new identity across renders.
  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    const isTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

    void (async () => {
      const tracked = await loadTracked();
      if (cancelled) return;
      const { subscriptions: loaded, migratingPinIds } = migrateLegacyTracked(tracked);
      pendingPinMigrationRef.current = migratingPinIds;
      setSubscriptions(loaded);
      lastSavedRef.current = JSON.stringify(toTrackedAccounts(loaded));
      hasLoadedRef.current = true;

      if (!isTauri) {
        void Promise.allSettled(
          loaded.map((s) =>
            refreshOneGuarded({ id: s.id, provider: s.provider, config_dir: s.configDir }),
          ),
        );
        return;
      }

      unlisten = await onQuotaRefresh((event) => {
        const accountId = event.kind === "ok" ? event.snapshot.account_id : event.account_id;
        const existing = subscriptionsRef.current.find((s) => s.id === accountId);
        if (!existing || existing.pendingRemoval) return;
        const prior = capturePriorRead(existing);
        const fallbackLabel = deriveAccountLabel({
          id: accountId,
          provider: existing.provider,
          config_dir: existing.configDir,
        });
        if (event.kind === "ok") {
          applyRefreshResult(accountId, existing.provider, fallbackLabel, prior, {
            ok: true,
            raw: event.snapshot,
          });
        } else {
          applyRefreshResult(accountId, existing.provider, fallbackLabel, prior, {
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
  }, [applyRefreshResult, refreshOneGuarded]);

  /** What "tracked" means everywhere except the panel's own row list. A
   * row inside its undo window survives only as a slot in `subscriptions`,
   * returned below. */
  const trackedSubscriptions = useMemo(
    () => subscriptions.filter((s) => !s.pendingRemoval),
    [subscriptions],
  );

  // Ephemeral read state is deliberately not persisted, since a stale
  // number must never be shown as current after a restart. Compares
  // against what was last written and skips an identical save, since this
  // effect is keyed on the whole subscription list and would otherwise
  // also fire on every automatic read. The dedupe record is written before
  // the save settles; a save that then fails clears the record so the very
  // next change retries instead of the failure going silent.
  useEffect(() => {
    if (!hasLoadedRef.current) return;
    const tracked = toTrackedAccounts(trackedSubscriptions);
    const serialized = JSON.stringify(tracked);
    if (lastSavedRef.current === serialized) return;
    lastSavedRef.current = serialized;
    void (async () => {
      try {
        await saveTracked(tracked);
      } catch (error) {
        console.error(
          "Quotos: saving the tracked list failed; will retry on the next change",
          error,
        );
        if (lastSavedRef.current === serialized) lastSavedRef.current = null;
      }
    })();
  }, [trackedSubscriptions]);

  // A pin with no number yet draws nothing, never "!" or "…". Color comes
  // from severity, not the headline percentage's own magnitude, and any
  // stale contributing subscription turns every digit amber.
  useEffect(() => {
    const segments = buildStatusItemSegments(trackedSubscriptions);
    const worstUsedPercent = computeWorstActiveLimitPercent(trackedSubscriptions);
    renderStatusItem(segments, worstUsedPercent, buildStatusItemTooltip(trackedSubscriptions));
  }, [trackedSubscriptions]);

  /** Prefers whatever a real read last reported over the directory-derived
   * fallback, so an account reads the same in both halves of the
   * Subscriptions screen. */
  const displayLabelFor = useCallback(
    (account: AccountDescriptor) => knownLabels[account.id] ?? deriveAccountLabel(account),
    [knownLabels],
  );

  return {
    subscriptions,
    trackedSubscriptions,
    refreshAll,
    refreshAccountById,
    togglePin,
    renameSubscription,
    moveSubscription,
    addSubscription,
    removeSubscription,
    stopTracking,
    undoStopTracking,
    displayLabelFor,
    startSignIn,
    submitSignInCode,
    cancelSignIn,
  };
}
