import { describe, expect, test } from 'vitest'
import {
  asDragItem,
  asDropZone,
  type DragItem,
  groupDragId,
  landingSlot,
  memberOrder,
  pinDragId,
  resolveDrop,
  sameDrop,
  UNGROUPED_DROP_ID,
} from './customize-drag'

const looseA: DragItem = { kind: 'pin', key: 'a::session', groupId: null }
const looseB: DragItem = { kind: 'pin', key: 'b::session', groupId: null }
const memberOfOne: DragItem = { kind: 'pin', key: 'a::weekly', groupId: 'g1' }
const groupOne: DragItem = { kind: 'group', groupId: 'g1' }
const groupTwo: DragItem = { kind: 'group', groupId: 'g2' }

function zone(item: DragItem) {
  return asDropZone(item.kind === 'pin' ? pinDragId(item.key) : groupDragId(item.groupId), item)
}

describe('dropping a pin', () => {
  test('on a loose pin makes a group of the two, the one already there first', () => {
    expect(resolveDrop(looseB, zone(looseA))).toEqual({
      kind: 'groupTogether',
      keys: ['a::session', 'b::session'],
    })
  })

  test('on a grouped pin joins that group ahead of it', () => {
    expect(resolveDrop(looseA, zone(memberOfOne))).toEqual({
      kind: 'joinGroup',
      key: 'a::session',
      groupId: 'g1',
      beforeKey: 'a::weekly',
    })
  })

  test("on a group's box joins it at the end", () => {
    expect(resolveDrop(looseA, zone(groupOne))).toEqual({
      kind: 'joinGroup',
      key: 'a::session',
      groupId: 'g1',
      beforeKey: null,
    })
  })

  test('on itself does nothing', () => {
    expect(resolveDrop(looseA, zone(looseA))).toEqual({ kind: 'nothing' })
  })

  test('on nothing at all does nothing', () => {
    expect(resolveDrop(looseA, null)).toEqual({ kind: 'nothing' })
  })

  test('on a sibling in its own group reorders it ahead of that sibling', () => {
    const sibling: DragItem = { kind: 'pin', key: 'b::weekly', groupId: 'g1' }
    expect(resolveDrop(memberOfOne, zone(sibling))).toEqual({
      kind: 'joinGroup',
      key: 'a::weekly',
      groupId: 'g1',
      beforeKey: 'b::weekly',
    })
  })

  test('on a loose pin while it is in a group takes it out and pairs the two', () => {
    expect(resolveDrop(memberOfOne, zone(looseA))).toEqual({
      kind: 'groupTogether',
      keys: ['a::session', 'a::weekly'],
    })
  })
})

describe('dropping a pin on the way out of a group', () => {
  test('a member leaves its group', () => {
    expect(resolveDrop(memberOfOne, { kind: 'ungrouped' })).toEqual({
      kind: 'leaveGroup',
      key: 'a::weekly',
    })
  })

  test('a pin that is in no group has nothing to leave', () => {
    expect(resolveDrop(looseA, { kind: 'ungrouped' })).toEqual({ kind: 'nothing' })
  })
})

describe('dropping a group', () => {
  test('on another group puts it in front of that one', () => {
    expect(resolveDrop(groupTwo, zone(groupOne))).toEqual({
      kind: 'moveGroup',
      groupId: 'g2',
      beforeGroupId: 'g1',
    })
  })

  test("on another group's member counts as the group it belongs to", () => {
    expect(resolveDrop(groupTwo, zone(memberOfOne))).toEqual({
      kind: 'moveGroup',
      groupId: 'g2',
      beforeGroupId: 'g1',
    })
  })

  test('on itself, on one of its own members, or on a loose pin, does nothing', () => {
    expect(resolveDrop(groupOne, zone(groupOne))).toEqual({ kind: 'nothing' })
    expect(resolveDrop(groupOne, zone(memberOfOne))).toEqual({ kind: 'nothing' })
    expect(resolveDrop(groupOne, zone(looseB))).toEqual({ kind: 'nothing' })
  })

  test('on the way out of a group does nothing: only pins leave groups', () => {
    expect(resolveDrop(groupOne, { kind: 'ungrouped' })).toEqual({ kind: 'nothing' })
  })
})

