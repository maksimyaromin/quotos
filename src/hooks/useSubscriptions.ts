import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { loadTracked, saveTracked, type TrackedAccount } from "../lib/persistence";
import {
  cancelSignIn as cancelSignInIpc,
  fetchSnapshot,
  forgetSignIn,
  kickScheduler,
  onQuotaRefresh,
  onSignInFinished,
  setTrayStatus,
  startSignIn as startSignInIpc,
  submitSignInCode as submitSignInCodeIpc,
} from "../lib/tauriClient";
import { buildTraySegments, buildTrayTooltip, worstActiveLimitPercent } from "../lib/traySegments";
import { mapOutcomeFor, normalizeFor, providerDisplayName } from "../providers/registry";
import type {
  AccountDescriptor,
  FetchError,
  RawSnapshot,
  Subscription,
  SubscriptionState,
} from "../types/entities";
import { isFetchError } from "../types/entities";

/** The id-derived default label for an account before any read has come
 * back, or before a custom rename. Shared with the add-subscription flow
 * so a candidate shows the same name there as it will once tracked.
 *
 * Title-cased, so the same account never shows two spellings of its own
 * name across the panel and the Subscriptions screen. This is
 * presentation consistency, not a naming policy: the same words,
 * capitalized the way every other name in the panel is. */
export function accountLabel(account: AccountDescriptor): string {
  const slug = account.id.split(":")[1] ?? account.id;
  return slug
    .replace(/[-_]/g, " ")
    .replace(/\S+/g, (word) => word.charAt(0).toUpperCase() + word.slice(1));
}

