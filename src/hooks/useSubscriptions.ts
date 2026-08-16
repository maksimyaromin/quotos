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

/** The id-derived default label for an account before any read has come back
 * (or before a custom rename) — shared with the add-subscription flow so a
 * candidate shows the same name there as it will once tracked.
 *
 * R4-4: title-cased. The old version special-cased `claude` → "Claude" and
 * left every other slug exactly as the directory is spelled, so `~/.claude-team`
 * rendered "claude team" in the Subscriptions screen while the panel showed
 * "Claude Team" for the same account — two spellings of one thing, side by
 * side, in the captain's 2026-08-15 screencast. This is presentation
 * consistency, not a naming policy: the same words, capitalised the way every
 * other name in the panel is. */
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

/** How long "Stop tracking" stays undoable. The account is untracked from the
 * moment the button is pressed (R4-3); this is only how long the Undo row
 * keeps its slot in the panel. */
export const STOP_TRACKING_UNDO_MS = 5_000;

/** The persisted projection of a subscription: membership, custom name, pin —
 * and nothing that a read produced. */
function toTrackedAccounts(subscriptions: Subscription[]): TrackedAccount[] {
  return subscriptions.map((s) => ({
    id: s.id,
    provider: s.provider,
    config_dir: s.configDir,
    label: s.labelOverride,
    pinnedWindowIds: s.pinnedWindowIds,
  }));
}

