import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { loadTracked, saveTracked, type TrackedAccount } from '@/lib/persistence'
import {
  buildStatusItemSegments,
  buildStatusItemTooltip,
  computeWorstActiveLimitPercent,
} from '@/lib/status-item-segments'
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
} from '@/lib/tauri-client'
import { mapOutcomeFor, normalizeFor, resolveProviderDisplayName } from '@/providers/registry'
import type {
  AccountDescriptor,
  FetchError,
  PinGroup,
  RawSnapshot,
  Subscription,
  SubscriptionState,
} from '@/types/entities'
import { isFetchError } from '@/types/entities'

export function deriveAccountLabel(account: AccountDescriptor): string {
  const slug = account.id.split(':')[1] ?? account.id
  return slug
    .replace(/[-_]/g, ' ')
    .replace(/\S+/g, (word) => word.charAt(0).toUpperCase() + word.slice(1))
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
    state: 'connecting',
    severity: 'healthy',
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
  }
}

export const STOP_TRACKING_UNDO_MS = 5_000

function migrateLegacyTracked(tracked: TrackedAccount[]): {
  subscriptions: Subscription[]
  migratingPinIds: Set<string>
} {
  const migratingPinIds = new Set<string>()
  const subscriptions = tracked.map((t) => {
    const legacy = t as unknown as { pinnedWindowIds?: unknown; pinned?: unknown }
    const pinnedWindowIds = Array.isArray(legacy.pinnedWindowIds)
      ? (legacy.pinnedWindowIds as string[])
      : []
    if (!Array.isArray(legacy.pinnedWindowIds) && legacy.pinned === true) {
      migratingPinIds.add(t.id)
    }
    return buildInitialSubscription(
      { id: t.id, provider: t.provider, config_dir: t.config_dir },
      t.label,
      pinnedWindowIds,
    )
  })
  return { subscriptions, migratingPinIds }
}

function toTrackedAccounts(subscriptions: Subscription[]): TrackedAccount[] {
  return subscriptions.map((s) => ({
    id: s.id,
    provider: s.provider,
    config_dir: s.configDir,
    label: s.labelOverride,
    pinnedWindowIds: s.pinnedWindowIds,
  }))
}

interface PriorRead {
  hadGoodRead: boolean
  state: SubscriptionState
  reason: string | null
  needsSignIn: boolean
}

function capturePriorRead(sub: Subscription | undefined): PriorRead {
  return {
    hadGoodRead: !!sub?.lastReadAt,
    state: sub?.state ?? 'connecting',
    reason: sub?.reason ?? null,
    needsSignIn: sub?.needsSignIn ?? false,
  }
}

