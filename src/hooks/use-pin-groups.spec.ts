import { act, renderHook } from '@testing-library/react'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import { memberOrder, resolveDrop } from '@/lib/customize-drag'
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
      { id: 'g1', name: 'Money', collapsed: true, order: 0, color: 'teal', memberKeys: ['a::w'] },
    ]
    const { result } = await mountWith(saved)

    expect(result.current.groups).toEqual(saved)
    expect(savePinGroups).not.toHaveBeenCalled()
  })

  test('a collapse is persisted, so the layout survives a restart', async () => {
    const { result } = await mountWith([
      { id: 'g1', name: 'Money', collapsed: false, order: 0, color: 'teal', memberKeys: [] },
    ])

    act(() => result.current.toggleGroupCollapsed('g1'))
    await flush()

    expect(result.current.groups[0].collapsed).toBe(true)
    expect(stored()[0].collapsed).toBe(true)
  })

  test('a click on a group in the menu bar opens just that one out', async () => {
    const { result } = await mountWith([
      { id: 'g1', name: 'A', collapsed: true, order: 0, color: 'teal', memberKeys: [] },
      { id: 'g2', name: 'B', collapsed: true, order: 1, color: 'teal', memberKeys: [] },
    ])

    act(() => trayClick?.('g2'))
    await flush()

    expect(result.current.groups.map((g) => g.collapsed)).toEqual([true, false])
  })

  test('clicking the same group again rolls it back up', async () => {
    const { result } = await mountWith([
      { id: 'g1', name: 'A', collapsed: true, order: 0, color: 'teal', memberKeys: [] },
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
      { id: 'g1', name: 'A', collapsed: true, order: 0, color: 'teal', memberKeys: [] },
    ])

    act(() => trayClick?.('gone'))
    await flush()

    expect(result.current.groups[0].collapsed).toBe(true)
    expect(savePinGroups).not.toHaveBeenCalled()
  })

  test('grouping two pins together makes one rolled-up group of them, last in order', async () => {
    const { result } = await mountWith([
      { id: 'g1', name: 'A', collapsed: false, order: 0, color: 'teal', memberKeys: [] },
    ])

    act(() => {
      result.current.groupMembersTogether(['a::weekly', 'b::session'])
    })
    await flush()

    const created = result.current.groups[1]
    expect(created.memberKeys).toEqual(['a::weekly', 'b::session'])
    expect(created.order).toBe(1)
    expect(created.collapsed).toBe(true)
    expect(created.name).toBe('Group 2')
  })

  test('a new group takes a palette colour no other group is wearing', async () => {
    const { result } = await mountWith([
      { id: 'g1', name: 'A', collapsed: false, order: 0, color: 'teal', memberKeys: [] },
    ])

    act(() => {
      result.current.createEmptyGroup()
    })
    await flush()

    expect(result.current.groups[1].color).toBe('blue')
    expect(result.current.groups[1].memberKeys).toEqual([])
  })

  test('grouping reports the new group id, so the panel can open its name for editing', async () => {
    const { result } = await mountWith([])

    let id = ''
    act(() => {
      id = result.current.createEmptyGroup()
    })
    await flush()

    expect(result.current.groups[0].id).toBe(id)
  })

  test('filing a pin into a group takes it out of whatever it was in', async () => {
    const { result } = await mountWith([
      { id: 'g1', name: 'A', collapsed: false, order: 0, color: 'teal', memberKeys: ['a::w'] },
      { id: 'g2', name: 'B', collapsed: false, order: 1, color: 'blue', memberKeys: [] },
    ])

    act(() => result.current.addMemberToGroup('a::w', 'g2'))
    await flush()

    expect(result.current.groups.map((g) => g.memberKeys)).toEqual([[], ['a::w']])
  })

  test('a pin dropped on a member joins ahead of it, not at the end', async () => {
    const { result } = await mountWith([
      {
        id: 'g1',
        name: 'A',
        collapsed: false,
        order: 0,
        color: 'teal',
        memberKeys: ['a::w', 'b::w'],
      },
    ])

    act(() => result.current.addMemberToGroup('c::w', 'g1', 'b::w'))
    await flush()

    expect(result.current.groups[0].memberKeys).toEqual(['a::w', 'c::w', 'b::w'])
  })

  // End to end with the decision that feeds it: the screen shows a
  // space where a release would land, and the release has to land
  // there. Reordering a member downwards used to open the space and
  // then move nothing, because filing the pin clears it from the group
  // first and "ahead of the row below me" is where it already was.
  test('dragging a member onto the one below it really does move it past', async () => {
    const group = {
      id: 'g1',
      name: 'A',
      collapsed: false,
      order: 0,
      color: 'teal' as const,
      memberKeys: ['a::w', 'b::w', 'c::w'],
    }
    const { result } = await mountWith([group])
    const order = memberOrder([
      {
        ...group,
        members: group.memberKeys.map((key) => ({ key, label: key, used: 1 })),
      },
    ])

    const drop = resolveDrop(
      { kind: 'pin', key: 'a::w', groupId: 'g1' },
      { kind: 'pin', key: 'b::w', groupId: 'g1' },
      order,
    )
    if (drop.kind !== 'joinGroup') throw new Error(`expected a joinGroup, got ${drop.kind}`)
    act(() => result.current.addMemberToGroup(drop.key, drop.groupId, drop.beforeKey ?? undefined))
    await flush()

    expect(result.current.groups[0].memberKeys).toEqual(['b::w', 'a::w', 'c::w'])
  })

  test('dragging a group onto another takes its place in the order', async () => {
    const { result } = await mountWith([
      { id: 'g1', name: 'A', collapsed: false, order: 0, color: 'teal', memberKeys: [] },
      { id: 'g2', name: 'B', collapsed: false, order: 1, color: 'blue', memberKeys: [] },
      { id: 'g3', name: 'C', collapsed: false, order: 2, color: 'red', memberKeys: [] },
    ])

    act(() => result.current.moveGroupBefore('g3', 'g1'))
    await flush()

    expect(stored().map((g) => [g.id, g.order])).toEqual([
      ['g3', 0],
      ['g1', 1],
      ['g2', 2],
    ])
  })

  test('taking a member out of its group leaves the group behind', async () => {
    const { result } = await mountWith([
      {
        id: 'g1',
        name: 'A',
        collapsed: false,
        order: 0,
        color: 'teal',
        memberKeys: ['a::w', 'b::w'],
      },
    ])

    act(() => result.current.removeMember('a::w'))
    await flush()

    expect(result.current.groups[0].memberKeys).toEqual(['b::w'])
  })

  test('ungrouping empties the group without removing it', async () => {
    const { result } = await mountWith([
      {
        id: 'g1',
        name: 'A',
        collapsed: false,
        order: 0,
        color: 'teal',
        memberKeys: ['a::w', 'b::w'],
      },
    ])

    act(() => result.current.ungroupAll('g1'))
    await flush()

    expect(result.current.groups).toHaveLength(1)
    expect(result.current.groups[0].memberKeys).toEqual([])
  })

  test('deleting drops the group and takes no other group with it', async () => {
    const { result } = await mountWith([
      { id: 'g1', name: 'A', collapsed: false, order: 0, color: 'teal', memberKeys: ['a::w'] },
      { id: 'g2', name: 'B', collapsed: false, order: 1, color: 'teal', memberKeys: ['b::w'] },
    ])

    act(() => result.current.deleteGroup('g1'))
    await flush()

    expect(result.current.groups.map((g) => g.id)).toEqual(['g2'])
    expect(result.current.groups[0].memberKeys).toEqual(['b::w'])
  })

  test('renaming keeps the members and the collapse state', async () => {
    const { result } = await mountWith([
      { id: 'g1', name: 'A', collapsed: true, order: 0, color: 'teal', memberKeys: ['a::w'] },
    ])

    act(() => result.current.renameGroup('g1', 'Money'))
    await flush()

    expect(result.current.groups[0]).toEqual({
      id: 'g1',
      name: 'Money',
      collapsed: true,
      order: 0,
      color: 'teal',
      memberKeys: ['a::w'],
    })
  })

  test('a failed save is retried on the next change rather than swallowed', async () => {
    vi.spyOn(console, 'error').mockImplementation(() => {})
    const { result } = await mountWith([
      { id: 'g1', name: 'A', collapsed: false, order: 0, color: 'teal', memberKeys: [] },
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
