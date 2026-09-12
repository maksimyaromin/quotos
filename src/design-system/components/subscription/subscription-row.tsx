import * as React from 'react'
import { MenuItem, MenuSeparator, RowMenu, useRowMenu } from '../controls/row-menu'
import { Chevron, MenuDotsGlyph, PinGlyph } from '../glyphs'
import { Badge } from '../indicators/badge'
import { CapacityBar } from '../indicators/capacity-bar'
import type { SubscriptionState } from '../indicators/status-dot'
import { StatusDot } from '../indicators/status-dot'
import { LimitWindow, type LimitWindowProps } from './limit-window'
import styles from './subscription-row.module.css'

export interface SubscriptionRowProps {
  label: string
  provider?: string
  account?: string
  state?: SubscriptionState
  used?: number | null
  severity?: 'healthy' | 'warn' | 'critical'
  resetLabel?: string | null
  lastRead?: string | null
  windows?: LimitWindowProps[]
  reason?: string | null
  badge?: 'Not current' | 'Needs sign-in' | null
  pinnedCount?: number
  headlinePinned?: boolean
  expanded?: boolean
  menuOpen?: boolean
  actionLabel?: string | null
  actionDisabled?: boolean
  footerNote?: string | null
  onAction?: () => void
  onTogglePin?: () => void
  onToggleWindowPin?: (id: string) => void
  onToggleExpand?: () => void
  onToggleMenu?: () => void
  onRename?: (nextLabel: string | null) => void
  onReadNow?: () => void
  canMoveUp?: boolean
  canMoveDown?: boolean
  onMoveUp?: () => void
  onMoveDown?: () => void
  onStopTracking?: () => void
  signInInProgress?: boolean
  onSubmitSignInCode?: (code: string) => void
  onCancelSignIn?: () => void
  style?: React.CSSProperties
}

