import { useCallback, useEffect, useRef, useState } from 'react'
import { loadPinGroups, savePinGroups } from '@/lib/persistence'
import { onStatusItemGroupClicked } from '@/lib/tauri-client'
import type { PinGroup } from '@/types/entities'

function newGroupId(): string {
  return `group-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`
}

function withoutMember(groups: PinGroup[], key: string): PinGroup[] {
  return groups.map((group) =>
    group.memberKeys.includes(key)
      ? { ...group, memberKeys: group.memberKeys.filter((member) => member !== key) }
      : group,
  )
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
  // clears whatever it was in.
  const assignMember = useCallback((key: string, groupId: string | null) => {
    setGroups((prev) => {
      const cleared = withoutMember(prev, key)
      if (groupId === null) return cleared
      return cleared.map((group) =>
        group.id === groupId ? { ...group, memberKeys: [...group.memberKeys, key] } : group,
      )
    })
  }, [])

  const removeMember = useCallback((key: string) => {
    setGroups((prev) => withoutMember(prev, key))
  }, [])

  const createGroupWith = useCallback((name: string, key: string | null) => {
    setGroups((prev) => {
      const cleared = key === null ? prev : withoutMember(prev, key)
      const order = cleared.reduce((highest, group) => Math.max(highest, group.order + 1), 0)
      return [
        ...cleared,
        {
          id: newGroupId(),
          name,
          // Grouping is what buys menu bar width back, so a new group
          // starts rolled up to its one figure.
          collapsed: true,
          order,
          memberKeys: key === null ? [] : [key],
        },
      ]
    })
  }, [])

  const renameGroup = useCallback((id: string, name: string) => {
    setGroups((prev) => prev.map((group) => (group.id === id ? { ...group, name } : group)))
  }, [])

  const toggleGroupCollapsed = useCallback((id: string) => {
    setGroups((prev) =>
      prev.map((group) => (group.id === id ? { ...group, collapsed: !group.collapsed } : group)),
    )
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

  // Both of these only dissolve the grouping; the windows themselves stay
  // pinned, standalone.
  const ungroupAll = useCallback((id: string) => {
    setGroups((prev) =>
      prev.map((group) => (group.id === id ? { ...group, memberKeys: [] } : group)),
    )
  }, [])

  const deleteGroup = useCallback((id: string) => {
    setGroups((prev) => prev.filter((group) => group.id !== id))
  }, [])

  return {
    groups,
    assignMember,
    removeMember,
    createGroupWith,
    renameGroup,
    toggleGroupCollapsed,
    ungroupAll,
    deleteGroup,
  }
}
