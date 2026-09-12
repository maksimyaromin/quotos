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
import { AnimatePresence, motion, useReducedMotion } from 'framer-motion'
import { Fragment, useEffect, useRef, useState } from 'react'
import { QuotaGlyph } from '@/design-system'
import {
  asDragItem,
  asDropZone,
  type CustomizeGroup,
  type CustomizePin,
  type DragItem,
  type DropAction,
  groupDragId,
  type LandingSlot,
  landingSlot,
  pinDragId,
  resolveDrop,
  sameDrop,
  UNGROUPED_DROP_ID,
} from '@/lib/customize-drag'
import type { StatusItemSegment } from '@/types/entities'
import { dropSettle, grabTransition, pickedUp, reflowTransition } from './drag-motion'
import { DragHandleGlyph, TrashIcon, UngroupIcon } from './icons'
import styles from './customize-display-screen.module.css'

export type { CustomizeGroup, CustomizePin }

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

// A group's slot sits before one of its members, or, standing for no
// member at all, after the last of them.
const END_OF_GROUP = Symbol('end of group')

// The pointer decides, and only the pointer: whatever is under it is
// what a drop lands on, innermost first, so a member row is never
// shadowed by the group box around it. A drag that is over nothing is
// over nothing, rather than falling back to whichever box it happens to
// overlap, which is how a drop target stops being something anyone can
// aim at.
const collisionDetection: CollisionDetection = pointerWithin

interface RowProps {
  pin: CustomizePin
  groupId: string | null
  dropHint: string | null
  carried: boolean
  reduced: boolean
}

// The row closes up behind a drag that has taken it: what is under the
// pointer is the row itself, so leaving a copy of it in the list would
// be showing it twice.
function PinRow({ pin, groupId, dropHint, carried, reduced }: RowProps) {
  const { attributes, listeners, setNodeRef, setActivatorNodeRef } = useSortable({
    id: pinDragId(pin.key),
    data: { kind: 'pin', key: pin.key, groupId } satisfies DragItem,
  })

  return (
    <motion.div
      ref={setNodeRef}
      layout
      initial={false}
      animate={{ height: carried ? 0 : 'auto', opacity: carried ? 0 : 1 }}
      transition={reflowTransition(reduced)}
      data-carried={carried ? 'true' : undefined}
      className={styles.rowShell}
    >
      <div
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
    </motion.div>
  )
}

// The space a release would put the carried row in, opened where it
// would land and closed again the moment that changes. It draws the row
// it is holding the place for, so the list already reads as it will.
function LandingRow({
  pin,
  member,
  reduced,
}: {
  pin: CustomizePin
  member: boolean
  reduced: boolean
}) {
  return (
    <motion.div
      aria-hidden="true"
      layout
      initial={{ height: 0, opacity: 0 }}
      animate={{ height: 'auto', opacity: 1 }}
      exit={{ height: 0, opacity: 0 }}
      transition={reflowTransition(reduced)}
      className={styles.rowShell}
    >
      <div className={member ? styles.memberRow : styles.standaloneRow} data-slot="true">
        <span className={styles.handleSpacer} />
        <span className={styles.pinLabel}>{pin.label}</span>
        <span className={styles.pinValue}>{figureLabel(pin.used)}</span>
      </div>
    </motion.div>
  )
}

// What is under the pointer while a drag is on: the row it came from
// stays in the list, holding the place it is about to land in, so this
// stands in for it and carries the same three things it carries.
function DraggedRow({
  label,
  trailing,
  color,
  reduced,
}: {
  label: string
  trailing: string
  color?: CustomizeGroup['color']
  reduced: boolean
}) {
  return (
    <motion.div
      // A copy of a row that is still in the list: announcing it again
      // would say everything twice, and the library narrates the drag
      // itself through its own live region.
      aria-hidden="true"
      initial={{ scale: 1, rotate: 0 }}
      animate={pickedUp(reduced)}
      transition={grabTransition(reduced)}
      className={styles.overlayRow}
    >
      <DragHandleGlyph />
      {color === undefined ? null : <span className={styles.colorDot} data-color={color} />}
      <span className={styles.pinLabel}>{label}</span>
      <span className={styles.pinValue}>{trailing}</span>
    </motion.div>
  )
}

interface GroupBoxProps {
  group: CustomizeGroup
  joining: boolean
  carried: boolean
  slot: LandingSlot
  carriedPin: CustomizePin | undefined
  renaming: boolean
  draft: string
  reduced: boolean
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
  carried,
  slot,
  carriedPin,
  renaming,
  draft,
  reduced,
  nameInputRef,
  onDraftChange,
  onStartRename,
  onCommitRename,
  onCancelRename,
  onUngroup,
  onDelete,
}: GroupBoxProps) {
  const { attributes, listeners, setNodeRef, setActivatorNodeRef } = useSortable({
    id: groupDragId(group.id),
    data: { kind: 'group', groupId: group.id } satisfies DragItem,
  })
  const slotInside =
    slot.kind === 'inGroup' && slot.groupId === group.id ? (slot.beforeKey ?? END_OF_GROUP) : null

  return (
    <motion.div
      ref={setNodeRef}
      layout
      transition={reflowTransition(reduced)}
      role="group"
      aria-label={group.name}
      data-carried={carried ? 'true' : undefined}
      data-joining={joining ? 'true' : undefined}
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
        {group.members.length === 0 && slotInside === null ? (
          <div className={styles.emptyGroup}>Drag a pin in here.</div>
        ) : null}
        <AnimatePresence initial={false}>
          {group.members.flatMap((member) => [
            slotInside === member.key && carriedPin !== undefined ? (
              <LandingRow key="slot" pin={carriedPin} member reduced={reduced} />
            ) : null,
            <PinRow
              key={member.key}
              pin={member}
              groupId={group.id}
              dropHint={null}
              carried={carriedPin?.key === member.key}
              reduced={reduced}
            />,
          ])}
          {slotInside === END_OF_GROUP && carriedPin !== undefined ? (
            <LandingRow key="slot" pin={carriedPin} member reduced={reduced} />
          ) : null}
        </AnimatePresence>
      </SortableContext>
    </motion.div>
  )
}