export function SubscriptionRow({
  label,
  provider,
  account,
  state = 'working',
  used = null,
  severity = 'healthy',
  resetLabel = null,
  lastRead = null,
  windows = [],
  reason = null,
  badge = null,
  pinnedCount = 0,
  headlinePinned = false,
  expanded = false,
  menuOpen = false,
  actionLabel = null,
  actionDisabled = false,
  footerNote = null,
  onAction,
  onTogglePin,
  onToggleWindowPin,
  onToggleExpand,
  onToggleMenu,
  onRename,
  onReadNow,
  canMoveUp = false,
  canMoveDown = false,
  onMoveUp,
  onMoveDown,
  onStopTracking,
  signInInProgress = false,
  onSubmitSignInCode,
  onCancelSignIn,
  style,
}: SubscriptionRowProps) {
  const [renaming, setRenaming] = React.useState(false)
  const [draft, setDraft] = React.useState(label)
  const inputRef = React.useRef<HTMLInputElement>(null)
  const [codeDraft, setCodeDraft] = React.useState('')
  const codeInputRef = React.useRef<HTMLInputElement>(null)
  const menu = useRowMenu(menuOpen)

  React.useEffect(() => {
    if (signInInProgress) {
      setCodeDraft('')
      requestAnimationFrame(() => codeInputRef.current?.focus())
    }
  }, [signInInProgress])

  const submitCode = () => {
    const trimmed = codeDraft.trim()
    if (trimmed.length === 0) return
    onSubmitSignInCode?.(trimmed)
    setCodeDraft('')
  }

  React.useEffect(() => {
    if (renaming) {
      setDraft(label)
      requestAnimationFrame(() => inputRef.current?.select())
    }
  }, [renaming, label])

  const commitRename = () => {
    setRenaming(false)
    const trimmed = draft.trim()
    if (trimmed === label) return
    onRename?.(trimmed.length > 0 ? trimmed : null)
  }

  const stale = state === 'behind'
  const reading = state === 'reading' || state === 'connecting'
  const hasData = typeof used === 'number'
  const active = expanded || menuOpen
  const hasWindows = windows.length > 0

  const usedLevel = stale ? 'stale' : severity !== 'healthy' ? severity : undefined

  const handleRowClick = () => {
    if (menuOpen) {
      onToggleMenu?.()
      return
    }
    if (renaming) return
    onToggleExpand?.()
  }

  return (
    <div
      onClick={handleRowClick}
      onKeyDown={menu.onKeyDown}
      data-active={active ? 'true' : undefined}
      className={styles.row}
      style={style}
    >
      <div className={styles.header}>
        <StatusDot state={state} className={styles.statusDot} />
        <div className={styles.titleArea}>
          {renaming ? (
            <input
              ref={inputRef}
              value={draft}
              onClick={(e) => e.stopPropagation()}
              onChange={(e) => setDraft(e.target.value)}
              onBlur={commitRename}
              onKeyDown={(e) => {
                if (e.key === 'Enter') commitRename()
                if (e.key === 'Escape') {
                  e.stopPropagation()
                  setDraft(label)
                  setRenaming(false)
                }
              }}
              className={styles.renameInput}
            />
          ) : (
            <div className={styles.title}>{label}</div>
          )}
          {provider || account ? (
            <div className={styles.subtitle}>{[account, provider].filter(Boolean).join(' · ')}</div>
          ) : null}
        </div>
        {pinnedCount > 0 ? (
          <Badge tone="accent" className={styles.pinnedBadge}>
            <PinGlyph size={10} />
            {pinnedCount}
          </Badge>
        ) : null}
        {badge ? (
          <Badge tone={stale ? 'warn' : 'danger'} className={styles.stateBadge}>
            {badge}
          </Badge>
        ) : null}
        <button
          type="button"
          title="More"
          aria-label="More"
          aria-haspopup="menu"
          aria-expanded={menuOpen}
          data-quotos-menu-scope="true"
          data-open={menuOpen ? 'true' : undefined}
          ref={menu.triggerRef}
          onClick={(e) => {
            e.stopPropagation()
            onToggleMenu?.()
          }}
          className={styles.menuButton}
        >
          <MenuDotsGlyph />
        </button>
      </div>

      {signInInProgress ? (
        <div className={styles.signInBlock}>
          <div className={styles.signInText}>
            Finish signing in in the browser, then paste the code here.
          </div>
          <div className={styles.codeRow} onClick={(e) => e.stopPropagation()}>
            <input
              ref={codeInputRef}
              value={codeDraft}
              onChange={(e) => setCodeDraft(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === 'Enter') submitCode()
                if (e.key === 'Escape') {
                  e.stopPropagation()
                  setCodeDraft('')
                  onCancelSignIn?.()
                }
              }}
              placeholder="Paste code"
              className={styles.codeInput}
            />
            <button
              type="button"
              onClick={submitCode}
              disabled={codeDraft.trim().length === 0}
              className={styles.submitButton}
            >
              Submit
            </button>
            <button
              type="button"
              onClick={() => onCancelSignIn?.()}
              className={styles.cancelButton}
            >
              Cancel
            </button>
          </div>
        </div>
      ) : hasData ? (
        <>
          <div className={styles.dataRow}>
            <div className={styles.usedGroup}>
              <span className={styles.usedNumber} data-level={usedLevel}>
                {used}
                <span className={styles.percentSign}>%</span>
              </span>
              <span className={styles.usedLabel}>used</span>
            </div>
            {resetLabel ? <span className={styles.resetLabel}>{resetLabel}</span> : null}
          </div>
          <CapacityBar used={used} reading={reading} stale={stale} severity={severity} />
        </>
      ) : (
        <div className={styles.reasonText}>{reason || 'No limits reported yet.'}</div>
      )}

      <div className={styles.footer}>
        <span className={styles.footerNote} data-stale={stale ? 'true' : undefined}>
          {reading
            ? 'Reading…'
            : footerNote
              ? footerNote
              : lastRead
                ? `Read ${lastRead}`
                : 'Not read yet'}
        </span>
        {actionLabel ? (
          <button
            type="button"
            title={actionLabel}
            disabled={actionDisabled}
            onClick={(e) => {
              e.stopPropagation()
              onAction?.()
            }}
            className={styles.actionButton}
          >
            {actionLabel}
          </button>
        ) : null}
        {hasWindows ? (
          <button
            type="button"
            aria-expanded={expanded}
            onClick={(e) => {
              e.stopPropagation()
              onToggleExpand?.()
            }}
            className={styles.disclosureButton}
          >
            {windows.length} {windows.length === 1 ? 'limit' : 'limits'}
            <Chevron open={expanded} className={styles.chevron} />
          </button>
        ) : null}
      </div>

      <div
        aria-hidden={!expanded}
        data-expanded={expanded && hasWindows ? 'true' : undefined}
        className={styles.detailWrapper}
      >
        <div className={styles.detailInner}>
          {hasWindows ? (
            <div className={styles.windowsList}>
              {windows.map((w, i) => (
                <LimitWindow key={w.id ?? i} {...w} stale={stale} onTogglePin={onToggleWindowPin} />
              ))}
            </div>
          ) : null}
        </div>
      </div>

      {menuOpen ? (
        <RowMenu anchor={menu} label="Subscription actions">
          <MenuItem
            onClick={() => {
              onToggleMenu?.()
              onReadNow?.()
            }}
          >
            Read now
          </MenuItem>
          <MenuItem
            onClick={() => {
              onToggleMenu?.()
              setRenaming(true)
            }}
          >
            Rename
          </MenuItem>
          <MenuItem
            onClick={() => {
              onToggleMenu?.()
              onTogglePin?.()
            }}
          >
            {headlinePinned ? 'Hide from menu bar' : 'Show in menu bar'}
          </MenuItem>
          <MenuItem
            disabled={!canMoveUp}
            onClick={() => {
              onToggleMenu?.()
              onMoveUp?.()
            }}
          >
            Move up
          </MenuItem>
          <MenuItem
            disabled={!canMoveDown}
            onClick={() => {
              onToggleMenu?.()
              onMoveDown?.()
            }}
          >
            Move down
          </MenuItem>
          <MenuSeparator />
          <MenuItem
            danger
            onClick={() => {
              onToggleMenu?.()
              onStopTracking?.()
            }}
          >
            Stop tracking
          </MenuItem>
        </RowMenu>
      ) : null}
    </div>
  )
}