export function useSubscriptions(pinGroups: PinGroup[] = []) {
  const [subscriptions, setSubscriptions] = useState<Subscription[]>([])
  const subscriptionsRef = useRef<Subscription[]>(subscriptions)
  subscriptionsRef.current = subscriptions

  const [knownLabels, setKnownLabels] = useState<Record<string, string>>({})
  const knownLabelsRef = useRef<Record<string, string>>(knownLabels)
  knownLabelsRef.current = knownLabels

  const removalTimers = useRef<Record<string, ReturnType<typeof setTimeout>>>({})

  const lastSavedRef = useRef<string | null>(null)

  const [saveError, setSaveError] = useState<Error | null>(null)

  const pendingPinMigrationRef = useRef<Set<string>>(new Set())

  const hasLoadedRef = useRef(false)

  const patch = useCallback((id: string, changes: Partial<Subscription>) => {
    setSubscriptions((prev) => prev.map((s) => (s.id === id ? { ...s, ...changes } : s)))
  }, [])

  const applyRefreshResult = useCallback(
    (
      accountId: string,
      provider: string,
      fallbackLabel: string,
      prior: PriorRead,
      outcome: { ok: true; raw: RawSnapshot } | { ok: false; error: FetchError | null },
    ) => {
      if (outcome.ok) {
        const raw = outcome.raw
        const normalized = normalizeFor(provider, raw.usage, raw.profile, fallbackLabel, {
          fetchedAt: raw.fetched_at,
          statuslineFeed: raw.statusline,
        })
        const mapped = mapOutcomeFor(provider, { kind: 'ok', normalized }, prior.hadGoodRead)
        setKnownLabels((prev) =>
          prev[accountId] === normalized.label ? prev : { ...prev, [accountId]: normalized.label },
        )
        const pinnedWindowIds = pendingPinMigrationRef.current.has(accountId)
          ? normalized.headlineWindowId
            ? [normalized.headlineWindowId]
            : []
          : undefined
        pendingPinMigrationRef.current.delete(accountId)
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
        })
        return
      }

      const err = outcome.error
      if (err && err.kind === 'rate_limited') {
        const until = new Date(Date.now() + err.retry_after_secs * 1000).toISOString()
        // A rate limit caught mid-attempt can't write the in-flight state
        // back verbatim, or the row would read "reading" forever.
        const settledPrior =
          prior.state === 'connecting' || prior.state === 'reading'
            ? prior.hadGoodRead
              ? 'working'
              : 'idle'
            : prior.state
        patch(accountId, {
          state: settledPrior,
          reason: prior.reason,
          needsSignIn: prior.needsSignIn,
          rateLimitedUntil: until,
        })
        return
      }
      const mapped = mapOutcomeFor(provider, { kind: 'error', error: err }, prior.hadGoodRead)
      patch(accountId, {
        state: mapped.state,
        reason: mapped.reason,
        needsSignIn: mapped.needsSignIn,
        rateLimitedUntil: null,
      })
    },
    [patch],
  )

  const refreshOne = useCallback(
    async (account: AccountDescriptor) => {
      const prior = capturePriorRead(subscriptionsRef.current.find((s) => s.id === account.id))
      patch(account.id, { state: prior.hadGoodRead ? 'reading' : 'connecting' })

      try {
        const raw = await fetchSnapshot(account)
        applyRefreshResult(account.id, account.provider, deriveAccountLabel(account), prior, {
          ok: true,
          raw,
        })
      } catch (err) {
        applyRefreshResult(account.id, account.provider, deriveAccountLabel(account), prior, {
          ok: false,
          error: isFetchError(err) ? err : null,
        })
      }
    },
    [patch, applyRefreshResult],
  )

  const refreshAllInFlight = useRef<Promise<void> | null>(null)
  const refreshOneInFlight = useRef<Map<string, Promise<void>>>(new Map())

  const refreshOneGuarded = useCallback(
    async (account: AccountDescriptor) => {
      const inFlight = refreshOneInFlight.current.get(account.id)
      if (inFlight) return inFlight
      const run = refreshOne(account)
      refreshOneInFlight.current.set(account.id, run)
      try {
        await run
      } finally {
        refreshOneInFlight.current.delete(account.id)
      }
    },
    [refreshOne],
  )

  const refreshAll = useCallback(async () => {
    if (refreshAllInFlight.current) return refreshAllInFlight.current
    const run = (async () => {
      const targets = subscriptionsRef.current.filter((s) => !s.pendingRemoval)
      await Promise.allSettled(
        targets.map((s) =>
          refreshOneGuarded({ id: s.id, provider: s.provider, config_dir: s.configDir }),
        ),
      )
    })()
    refreshAllInFlight.current = run
    try {
      await run
    } finally {
      refreshAllInFlight.current = null
    }
  }, [refreshOneGuarded])

  const refreshAccountById = useCallback(
    async (id: string) => {
      const sub = subscriptionsRef.current.find((s) => s.id === id)
      if (!sub || sub.pendingRemoval) return
      return refreshOneGuarded({ id: sub.id, provider: sub.provider, config_dir: sub.configDir })
    },
    [refreshOneGuarded],
  )

  const togglePin = useCallback((id: string, windowId: string | null) => {
    if (windowId === null) return
    setSubscriptions((prev) =>
      prev.map((s) => {
        if (s.id !== id) return s
        const has = s.pinnedWindowIds.includes(windowId)
        return {
          ...s,
          pinnedWindowIds: has
            ? s.pinnedWindowIds.filter((w) => w !== windowId)
            : [...s.pinnedWindowIds, windowId],
        }
      }),
    )
  }, [])

  const renameSubscription = useCallback((id: string, label: string | null) => {
    setSubscriptions((prev) => prev.map((s) => (s.id === id ? { ...s, labelOverride: label } : s)))
  }, [])

  const moveSubscription = useCallback((id: string, direction: 'up' | 'down') => {
    setSubscriptions((prev) => {
      const index = prev.findIndex((s) => s.id === id)
      if (index === -1) return prev
      const neighbor = direction === 'up' ? index - 1 : index + 1
      if (neighbor < 0 || neighbor >= prev.length) return prev
      const next = [...prev]
      ;[next[index], next[neighbor]] = [next[neighbor], next[index]]
      return next
    })
  }, [])

  const clearRemovalTimer = useCallback((id: string) => {
    const timer = removalTimers.current[id]
    if (timer === undefined) return
    clearTimeout(timer)
    delete removalTimers.current[id]
  }, [])

  const addSubscription = useCallback(
    (account: AccountDescriptor) => {
      clearRemovalTimer(account.id)
      setSubscriptions((prev) => {
        const existing = prev.find((s) => s.id === account.id)
        if (existing) {
          return existing.pendingRemoval
            ? prev.map((s) => (s.id === account.id ? { ...s, pendingRemoval: false } : s))
            : prev
        }
        return [
          ...prev,
          buildInitialSubscription(account, null, [], knownLabelsRef.current[account.id]),
        ]
      })
      void refreshOneGuarded(account)
    },
    [refreshOneGuarded, clearRemovalTimer],
  )

  const removeSubscription = useCallback(
    (id: string) => {
      clearRemovalTimer(id)
      setSubscriptions((prev) => prev.filter((s) => s.id !== id))
      void cancelSignInIpc(id)
    },
    [clearRemovalTimer],
  )

  const stopTracking = useCallback(
    (id: string) => {
      clearRemovalTimer(id)
      setSubscriptions((prev) =>
        prev.map((s) => (s.id === id ? { ...s, pendingRemoval: true } : s)),
      )
      void cancelSignInIpc(id)
      removalTimers.current[id] = setTimeout(() => {
        delete removalTimers.current[id]
        setSubscriptions((prev) => prev.filter((s) => !(s.id === id && s.pendingRemoval)))
      }, STOP_TRACKING_UNDO_MS)
    },
    [clearRemovalTimer],
  )

  const undoStopTracking = useCallback(
    (id: string) => {
      clearRemovalTimer(id)
      setSubscriptions((prev) =>
        prev.map((s) => (s.id === id ? { ...s, pendingRemoval: false } : s)),
      )
    },
    [clearRemovalTimer],
  )

  useEffect(() => {
    const timers = removalTimers
    return () => {
      Object.values(timers.current).forEach(clearTimeout)
      timers.current = {}
    }
  }, [])

  const startSignIn = useCallback(
    async (id: string) => {
      const sub = subscriptionsRef.current.find((s) => s.id === id)
      if (!sub) return
      patch(id, { signInInProgress: true })
      try {
        await startSignInIpc(id, sub.configDir)
      } catch (err) {
        const message = typeof err === 'string' ? err : err instanceof Error ? err.message : null
        patch(id, {
          signInInProgress: false,
          reason: message ?? "Quotos couldn't start the Claude Code sign-in.",
        })
      }
    },
    [patch],
  )

  const submitSignInCode = useCallback(async (id: string, code: string) => {
    await submitSignInCodeIpc(id, code)
  }, [])

  const cancelSignIn = useCallback(
    async (id: string) => {
      await cancelSignInIpc(id)
      patch(id, { signInInProgress: false })
    },
    [patch],
  )

  useEffect(() => {
    let cancelled = false
    let unlisten: (() => void) | undefined
    void onSignInFinished((event) => {
      patch(event.account_id, { signInInProgress: false })
      void forgetSignIn(event.account_id)
      if (subscriptionsRef.current.some((s) => s.id === event.account_id)) {
        void refreshAccountById(event.account_id)
      }
    }).then((fn) => {
      if (cancelled) fn()
      else unlisten = fn
    })
    return () => {
      cancelled = true
      unlisten?.()
    }
  }, [patch, refreshAccountById])

  useEffect(() => {
    let cancelled = false
    let unlisten: (() => void) | undefined
    const isTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window

    void (async () => {
      const tracked = await loadTracked()
      if (cancelled) return
      const { subscriptions: loaded, migratingPinIds } = migrateLegacyTracked(tracked)
      pendingPinMigrationRef.current = migratingPinIds
      setSubscriptions(loaded)
      lastSavedRef.current = JSON.stringify(toTrackedAccounts(loaded))
      hasLoadedRef.current = true

      if (!isTauri) {
        void Promise.allSettled(
          loaded.map((s) =>
            refreshOneGuarded({ id: s.id, provider: s.provider, config_dir: s.configDir }),
          ),
        )
        return
      }

      unlisten = await onQuotaRefresh((event) => {
        const accountId = event.kind === 'ok' ? event.snapshot.account_id : event.account_id
        const existing = subscriptionsRef.current.find((s) => s.id === accountId)
        if (!existing || existing.pendingRemoval) return
        const prior = capturePriorRead(existing)
        const fallbackLabel = deriveAccountLabel({
          id: accountId,
          provider: existing.provider,
          config_dir: existing.configDir,
        })
        if (event.kind === 'ok') {
          applyRefreshResult(accountId, existing.provider, fallbackLabel, prior, {
            ok: true,
            raw: event.snapshot,
          })
        } else {
          applyRefreshResult(accountId, existing.provider, fallbackLabel, prior, {
            ok: false,
            error: event.error,
          })
        }
      })
      if (cancelled) {
        unlisten()
        unlisten = undefined
        return
      }
      void kickScheduler()
    })()

    return () => {
      cancelled = true
      unlisten?.()
    }
  }, [applyRefreshResult, refreshOneGuarded])

  const trackedSubscriptions = useMemo(
    () => subscriptions.filter((s) => !s.pendingRemoval),
    [subscriptions],
  )

  useEffect(() => {
    if (!hasLoadedRef.current) return
    const tracked = toTrackedAccounts(trackedSubscriptions)
    const serialized = JSON.stringify(tracked)
    if (lastSavedRef.current === serialized) return
    lastSavedRef.current = serialized
    void (async () => {
      try {
        await saveTracked(tracked)
        setSaveError(null)
      } catch (error) {
        console.error(
          'Quotos: saving the tracked list failed; will retry on the next change',
          error,
        )
        if (lastSavedRef.current === serialized) lastSavedRef.current = null
        setSaveError(error instanceof Error ? error : new Error(String(error)))
      }
    })()
  }, [trackedSubscriptions])

  // The one segment list there is: the menu bar is drawn from it, and
  // the customize screen previews it rather than deriving its own.
  const statusItemSegments = useMemo(
    () => buildStatusItemSegments(trackedSubscriptions, pinGroups),
    [trackedSubscriptions, pinGroups],
  )

  useEffect(() => {
    const worstUsedPercent = computeWorstActiveLimitPercent(trackedSubscriptions)
    renderStatusItem(
      statusItemSegments,
      worstUsedPercent,
      buildStatusItemTooltip(trackedSubscriptions, pinGroups),
    )
  }, [statusItemSegments, trackedSubscriptions, pinGroups])

  const displayLabelFor = useCallback(
    (account: AccountDescriptor) => knownLabels[account.id] ?? deriveAccountLabel(account),
    [knownLabels],
  )

  return {
    subscriptions,
    trackedSubscriptions,
    statusItemSegments,
    saveError,
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
  }
}
