import {
  type CollisionDetection,
  DndContext,
  type DragEndEvent,
  DragOverlay,
  type DragOverEvent,
  type DragStartEvent,
  KeyboardSensor,
  PointerSensor,
  pointerWithin,
  rectIntersection,
  useDroppable,
  useSensor,
  useSensors,
} from '@dnd-kit/core'
import { restrictToVerticalAxis, restrictToWindowEdges } from '@dnd-kit/modifiers'
import {
  SortableContext,
  sortableKeyboardCoordinates,
  useSortable,
  verticalListSortingStrategy,
} from '@dnd-kit/sortable'
import { CSS } from '@dnd-kit/utilities'
import { Fragment, useEffect, useRef, useState } from 'react'
import { QuotaGlyph } from '@/design-system'
import {
  asDragItem,
  asDropZone,
  type DragItem,
  type DropAction,
  groupDragId,
  pinDragId,
  resolveDrop,
  UNGROUPED_DROP_ID,
} from '@/lib/customize-drag'
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

function figureLabel(used: number | null): string {
  return typeof used === 'number' ? `${used}%` : '—'
}

// The pointer decides: whatever is under it is what a drop lands on,
// and the innermost of those wins, so a member row is never shadowed by
// the group box around it. Only a drag that has left every target at
// once falls back to the boxes it overlaps.
const collisionDetection: CollisionDetection = (args) => {
  const underPointer = pointerWithin(args)
  return underPointer.length > 0 ? underPointer : rectIntersection(args)
}

interface RowProps {
  pin: CustomizePin
  groupId: string | null
  dropHint: string | null
  insertBefore: boolean
}

function PinRow({ pin, groupId, dropHint, insertBefore }: RowProps) {
  const {
    attributes,
    listeners,
    setNodeRef,
    setActivatorNodeRef,
    transform,
    transition,
    isDragging,
  } = useSortable({
    id: pinDragId(pin.key),
    data: { kind: 'pin', key: pin.key, groupId } satisfies DragItem,
  })

  return (
    <div
      ref={setNodeRef}
      style={{ transform: CSS.Translate.toString(transform), transition }}
      data-dragging={isDragging ? 'true' : undefined}
      data-insert-before={insertBefore ? 'true' : undefined}
      data-paired={dropHint === null ? undefined : 'true'}
      className={groupId === null ? styles.standaloneRow : styles.memberRow}
    >
      <button
        type="button"
        ref={setActivatorNodeRef}
        aria-label={`Reorder ${pin.label}`}
        className={styles.dragHandle}
        {...attributes}
        {...listeners}
      >
        <DragHandleGlyph />
      </button>
      <span className={styles.pinLabel}>{pin.label}</span>
      {dropHint === null ? null : <span className={styles.dropHint}>{dropHint}</span>}
      <span className={styles.pinValue}>{figureLabel(pin.used)}</span>
    </div>
  )
}

// What is under the pointer while a drag is on: the row itself would
// have to stay in the list to leave a gap behind, so this stands in for
// it, carrying the same three things the row it came from carries.
function DraggedRow({
  label,
  trailing,
  color,
}: {
  label: string
  trailing: string
  color?: GroupColor
}) {
  return (
    <div className={styles.overlayRow}>
      <DragHandleGlyph />
      {color === undefined ? null : <span className={styles.colorDot} data-color={color} />}
      <span className={styles.pinLabel}>{label}</span>
      <span className={styles.pinValue}>{trailing}</span>
    </div>
  )
}

interface GroupBoxProps {
  group: CustomizeGroup
  joining: boolean
  insertBefore: boolean
  insertMemberBefore: string | null
  renaming: boolean
  draft: string
  nameInputRef: React.RefObject<HTMLInputElement | null>
  onDraftChange: (value: string) => void
  onStartRename: () => void
  onCommitRename: () => void
  onCancelRename: () => void
  onUngroup: () => void
  onDelete: () => void
}

