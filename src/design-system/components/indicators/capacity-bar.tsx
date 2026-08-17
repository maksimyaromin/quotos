import type * as React from 'react'
import styles from './capacity-bar.module.css'

type CapacityLevel = 'healthy' | 'warn' | 'critical'

function capacityLevel(used: number): CapacityLevel {
  if (used >= 90) return 'critical'
  if (used >= 75) return 'warn'
  return 'healthy'
}

function levelColor(level: CapacityLevel): string {
  if (level === 'critical') return 'var(--cap-critical)'
  if (level === 'warn') return 'var(--cap-warn)'
  return 'var(--cap-healthy)'
}

export function capacityColor(used: number): string {
  return levelColor(capacityLevel(used))
}

export interface CapacityBarProps {
  used?: number
  reading?: boolean
  stale?: boolean
  severity?: CapacityLevel | null
  height?: string
  style?: React.CSSProperties
}

export function CapacityBar({
  used = 0,
  reading = false,
  stale = false,
  severity = null,
  height,
  style,
}: CapacityBarProps) {
  const fill = Math.max(0, Math.min(100, used))
  const level = severity ?? capacityLevel(used)
  return (
    <div className={styles.track} style={{ height, ...style }}>
      <div
        className={styles.fill}
        data-color={level}
        data-stale={stale ? 'true' : undefined}
        style={{ width: `${fill}%` }}
      />
      {reading ? <div data-quotos-shimmer="" className={styles.shimmer} /> : null}
    </div>
  )
}
