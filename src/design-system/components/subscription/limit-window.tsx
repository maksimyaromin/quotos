import type * as React from 'react'
import { PinGlyph } from '../glyphs'
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
  style,
}: LimitWindowProps) {
  const hasPct = typeof used === 'number'
  const pinLabel = pinned ? 'Remove from menu bar' : 'Show in menu bar'

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
        <span className={styles.value} data-level={hasPct ? numberLevel(used) : undefined}>
          {hasPct ? `${used}%` : '—'}
        </span>
      </div>
      {hasPct ? <CapacityBar used={used} stale={stale} height="3px" /> : null}
      {resetLabel ? <span className={styles.resetLabel}>{resetLabel}</span> : null}
    </div>
  )
}
