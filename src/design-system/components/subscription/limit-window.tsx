import type * as React from 'react'
import { RowMenu, useRowMenu } from '../controls/row-menu'
import { PinGlyph } from '../glyphs'
import { Badge } from '../indicators/badge'
import { CapacityBar } from '../indicators/capacity-bar'
import styles from './limit-window.module.css'
import { PinDestinationItems, type PinDestinationGroup } from './pin-destination-items'

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
  // Pin groups: with destinations on offer, pinning opens a menu instead
  // of pinning standalone outright. Unpinning stays one click.
  pinGroups?: PinDestinationGroup[]
  groupId?: string | null
  pinMenuOpen?: boolean
  onTogglePinMenu?: (id: string) => void
  onPin?: (id: string, groupId: string | null) => void
  onCreateGroupWithWindow?: (id: string, name: string) => void
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
  pinGroups,
  groupId = null,
  pinMenuOpen = false,
  onTogglePinMenu,
  onPin,
  onCreateGroupWithWindow,
  style,
}: LimitWindowProps) {
  const hasPct = typeof used === 'number'
  const offersDestinations = !pinned && onTogglePinMenu !== undefined
  const menu = useRowMenu(pinMenuOpen)
  const pinLabel = pinned ? 'Remove from menu bar' : 'Show in menu bar'

  return (
    <div className={styles.row} style={style}>
      <div className={styles.header}>
        <button
          type="button"
          title={pinLabel}
          aria-label={pinLabel}
          aria-pressed={pinned}
          aria-haspopup={offersDestinations ? 'menu' : undefined}
          aria-expanded={offersDestinations ? pinMenuOpen : undefined}
          data-quotos-menu-scope={offersDestinations ? 'true' : undefined}
          ref={menu.triggerRef}
          onClick={(e) => {
            e.stopPropagation()
            if (id === undefined) return
            if (offersDestinations) onTogglePinMenu?.(id)
            else onTogglePin?.(id)
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
      {pinMenuOpen && id !== undefined ? (
        <div onKeyDown={menu.onKeyDown}>
          <RowMenu anchor={menu} label="Where to pin this limit">
            <PinDestinationItems
              groups={pinGroups}
              currentGroupId={groupId}
              onPin={(destination) => {
                onTogglePinMenu?.(id)
                onPin?.(id, destination)
              }}
              onCreateGroup={(groupName) => {
                onTogglePinMenu?.(id)
                onCreateGroupWithWindow?.(id, groupName)
              }}
            />
          </RowMenu>
        </div>
      ) : null}
    </div>
  )
}
