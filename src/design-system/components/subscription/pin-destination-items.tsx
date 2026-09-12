import * as React from 'react'
import { MenuItem, MenuSeparator } from '../controls/row-menu'
import styles from './pin-destination-items.module.css'

export interface PinDestinationGroup {
  id: string
  name: string
}

export interface PinDestinationItemsProps {
  groups?: PinDestinationGroup[]
  currentGroupId?: string | null
  onPin?: (groupId: string | null) => void
  onCreateGroup?: (name: string) => void
}

function CheckGlyph() {
  return (
    <svg
      width="13"
      height="13"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2.4"
      strokeLinecap="round"
      strokeLinejoin="round"
      className={styles.check}
    >
      <path d="M5 12l5 5L20 7" />
    </svg>
  )
}

// The menu items shared by both places a window can be pinned from: the
// row's own menu and a limit window's pin button.
export function PinDestinationItems({
  groups = [],
  currentGroupId = null,
  onPin,
  onCreateGroup,
}: PinDestinationItemsProps) {
  const [naming, setNaming] = React.useState(false)
  const [draft, setDraft] = React.useState('')
  const inputRef = React.useRef<HTMLInputElement>(null)
  // A click on Create blurs the field first; without this the name
  // would be committed twice, once per path.
  const settled = React.useRef(false)

  React.useEffect(() => {
    if (!naming) return
    settled.current = false
    requestAnimationFrame(() => inputRef.current?.focus())
  }, [naming])

  const commit = () => {
    if (settled.current) return
    settled.current = true
    const trimmed = draft.trim()
    setNaming(false)
    setDraft('')
    if (trimmed.length > 0) onCreateGroup?.(trimmed)
  }

  const cancel = () => {
    settled.current = true
    setNaming(false)
    setDraft('')
  }

  return (
    <>
      <MenuItem onClick={() => onPin?.(null)}>Pin standalone</MenuItem>
      {groups.length > 0 ? <MenuSeparator /> : null}
      {groups.map((group) => (
        <MenuItem key={group.id} onClick={() => onPin?.(group.id)}>
          <span className={styles.destinationName}>Add to {group.name}</span>
          {currentGroupId === group.id ? <CheckGlyph /> : null}
        </MenuItem>
      ))}
      <MenuSeparator />
      {naming ? (
        <div className={styles.nameRow}>
          <input
            ref={inputRef}
            value={draft}
            placeholder="Group name"
            onChange={(e) => setDraft(e.target.value)}
            onBlur={commit}
            onKeyDown={(e) => {
              if (e.key === 'Enter') commit()
              if (e.key === 'Escape') {
                e.stopPropagation()
                cancel()
              }
            }}
            className={styles.nameInput}
          />
          <button
            type="button"
            onMouseDown={(e) => e.preventDefault()}
            onClick={commit}
            className={styles.createButton}
          >
            Create
          </button>
        </div>
      ) : (
        <MenuItem onClick={() => setNaming(true)}>New group…</MenuItem>
      )}
    </>
  )
}
