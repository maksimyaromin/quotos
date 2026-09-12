import type * as React from 'react'
import type { SubscriptionState } from '../indicators/status-dot'
import styles from './menu-bar-tile.module.css'

// The brand mark as the menu bar draws it, in the design system's own
// preview of that bar: two spend chevrons at full colour, and the limit
// chevron twice — faint at full extent, saturated over the part `used`
// has reached. `status_item_render.rs` is the drawing that actually
// ships; this one stands for it inside the design system.
export function QuotaGlyph({ size = 15, used = 0 }: { size?: number; used?: number }) {
  const filled = Math.max(0, Math.min(1, used / 100))
  return (
    <svg
      width={(size * MARK_ASPECT).toFixed(2)}
      height={size}
      viewBox="7.7 2.7 12.6 24.1"
      fill="none"
      strokeWidth="2.6"
      strokeLinecap="round"
      strokeLinejoin="round"
      className={styles.glyph}
    >
      <path d={LIMIT_CHEVRON} className={styles.limitTrack} />
      <path
        d={LIMIT_CHEVRON}
        className={styles.limitFill}
        pathLength={1}
        // Each arm is half the path, and both fill away from the vertex
        // the two meet at, so the visible run is centred on it.
        strokeDasharray={`${filled} ${1 - filled}`}
        strokeDashoffset={(filled / 2 - 0.5).toFixed(4)}
      />
      <path d="M9 18 L14 13 L19 18" className={styles.spend} />
      <path d="M9 25.5 L14 20.5 L19 25.5" className={styles.spend} />
    </svg>
  )
}

const LIMIT_CHEVRON = 'M9 4 L14 9 L19 4'
// The mark's own ink box on its 28-unit grid, round caps included; the
// viewBox above is that box, so the drawn width follows from it.
const MARK_ASPECT = 12.6 / 24.1

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

// The gauge's own default source: the arithmetic mean of every figure
// that reports one.
function meanUsed(pins: PinnedFigure[]): number {
  const figures = pins
    .map((pin) => pin.used)
    .filter((used): used is number => typeof used === 'number')
  if (figures.length === 0) return 0
  return Math.round(figures.reduce((sum, used) => sum + used, 0) / figures.length)
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
      <QuotaGlyph used={meanUsed(pins)} />
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
