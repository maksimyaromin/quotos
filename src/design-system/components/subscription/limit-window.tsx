import type * as React from 'react'
import { MarkGlyph, PinGlyph } from '../glyphs'
import { Badge } from '../indicators/badge'
import { CapacityBar } from '../indicators/capacity-bar'
import styles from './limit-window.module.css'

function numberLevel(used: number): 'critical' | 'warn' | 'neutral' {
  if (used >= 90) return 'critical'
  if (used >= 75) return 'warn'
  return 'neutral'
}

export interface LimitWindowProps {
  id?: string
  name: string
  used?: number | null
  resetLabel?: string | null
  scope?: string | null
  stale?: boolean
  pinned?: boolean
  onTogglePin?: (id: string) => void
  // Whether this window is the one the menu bar mark's gauge reads
  // from. Exactly one window anywhere carries it.
  iconSource?: boolean
  onToggleIconSource?: (id: string) => void
  style?: React.CSSProperties
}

export function LimitWindow({
  id,
  name,
  used = null,
  resetLabel = null,
  scope = null,
  stale = false,
  pinned = false,
  onTogglePin,
  iconSource = false,
  onToggleIconSource,
  style,
}: LimitWindowProps) {
  const hasPct = typeof used === 'number'
  const pinLabel = pinned ? 'Remove from menu bar' : 'Show in menu bar'
  const iconSourceLabel = iconSource
    ? 'Stop filling the menu bar icon from this'
    : 'Fill the menu bar icon from this'

  return (
    <div className={styles.row} style={style}>
      <div className={styles.header}>
        <button
          type="button"
          title={pinLabel}
          aria-label={pinLabel}
          aria-pressed={pinned}
          onClick={(e) => {
            e.stopPropagation()
            if (id === undefined) return
            onTogglePin?.(id)
          }}
          data-pinned={pinned ? 'true' : undefined}
          className={styles.pinButton}
        >
          <PinGlyph />
        </button>
        <span className={styles.name}>{name}</span>
        {scope ? (
          <Badge tone="neutral" className={styles.scopeBadge}>
            <span className={styles.scopeInner}>{scope}</span>
          </Badge>
        ) : null}
        <button
          type="button"
          title={iconSourceLabel}
          aria-label={iconSourceLabel}
          aria-pressed={iconSource}
          onClick={(e) => {
            e.stopPropagation()
            if (id === undefined) return
            onToggleIconSource?.(id)
          }}
          data-selected={iconSource ? 'true' : undefined}
          className={styles.iconSourceButton}
        >
          <MarkGlyph />
        </button>
        <span className={styles.value} data-level={hasPct ? numberLevel(used) : undefined}>
          {hasPct ? `${used}%` : '—'}
        </span>
      </div>
      {hasPct ? <CapacityBar used={used} stale={stale} height="3px" /> : null}
      {resetLabel ? <span className={styles.resetLabel}>{resetLabel}</span> : null}
    </div>
  )
}
