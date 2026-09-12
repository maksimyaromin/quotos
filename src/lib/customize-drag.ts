// What the customize screen's drag gesture means, kept apart from the
// drag library that reports it: the library says which element the
// pointer is over, this says what dropping there would do. The screen
// asks the same question mid-drag, to show what a drop will land on,
// and again on release, to carry it out.

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
