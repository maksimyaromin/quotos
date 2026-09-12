import { Fragment, useEffect, useRef, useState } from 'react'
import { QuotaGlyph } from '@/design-system'
import type { GroupColor, StatusItemSegment } from '@/types/entities'
import { DragHandleGlyph, TrashIcon, UngroupIcon } from './icons'
import styles from './customize-display-screen.module.css'

export interface CustomizePin {
  key: string
  label: string
  used: number | null
}

export interface CustomizeGroup {
  id: string
  name: string
  color: GroupColor
  members: CustomizePin[]
}

export interface CustomizeDisplayScreenProps {
  // The real segment list the menu bar is drawn from, so the preview
  // cannot drift from what the status item actually shows.
  preview: StatusItemSegment[]
  groups: CustomizeGroup[]
  standalone: CustomizePin[]
  onAddToGroup: (key: string, groupId: string, beforeKey?: string) => void
  onMakeStandalone: (key: string) => void
  onGroupTogether: (keys: string[]) => void
  onMoveGroup: (id: string, beforeId: string) => void
  onRenameGroup: (id: string, name: string) => void
  onUngroup: (id: string) => void
  onDeleteGroup: (id: string) => void
  onNewGroup: () => string
}

type DropTarget =
  | { kind: 'group'; id: string }
  | { kind: 'member'; key: string; groupId: string }
  | { kind: 'standalone'; key: string }
  | { kind: 'ungrouped' }

interface Drag {
  pin?: string
  group?: string
}

function figureLabel(used: number | null): string {
  return typeof used === 'number' ? `${used}%` : '—'
}