/** Everything a read outcome needs to know about what came *before* it —
 * captured before the optimistic patch that starts a manual refresh, so a
 * rate-limited answer restores the real prior diagnosis rather than that
 * patch (the B6 regression). */
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

  // R4-4: the best name we have ever learned for an account, by id — the
  // provider's own (`organization.name`, e.g. "Claude Max"), kept after the
  // account stops being tracked. Without it, untracking an account renamed the
  // captain's "Claude Max" back to a bare "Claude" in the very same list, the
  // instant it moved from the tracked half to the untracked half, because the
  // untracked half had nothing but the config directory's name to go on. A
  // session-lifetime cache, deliberately: it is a presentation nicety derived
  // from reads this session actually made, not a new thing to persist.
  const [knownLabels, setKnownLabels] = useState<Record<string, string>>({});
  const knownLabelsRef = useRef<Record<string, string>>(knownLabels);
  knownLabelsRef.current = knownLabels;

  // R4-3: the live "Stop tracking" undo timers, by id. Owned here rather than
  // in App.tsx because the state they resolve into (membership) is owned here
  // — that split is exactly what let the two views disagree.
  const removalTimers = useRef<Record<string, ReturnType<typeof setTimeout>>>({});

  /** The exact JSON last handed to `saveTracked` — see the save effect. */
  const lastSavedRef = useRef<string | null>(null);

  // v4 migration (firstmate-recorded decision, docs/design/NOTES.md §2): a
  // pre-v4 tracked record's `pinned: true` becomes "that subscription's
  // headline window is pinned" — the same window the "…" menu's toggle now
  // targets. Which window is the headline is only known once a read comes
  // back (`NormalizedRead.headlineWindowId`), so this just remembers *which*
  // accounts still need the one-shot migration; `applyRefreshResult`'s ok
  // branch is the only place that consumes an id from here, on the first
  // *successful* read after load (a failed attempt leaves the flag pending,
  // so the migration simply waits for a read that actually has an answer).
  const pendingPinMigrationRef = useRef<Set<string>>(new Set());

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
        // R4-4: remember it for the Subscriptions screen, which otherwise has
        // only the config directory's name to show once this account stops
        // being tracked.
        setKnownLabels((prev) =>
          prev[accountId] === normalized.label ? prev : { ...prev, [accountId]: normalized.label },
        );
        // v4 pin migration: consumed at most once per account, on whichever
        // read (successful or not — see below) lands first after load.
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
        // B5/B6: a self-imposed wait is a separate fact, not a health
        // state — never routed through the provider's outcome mapper.
        // Restore exactly the health state/reason this account had before
        // this attempt started; a real diagnosis (e.g. Broken/expired
        // login) must never decay into a generic wait.
        //
        // R5: unless that prior state is itself an in-flight presentation
        // ("connecting"/"reading" — an attempt, not a diagnosis). Writing
        // one back leaves the row claiming "Reading…" forever with nothing
        // in flight, and the footer's reading branch then masks the wait
        // note rowPresentation composes. A never-read account settles back
        // to idle; one with data settles to working (transient either way —
        // an in-flight prior means another attempt's own result is coming).
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
      // Captured *before* the optimistic patch below — otherwise a
      // rate-limited outcome restoring "prior" state would read back its
      // own optimistic "reading"/"connecting" patch instead of the real
      // diagnosis that came before it (the exact B6 regression).
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

  // R2-4: "the manual refresh control is debounced" — concurrent calls
  // collapse into the single in-flight one instead of starting a second
  // fetch (the header button already disables itself while its own promise
  // is pending, but that's a courtesy, not the source of truth).
  const refreshAllInFlight = useRef<Promise<void> | null>(null);
  const refreshOneInFlight = useRef<Map<string, Promise<void>>>(new Map());

  // F4: the per-account guard itself. Every path that spends a real fetch
  // on one account — a row's "Read now", the header refresh's per-account
  // fan-out, the launch read, the read a newly-added subscription gets —
  // goes through here, so two of them hitting the same account at once join
  // one in-flight read instead of spending two of the shared 5-per-300s
  // budget slots on it. The invariant: `refreshOne` has exactly one caller,
  // and it is this function.
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

  // R3-4: no local "is it blocked?" filter here any more. Skipping
  // rate-limited subscriptions client-side is what left the captain with no
  // way out at all: once a long provider-issued wait was pending, the panel
  // refresh control and "Read now" both returned without doing anything,
  // silently, so a wrong diagnosis could never be re-tested. The Rust
  // limiter is the single authority and refuses without spending anything,
  // so always attempting costs nothing and keeps every revival path live —
  // the moment the budget frees up, the very next press reads for real.
  const refreshAll = useCallback(async () => {
    if (refreshAllInFlight.current) return refreshAllInFlight.current;
    const run = (async () => {
      // R4-3: a row inside its "Stop tracking" undo window is untracked
      // already — never spend a read on it.
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

  // v4: pinning is per-window (docs/design/NOTES.md §2) — `windowId` is either a
  // specific window's own id (the per-window pin buttons) or the
  // subscription's current headline window (the "…" menu's toggle, wired by
  // the caller). A `null` windowId (no headline yet — nothing read) is a
  // no-op, since there's nothing to pin.
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

  /** The "…" menu's "Move up"/"Move down": swaps the row with its neighbor
   * in the panel's own slot order (an Undo row is a slot too, so moving past
   * one is visible and coherent). Panel order is the one order everywhere —
   * the persisted list and the tray's digit order both derive from it, so a
   * swap here reorders all three together. */
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

  /** R4-3: cancels a pending "Stop tracking" timer, if one is running. */
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
        // Adding back something whose undo window is still open is the same
        // action as undoing it — same account, same slot, same name.
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
      // Brief §5.3 step 4: verify by reading once immediately, so the
      // person sees what came back rather than a cold placeholder. F4: this
      // spends a real budget slot, so it goes through the guard like every
      // other fetch — calling `refreshOne` here left the read unregistered,
      // and a refresh landing during it (widest window: the bounded-20s CLI
      // renewal an aged-out token needs) started a second request for the
      // same account.
      void refreshOneGuarded(account);
    },
    [refreshOneGuarded, clearRemovalTimer],
  );

  /** Hard, immediate removal — the Subscriptions screen's own [Remove], which
   * has no undo affordance of its own. */
  const removeSubscription = useCallback(
    (id: string) => {
      clearRemovalTimer(id);
      setSubscriptions((prev) => prev.filter((s) => s.id !== id));
      void cancelSignInIpc(id);
    },
    [clearRemovalTimer],
  );

  /** R4-3: the panel row menu's "Stop tracking". Untracks the account *now*
   * — it leaves the tray, stops being read, and stops being persisted this
   * instant — and keeps its slot in the panel for [`STOP_TRACKING_UNDO_MS`] so
   * the Undo row can sit there.
   *
   * The previous shape deferred the entire removal by five seconds, which is
   * how the Subscriptions screen came to be showing [Remove] for two accounts
   * the captain had already stopped tracking (his 2026-08-15 screencast, at
   * t=25.0: both Undo rows visible in the panel, both accounts still listed as
   * tracked; "Claude Team" only flipped at t≈26.0 and "Claude Max" at t≈29.2 —
   * both exactly five seconds after their own click, because the timer, not
   * the click, was what actually untracked them). */
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

  // R2-6: starts Claude Code's own sign-in for a broken row — see
  // signin.rs. Quotos never touches the Keychain or a credential; it only
  // starts the process and later relays a pasted code back into it.
  const startSignIn = useCallback(
    async (id: string) => {
      const sub = subscriptionsRef.current.find((s) => s.id === id);
      if (!sub) return;
      patch(id, { signInInProgress: true });
      try {
        await startSignInIpc(id, sub.configDir);
      } catch (err) {
        // R3-4: a failed start used to just drop the row back out of the
        // flow, which is what the captain saw as the paste-code field
        // flashing up and vanishing — with no PATH in a Finder-launched
        // app, the spawn failed instantly and nothing said so. Say what
        // went wrong in the row's own reason line instead.
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

  // R2-6: fires once when a sign-in process exits (success or failure — see
  // signin.rs's doc comment on why `success` there is only the process's
  // own exit status). Either way, Quotos re-reads the account itself; that
  // read is the real proof, never the process's exit code alone.
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

  // Loads the tracked list once at mount (async — see hasLoadedRef above),
  // then either:
  //  - native: subscribes to the Rust scheduler's `quota-refresh` push and
  //    kicks it once so the launch read is near-instant (R2-4); or
  //  - browser/mock harness: there's no Rust scheduler to push from, so this
  //    does the one-time initial read itself, exactly as before R2-4 moved
  //    cadence onto the Rust side.
  // biome-ignore lint/correctness/useExhaustiveDependencies: deliberately mount-once (see the comment above, and the prior eslint-disable this replaces); adding applyRefreshResult/refreshOneGuarded risks re-running the load on every render.
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
      // v4 migration: a record still in the pre-v4 shape carries `pinned:
      // boolean` instead of `pinnedWindowIds` — flag it for the one-shot
      // migration in `applyRefreshResult` rather than guessing a window id
      // here (see `pendingPinMigrationRef`'s own doc comment).
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
      // What is already on disk *is* the last saved state — recording it here
      // keeps the save effect's first run from writing it straight back
      // (an `fsync` at launch that changes nothing).
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
        // No longer tracked — either already gone, or inside its "Stop
        // tracking" undo window, which is the same thing everywhere but the
        // panel's own row list (R4-3). Either way an in-flight read's result
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

  /** R4-3: what "tracked" means everywhere except the panel's own row list —
   * persistence, the tray, the Subscriptions screen. A row inside its undo
   * window is already gone from all three; it survives only as a slot in the
   * panel, which is why `subscriptions` (returned below) still carries it. */
  const trackedSubscriptions = useMemo(
    () => subscriptions.filter((s) => !s.pendingRemoval),
    [subscriptions],
  );

  // Persist the user-owned parts of the list (membership, custom labels,
  // pins) on every change, once the initial load has actually committed.
  // Ephemeral read state (windows, used %, reason) is deliberately not
  // persisted — a stale number must never be shown as current after a
  // restart; every subscription re-verifies on launch.
  //
  // R4-2: compares against what was last written and skips an identical save.
  // This effect is keyed on the whole subscription list, so before the compare
  // it also fired on every automatic read — and the native `save_tracked` it
  // calls is a temp-file write plus an `fsync` plus a rename (persistence.rs).
  // Two tracked accounts read once a minute each meant two durable, blocking
  // writes a minute that changed nothing.
  //
  // R2: the dedupe record is written before the save settles (so an effect
  // re-run with identical data never double-writes), but a save that then
  // *fails* must not stay recorded as saved — that would silence every
  // retry and the change would die with the process. Clearing the record
  // (unless a newer save already superseded it) makes the very next effect
  // run — even one where only read state changed — write again.
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

  // Mirror pinned subscriptions' headline figures beside the tray glyph
  // (I2: consumed, not remaining). Handoff, verbatim: "Никаких знаков и
  // многоточий там, где ожидается число. Либо цифра, либо ничего" — so a
  // pin with no number yet (including a broken one) draws nothing at all,
  // never "!" or "…"; the glyph alone is the signal that something needs
  // attention (the row's own badge carries the detail). Unpinned means gone
  // — this effect is a pure function of current state every time it runs.
  //
  // R2-2: color comes from severity (any window >=90%/75%), not from the
  // headline percentage's own magnitude, so a 20%-headline account with a
  // near-exhausted session still reads amber in the tray. Followup-2, same
  // handoff table: "если хотя бы одно значение устарело — все цифры
  // янтарные" — if *any* pinned value is stale, every digit in the tray
  // turns amber, not just that one account's own.
  //
  // R4-3: keyed on the *tracked* list, so "Stop tracking" drops a pinned
  // account's digits from the menu bar the instant it is pressed rather than
  // when its undo window expires.
  useEffect(() => {
    const segments = buildTraySegments(trackedSubscriptions);
    const worstUsedPercent = worstActiveLimitPercent(trackedSubscriptions);
    setTrayStatus(segments, worstUsedPercent, buildTrayTooltip(trackedSubscriptions));
  }, [trackedSubscriptions]);

  /** R4-4: the name to show for an account the panel isn't currently
   * rendering — the Subscriptions screen's untracked half. Prefers whatever a
   * real read last reported over the directory-derived fallback, so one
   * account reads the same in both halves of that list. */
  const displayLabelFor = useCallback(
    (account: AccountDescriptor) => knownLabels[account.id] ?? accountLabel(account),
    [knownLabels],
  );

  return {
    /** Everything the panel draws, in order — including rows inside their
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
