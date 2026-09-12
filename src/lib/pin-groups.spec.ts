import { describe, expect, test } from 'vitest'
import {
  GROUP_COLORS,
  type LimitWindowEntity,
  type PinGroup,
  type Subscription,
} from '@/types/entities'
import {
  collectPinnedEntries,
  findGroupForMember,
  isGroupColor,
  layoutPinnedEntries,
  nextGroupColor,
  pinMemberKey,
  rollUpUsed,
  sortPinGroups,
  splitPinMemberKey,
} from './pin-groups'

function window(overrides: Partial<LimitWindowEntity> & { id: string }): LimitWindowEntity {
  return { name: 'Window', used: null, resetsAt: null, scope: null, isActive: true, ...overrides }
}

function subscription(overrides: Partial<Subscription> & { id: string }): Subscription {
  return {
    provider: 'claude',
    providerName: 'Anthropic',
    label: overrides.id,
    labelOverride: null,
    account: null,
    state: 'working',
    severity: 'healthy',
    used: null,
    resetsAt: null,
    lastReadAt: null,
    windows: [],
    reason: null,
    needsSignIn: false,
    pinnedWindowIds: [],
    headlineWindowId: null,
    configDir: '~/.claude',
    rateLimitedUntil: null,
    signInInProgress: false,
    pendingRemoval: false,
    ...overrides,
  }
}

function group(overrides: Partial<PinGroup> & { id: string }): PinGroup {
  return {
    name: overrides.id,
    collapsed: false,
    order: 0,
    color: 'teal',
    memberKeys: [],
    ...overrides,
  }
}

describe('pinMemberKey', () => {
  test('round-trips a subscription id that contains the separator itself', () => {
    const key = pinMemberKey('claude:claude', 'weekly_scoped:Fable')
    expect(splitPinMemberKey(key)).toEqual({
      subscriptionId: 'claude:claude',
      windowId: 'weekly_scoped:Fable',
    })
  })

  test('never lets two different pins collide, however the colons fall', () => {
    const left = pinMemberKey('a::b', 'c')
    const right = pinMemberKey('a', 'b::c')
    expect(left).not.toBe(right)
    expect(splitPinMemberKey(left)).toEqual({ subscriptionId: 'a::b', windowId: 'c' })
    expect(splitPinMemberKey(right)).toEqual({ subscriptionId: 'a', windowId: 'b::c' })
  })

  test('is null for a string that was never a member key', () => {
    expect(splitPinMemberKey('weekly_all')).toBeNull()
  })
})

describe('collectPinnedEntries', () => {
  test("walks subscriptions in panel order, then each one's own window order", () => {
    const subs = [
      subscription({
        id: 'a',
        pinnedWindowIds: ['session', 'weekly_all'],
        windows: [window({ id: 'session' }), window({ id: 'other' }), window({ id: 'weekly_all' })],
      }),
      subscription({ id: 'b', pinnedWindowIds: ['w'], windows: [window({ id: 'w' })] }),
    ]
    expect(collectPinnedEntries(subs).map((e) => e.key)).toEqual([
      pinMemberKey('a', 'session'),
      pinMemberKey('a', 'weekly_all'),
      pinMemberKey('b', 'w'),
    ])
  })

  test('leaves out a pinned id whose window is gone from the latest read', () => {
    const subs = [
      subscription({ id: 'a', pinnedWindowIds: ['gone'], windows: [window({ id: 'session' })] }),
    ]
    expect(collectPinnedEntries(subs)).toEqual([])
  })
})

describe('layoutPinnedEntries', () => {
  const subs = [
    subscription({
      id: 'a',
      pinnedWindowIds: ['session', 'weekly_all'],
      windows: [window({ id: 'session', used: 99 }), window({ id: 'weekly_all', used: 12 })],
    }),
    subscription({
      id: 'b',
      pinnedWindowIds: ['session'],
      windows: [window({ id: 'session', used: 47 })],
    }),
  ]

  test('files each pin under its group and leaves the rest standalone', () => {
    const groups = [
      group({
        id: 'g1',
        memberKeys: [pinMemberKey('a', 'session'), pinMemberKey('b', 'session')],
      }),
    ]
    const layout = layoutPinnedEntries(subs, groups)
    expect(layout.groups).toHaveLength(1)
    expect(layout.groups[0].members.map((m) => m.window.used)).toEqual([99, 47])
    expect(layout.standalone.map((m) => m.window.used)).toEqual([12])
  })

  test('reports a group whose members are all unpinned, so it can still be dissolved', () => {
    const groups = [group({ id: 'g1', memberKeys: [pinMemberKey('a', 'never-pinned')] })]
    const layout = layoutPinnedEntries(subs, groups)
    expect(layout.groups[0].members).toEqual([])
    expect(layout.standalone).toHaveLength(3)
  })

  test('orders groups by their own order field, not by insertion', () => {
    const groups = [group({ id: 'second', order: 2 }), group({ id: 'first', order: 1 })]
    expect(layoutPinnedEntries(subs, groups).groups.map((g) => g.group.id)).toEqual([
      'first',
      'second',
    ])
  })

  test('keeps insertion order between two groups claiming the same order number', () => {
    const groups = [group({ id: 'x', order: 0 }), group({ id: 'y', order: 0 })]
    expect(sortPinGroups(groups).map((g) => g.id)).toEqual(['x', 'y'])
  })
})

describe('rollUpUsed', () => {
  test("is the worst member's figure, ignoring the ones with no number yet", () => {
    const subs = [
      subscription({
        id: 'a',
        pinnedWindowIds: ['x', 'y', 'z'],
        windows: [
          window({ id: 'x', used: 40 }),
          window({ id: 'y', used: null }),
          window({ id: 'z', used: 84 }),
        ],
      }),
    ]
    expect(rollUpUsed(collectPinnedEntries(subs))).toBe(84)
  })

  test('is null when no member has a figure at all', () => {
    expect(rollUpUsed([])).toBeNull()
  })
})

describe('findGroupForMember', () => {
  test('names the group a pin is filed under, or none', () => {
    const groups = [group({ id: 'g1', memberKeys: [pinMemberKey('a', 'session')] })]
    expect(findGroupForMember(groups, pinMemberKey('a', 'session'))?.id).toBe('g1')
    expect(findGroupForMember(groups, pinMemberKey('a', 'weekly_all'))).toBeNull()
  })
})

describe('group colours', () => {
  test('a new group takes the first palette colour no group is wearing', () => {
    expect(nextGroupColor([])).toBe('teal')
    expect(nextGroupColor([group({ id: 'g1', color: 'teal' })])).toBe('blue')
    expect(
      nextGroupColor([group({ id: 'g1', color: 'teal' }), group({ id: 'g2', color: 'violet' })]),
    ).toBe('blue')
  })

  test('once the palette is spent it repeats in palette order, not at random', () => {
    const spent = GROUP_COLORS.map((color, index) => group({ id: `g${index}`, color }))
    expect(nextGroupColor(spent)).toBe(GROUP_COLORS[spent.length % GROUP_COLORS.length])
  })

  test('a colour that is not in the palette is not mistaken for one', () => {
    expect(isGroupColor('teal')).toBe(true)
    expect(isGroupColor('chartreuse')).toBe(false)
    expect(isGroupColor(undefined)).toBe(false)
  })
})
