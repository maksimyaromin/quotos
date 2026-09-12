import * as React from 'react'
import styles from './row-menu.module.css'

const MENU_GAP = 4
const MENU_VIEWPORT_MARGIN = 8
export const MENU_MIN_WIDTH = 168

interface MenuPosition {
  top: number
  left: number
}

export interface RowMenuAnchor {
  open: boolean
  triggerRef: React.RefObject<HTMLButtonElement | null>
  menuRef: React.RefObject<HTMLDivElement | null>
  position: MenuPosition | null
  onKeyDown: (event: React.KeyboardEvent) => void
}

function nextMenuIndex(key: string, current: number, length: number): number {
  if (key === 'Home') return 0
  if (key === 'End') return length - 1
  if (key === 'ArrowDown') return current < 0 ? 0 : (current + 1) % length
  return current < 0 ? length - 1 : (current - 1 + length) % length
}

// Owns everything a row's overflow menu needs to sit against the
// viewport rather than inside the panel's scroll box: where to place it,
// arrow-key traversal, and handing focus back when it closes.
export function useRowMenu(open: boolean): RowMenuAnchor {
  const triggerRef = React.useRef<HTMLButtonElement>(null)
  const menuRef = React.useRef<HTMLDivElement>(null)
  const [position, setPosition] = React.useState<MenuPosition | null>(null)

  React.useLayoutEffect(() => {
    if (!open) {
      setPosition(null)
      return undefined
    }
    const place = () => {
      const trigger = triggerRef.current
      const menu = menuRef.current
      if (!trigger || !menu) return
      const anchor = trigger.getBoundingClientRect()
      const width = menu.offsetWidth || MENU_MIN_WIDTH
      const height = menu.offsetHeight
      const below = anchor.bottom + MENU_GAP
      const top =
        below + height <= window.innerHeight - MENU_VIEWPORT_MARGIN
          ? below
          : Math.max(MENU_VIEWPORT_MARGIN, anchor.top - MENU_GAP - height)
      const left = Math.min(
        Math.max(MENU_VIEWPORT_MARGIN, anchor.right - width),
        Math.max(MENU_VIEWPORT_MARGIN, window.innerWidth - width - MENU_VIEWPORT_MARGIN),
      )
      setPosition((prev) => (prev && prev.top === top && prev.left === left ? prev : { top, left }))
    }
    place()
    window.addEventListener('scroll', place, true)
    window.addEventListener('resize', place)
    return () => {
      window.removeEventListener('scroll', place, true)
      window.removeEventListener('resize', place)
    }
  }, [open])

  const wasOpen = React.useRef(false)
  React.useEffect(() => {
    if (wasOpen.current && !open && document.activeElement === document.body) {
      triggerRef.current?.focus()
    }
    wasOpen.current = open
  }, [open])

  const onKeyDown = (event: React.KeyboardEvent) => {
    if (!open || !menuRef.current) return
    if ((event.target as HTMLElement).tagName === 'INPUT') return
    const { key } = event
    if (key !== 'ArrowDown' && key !== 'ArrowUp' && key !== 'Home' && key !== 'End') return
    const items = Array.from(menuRef.current.querySelectorAll('button')).filter((b) => !b.disabled)
    if (items.length === 0) return
    event.preventDefault()
    const current = items.indexOf(document.activeElement as HTMLButtonElement)
    items[nextMenuIndex(key, current, items.length)].focus()
  }

  return { open, triggerRef, menuRef, position, onKeyDown }
}

export function MenuItem({
  danger,
  disabled,
  onClick,
  children,
}: {
  danger?: boolean
  disabled?: boolean
  onClick?: () => void
  children?: React.ReactNode
}) {
  return (
    <button
      type="button"
      role="menuitem"
      disabled={disabled}
      onClick={onClick}
      data-danger={danger ? 'true' : undefined}
      className={styles.menuItem}
    >
      {children}
    </button>
  )
}

export function MenuSeparator() {
  return <div role="separator" className={styles.separator} />
}

export function RowMenu({
  anchor,
  label,
  children,
}: {
  anchor: RowMenuAnchor
  label: string
  children: React.ReactNode
}) {
  const { menuRef, position } = anchor
  return (
    <div
      ref={menuRef}
      role="menu"
      aria-label={label}
      data-quotos-menu-scope="true"
      onClick={(e) => e.stopPropagation()}
      className={styles.rowMenu}
      style={{
        top: position ? position.top : 0,
        left: position ? position.left : 0,
        visibility: position ? 'visible' : 'hidden',
        minWidth: MENU_MIN_WIDTH,
      }}
    >
      {children}
    </div>
  )
}
