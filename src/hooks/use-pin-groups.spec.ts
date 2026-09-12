import { act, renderHook } from '@testing-library/react'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import type { PinGroup } from '@/types/entities'

const loadPinGroups = vi.fn<() => Promise<PinGroup[]>>(() => Promise.resolve([]))
const savePinGroups = vi.fn<(groups: PinGroup[]) => Promise<void>>(() => Promise.resolve())

vi.mock('@/lib/persistence', () => ({
  loadPinGroups: () => loadPinGroups(),
  savePinGroups: (groups: PinGroup[]) => savePinGroups(groups),
}))

let trayClick: ((groupId: string) => void) | null = null

vi.mock('@/lib/tauri-client', () => ({
  onStatusItemGroupClicked: (callback: (groupId: string) => void) => {
    trayClick = callback
    return Promise.resolve(() => {
      trayClick = null
    })
  },
}))

import { usePinGroups } from './use-pin-groups'

async function flush() {
  await act(async () => {
    await Promise.resolve()
  })
}

function stored(): PinGroup[] {
  const calls = savePinGroups.mock.calls
  return calls[calls.length - 1]?.[0] ?? []
}

async function mountWith(groups: PinGroup[]) {
  loadPinGroups.mockResolvedValue(groups)
  const hook = renderHook(() => usePinGroups())
  await flush()
  return hook
}

describe('usePinGroups', () => {
  beforeEach(() => {
    loadPinGroups.mockReset()
    loadPinGroups.mockResolvedValue([])
    savePinGroups.mockReset()
    savePinGroups.mockResolvedValue(undefined)
  })

  test('starts from what was persisted, and writes nothing until something changes', async () => {
    const saved: PinGroup[] = [
      { id: 'g1', name: 'Money', collapsed: true, order: 0, memberKeys: ['a::w'] },
    ]
    const { result } = await mountWith(saved)

    expect(result.current.groups).toEqual(saved)
    expect(savePinGroups).not.toHaveBeenCalled()
  })

  test('a collapse is persisted, so the layout survives a restart', async () => {
    const { result } = await mountWith([
      { id: 'g1', name: 'Money', collapsed: false, order: 0, memberKeys: [] },
    ])

    act(() => result.current.toggleGroupCollapsed('g1'))
    await flush()

    expect(result.current.groups[0].collapsed).toBe(true)
    expect(stored()[0].collapsed).toBe(true)
  })

  test('a click on a group in the menu bar opens just that one out', async () => {
    const { result } = await mountWith([
      { id: 'g1', name: 'A', collapsed: true, order: 0, memberKeys: [] },
      { id: 'g2', name: 'B', collapsed: true, order: 1, memberKeys: [] },
    ])

    act(() => trayClick?.('g2'))
    await flush()

    expect(result.current.groups.map((g) => g.collapsed)).toEqual([true, false])
  })

  test('clicking the same group again rolls it back up', async () => {
    const { result } = await mountWith([
      { id: 'g1', name: 'A', collapsed: true, order: 0, memberKeys: [] },
    ])

    act(() => trayClick?.('g1'))
    await flush()
    act(() => trayClick?.('g1'))
    await flush()

    expect(result.current.groups[0].collapsed).toBe(true)
    expect(stored()[0].collapsed).toBe(true)
  })

  test('a click naming a group that no longer exists changes nothing', async () => {
    const { result } = await mountWith([
      { id: 'g1', name: 'A', collapsed: true, order: 0, memberKeys: [] },
    ])

    act(() => trayClick?.('gone'))
    await flush()

    expect(result.current.groups[0].collapsed).toBe(true)
    expect(savePinGroups).not.toHaveBeenCalled()
  })

  test('a new group lands after the existing ones, rolled up, holding the pin that made it', async () => {
    const { result } = await mountWith([
      { id: 'g1', name: 'A', collapsed: false, order: 3, memberKeys: [] },
    ])

    act(() => result.current.createGroupWith('Money', 'a::weekly'))
    await flush()

    const created = result.current.groups[1]
    expect(created.name).toBe('Money')
    expect(created.order).toBe(4)
    expect(created.memberKeys).toEqual(['a::weekly'])
    expect(created.collapsed).toBe(true)
  })

  test('filing a pin into a group takes it out of whatever it was in', async () => {
    const { result } = await mountWith([
      { id: 'g1', name: 'A', collapsed: false, order: 0, memberKeys: ['a::w'] },
      { id: 'g2', name: 'B', collapsed: false, order: 1, memberKeys: [] },
    ])

    act(() => result.current.assignMember('a::w', 'g2'))
    await flush()

    expect(result.current.groups.map((g) => g.memberKeys)).toEqual([[], ['a::w']])
  })

  test('pinning standalone leaves it in no group at all', async () => {
    const { result } = await mountWith([
      { id: 'g1', name: 'A', collapsed: false, order: 0, memberKeys: ['a::w'] },
    ])

    act(() => result.current.assignMember('a::w', null))
    await flush()

    expect(result.current.groups[0].memberKeys).toEqual([])
  })

  test('unpinning removes just that member, leaving the rest of the group alone', async () => {
    const { result } = await mountWith([
      { id: 'g1', name: 'A', collapsed: false, order: 0, memberKeys: ['a::w', 'b::w'] },
    ])

    act(() => result.current.removeMember('a::w'))
    await flush()

    expect(result.current.groups[0].memberKeys).toEqual(['b::w'])
  })

  test('ungrouping empties the group without removing it', async () => {
    const { result } = await mountWith([
      { id: 'g1', name: 'A', collapsed: false, order: 0, memberKeys: ['a::w', 'b::w'] },
    ])

    act(() => result.current.ungroupAll('g1'))
    await flush()

    expect(result.current.groups).toHaveLength(1)
    expect(result.current.groups[0].memberKeys).toEqual([])
  })

  test('deleting drops the group and takes no other group with it', async () => {
    const { result } = await mountWith([
      { id: 'g1', name: 'A', collapsed: false, order: 0, memberKeys: ['a::w'] },
      { id: 'g2', name: 'B', collapsed: false, order: 1, memberKeys: ['b::w'] },
    ])

    act(() => result.current.deleteGroup('g1'))
    await flush()

    expect(result.current.groups.map((g) => g.id)).toEqual(['g2'])
    expect(result.current.groups[0].memberKeys).toEqual(['b::w'])
  })

  test('renaming keeps the members and the collapse state', async () => {
    const { result } = await mountWith([
      { id: 'g1', name: 'A', collapsed: true, order: 0, memberKeys: ['a::w'] },
    ])

    act(() => result.current.renameGroup('g1', 'Money'))
    await flush()

    expect(result.current.groups[0]).toEqual({
      id: 'g1',
      name: 'Money',
      collapsed: true,
      order: 0,
      memberKeys: ['a::w'],
    })
  })

  test('a failed save is retried on the next change rather than swallowed', async () => {
    vi.spyOn(console, 'error').mockImplementation(() => {})
    const { result } = await mountWith([
      { id: 'g1', name: 'A', collapsed: false, order: 0, memberKeys: [] },
    ])
    savePinGroups.mockRejectedValueOnce(new Error('disk full'))

    act(() => result.current.toggleGroupCollapsed('g1'))
    await flush()
    act(() => result.current.renameGroup('g1', 'B'))
    await flush()

    expect(savePinGroups).toHaveBeenCalledTimes(2)
    expect(stored()[0].name).toBe('B')
  })
})
