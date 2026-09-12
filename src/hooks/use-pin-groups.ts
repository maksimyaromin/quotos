import { useCallback, useEffect, useRef, useState } from 'react'
import { loadPinGroups, savePinGroups } from '@/lib/persistence'
import { nextGroupColor, sortPinGroups } from '@/lib/pin-groups'
import { onStatusItemGroupClicked } from '@/lib/tauri-client'
import type { PinGroup } from '@/types/entities'

function newGroupId(): string {
  return `group-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`
}

// A group is named the moment it exists, since a nameless box in the
// customize screen would have nothing to drag onto; the name is meant
// to be replaced in place.
function untakenName(groups: PinGroup[]): string {
  const taken = new Set(groups.map((group) => group.name))
  for (let n = groups.length + 1; ; n += 1) {
    const candidate = `Group ${n}`
    if (!taken.has(candidate)) return candidate
  }
}

function withoutMember(groups: PinGroup[], key: string): PinGroup[] {
  return groups.map((group) =>
    group.memberKeys.includes(key)
      ? { ...group, memberKeys: group.memberKeys.filter((member) => member !== key) }
      : group,
  )
}

// Group order is the list's own order, rewritten whole after a move, so
// no two groups can end up sharing a rank.
function reordered(groups: PinGroup[]): PinGroup[] {
  return groups.map((group, index) => (group.order === index ? group : { ...group, order: index }))
}

export function usePinGroups() {
  const [groups, setGroups] = useState<PinGroup[]>([])
  const hasLoadedRef = useRef(false)
  const lastSavedRef = useRef<string | null>(null)

  useEffect(() => {
    let cancelled = false
    void (async () => {
      const loaded = await loadPinGroups()
      if (cancelled) return
      setGroups(loaded)
      lastSavedRef.current = JSON.stringify(loaded)
      hasLoadedRef.current = true
    })()
    return () => {
      cancelled = true
    }
  }, [])

  useEffect(() => {
    if (!hasLoadedRef.current) return
    const serialized = JSON.stringify(groups)
    if (lastSavedRef.current === serialized) return
    lastSavedRef.current = serialized
    void (async () => {
      try {
        await savePinGroups(groups)
      } catch (error) {
        console.error('Quotos: saving the pin groups failed; will retry on the next change', error)
        if (lastSavedRef.current === serialized) lastSavedRef.current = null
      }
    })()
  }, [groups])

  // A window belongs to at most one group, so filing it anywhere first
  // clears whatever it was in. `beforeKey` is the member it was dropped
  // onto; without one it joins at the end.
  const addMemberToGroup = useCallback((key: string, groupId: string, beforeKey?: string) => {
    setGroups((prev) =>
      withoutMember(prev, key).map((group) => {
        if (group.id !== groupId) return group
        const at = beforeKey === undefined ? -1 : group.memberKeys.indexOf(beforeKey)
        const memberKeys = [...group.memberKeys]
        memberKeys.splice(at === -1 ? memberKeys.length : at, 0, key)
        return { ...group, memberKeys }
      }),
    )
  }, [])

  // Grouping and ungrouping never touch a pin: a window taken out of a
  // group stays pinned, standing on its own in the menu bar.
  const removeMember = useCallback((key: string) => {
    setGroups((prev) => withoutMember(prev, key))
  }, [])

  const groupMembersTogether = useCallback((keys: string[]) => {
    const id = newGroupId()
    setGroups((prev) => {
      const cleared = keys.reduce(withoutMember, prev)
      return [
        ...cleared,
        {
          id,
          name: untakenName(cleared),
          // Grouping is what buys menu bar width back, so a new group
          // starts rolled up to its one figure.
          collapsed: true,
          order: cleared.length,
          color: nextGroupColor(cleared),
          memberKeys: keys,
        },
      ]
    })
    return id
  }, [])

  const createEmptyGroup = useCallback(() => groupMembersTogether([]), [groupMembersTogether])

  const renameGroup = useCallback((id: string, name: string) => {
    setGroups((prev) => prev.map((group) => (group.id === id ? { ...group, name } : group)))
  }, [])

  const toggleGroupCollapsed = useCallback((id: string) => {
    setGroups((prev) =>
      prev.map((group) => (group.id === id ? { ...group, collapsed: !group.collapsed } : group)),
    )
  }, [])

  const moveGroupBefore = useCallback((id: string, beforeId: string) => {
    setGroups((prev) => {
      if (id === beforeId) return prev
      const ordered = sortPinGroups(prev)
      const from = ordered.findIndex((group) => group.id === id)
      const to = ordered.findIndex((group) => group.id === beforeId)
      if (from === -1 || to === -1) return prev
      const [moved] = ordered.splice(from, 1)
      ordered.splice(to, 0, moved)
      return reordered(ordered)
    })
  }, [])

  // A click on a group's own figure in the menu bar opens it out to
  // every member, or rolls it back up.
  useEffect(() => {
    let cancelled = false
    let unlisten: (() => void) | undefined
    void onStatusItemGroupClicked((groupId) => toggleGroupCollapsed(groupId)).then((fn) => {
      if (cancelled) fn()
      else unlisten = fn
    })
    return () => {
      cancelled = true
      unlisten?.()
    }
  }, [toggleGroupCollapsed])

  const ungroupAll = useCallback((id: string) => {
    setGroups((prev) =>
      prev.map((group) => (group.id === id ? { ...group, memberKeys: [] } : group)),
    )
  }, [])

  const deleteGroup = useCallback((id: string) => {
    setGroups((prev) => reordered(sortPinGroups(prev).filter((group) => group.id !== id)))
  }, [])

  return {
    groups,
    addMemberToGroup,
    removeMember,
    groupMembersTogether,
    createEmptyGroup,
    renameGroup,
    toggleGroupCollapsed,
    moveGroupBefore,
    ungroupAll,
    deleteGroup,
  }
}
