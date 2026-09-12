import type * as React from 'react'
import { useId, useLayoutEffect, useRef, useState } from 'react'
import { joinClassNames } from '../../join-class-names'
import styles from './panel.module.css'

const PANEL_WIDTH = 332
export const PANEL_RADIUS = 12

// Synced by hand across files: BEAK_BASE_HALF doubled must equal geometry.rs's
// BEAK_BASE_WIDTH, and NOTCH_RESERVE must equal app.css's panel padding-top.
export const BEAK_BASE_HALF = 10
export const BEAK_HEIGHT = 10
const BEAK_TIP_ROUND = 3
export const NOTCH_RESERVE = 12

export function buildPanelOutlinePath(width: number, height: number, beakLeft: number | null) {
  const rectTop = NOTCH_RESERVE
  const rectBottom = NOTCH_RESERVE + height

  let leftRadius = PANEL_RADIUS
  let rightRadius = PANEL_RADIUS
  if (beakLeft != null) {
    leftRadius = Math.max(0, Math.min(PANEL_RADIUS, beakLeft))
    rightRadius = Math.max(0, Math.min(PANEL_RADIUS, width - (beakLeft + BEAK_BASE_HALF * 2)))
  }

  const segments = [`M ${leftRadius} ${rectTop}`]

  if (beakLeft != null) {
    const baseLeftX = beakLeft
    const baseRightX = beakLeft + BEAK_BASE_HALF * 2
    const apexX = beakLeft + BEAK_BASE_HALF
    const apexY = rectTop - BEAK_HEIGHT

    const leftLen = Math.hypot(baseLeftX - apexX, rectTop - apexY)
    const p1x = apexX + ((baseLeftX - apexX) / leftLen) * BEAK_TIP_ROUND
    const p1y = apexY + ((rectTop - apexY) / leftLen) * BEAK_TIP_ROUND
    const rightLen = Math.hypot(baseRightX - apexX, rectTop - apexY)
    const p2x = apexX + ((baseRightX - apexX) / rightLen) * BEAK_TIP_ROUND
    const p2y = apexY + ((rectTop - apexY) / rightLen) * BEAK_TIP_ROUND

    segments.push(
      `L ${baseLeftX} ${rectTop}`,
      `L ${p1x} ${p1y}`,
      `Q ${apexX} ${apexY} ${p2x} ${p2y}`,
      `L ${baseRightX} ${rectTop}`,
    )
  }

  const r = PANEL_RADIUS
  segments.push(
    `L ${width - rightRadius} ${rectTop}`,
    `A ${rightRadius} ${rightRadius} 0 0 1 ${width} ${rectTop + rightRadius}`,
    `L ${width} ${rectBottom - r}`,
    `A ${r} ${r} 0 0 1 ${width - r} ${rectBottom}`,
    `L ${r} ${rectBottom}`,
    `A ${r} ${r} 0 0 1 0 ${rectBottom - r}`,
    `L 0 ${rectTop + leftRadius}`,
    `A ${leftRadius} ${leftRadius} 0 0 1 ${leftRadius} ${rectTop}`,
    'Z',
  )
  return segments.join(' ')
}

export interface PanelProps {
  // A node, not a string: the panel's own header sets the product's
  // wordmark, whose three parts are coloured apart from each other.
  title?: React.ReactNode
  docked?: boolean
  beakLeft?: number
  dragging?: boolean
  onHeaderPointerDown?: (event: React.MouseEvent) => void
  leading?: React.ReactNode
  headerActions?: React.ReactNode
  footer?: React.ReactNode
  maxBodyHeight?: number
  position?: { x: number; y: number } | null
  children?: React.ReactNode
  style?: React.CSSProperties
}

export function Panel({
  title = 'Quotos',
  docked = true,
  beakLeft = 24,
  dragging = false,
  onHeaderPointerDown,
  leading = null,
  headerActions = null,
  footer = null,
  maxBodyHeight = 452,
  position = null,
  children,
  style,
}: PanelProps) {
  const detachedFixed: React.CSSProperties | null =
    !docked && position ? { position: 'fixed', left: position.x, top: position.y, margin: 0 } : null
  const clipId = useId()

  const contentRef = useRef<HTMLDivElement>(null)
  const [contentHeight, setContentHeight] = useState(0)
  useLayoutEffect(() => {
    const el = contentRef.current
    if (!el) return undefined
    setContentHeight(el.getBoundingClientRect().height)
    const observer = new ResizeObserver((entries) => {
      const next = entries[0]?.contentRect.height
      if (typeof next === 'number') setContentHeight(next)
    })
    observer.observe(el)
    return () => observer.disconnect()
  }, [])

  const showBeak = docked && !detachedFixed
  const pathD =
    contentHeight > 0
      ? buildPanelOutlinePath(PANEL_WIDTH, contentHeight, showBeak ? beakLeft : null)
      : null
  const beakBoxHeight = contentHeight + NOTCH_RESERVE

  return (
    <div data-quotos-panel="true" className={styles.root} style={{ ...detachedFixed, ...style }}>
      {pathD ? (
        <>
          <svg width="0" height="0" style={{ position: 'absolute' }}>
            <defs>
              <clipPath id={clipId}>
                <path d={pathD} />
              </clipPath>
            </defs>
          </svg>
          <div
            data-quotos-panel-fill="true"
            style={{
              position: 'absolute',
              top: -NOTCH_RESERVE,
              left: 0,
              width: PANEL_WIDTH,
              height: beakBoxHeight,
              background: 'var(--bg-panel)',
              boxShadow: 'var(--shadow-popover)',
              clipPath: `url(#${clipId})`,
              WebkitClipPath: `url(#${clipId})`,
            }}
          />
        </>
      ) : null}
      <div ref={contentRef} className={styles.content}>
        <div
          onMouseDown={onHeaderPointerDown}
          data-dragging={dragging ? 'true' : undefined}
          className={styles.header}
        >
          {leading}
          <span className={styles.title}>{title}</span>
          {headerActions}
        </div>
        <div
          className={joinClassNames('quotos-scroll', styles.body)}
          style={{ maxHeight: maxBodyHeight }}
        >
          {children}
        </div>
        {footer ? <div className={styles.footer}>{footer}</div> : null}
      </div>
      {pathD ? (
        <svg
          aria-hidden="true"
          width={PANEL_WIDTH}
          height={beakBoxHeight}
          viewBox={`0 0 ${PANEL_WIDTH} ${beakBoxHeight}`}
          style={{ top: -NOTCH_RESERVE }}
          className={styles.strokeLayer}
        >
          <path d={pathD} fill="none" className={styles.strokePath} strokeWidth={0.5} />
        </svg>
      ) : null}
    </div>
  )
}