function GroupBox({
  group,
  joining,
  insertBefore,
  insertMemberBefore,
  renaming,
  draft,
  nameInputRef,
  onDraftChange,
  onStartRename,
  onCommitRename,
  onCancelRename,
  onUngroup,
  onDelete,
}: GroupBoxProps) {
  const {
    attributes,
    listeners,
    setNodeRef,
    setActivatorNodeRef,
    transform,
    transition,
    isDragging,
  } = useSortable({
    id: groupDragId(group.id),
    data: { kind: 'group', groupId: group.id } satisfies DragItem,
  })

  return (
    <div
      ref={setNodeRef}
      style={{ transform: CSS.Translate.toString(transform), transition }}
      data-dragging={isDragging ? 'true' : undefined}
      data-joining={joining ? 'true' : undefined}
      data-insert-before={insertBefore ? 'true' : undefined}
      className={styles.groupBox}
    >
      <div className={styles.groupHeader}>
        <button
          type="button"
          ref={setActivatorNodeRef}
          aria-label={`Reorder ${group.name}`}
          className={styles.dragHandle}
          {...attributes}
          {...listeners}
        >
          <DragHandleGlyph />
        </button>
        <span className={styles.colorDot} data-color={group.color} />
        {renaming ? (
          <input
            ref={nameInputRef}
            value={draft}
            aria-label="Group name"
            onChange={(event) => onDraftChange(event.target.value)}
            onBlur={onCommitRename}
            onKeyDown={(event) => {
              if (event.key === 'Enter') onCommitRename()
              if (event.key === 'Escape') {
                event.stopPropagation()
                onCancelRename()
              }
            }}
            className={styles.nameInput}
          />
        ) : (
          <button type="button" onClick={onStartRename} className={styles.groupName}>
            {group.name}
          </button>
        )}
        <span className={styles.memberCount}>{group.members.length}</span>
        <button
          type="button"
          title={`Ungroup ${group.name}`}
          aria-label={`Ungroup ${group.name}`}
          disabled={group.members.length === 0}
          onClick={onUngroup}
          className={styles.headerButton}
        >
          <UngroupIcon />
        </button>
        <button
          type="button"
          title={`Delete ${group.name}`}
          aria-label={`Delete ${group.name}`}
          onClick={onDelete}
          className={styles.headerButton}
        >
          <TrashIcon />
        </button>
      </div>
      <SortableContext
        items={group.members.map((member) => pinDragId(member.key))}
        strategy={verticalListSortingStrategy}
      >
        {group.members.length === 0 ? (
          <div className={styles.emptyGroup}>Drag a pin in here.</div>
        ) : (
          group.members.map((member) => (
            <PinRow
              key={member.key}
              pin={member}
              groupId={group.id}
              dropHint={null}
              insertBefore={insertMemberBefore === member.key}
            />
          ))
        )}
      </SortableContext>
      {joining && insertMemberBefore === null ? (
        <div className={styles.joinHint}>Drop to add to {group.name}</div>
      ) : null}
    </div>
  )
}