// The bug this reproduces: the list closes up behind a drag before the
// drop is applied, so "land ahead of the member below me" is where the
// row already was. The slot opened under the pointer and the release
// then moved nothing.
describe('dropping a pin among the members of its own group', () => {
  const order = memberOrder([
    {
      id: 'g1',
      name: 'Money',
      color: 'teal',
      members: [
        { key: 'one', label: 'One', used: 1 },
        { key: 'two', label: 'Two', used: 2 },
        { key: 'three', label: 'Three', used: 3 },
      ],
    },
  ])
  const member = (key: string): DragItem => ({ kind: 'pin', key, groupId: 'g1' })

  test('onto the member below it lands after that member, which is a real move', () => {
    expect(resolveDrop(member('one'), zone(member('two')), order)).toEqual({
      kind: 'joinGroup',
      key: 'one',
      groupId: 'g1',
      beforeKey: 'three',
    })
  })

  test('onto the last member lands at the end', () => {
    expect(resolveDrop(member('one'), zone(member('three')), order)).toEqual({
      kind: 'joinGroup',
      key: 'one',
      groupId: 'g1',
      beforeKey: null,
    })
  })

  test('onto a member above it lands ahead of that member', () => {
    expect(resolveDrop(member('three'), zone(member('one')), order)).toEqual({
      kind: 'joinGroup',
      key: 'three',
      groupId: 'g1',
      beforeKey: 'one',
    })
  })

  test('a pin from outside the group lands ahead of whatever it was dropped on', () => {
    expect(resolveDrop(looseA, zone(member('two')), order)).toEqual({
      kind: 'joinGroup',
      key: 'a::session',
      groupId: 'g1',
      beforeKey: 'two',
    })
  })

  test('every one of those drops opens its space exactly where it lands', () => {
    for (const [active, over] of [
      [member('one'), member('two')],
      [member('one'), member('three')],
      [member('three'), member('one')],
    ] as const) {
      const action = resolveDrop(active, zone(over), order)
      expect(landingSlot(action)).toEqual({
        kind: 'inGroup',
        groupId: 'g1',
        beforeKey: (action as { beforeKey: string | null }).beforeKey,
      })
    }
  })

  test('without the order to read, it falls back to landing ahead of the member', () => {
    expect(resolveDrop(member('one'), zone(member('two')))).toMatchObject({ beforeKey: 'two' })
  })
})

describe('reading back what the drag library reports', () => {
  test('an id names one pin or one group, unambiguously', () => {
    expect(pinDragId('a::session')).not.toBe(groupDragId('a::session'))
  })

  test('the way out of a group is recognised by its id alone', () => {
    expect(asDropZone(UNGROUPED_DROP_ID, undefined)).toEqual({ kind: 'ungrouped' })
  })

  test('anything that is not one of ours is no target at all', () => {
    expect(asDragItem(undefined)).toBeNull()
    expect(asDragItem({ kind: 'something-else' })).toBeNull()
    expect(asDragItem({ kind: 'pin' })).toBeNull()
    expect(asDropZone('whatever', {})).toBeNull()
  })

  test('a pin reported without a group is read as standing on its own', () => {
    expect(asDragItem({ kind: 'pin', key: 'a::session' })).toEqual(looseA)
  })

  test('nothing under the pointer resolves to no zone', () => {
    expect(asDropZone(undefined, undefined)).toBeNull()
  })
})

describe('the slot a release would fill', () => {
  test('opens inside the group a pin would join, at its end', () => {
    expect(
      landingSlot({ kind: 'joinGroup', key: 'a::session', groupId: 'g1', beforeKey: null }),
    ).toEqual({ kind: 'inGroup', groupId: 'g1', beforeKey: null })
  })

  test('opens ahead of the member a pin was dropped onto', () => {
    expect(
      landingSlot({ kind: 'joinGroup', key: 'a::session', groupId: 'g1', beforeKey: 'a::weekly' }),
    ).toEqual({ kind: 'inGroup', groupId: 'g1', beforeKey: 'a::weekly' })
  })

  test('opens among the loose rows for a pin leaving its group', () => {
    expect(landingSlot({ kind: 'leaveGroup', key: 'a::weekly' })).toEqual({ kind: 'loose' })
  })

  test('opens above the group a dragged group would land in front of', () => {
    expect(landingSlot({ kind: 'moveGroup', groupId: 'g2', beforeGroupId: 'g1' })).toEqual({
      kind: 'beforeGroup',
      groupId: 'g1',
    })
  })

  test('opens nowhere for a drop that makes a group, or does nothing at all', () => {
    expect(landingSlot({ kind: 'groupTogether', keys: ['a', 'b'] })).toEqual({ kind: 'none' })
    expect(landingSlot({ kind: 'nothing' })).toEqual({ kind: 'none' })
  })
})

describe('one answer twice is the same answer', () => {
  test('two drops that would do the same thing compare equal', () => {
    expect(
      sameDrop(
        { kind: 'joinGroup', key: 'a', groupId: 'g1', beforeKey: null },
        { kind: 'joinGroup', key: 'a', groupId: 'g1', beforeKey: null },
      ),
    ).toBe(true)
    expect(sameDrop({ kind: 'nothing' }, { kind: 'nothing' })).toBe(true)
    expect(
      sameDrop(
        { kind: 'groupTogether', keys: ['a', 'b'] },
        { kind: 'groupTogether', keys: ['a', 'b'] },
      ),
    ).toBe(true)
  })

  test('a different target, a different member, or a different kind does not', () => {
    expect(
      sameDrop(
        { kind: 'joinGroup', key: 'a', groupId: 'g1', beforeKey: null },
        { kind: 'joinGroup', key: 'a', groupId: 'g2', beforeKey: null },
      ),
    ).toBe(false)
    expect(
      sameDrop(
        { kind: 'joinGroup', key: 'a', groupId: 'g1', beforeKey: null },
        { kind: 'joinGroup', key: 'a', groupId: 'g1', beforeKey: 'b' },
      ),
    ).toBe(false)
    expect(sameDrop({ kind: 'leaveGroup', key: 'a' }, { kind: 'leaveGroup', key: 'b' })).toBe(false)
    expect(
      sameDrop(
        { kind: 'moveGroup', groupId: 'g1', beforeGroupId: 'g2' },
        { kind: 'moveGroup', groupId: 'g1', beforeGroupId: 'g3' },
      ),
    ).toBe(false)
    expect(sameDrop({ kind: 'nothing' }, { kind: 'leaveGroup', key: 'a' })).toBe(false)
  })
})