export function CustomizeDisplayScreen({
  preview,
  groups,
  standalone,
  onAddToGroup,
  onMakeStandalone,
  onGroupTogether,
  onMoveGroup,
  onRenameGroup,
  onUngroup,
  onDeleteGroup,
  onNewGroup,
}: CustomizeDisplayScreenProps) {
  const [drag, setDrag] = useState<Drag | null>(null)
  const [target, setTarget] = useState<DropTarget | null>(null)
  const [renamingId, setRenamingId] = useState<string | null>(null)
  const [draft, setDraft] = useState('')
  const nameInputRef = useRef<HTMLInputElement>(null)
  const seededDraftFor = useRef<string | null>(null)

  // A group opened for renaming seeds the field from its own name,
  // including one created a moment ago that has yet to be rendered.
  useEffect(() => {
    if (renamingId === null) {
      seededDraftFor.current = null
      return
    }
    if (seededDraftFor.current === renamingId) return
    const group = groups.find((g) => g.id === renamingId)
    if (group === undefined) return
    seededDraftFor.current = renamingId
    setDraft(group.name)
    requestAnimationFrame(() => nameInputRef.current?.select())
  }, [renamingId, groups])

  // A drag is carried on plain mouse events rather than the HTML5 drag
  // API: the panel is a non-activating window, and pointer dragging is
  // what already works there for moving the panel itself. The drop
  // target comes from the rows' own `mousemove`, not from measured
  // geometry, so nothing here depends on a layout engine.
  useEffect(() => {
    if (drag === null) return
    const finish = () => {
      if (target !== null) {
        if (drag.group !== undefined && target.kind === 'group' && target.id !== drag.group) {
          onMoveGroup(drag.group, target.id)
        } else if (drag.pin !== undefined) {
          const pin = drag.pin
          if (target.kind === 'group') onAddToGroup(pin, target.id)
          else if (target.kind === 'member' && target.key !== pin)
            onAddToGroup(pin, target.groupId, target.key)
          else if (target.kind === 'standalone' && target.key !== pin)
            onGroupTogether([target.key, pin])
          else if (target.kind === 'ungrouped') onMakeStandalone(pin)
        }
      }
      setDrag(null)
      setTarget(null)
    }
    window.addEventListener('mouseup', finish)
    return () => window.removeEventListener('mouseup', finish)
  }, [drag, target, onAddToGroup, onGroupTogether, onMakeStandalone, onMoveGroup])

  const commitRename = () => {
    const id = renamingId
    setRenamingId(null)
    if (id === null) return
    const trimmed = draft.trim()
    if (trimmed.length > 0) onRenameGroup(id, trimmed)
  }

  const startDrag = (next: Drag) => (event: React.MouseEvent) => {
    if (event.button !== 0) return
    // The name reads as a button but is still part of the grab surface:
    // a press that never moves ends as a no-op drag and goes on to open
    // the rename. Only the header's own actions are excluded.
    if ((event.target as HTMLElement).closest('[data-no-drag], input')) return
    // The press cannot blur an open rename field, since preventing the
    // default is what stops the drag selecting text, so commit it here
    // the way a blur would.
    commitRename()
    event.preventDefault()
    setDrag(next)
    setTarget(null)
  }

  const hover = (next: DropTarget) => (event: React.MouseEvent) => {
    event.stopPropagation()
    if (drag === null) return
    setTarget(next)
  }

  const isTarget = (match: (t: DropTarget) => boolean) => target !== null && match(target)

  const pinRow = (pin: CustomizePin, inGroup: string | null) => (
    <div
      key={pin.key}
      onMouseDown={startDrag({ pin: pin.key })}
      onMouseMove={hover(
        inGroup === null
          ? { kind: 'standalone', key: pin.key }
          : { kind: 'member', key: pin.key, groupId: inGroup },
      )}
      data-dragging={drag?.pin === pin.key ? 'true' : undefined}
      data-drop={
        isTarget((t) => (t.kind === 'member' || t.kind === 'standalone') && t.key === pin.key) &&
        drag?.pin !== pin.key
          ? 'true'
          : undefined
      }
      className={inGroup === null ? styles.standaloneRow : styles.memberRow}
    >
      <DragHandleGlyph />
      <span className={styles.pinLabel}>{pin.label}</span>
      <span className={styles.pinValue}>{figureLabel(pin.used)}</span>
    </div>
  )

  const anyPinned = groups.some((g) => g.members.length > 0) || standalone.length > 0

  return (
    <div className={styles.screen}>
      <div className={styles.previewLabel}>In the menu bar</div>
      <div className={styles.previewStrip} role="img" aria-label="Menu bar preview">
        <QuotaGlyph size={14} />
        {preview.map((segment, index) => (
          // biome-ignore lint/suspicious/noArrayIndexKey: a segment is a position in the strip rather than an entity, and carries no state of its own.
          <Fragment key={`${segment.groupId ?? 'pin'}-${segment.text}-${index}`}>
            {segment.groupStart ? <span className={styles.previewDivider} /> : null}
            <span className={styles.previewFigure} data-color={segment.color}>
              {segment.text}
              <span
                className={styles.previewUnderline}
                data-color={segment.groupColor ?? undefined}
              />
            </span>
          </Fragment>
        ))}
      </div>

      <p className={styles.description}>
        Drag a pin onto another to group them, or onto a group to join it. Grouping never unpins
        anything.
      </p>

      {groups.map((group) => (
        <div
          key={group.id}
          onMouseMove={hover({ kind: 'group', id: group.id })}
          data-dragging={drag?.group === group.id ? 'true' : undefined}
          data-drop={
            isTarget((t) => t.kind === 'group' && t.id === group.id) && drag?.group !== group.id
              ? 'true'
              : undefined
          }
          className={styles.groupBox}
        >
          <div className={styles.groupHeader} onMouseDown={startDrag({ group: group.id })}>
            <DragHandleGlyph />
            <span className={styles.colorDot} data-color={group.color} />
            {renamingId === group.id ? (
              <input
                ref={nameInputRef}
                value={draft}
                aria-label="Group name"
                onChange={(e) => setDraft(e.target.value)}
                onBlur={commitRename}
                onKeyDown={(e) => {
                  if (e.key === 'Enter') commitRename()
                  if (e.key === 'Escape') {
                    e.stopPropagation()
                    setRenamingId(null)
                  }
                }}
                className={styles.nameInput}
              />
            ) : (
              <button
                type="button"
                onClick={() => setRenamingId(group.id)}
                className={styles.groupName}
              >
                {group.name}
              </button>
            )}
            <span className={styles.memberCount}>{group.members.length}</span>
            <button
              type="button"
              title={`Ungroup ${group.name}`}
              aria-label={`Ungroup ${group.name}`}
              disabled={group.members.length === 0}
              data-no-drag="true"
              onClick={() => onUngroup(group.id)}
              className={styles.headerButton}
            >
              <UngroupIcon />
            </button>
            <button
              type="button"
              title={`Delete ${group.name}`}
              aria-label={`Delete ${group.name}`}
              data-no-drag="true"
              onClick={() => onDeleteGroup(group.id)}
              className={styles.headerButton}
            >
              <TrashIcon />
            </button>
          </div>
          {group.members.length === 0 ? (
            <div className={styles.emptyGroup}>Drag a pin in here.</div>
          ) : (
            group.members.map((member) => pinRow(member, group.id))
          )}
        </div>
      ))}

      {standalone.map((pin) => pinRow(pin, null))}

      {drag?.pin !== undefined && groups.some((g) => g.members.some((m) => m.key === drag.pin)) ? (
        <div
          onMouseMove={hover({ kind: 'ungrouped' })}
          data-drop={isTarget((t) => t.kind === 'ungrouped') ? 'true' : undefined}
          className={styles.ungroupedZone}
        >
          Drop here to leave the group
        </div>
      ) : null}

      {anyPinned ? null : (
        <div className={styles.emptyState}>
          Nothing is pinned yet. Pin a limit from the list to arrange it here.
        </div>
      )}

      <button
        type="button"
        onClick={() => setRenamingId(onNewGroup())}
        className={styles.newGroupButton}
      >
        + New empty group
      </button>
    </div>
  )
}
