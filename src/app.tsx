import { useEffect, useState } from 'react'
import { BackIcon, DebugIcon, PlusIcon, RefreshIcon, SnapBackIcon } from '@/components/icons'
import { SubscriptionsScreen } from '@/components/subscriptions-screen'
import { UndoRow } from '@/components/undo-row'
import { Button, IconButton, Panel, SubscriptionRow } from '@/design-system'
import { useSubscriptions } from '@/hooks/use-subscriptions'
import { presentRow } from '@/lib/row-presentation'
import {
  debugRateLimitSnapshot,
  dragWindowStep,
  endWindowDrag,
  hidePanel,
  onPanelBeakOffset,
  onPanelVisibility,
  setDetached as setDetachedIpc,
} from '@/lib/tauri-client'
import { formatClockTime, formatExactReset, formatRelativePast } from '@/lib/time'
import './app.css'
import styles from './app.module.css'

const NOW_TICK_MS = 30_000

const BEAK_LEFT_MOCK_FALLBACK = 24

const isTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window

function isBlocked(rateLimitedUntil: string | null, now: number): boolean {
  return !!rateLimitedUntil && new Date(rateLimitedUntil).getTime() > now
}

export default function App() {
  const {
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
  } = useSubscriptions()
  const [expanded, setExpanded] = useState<Set<string>>(new Set())
  const [openMenuId, setOpenMenuId] = useState<string | null>(null)
  const [refreshing, setRefreshing] = useState(false)
  const [screen, setScreen] = useState<'list' | 'manage'>('list')
  const [detached, setDetached] = useState(false)
  const [dragging, setDragging] = useState(false)
  const [position, setPosition] = useState<{ x: number; y: number } | null>(null)
  const [now, setNow] = useState(() => Date.now())
  // The native build starts at null rather than this same fallback: a
  // stale beak position drawn confidently is worse than one that visibly
  // fails to draw.
  const [beakLeft, setBeakLeft] = useState<number | null>(isTauri ? null : BEAK_LEFT_MOCK_FALLBACK)

  useEffect(() => {
    let cancelled = false
    let unlisten: (() => void) | undefined
    void onPanelBeakOffset(setBeakLeft).then((fn) => {
      if (cancelled) fn()
      else unlisten = fn
    })
    return () => {
      cancelled = true
      unlisten?.()
    }
  }, [])

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== 'Escape') return
      if (openMenuId) {
        setOpenMenuId(null)
        return
      }
      hidePanel()
    }
    window.addEventListener('keydown', onKeyDown)
    return () => window.removeEventListener('keydown', onKeyDown)
  }, [openMenuId])

  useEffect(() => {
    let cancelled = false
    let unlisten: (() => void) | undefined
    void onPanelVisibility((visible) => {
      if (visible) setNow(Date.now())
      else setOpenMenuId(null)
    }).then((fn) => {
      if (cancelled) fn()
      else unlisten = fn
    })
    return () => {
      cancelled = true
      unlisten?.()
    }
  }, [])

  useEffect(() => {
    const interval = setInterval(() => setNow(Date.now()), NOW_TICK_MS)
    return () => clearInterval(interval)
  }, [])

  useEffect(() => {
    if (!openMenuId) return
    const onClickCapture = (event: MouseEvent) => {
      const target = event.target as HTMLElement | null
      if (target?.closest('[data-quotos-menu-scope]')) return
      event.preventDefault()
      event.stopPropagation()
      setOpenMenuId(null)
    }
    window.addEventListener('click', onClickCapture, true)
    return () => window.removeEventListener('click', onClickCapture, true)
  }, [openMenuId])

  const toggleExpand = (id: string) => {
    setExpanded((prev) => {
      const next = new Set(prev)
      if (next.has(id)) next.delete(id)
      else next.add(id)
      return next
    })
  }

  const handleManualRefresh = async () => {
    setRefreshing(true)
    try {
      await refreshAll()
    } finally {
      setRefreshing(false)
    }
  }

  const handleHeaderPointerDown = (event: React.MouseEvent) => {
    if (event.button !== 0) return
    if ((event.target as HTMLElement).closest('button, input')) return
    const startX = event.clientX
    const startY = event.clientY
    const panelEl = document.querySelector('[data-quotos-panel]')
    const startRect = panelEl?.getBoundingClientRect() ?? null
    let started = false

    const cleanup = () => {
      window.removeEventListener('mousemove', onMove)
      window.removeEventListener('mouseup', onUp)
    }
    const onMove = (moveEvent: MouseEvent) => {
      if (!started) {
        started = true
        setDragging(true)
        setDetached(true)
        void setDetachedIpc(true)
      }
      if (isTauri) {
        void dragWindowStep()
        return
      }
      if (startRect) {
        setPosition({
          x: Math.max(8, startRect.left + (moveEvent.clientX - startX)),
          y: Math.max(8, startRect.top + (moveEvent.clientY - startY)),
        })
      }
    }
    const onUp = () => {
      setDragging(false)
      if (isTauri) {
        void endWindowDrag()
      }
      cleanup()
    }
    window.addEventListener('mousemove', onMove)
    window.addEventListener('mouseup', onUp)
  }

  const handleSnapBack = async () => {
    setDetached(false)
    setDragging(false)
    setPosition(null)
    await setDetachedIpc(false)
  }

  const goManage = () => {
    setOpenMenuId(null)
    setScreen('manage')
  }
  const goList = () => setScreen('list')

  const handleDebugDump = async () => {
    const rateLimit = await debugRateLimitSnapshot()
    const dump = {
      dumpedAt: new Date().toISOString(),
      subscriptions,
      rateLimit,
    }
    const json = JSON.stringify(dump, null, 2)
    console.log('[quotos debug state]', dump)
    try {
      await navigator.clipboard.writeText(json)
    } catch {}
  }

  const nowDate = new Date(now)
  const blockedSubs = trackedSubscriptions.filter(
    (s) => !s.needsSignIn && isBlocked(s.rateLimitedUntil, now),
  )
  const allBlocked =
    trackedSubscriptions.length > 0 && blockedSubs.length === trackedSubscriptions.length
  const earliestAvailable = blockedSubs.length
    ? blockedSubs.reduce((min, s) =>
        new Date(s.rateLimitedUntil as string).getTime() <
        new Date(min.rateLimitedUntil as string).getTime()
          ? s
          : min,
      ).rateLimitedUntil
    : null
  const refreshLabel = refreshing
    ? 'Reading…'
    : allBlocked
      ? `Waiting for the rate budget — retry at ${formatClockTime(earliestAvailable)}`
      : 'Read all now'
  const mostRecentRead = trackedSubscriptions.reduce<string | null>((latest, s) => {
    if (!s.lastReadAt) return latest
    if (!latest || new Date(s.lastReadAt).getTime() > new Date(latest).getTime())
      return s.lastReadAt
    return latest
  }, null)
  const footerSummary = refreshing
    ? 'Reading…'
    : mostRecentRead
      ? `Last read ${formatRelativePast(mostRecentRead, nowDate)}`
      : ''

  return (
    <Panel
      title={screen === 'manage' ? 'Subscriptions' : 'Quotos'}
      docked={!detached}
      beakLeft={beakLeft ?? undefined}
      dragging={dragging}
      onHeaderPointerDown={handleHeaderPointerDown}
      position={position}
      leading={
        screen === 'manage' ? (
          <IconButton label="Back" onClick={goList} className={styles.backButton}>
            <BackIcon />
          </IconButton>
        ) : null
      }
      headerActions={
        <>
          {import.meta.env.DEV && screen === 'list' ? (
            <IconButton label="Copy debug state (dev only)" onClick={handleDebugDump}>
              <DebugIcon />
            </IconButton>
          ) : null}
          {screen === 'list' ? (
            <IconButton label={refreshLabel} onClick={handleManualRefresh} disabled={refreshing}>
              <RefreshIcon spinning={refreshing} />
            </IconButton>
          ) : null}
          {detached ? (
            <IconButton label="Snap back to the menu bar" onClick={handleSnapBack}>
              <SnapBackIcon />
            </IconButton>
          ) : null}
        </>
      }
      footer={
        screen === 'list' ? (
          <>
            <Button variant="ghost" size="sm" icon={<PlusIcon />} onClick={goManage}>
              Add subscription
            </Button>
            <div className={styles.footerSpacer} />
            <span className={styles.footerSummary}>{footerSummary}</span>
          </>
        ) : null
      }
    >
      {screen === 'manage' ? (
        <SubscriptionsScreen
          tracked={trackedSubscriptions}
          onAdd={addSubscription}
          onRemove={removeSubscription}
          displayLabelFor={displayLabelFor}
        />
      ) : subscriptions.length === 0 ? (
        <div className="quotos-empty">
          <p>Nothing tracked yet. Add a subscription and you'll see what's left on it here.</p>
        </div>
      ) : (
        subscriptions.map((sub, index) => {
          if (sub.pendingRemoval) {
            return (
              <UndoRow
                key={sub.id}
                label={sub.labelOverride ?? sub.label}
                onUndo={() => undoStopTracking(sub.id)}
              />
            )
          }
          const presentation = presentRow(sub, now)
          const label = sub.labelOverride ?? sub.label
          const windows = sub.windows.map((w) => ({
            id: w.id,
            name: w.name,
            used: w.used,
            resetLabel: formatExactReset(w.resetsAt, nowDate),
            scope: w.scope,
            pinned: sub.pinnedWindowIds.includes(w.id),
          }))
          return (
            <SubscriptionRow
              key={sub.id}
              label={label}
              provider={sub.providerName}
              account={sub.account ?? undefined}
              state={sub.state}
              used={sub.used}
              severity={sub.severity}
              resetLabel={formatExactReset(sub.resetsAt, nowDate) ?? undefined}
              lastRead={formatRelativePast(sub.lastReadAt, nowDate) ?? undefined}
              windows={windows}
              reason={sub.reason ?? undefined}
              badge={presentation.badge}
              pinnedCount={windows.filter((w) => w.pinned).length}
              headlinePinned={
                sub.headlineWindowId !== null && sub.pinnedWindowIds.includes(sub.headlineWindowId)
              }
              expanded={expanded.has(sub.id)}
              menuOpen={openMenuId === sub.id}
              actionLabel={presentation.actionLabel}
              footerNote={presentation.footerNote}
              onAction={() => (sub.needsSignIn ? startSignIn(sub.id) : refreshAccountById(sub.id))}
              onReadNow={() => refreshAccountById(sub.id)}
              onTogglePin={() => togglePin(sub.id, sub.headlineWindowId)}
              onToggleWindowPin={(windowId: string) => togglePin(sub.id, windowId)}
              onToggleExpand={() => toggleExpand(sub.id)}
              onToggleMenu={() => setOpenMenuId((prev) => (prev === sub.id ? null : sub.id))}
              onRename={(next: string | null) => renameSubscription(sub.id, next)}
              canMoveUp={index > 0}
              canMoveDown={index < subscriptions.length - 1}
              onMoveUp={() => moveSubscription(sub.id, 'up')}
              onMoveDown={() => moveSubscription(sub.id, 'down')}
              onStopTracking={() => stopTracking(sub.id)}
              signInInProgress={sub.signInInProgress}
              onSubmitSignInCode={(code: string) => submitSignInCode(sub.id, code)}
              onCancelSignIn={() => cancelSignIn(sub.id)}
            />
          )
        })
      )}
    </Panel>
  )
}
