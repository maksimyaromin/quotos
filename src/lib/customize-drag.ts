// What the customize screen's drag gesture means, kept apart from the
// drag library that reports it: the library says which element the
// pointer is over, this says what dropping there would do. The screen
// asks the same question mid-drag, to show what a drop will land on,
// and again on release, to carry it out.

import type { GroupColor } from '@/types/entities'

// The arrangement a drop rearranges: one row per pinned limit window,
// boxed under a group or standing on its own.
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

export type DragItem =
  | { kind: 'pin'; key: string; groupId: string | null }
  | { kind: 'group'; groupId: string }

export type DropZone =
  | { kind: 'pin'; key: string; groupId: string | null }
  | { kind: 'group'; groupId: string }
  | { kind: 'ungrouped' }

export type DropAction =
  | { kind: 'nothing' }
  // `beforeKey` is the member the pin was dropped onto; without one it
  // joins at the end.
  | { kind: 'joinGroup'; key: string; groupId: string; beforeKey: string | null }
  | { kind: 'groupTogether'; keys: [string, string] }
  | { kind: 'leaveGroup'; key: string }
  | { kind: 'moveGroup'; groupId: string; beforeGroupId: string }

const PIN_PREFIX = 'pin:'
const GROUP_PREFIX = 'group:'

export const UNGROUPED_DROP_ID = 'ungrouped'

export function pinDragId(key: string): string {
  return `${PIN_PREFIX}${key}`
}

export function groupDragId(groupId: string): string {
  return `${GROUP_PREFIX}${groupId}`
}

// The drag library hands back whatever was attached to the element it
// is reporting, typed as unknown; nothing guarantees it is one of ours.
export function asDragItem(data: unknown): DragItem | null {
  const item = data as Partial<DragItem> | undefined
  if (item?.kind === 'pin' && typeof item.key === 'string') {
    return { kind: 'pin', key: item.key, groupId: item.groupId ?? null }
  }
  if (item?.kind === 'group' && typeof item.groupId === 'string') {
    return { kind: 'group', groupId: item.groupId }
  }
  return null
}

export function asDropZone(id: string | number | undefined, data: unknown): DropZone | null {
  if (id === UNGROUPED_DROP_ID) return { kind: 'ungrouped' }
  const item = asDragItem(data)
  if (item === null) return null
  return item.kind === 'pin'
    ? { kind: 'pin', key: item.key, groupId: item.groupId }
    : { kind: 'group', groupId: item.groupId }
}

function dropPin(active: Extract<DragItem, { kind: 'pin' }>, over: DropZone): DropAction {
  switch (over.kind) {
    case 'ungrouped':
      return active.groupId === null ? { kind: 'nothing' } : { kind: 'leaveGroup', key: active.key }
    case 'group':
      return { kind: 'joinGroup', key: active.key, groupId: over.groupId, beforeKey: null }
    case 'pin':
      if (over.key === active.key) return { kind: 'nothing' }
      // A pin dropped on a grouped one joins that group ahead of it;
      // dropped on a loose one, the two become a group of their own,
      // the one that was standing there first.
      return over.groupId === null
        ? { kind: 'groupTogether', keys: [over.key, active.key] }
        : { kind: 'joinGroup', key: active.key, groupId: over.groupId, beforeKey: over.key }
  }
}

// A group is dragged to reorder it, so it only ever lands on another
// group: on its box, or on any of its members.
function dropGroup(active: Extract<DragItem, { kind: 'group' }>, over: DropZone): DropAction {
  const target =
    over.kind === 'ungrouped' ? null : over.kind === 'group' ? over.groupId : over.groupId
  if (target === null || target === active.groupId) return { kind: 'nothing' }
  return { kind: 'moveGroup', groupId: active.groupId, beforeGroupId: target }
}

export function resolveDrop(active: DragItem | null, over: DropZone | null): DropAction {
  if (active === null || over === null) return { kind: 'nothing' }
  return active.kind === 'pin' ? dropPin(active, over) : dropGroup(active, over)
}

// Where the slot a release would fill opens up: inside a group, ahead
// of one of its members or at its end; among the loose rows; or above a
// group's whole box. Pairing two loose pins opens none, since the group
// it would make does not exist yet.
export type LandingSlot =
  | { kind: 'inGroup'; groupId: string; beforeKey: string | null }
  | { kind: 'loose' }
  | { kind: 'beforeGroup'; groupId: string }
  | { kind: 'none' }

export function landingSlot(action: DropAction): LandingSlot {
  switch (action.kind) {
    case 'joinGroup':
      return { kind: 'inGroup', groupId: action.groupId, beforeKey: action.beforeKey }
    case 'leaveGroup':
      return { kind: 'loose' }
    case 'moveGroup':
      return { kind: 'beforeGroup', groupId: action.beforeGroupId }
    default:
      return { kind: 'none' }
  }
}

// Two answers to the same question are the same answer: the screen
// redraws only when what a release would do actually changes, which is
// also what keeps a redraw from being read back as a new answer.
export function sameDrop(a: DropAction, b: DropAction): boolean {
  if (a.kind !== b.kind) return false
  switch (a.kind) {
    case 'joinGroup': {
      const other = b as Extract<DropAction, { kind: 'joinGroup' }>
      return a.key === other.key && a.groupId === other.groupId && a.beforeKey === other.beforeKey
    }
    case 'groupTogether': {
      const other = b as Extract<DropAction, { kind: 'groupTogether' }>
      return a.keys[0] === other.keys[0] && a.keys[1] === other.keys[1]
    }
    case 'leaveGroup':
      return a.key === (b as Extract<DropAction, { kind: 'leaveGroup' }>).key
    case 'moveGroup': {
      const other = b as Extract<DropAction, { kind: 'moveGroup' }>
      return a.groupId === other.groupId && a.beforeGroupId === other.beforeGroupId
    }
    default:
      return true
  }
}