// Offered only while a grouped member is in the air, and it arrives at
// its full height at once: the drag library measures a target the
// moment it appears, and a strip still growing would be measured as the
// sliver it was. What animates is what nothing is measured from.
function LeaveGroupZone({ active, reduced }: { active: boolean; reduced: boolean }) {
  const { setNodeRef } = useDroppable({ id: UNGROUPED_DROP_ID })
  return (
    <motion.div
      ref={setNodeRef}
      initial={{ opacity: 0, y: 6, scale: 0.99 }}
      animate={{ opacity: 1, y: 0, scale: 1 }}
      exit={{ opacity: 0, y: 6, scale: 0.99 }}
      transition={reflowTransition(reduced)}
      data-offered="true"
      data-drop={active ? 'true' : undefined}
      className={styles.ungroupedZone}
    >
      <span className={styles.ungroupedZoneLabel}>Drop here to leave the group</span>
    </motion.div>
  )
}

// The space a dragged group would drop into, above the box it was
// carried over.
function GroupSlot({ name, reduced }: { name: string; reduced: boolean }) {
  return (
    <motion.div
      aria-hidden="true"
      layout
      initial={{ height: 0, opacity: 0 }}
      animate={{ height: 'auto', opacity: 1 }}
      exit={{ height: 0, opacity: 0 }}
      transition={reflowTransition(reduced)}
      className={styles.rowShell}
    >
      <div className={styles.standaloneRow} data-slot="true">
        <span className={styles.handleSpacer} />
        <span className={styles.pinLabel}>{name}</span>
      </div>
    </motion.div>
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
  const reduced = useReducedMotion() ?? false

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

  // What a release would do decides everything the screen shows about
  // it: where the space for it opens, which box lights up, which row is
  // marked. The release then carries out that same answer, so the two
  // can never disagree.
  const slot = landingSlot(action)
  const joiningGroupId = action.kind === 'joinGroup' ? action.groupId : null
  const pairingWithKey = action.kind === 'groupTogether' ? action.keys[0] : null

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
        onDragOver={(event) => {
          const next = actionFor(event)
          setAction((current) => (sameDrop(current, next) ? current : next))
        }}
        onDragEnd={handleDragEnd}
        onDragCancel={cancelDrag}
      >
        <SortableContext items={outerItems} strategy={verticalListSortingStrategy}>
          <AnimatePresence initial={false}>
            {groups.flatMap((group) => [
              slot.kind === 'beforeGroup' &&
              slot.groupId === group.id &&
              draggedGroup !== undefined ? (
                <GroupSlot key="group-slot" name={draggedGroup.name} reduced={reduced} />
              ) : null,
              <GroupBox
                key={group.id}
                group={group}
                joining={joiningGroupId === group.id}
                carried={draggedGroup?.id === group.id}
                slot={slot}
                carriedPin={draggedPin}
                renaming={renamingId === group.id}
                draft={draft}
                reduced={reduced}
                nameInputRef={nameInputRef}
                onDraftChange={setDraft}
                onStartRename={() => setRenamingId(group.id)}
                onCommitRename={commitRename}
                onCancelRename={() => setRenamingId(null)}
                onUngroup={() => onUngroup(group.id)}
                onDelete={() => onDeleteGroup(group.id)}
              />,
            ])}

            {standalone.map((pin) => (
              <PinRow
                key={pin.key}
                pin={pin}
                groupId={null}
                dropHint={pairingWithKey === pin.key ? 'Drop to group these two' : null}
                carried={draggedPin?.key === pin.key}
                reduced={reduced}
              />
            ))}

            {slot.kind === 'loose' && draggedPin !== undefined ? (
              <LandingRow key="loose-slot" pin={draggedPin} member={false} reduced={reduced} />
            ) : null}
          </AnimatePresence>
        </SortableContext>

        <AnimatePresence>
          {dragging?.kind === 'pin' && dragging.groupId !== null ? (
            <LeaveGroupZone active={action.kind === 'leaveGroup'} reduced={reduced} />
          ) : null}
        </AnimatePresence>

        <DragOverlay dropAnimation={dropSettle(reduced)}>
          {draggedPin !== undefined ? (
            <DraggedRow
              label={draggedPin.label}
              trailing={figureLabel(draggedPin.used)}
              reduced={reduced}
            />
          ) : draggedGroup !== undefined ? (
            <DraggedRow
              label={draggedGroup.name}
              trailing={`${draggedGroup.members.length}`}
              color={draggedGroup.color}
              reduced={reduced}
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
