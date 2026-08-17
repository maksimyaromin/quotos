import type * as React from 'react'
import type { SubscriptionState } from '../indicators/status-dot'
import styles from './menu-bar-tile.module.css'

export function QuotaGlyph({ size = 15 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 16 16" fill="none" className={styles.glyph}>
      <circle cx="8" cy="8" r="5.4" stroke="currentColor" strokeWidth="1.4" opacity="0.28" />
      <path
        d="M4.46 12.02a5.4 5.4 0 1 1 7.08 0"
        stroke="currentColor"
        strokeWidth="1.9"
        strokeLinecap="round"
      />
    </svg>
  )
}

function AttentionMark() {
  return (
    <svg
      width="11"
      height="11"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2.2"
      strokeLinecap="round"
      strokeLinejoin="round"
    >
      <path d="M10.3 3.6 1.8 18a1.9 1.9 0 0 0 1.7 2.9h17a1.9 1.9 0 0 0 1.7-2.9L13.7 3.6a1.9 1.9 0 0 0-3.4 0Z" />
      <path d="M12 9v4M12 17h.01" />
    </svg>
  )
}

export interface PinnedFigure {
  used?: number | null
  state?: SubscriptionState
}

export interface MenuBarTileProps {
  pins?: PinnedFigure[]
  onClick?: () => void
  showStrip?: boolean
  style?: React.CSSProperties
}

function tintLevel(pin: PinnedFigure): 'amber' | 'red' | null {
  if (pin.state === 'broken' || pin.state === 'behind') return 'amber'
  if (typeof pin.used === 'number' && pin.used >= 90) return 'red'
  if (typeof pin.used === 'number' && pin.used >= 75) return 'amber'
  return null
}

export function MenuBarTile({ pins = [], onClick, showStrip = true, style }: MenuBarTileProps) {
  const tile = (
    <button type="button" onClick={onClick} className={styles.tile}>
      <QuotaGlyph />
      {pins.map((pin, i) => (
        // biome-ignore lint/suspicious/noArrayIndexKey: PinnedFigure has no stable id, and each pin's span carries no internal state, so index-keyed reuse is safe.
        <span key={i} className={styles.pin} data-attention={tintLevel(pin) ?? undefined}>
          {pin.state === 'broken' ? (
            <AttentionMark />
          ) : (
            <>
              {typeof pin.used === 'number' ? `${pin.used}%` : '—'}
              {pin.state === 'behind' ? (
                <span className={styles.behindMark}>
                  <AttentionMark />
                </span>
              ) : null}
            </>
          )}
        </span>
      ))}
    </button>
  )

  if (!showStrip) return <span style={style}>{tile}</span>

  return (
    <div className={styles.stripWrapper} style={style}>
      {tile}
      <span className={styles.neighborBattery}>
        100%
        <svg width="22" height="12" viewBox="0 0 26 13" fill="none">
          <rect x="0.5" y="0.5" width="22" height="12" rx="3" stroke="currentColor" opacity="0.7" />
          <rect x="2" y="2" width="19" height="9" rx="1.5" fill="currentColor" />
          <rect x="23.5" y="4" width="2" height="5" rx="1" fill="currentColor" opacity="0.7" />
        </svg>
      </span>
      <span className={styles.neighborClock}>Mon 9:41</span>
    </div>
  )
}