function initialSubscription(
  account: AccountDescriptor,
  labelOverride: string | null,
  pinnedWindowIds: string[],
  label = accountLabel(account),
): Subscription {
  return {
    id: account.id,
    provider: account.provider,
    providerName: providerDisplayName(account.provider),
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

/** How long "Stop tracking" stays undoable. The account is untracked from
 * the moment the button is pressed. This is only how long the Undo row
 * keeps its slot in the panel. */
export const STOP_TRACKING_UNDO_MS = 5_000;

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

/** Everything a read outcome needs to know about what came before it.
 * Captured before the optimistic patch that starts a manual refresh, so a
 * rate-limited answer restores the real prior diagnosis rather than that
 * patch. */
interface PriorRead {
  hadGoodRead: boolean;
  state: SubscriptionState;
  reason: string | null;
  needsSignIn: boolean;
}

function priorReadOf(sub: Subscription | undefined): PriorRead {
  return {
    hadGoodRead: !!sub?.lastReadAt,
    state: sub?.state ?? "connecting",
    reason: sub?.reason ?? null,
    needsSignIn: sub?.needsSignIn ?? false,
  };
}

/** The single owner of tracked-subscription membership and per-subscription
 * state. Every failure is scoped to its own subscription, so one broken
 * account is never allowed to blank or block the others.
 *
 * Nothing is tracked by default. The list here is exactly what the user
 * added, persisted via lib/persistence.ts. Discovery, `tauriClient`'s
 * `listAccounts`, only ever feeds the add-subscription flow. It is never
 * rendered directly.
 *
 * Refresh cadence is not owned here. It lives in the Rust scheduler,
 * `src-tauri/src/scheduler.rs`, which pushes results via the
 * `quota-refresh` event. This hook's job for automatic reads is just to
 * apply what the scheduler reports, the same way it applies a manual
 * refresh's direct result. Opening or closing the panel spends no budget
 * at all, since nothing here is tied to visibility. */
export function useSubscriptions() {
  const [subscriptions, setSubscriptions] = useState<Subscription[]>([]);
  const subscriptionsRef = useRef<Subscription[]>(subscriptions);
  subscriptionsRef.current = subscriptions;

  // The best name ever learned for an account this session, by id: the
  // provider's own `organization.name`, for example "Claude Max", kept
  // after the account stops being tracked. Without it, untracking an
  // account would rename it back to a bare directory-derived label the
  // instant it moved from the tracked half of the Subscriptions screen to
  // the untracked half, since that half has nothing but the config
  // directory's name to go on. A session-lifetime cache, deliberately: it
  // is a presentation nicety derived from reads this session actually
  // made, not a new thing to persist.
  const [knownLabels, setKnownLabels] = useState<Record<string, string>>({});
  const knownLabelsRef = useRef<Record<string, string>>(knownLabels);
  knownLabelsRef.current = knownLabels;

  // The live "Stop tracking" undo timers, by id. Owned here rather than in
  // App.tsx, because the state they resolve into, membership, is owned
  // here too, so the two never disagree.
  const removalTimers = useRef<Record<string, ReturnType<typeof setTimeout>>>({});

  /** The exact JSON last handed to `saveTracked`. See the save effect. */
  const lastSavedRef = useRef<string | null>(null);

  // An older tracked record's `pinned: true` becomes "that subscription's
  // headline window is pinned", the same window the "…" menu's toggle now
  // targets. Which window is the headline is only known once a read comes
  // back, `NormalizedRead.headlineWindowId`, so this just remembers which
  // accounts still need the one-shot migration. `applyRefreshResult`'s ok
  // branch is the only place that consumes an id from here, on the first
  // successful read after load. A failed attempt leaves the flag pending,
  // so the migration simply waits for a read that actually has an answer.
  const pendingPinMigrationRef = useRef<Set<string>>(new Set());

  // Loading the tracked list is inherently async, a real IPC round-trip to
  // the Rust-owned file, see persistence.ts, so it cannot be seeded
  // synchronously before first render. This flag guards the
  // persistence-writing effect below: that effect must never fire before
  // the loaded value has actually committed to state, or it would clobber
  // real stored data with `[]` on the very first render.
  const hasLoadedRef = useRef(false);

  const patch = useCallback((id: string, changes: Partial<Subscription>) => {
    setSubscriptions((prev) => prev.map((s) => (s.id === id ? { ...s, ...changes } : s)));
  }, []);

  // Applies a read outcome, success or failure, from either a manual
  // refresh's direct result or a pushed `quota-refresh` event, the same
  // way regardless of where it came from. `prior` is passed in rather than
  // looked up here because when it is captured matters, see `refreshOne`'s
  // comment below.
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
        // Remember it for the Subscriptions screen, which otherwise has
        // only the config directory's name to show once this account stops
        // being tracked.
        setKnownLabels((prev) =>
          prev[accountId] === normalized.label ? prev : { ...prev, [accountId]: normalized.label },
        );
        // Pin migration is consumed at most once per account, on whichever
        // read, successful or not, lands first after load.
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
        // A self-imposed wait is a separate fact, not a health state, and
        // is never routed through the provider's outcome mapper. Restore
        // exactly the health state and reason this account had before this
        // attempt started, so a real diagnosis such as an expired login
        // never decays into a generic wait.
        //
        // The one exception is when that prior state is itself an
        // in-flight presentation, "connecting" or "reading", an attempt
        // rather than a diagnosis. Writing one back would leave the row
        // claiming "Reading…" forever with nothing in flight, and the
        // footer's reading branch would then mask the wait note
        // rowPresentation composes. A never-read account settles back to
        // idle, and one with data settles to working, since either way an
        // in-flight prior means another attempt's own result is coming.
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
      // Captured before the optimistic patch below. Otherwise a
      // rate-limited outcome restoring `prior` state would read back its
      // own optimistic "reading" or "connecting" patch instead of the real
      // diagnosis that came before it.
      const prior = priorReadOf(subscriptionsRef.current.find((s) => s.id === account.id));
      patch(account.id, { state: prior.hadGoodRead ? "reading" : "connecting" });

      try {
        const raw = await fetchSnapshot(account);
        applyRefreshResult(account.id, account.provider, accountLabel(account), prior, {
          ok: true,
          raw,
        });
      } catch (err) {
        applyRefreshResult(account.id, account.provider, accountLabel(account), prior, {
          ok: false,
          error: isFetchError(err) ? err : null,
        });
      }
    },
    [patch, applyRefreshResult],
  );

  // The manual refresh control is debounced: concurrent calls collapse
  // into the single in-flight one instead of starting a second fetch. The
  // header button already disables itself while its own promise is
  // pending, but that is a courtesy, not the source of truth.
  const refreshAllInFlight = useRef<Promise<void> | null>(null);
  const refreshOneInFlight = useRef<Map<string, Promise<void>>>(new Map());

  // The per-account guard. Every path that spends a real fetch on one
  // account, a row's "Read now", the header refresh's per-account fan-out,
  // the launch read, and the read a newly added subscription gets, goes
  // through here, so two of them hitting the same account at once join one
  // in-flight read instead of spending two of the shared 5-per-300s budget
  // slots. `refreshOne` must have exactly one caller, and it is this
  // function.
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

  // No local "is it blocked?" filter here. Skipping rate-limited
  // subscriptions client-side would mean that once a long provider-issued
  // wait is pending, the panel refresh control and "Read now" both return
  // without doing anything, silently, so a wrong diagnosis could never be
  // re-tested. The Rust limiter is the single authority and refuses
  // without spending anything, so always attempting costs nothing and
  // keeps every revival path live: the moment the budget frees up, the
  // very next press reads for real.
  const refreshAll = useCallback(async () => {
    if (refreshAllInFlight.current) return refreshAllInFlight.current;
    const run = (async () => {
      // A row inside its "Stop tracking" undo window is untracked already.
      // Never spend a read on it.
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

  // Pinning is per window. `windowId` is either a specific window's own
  // id, from the per-window pin buttons, or the subscription's current
  // headline window, from the "…" menu's toggle wired by the caller. A
  // `null` windowId means no headline yet, nothing read, and is a no-op
  // since there is nothing to pin.
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

  /** The "…" menu's "Move up" and "Move down": swaps the row with its
   * neighbor in the panel's own slot order. An Undo row is a slot too, so
   * moving past one is visible and coherent. Panel order is the one order
   * everywhere, since the persisted list and the tray's digit order both
   * derive from it, so a swap here reorders all three together. */
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

  /** Cancels a pending "Stop tracking" timer, if one is running. */
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
        // Adding back something whose undo window is still open is the
        // same action as undoing it. Same account, same slot, same name.
        const existing = prev.find((s) => s.id === account.id);
        if (existing) {
          return existing.pendingRemoval
            ? prev.map((s) => (s.id === account.id ? { ...s, pendingRemoval: false } : s))
            : prev;
        }
        return [
          ...prev,
          initialSubscription(account, null, [], knownLabelsRef.current[account.id]),
        ];
      });
      // Verifies by reading once immediately, per docs/design/brief.md
      // section 5.3 step 4, so the person sees what came back rather than
      // a cold placeholder. This spends a real budget slot, so it goes
      // through the guard like every other fetch. Calling `refreshOne`
      // directly here would leave the read unregistered, and a refresh
      // landing during it, the widest window being the bounded-20s CLI
      // renewal an aged-out token needs, would start a second request for
      // the same account.
      void refreshOneGuarded(account);
    },
    [refreshOneGuarded, clearRemovalTimer],
  );

  /** Hard, immediate removal. The Subscriptions screen's own [Remove],
   * which has no undo affordance of its own. */
  const removeSubscription = useCallback(
    (id: string) => {
      clearRemovalTimer(id);
      setSubscriptions((prev) => prev.filter((s) => s.id !== id));
      void cancelSignInIpc(id);
    },
    [clearRemovalTimer],
  );

  /** The panel row menu's "Stop tracking". Untracks the account now: it
   * leaves the tray, stops being read, and stops being persisted this
   * instant. It keeps its slot in the panel for `STOP_TRACKING_UNDO_MS` so
   * the Undo row can sit there. */
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

  // Starts Claude Code's own sign-in for a broken row, see signin.rs.
  // Quotos never touches the Keychain or a credential. It only starts the
  // process and later relays a pasted code back into it.
  const startSignIn = useCallback(
    async (id: string) => {
      const sub = subscriptionsRef.current.find((s) => s.id === id);
      if (!sub) return;
      patch(id, { signInInProgress: true });
      try {
        await startSignInIpc(id, sub.configDir);
      } catch (err) {
        // A spawn failure, such as no `claude` on PATH in a Finder-launched
        // app, fails instantly with nothing else said. Show what went
        // wrong in the row's own reason line instead.
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

  // Fires once when a sign-in process exits, success or failure. See
  // signin.rs's doc comment for why `success` there is only the process's
  // own exit status. Either way, Quotos re-reads the account itself, and
  // that read is the real proof, never the process's exit code alone.
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

  // Loads the tracked list once at mount, async, see hasLoadedRef above,
  // then either:
  //  - native: subscribes to the Rust scheduler's `quota-refresh` push and
  //    kicks it once so the launch read is near-instant; or
  //  - browser/mock harness: there is no Rust scheduler to push from, so
  //    this does the one-time initial read itself.
  // biome-ignore lint/correctness/useExhaustiveDependencies: deliberately mount-once, see the comment above. Adding applyRefreshResult or refreshOneGuarded risks re-running the load on every render.
  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    // Read fresh on every mount, not cached at module scope, so tests can
    // toggle it. In the real app, Tauri injects this global before any app
    // JS runs, so this is equivalent to a constant in practice.
    const isTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

    void (async () => {
      const tracked = await loadTracked();
      if (cancelled) return;
      // An older record carries `pinned: boolean` instead of
      // `pinnedWindowIds`. Flag it for the one-shot migration in
      // `applyRefreshResult` rather than guessing a window id here. See
      // `pendingPinMigrationRef`'s own doc comment.
      const migrating = new Set<string>();
      const loaded = tracked.map((t) => {
        const legacy = t as unknown as { pinnedWindowIds?: unknown; pinned?: unknown };
        const pinnedWindowIds = Array.isArray(legacy.pinnedWindowIds)
          ? (legacy.pinnedWindowIds as string[])
          : [];
        if (!Array.isArray(legacy.pinnedWindowIds) && legacy.pinned === true) {
          migrating.add(t.id);
        }
        return initialSubscription(
          { id: t.id, provider: t.provider, config_dir: t.config_dir },
          t.label,
          pinnedWindowIds,
        );
      });
      pendingPinMigrationRef.current = migrating;
      setSubscriptions(loaded);
      // What is already on disk is the last saved state. Recording it here
      // keeps the save effect's first run from writing it straight back
      // with an `fsync` at launch that changes nothing.
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
        // No longer tracked, either already gone or inside its "Stop
        // tracking" undo window, which is the same thing everywhere except
        // the panel's own row list. Either way, an in-flight read's result
        // must not resurrect it.
        if (!existing || existing.pendingRemoval) return;
        const prior = priorReadOf(existing);
        const fallbackLabel = accountLabel({
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
  }, []);

  /** What "tracked" means everywhere except the panel's own row list:
   * persistence, the tray, the Subscriptions screen. A row inside its undo
   * window is already gone from all three. It survives only as a slot in
   * the panel, which is why `subscriptions`, returned below, still carries
   * it. */
  const trackedSubscriptions = useMemo(
    () => subscriptions.filter((s) => !s.pendingRemoval),
    [subscriptions],
  );

  // Persists the user-owned parts of the list, membership, custom labels,
  // pins, on every change, once the initial load has actually committed.
  // Ephemeral read state such as windows, used percent and reason is
  // deliberately not persisted, since a stale number must never be shown
  // as current after a restart. Every subscription re-verifies on launch.
  //
  // Compares against what was last written and skips an identical save.
  // This effect is keyed on the whole subscription list, so without the
  // compare it would also fire on every automatic read, and the native
  // `save_tracked` it calls is a temp-file write plus an `fsync` plus a
  // rename, see persistence.rs. Two tracked accounts read once a minute
  // each would mean two durable, blocking writes a minute that changed
  // nothing.
  //
  // The dedupe record is written before the save settles, so an effect
  // re-run with identical data never double-writes. A save that then
  // fails must not stay recorded as saved, since that would silence every
  // retry and the change would die with the process. Clearing the record,
  // unless a newer save already superseded it, makes the very next effect
  // run, even one where only read state changed, write again.
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

  // Mirrors pinned subscriptions' headline figures beside the tray glyph,
  // in consumed-percent terms, never remaining. A pin with no number yet,
  // including a broken one, draws nothing at all, never "!" or "…". The
  // glyph alone is the signal that something needs attention, and the
  // row's own badge carries the detail. Unpinned means gone, since this
  // effect is a pure function of current state every time it runs.
  //
  // Color comes from severity, any window at least 90% or 75% used, not
  // from the headline percentage's own magnitude, so a 20%-headline
  // account with a near-exhausted session still reads amber in the tray.
  // If any pinned value is stale, every digit in the tray turns amber, not
  // just that one account's own.
  //
  // Keyed on the tracked list, so "Stop tracking" drops a pinned account's
  // digits from the menu bar the instant it is pressed rather than when
  // its undo window expires.
  useEffect(() => {
    const segments = buildTraySegments(trackedSubscriptions);
    const worstUsedPercent = worstActiveLimitPercent(trackedSubscriptions);
    setTrayStatus(segments, worstUsedPercent, buildTrayTooltip(trackedSubscriptions));
  }, [trackedSubscriptions]);

  /** The name to show for an account the panel is not currently
   * rendering, the Subscriptions screen's untracked half. Prefers whatever
   * a real read last reported over the directory-derived fallback, so one
   * account reads the same in both halves of that list. */
  const displayLabelFor = useCallback(
    (account: AccountDescriptor) => knownLabels[account.id] ?? accountLabel(account),
    [knownLabels],
  );

  return {
    /** Everything the panel draws, in order, including rows inside their
     * "Stop tracking" undo window, which is what an Undo row is. */
    subscriptions,
    /** Everything that is actually tracked. What persistence, the tray and
     * the Subscriptions screen see. */
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