function LeaveGroupZone({ active }: { active: boolean }) {
  const { setNodeRef } = useDroppable({ id: UNGROUPED_DROP_ID })
  return (
    <div ref={setNodeRef} data-drop={active ? 'true' : undefined} className={styles.ungroupedZone}>
      Drop here to leave the group
    </div>
  )
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
  const [dragging, setDragging] = useState<DragItem | null>(null)
  const [action, setAction] = useState<DropAction>({ kind: 'nothing' })
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

  // A press has to travel before it counts as a drag, so clicking a name
  // opens the rename rather than nudging the row; the keyboard sensor is
  // what makes the same arrangement reachable without a pointer.
  const sensors = useSensors(
    useSensor(PointerSensor, { activationConstraint: { distance: 4 } }),
    useSensor(KeyboardSensor, { coordinateGetter: sortableKeyboardCoordinates }),
  )

  const commitRename = () => {
    const id = renamingId
    setRenamingId(null)
    if (id === null) return
    const trimmed = draft.trim()
    if (trimmed.length > 0) onRenameGroup(id, trimmed)
  }

  const actionFor = (event: DragOverEvent | DragEndEvent): DropAction =>
    resolveDrop(
      asDragItem(event.active.data.current),
      asDropZone(event.over?.id, event.over?.data.current),
    )

  const handleDragStart = (event: DragStartEvent) => {
    commitRename()
    setDragging(asDragItem(event.active.data.current))
    setAction({ kind: 'nothing' })
  }

  const handleDragEnd = (event: DragEndEvent) => {
    const dropped = actionFor(event)
    setDragging(null)
    setAction({ kind: 'nothing' })
    switch (dropped.kind) {
      case 'joinGroup':
        onAddToGroup(dropped.key, dropped.groupId, dropped.beforeKey ?? undefined)
        break
      case 'groupTogether':
        onGroupTogether(dropped.keys)
        break
      case 'leaveGroup':
        onMakeStandalone(dropped.key)
        break
      case 'moveGroup':
        onMoveGroup(dropped.groupId, dropped.beforeGroupId)
        break
      case 'nothing':
        break
    }
  }

  const cancelDrag = () => {
    setDragging(null)
    setAction({ kind: 'nothing' })
  }

  // What the drop would do is also what it looks like it will do: every
  // highlight below is read off the same answer the release acts on.
  const joiningGroupId = action.kind === 'joinGroup' ? action.groupId : null
  const insertMemberBefore = action.kind === 'joinGroup' ? action.beforeKey : null
  const pairingWithKey = action.kind === 'groupTogether' ? action.keys[0] : null
  const movingBeforeGroupId = action.kind === 'moveGroup' ? action.beforeGroupId : null

  const draggedPin =
    dragging?.kind === 'pin'
      ? [...groups.flatMap((group) => group.members), ...standalone].find(
          (pin) => pin.key === dragging.key,
        )
      : undefined
  const draggedGroup =
    dragging?.kind === 'group' ? groups.find((group) => group.id === dragging.groupId) : undefined

  const anyPinned = groups.some((g) => g.members.length > 0) || standalone.length > 0
  const outerItems = [
    ...groups.map((group) => groupDragId(group.id)),
    ...standalone.map((pin) => pinDragId(pin.key)),
  ]

  return (
    <div className={styles.screen}>
      <div className={styles.previewLabel}>In the menu bar</div>
      <div className={styles.previewStrip} role="img" aria-label="Menu bar preview">
        <QuotaGlyph size={14} />
        {preview.map((segment, index) => (
          // biome-ignore lint/suspicious/noArrayIndexKey: a segment is a position in the strip rather than an entity, and carries no state of its own.
          <Fragment key={`${segment.groupId ?? 'pin'}-${segment.text}-${index}`}>
            {segment.groupStart ? <span className={styles.previewDivider} /> : null}
            {segment.slug === null ? null : (
              <span className={styles.previewSlug}>{segment.slug}</span>
            )}
            <span className={styles.previewFigure} data-color={segment.color}>
              {segment.text}
            </span>
          </Fragment>
        ))}
      </div>

      <p className={styles.description}>
        Drag a pin onto another to group them, or onto a group to join it. Grouping never unpins
        anything.
      </p>

      <DndContext
        sensors={sensors}
        collisionDetection={collisionDetection}
        modifiers={[restrictToVerticalAxis, restrictToWindowEdges]}
        onDragStart={handleDragStart}
        onDragOver={(event) => setAction(actionFor(event))}
        onDragEnd={handleDragEnd}
        onDragCancel={cancelDrag}
      >
        <SortableContext items={outerItems} strategy={verticalListSortingStrategy}>
          {groups.map((group) => (
            <GroupBox
              key={group.id}
              group={group}
              joining={joiningGroupId === group.id}
              insertBefore={movingBeforeGroupId === group.id}
              insertMemberBefore={joiningGroupId === group.id ? insertMemberBefore : null}
              renaming={renamingId === group.id}
              draft={draft}
              nameInputRef={nameInputRef}
              onDraftChange={setDraft}
              onStartRename={() => setRenamingId(group.id)}
              onCommitRename={commitRename}
              onCancelRename={() => setRenamingId(null)}
              onUngroup={() => onUngroup(group.id)}
              onDelete={() => onDeleteGroup(group.id)}
            />
          ))}

          {standalone.map((pin) => (
            <PinRow
              key={pin.key}
              pin={pin}
              groupId={null}
              dropHint={pairingWithKey === pin.key ? 'Drop to group these two' : null}
              insertBefore={false}
            />
          ))}
        </SortableContext>

        {dragging?.kind === 'pin' && dragging.groupId !== null ? (
          <LeaveGroupZone active={action.kind === 'leaveGroup'} />
        ) : null}

        <DragOverlay>
          {draggedPin !== undefined ? (
            <DraggedRow label={draggedPin.label} trailing={figureLabel(draggedPin.used)} />
          ) : draggedGroup !== undefined ? (
            <DraggedRow
              label={draggedGroup.name}
              trailing={`${draggedGroup.members.length}`}
              color={draggedGroup.color}
            />
          ) : null}
        </DragOverlay>
      </DndContext>

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
