import { describe, expect, test } from 'vitest'
import type { LimitWindowEntity, PinGroup, Subscription } from '@/types/entities'
import { pinMemberKey } from './pin-groups'
import {
  buildStatusItemSegments,
  buildStatusItemTooltip,
  computeWorstActiveLimitPercent,
} from './status-item-segments'

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
    collapsed: true,
    order: 0,
    color: 'teal',
    memberKeys: [],
    ...overrides,
  }
}

describe('buildStatusItemSegments', () => {
  test('emits nothing when nothing is pinned', () => {
    const subs = [subscription({ id: 'a', windows: [window({ id: 'session', used: 10 })] })]
    expect(buildStatusItemSegments(subs)).toEqual([])
  })

  test("skips a pinned window with no numeric value yet, never rendering '!' or '…'", () => {
    const subs = [
      subscription({
        id: 'a',
        pinnedWindowIds: ['session'],
        windows: [window({ id: 'session', used: null })],
      }),
    ]
    expect(buildStatusItemSegments(subs)).toEqual([])
  })

  test('skips a pinned id whose window no longer exists in the latest read', () => {
    const subs = [
      subscription({
        id: 'a',
        pinnedWindowIds: ['gone'],
        windows: [window({ id: 'session', used: 10 })],
      }),
    ]
    expect(buildStatusItemSegments(subs)).toEqual([])
  })

  test("orders figures in panel order, then each subscription's own window order, marking group starts", () => {
    const subs = [
      subscription({
        id: 'a',
        pinnedWindowIds: ['session', 'weekly_all'],
        windows: [window({ id: 'session', used: 61 }), window({ id: 'weekly_all', used: 74 })],
      }),
      subscription({
        id: 'b',
        pinnedWindowIds: ['weekly_all'],
        windows: [window({ id: 'weekly_all', used: 52 })],
      }),
    ]
    expect(buildStatusItemSegments(subs)).toEqual([
      { text: '61%', color: 'neutral', groupStart: false, groupId: null, slug: null },
      { text: '74%', color: 'neutral', groupStart: false, groupId: null, slug: null },
      { text: '52%', color: 'neutral', groupStart: true, groupId: null, slug: null },
    ])
  })

  test("colors a figure from its own window's used%, not the subscription's aggregate severity", () => {
    const subs = [
      subscription({
        id: 'a',
        severity: 'warn',
        pinnedWindowIds: ['weekly_all'],
        windows: [window({ id: 'session', used: 85 }), window({ id: 'weekly_all', used: 20 })],
      }),
    ]
    expect(buildStatusItemSegments(subs)).toEqual([
      { text: '20%', color: 'neutral', groupStart: false, groupId: null, slug: null },
    ])
  })

  test('colors past 75 amber and past 90 red, the boundary itself included', () => {
    const subs = [
      subscription({ id: 'a', pinnedWindowIds: ['w'], windows: [window({ id: 'w', used: 80 })] }),
      subscription({ id: 'b', pinnedWindowIds: ['w'], windows: [window({ id: 'w', used: 95 })] }),
    ]
    expect(buildStatusItemSegments(subs).map((s) => s.color)).toEqual(['amber', 'red'])

    const atBoundary = [
      subscription({ id: 'a', pinnedWindowIds: ['w'], windows: [window({ id: 'w', used: 75 })] }),
      subscription({ id: 'b', pinnedWindowIds: ['w'], windows: [window({ id: 'w', used: 90 })] }),
    ]
    expect(buildStatusItemSegments(atBoundary).map((s) => s.color)).toEqual(['amber', 'red'])
  })

  test('turns every contributing figure amber when any contributing subscription is stale', () => {
    const subs = [
      subscription({ id: 'a', pinnedWindowIds: ['w'], windows: [window({ id: 'w', used: 10 })] }),
      subscription({
        id: 'b',
        state: 'behind',
        pinnedWindowIds: ['w'],
        windows: [window({ id: 'w', used: 20 })],
      }),
    ]
    expect(buildStatusItemSegments(subs).map((s) => s.color)).toEqual(['amber', 'amber'])
  })

  test("a stale subscription with nothing pinned doesn't taint the bar", () => {
    const subs = [
      subscription({ id: 'a', pinnedWindowIds: ['w'], windows: [window({ id: 'w', used: 10 })] }),
      subscription({
        id: 'b',
        state: 'behind',
        pinnedWindowIds: [],
        windows: [window({ id: 'w', used: 99 })],
      }),
    ]
    expect(buildStatusItemSegments(subs)).toEqual([
      { text: '10%', color: 'neutral', groupStart: false, groupId: null, slug: null },
    ])
  })
})

describe('buildStatusItemTooltip', () => {
  test('is just the product name when nothing contributes a figure', () => {
    expect(buildStatusItemTooltip([])).toBe('Quotos')
    const subs = [
      subscription({
        id: 'a',
        pinnedWindowIds: ['session'],
        windows: [window({ id: 'session', used: null })],
      }),
    ]
    expect(buildStatusItemTooltip(subs)).toBe('Quotos')
  })

  test('names each contributing figure, one line per subscription, in bar order', () => {
    const subs = [
      subscription({
        id: 'a',
        label: 'Claude Max',
        pinnedWindowIds: ['weekly_all', 'session'],
        windows: [
          window({ id: 'session', name: 'Session', used: 70 }),
          window({ id: 'weekly_all', name: 'Weekly', used: 40 }),
          window({ id: 'weekly_scoped:Fable', name: 'Weekly', scope: 'Fable', used: 12 }),
        ],
      }),
      subscription({
        id: 'b',
        label: 'Claude Pro',
        pinnedWindowIds: ['weekly_scoped:Opus'],
        windows: [window({ id: 'weekly_scoped:Opus', name: 'Weekly', scope: 'Opus', used: 55 })],
      }),
    ]
    expect(buildStatusItemTooltip(subs)).toBe(
      'Quotos\nClaude Max: Session 70% · Weekly 40%\nClaude Pro: Weekly (Opus) 55%',
    )
  })

  test("prefers the user's rename over the provider label", () => {
    const subs = [
      subscription({
        id: 'a',
        label: 'Claude Max',
        labelOverride: 'Work',
        pinnedWindowIds: ['w'],
        windows: [window({ id: 'w', name: 'Weekly', used: 40 })],
      }),
    ]
    expect(buildStatusItemTooltip(subs)).toBe('Quotos\nWork: Weekly 40%')
  })

  test("marks only the stale subscription's own line, in the row badge's words", () => {
    const subs = [
      subscription({
        id: 'a',
        label: 'Claude Max',
        state: 'behind',
        pinnedWindowIds: ['w'],
        windows: [window({ id: 'w', name: 'Weekly', used: 40 })],
      }),
      subscription({
        id: 'b',
        label: 'Claude Pro',
        pinnedWindowIds: ['w'],
        windows: [window({ id: 'w', name: 'Weekly', used: 55 })],
      }),
    ]
    expect(buildStatusItemTooltip(subs)).toBe(
      'Quotos\nClaude Max: Weekly 40% — not current\nClaude Pro: Weekly 55%',
    )
  })
})

describe('computeWorstActiveLimitPercent', () => {
  test('is 0 when nothing tracked has any active numeric window', () => {
    expect(computeWorstActiveLimitPercent([])).toBe(0)
    expect(computeWorstActiveLimitPercent([subscription({ id: 'a' })])).toBe(0)
  })

  test('is the max used% across every active window, pinned or not', () => {
    const subs = [
      subscription({ id: 'a', windows: [window({ id: 'w1', used: 61, isActive: true })] }),
      subscription({
        id: 'b',
        windows: [
          window({ id: 'w2', used: 74, isActive: true }),
          window({ id: 'w3', used: 99, isActive: false }),
        ],
      }),
    ]
    expect(computeWorstActiveLimitPercent(subs)).toBe(74)
  })
})

describe('buildStatusItemSegments with pin groups', () => {
  const twoSubscriptions = [
    subscription({
      id: 'a',
      label: 'Work',
      pinnedWindowIds: ['session', 'weekly_all'],
      windows: [
        window({ id: 'session', name: 'Session', used: 99 }),
        window({ id: 'weekly_all', name: 'Weekly', used: 12 }),
      ],
    }),
    subscription({
      id: 'b',
      label: 'Home',
      pinnedWindowIds: ['session'],
      windows: [window({ id: 'session', name: 'Session', used: 47 })],
    }),
  ]

  const currentLimit = group({
    id: 'g1',
    name: 'Current limit',
    memberKeys: [pinMemberKey('a', 'session'), pinMemberKey('b', 'session')],
  })

  test("shows one rolled-up figure for a group, the worst member's, and none of its members", () => {
    expect(buildStatusItemSegments(twoSubscriptions, [currentLimit])).toEqual([
      { text: '99%', color: 'red', groupStart: false, groupId: 'g1', slug: 'CUR' },
      { text: '12%', color: 'neutral', groupStart: true, groupId: null, slug: null },
    ])
  })

  test("opens a group out to every member's own figure once it is not collapsed", () => {
    const opened = { ...currentLimit, collapsed: false }
    expect(buildStatusItemSegments(twoSubscriptions, [opened])).toEqual([
      { text: '99%', color: 'red', groupStart: false, groupId: 'g1', slug: 'CUR' },
      { text: '47%', color: 'neutral', groupStart: false, groupId: 'g1', slug: null },
      { text: '12%', color: 'neutral', groupStart: true, groupId: null, slug: null },
    ])
  })

  test("marks every one of an opened group's figures with the group, so any of them rolls it back up", () => {
    const opened = { ...currentLimit, collapsed: false }
    const grouped = buildStatusItemSegments(twoSubscriptions, [opened]).filter(
      (s) => s.groupId !== null,
    )
    expect(grouped).toHaveLength(2)
    expect(grouped.every((s) => s.groupId === 'g1')).toBe(true)
  })

  test("leads a group's figure with its name's first three letters, and a standalone pin with nothing", () => {
    const segments = buildStatusItemSegments(twoSubscriptions, [currentLimit])
    expect(segments[0].slug).toBe('CUR')
    expect(segments[segments.length - 1].slug).toBeNull()
  })

  test('derives the slug from whatever the group is called now, never from a stored one', () => {
    const renamed = { ...currentLimit, name: 'Fable' }
    expect(buildStatusItemSegments(twoSubscriptions, [renamed])[0].slug).toBe('FAB')
  })

  test('an opened group is named once, by the figure it leads, not by every member', () => {
    const opened = { ...currentLimit, collapsed: false }
    const grouped = buildStatusItemSegments(twoSubscriptions, [opened]).filter(
      (s) => s.groupId !== null,
    )
    expect(grouped.map((s) => s.slug)).toEqual(['CUR', null])
  })

  test('a group named in one or two characters keeps the whole of its name', () => {
    const short = { ...currentLimit, name: 'Q' }
    expect(buildStatusItemSegments(twoSubscriptions, [short])[0].slug).toBe('Q')
  })

  test("an opened group's members come out in the order the group holds them", () => {
    const opened = {
      ...currentLimit,
      collapsed: false,
      memberKeys: [pinMemberKey('b', 'session'), pinMemberKey('a', 'session')],
    }
    expect(
      buildStatusItemSegments(twoSubscriptions, [opened])
        .filter((s) => s.groupId !== null)
        .map((s) => s.text),
    ).toEqual(['47%', '99%'])
  })

  test('leaves a standalone pin carrying no group, so clicking it opens the panel', () => {
    const segments = buildStatusItemSegments(twoSubscriptions, [currentLimit])
    expect(segments[segments.length - 1].groupId).toBeNull()
  })

  test('leaves a pin in no group emitting its own figure, exactly as before', () => {
    const grouped = buildStatusItemSegments(twoSubscriptions, [currentLimit])
    const ungrouped = buildStatusItemSegments(twoSubscriptions, [])
    expect(grouped[grouped.length - 1]).toEqual({
      text: '12%',
      color: 'neutral',
      groupStart: true,
      groupId: null,
      slug: null,
    })
    expect(ungrouped).toHaveLength(3)
  })

  test('colors the rolled-up figure by the worst member, not by the first one', () => {
    const money = group({
      id: 'g2',
      name: 'Money',
      memberKeys: [pinMemberKey('a', 'weekly_all'), pinMemberKey('b', 'session')],
    })
    expect(buildStatusItemSegments(twoSubscriptions, [money])[0]).toEqual({
      text: '47%',
      color: 'neutral',
      groupStart: false,
      groupId: 'g2',
      slug: 'MON',
    })
  })

  test('emits nothing for a group whose members are all unpinned or unread', () => {
    const empty = group({ id: 'g3', memberKeys: [pinMemberKey('a', 'gone')] })
    expect(buildStatusItemSegments(twoSubscriptions, [empty])).toHaveLength(3)
  })

  test('breaks between each group and again before the standalone pins', () => {
    const groups = [
      group({ id: 'g1', order: 1, memberKeys: [pinMemberKey('a', 'session')] }),
      group({ id: 'g2', order: 2, memberKeys: [pinMemberKey('b', 'session')] }),
    ]
    expect(buildStatusItemSegments(twoSubscriptions, groups).map((s) => s.groupStart)).toEqual([
      false,
      true,
      true,
    ])
  })

  test('still turns every figure amber when a grouped member is behind', () => {
    const stale = [
      twoSubscriptions[0],
      subscription({ ...twoSubscriptions[1], id: 'b', state: 'behind' }),
    ]
    expect(buildStatusItemSegments(stale, [currentLimit]).map((s) => s.color)).toEqual([
      'amber',
      'amber',
    ])
  })
})

describe('buildStatusItemTooltip with pin groups', () => {
  const subs = [
    subscription({
      id: 'a',
      label: 'Work',
      pinnedWindowIds: ['session', 'weekly_all'],
      windows: [
        window({ id: 'session', name: 'Session', used: 99 }),
        window({ id: 'weekly_all', name: 'Weekly', used: 12 }),
      ],
    }),
    subscription({
      id: 'b',
      label: 'Home',
      pinnedWindowIds: ['session'],
      windows: [window({ id: 'session', name: 'Session', scope: 'Fable', used: 47 })],
    }),
  ]
  const currentLimit = group({
    id: 'g1',
    name: 'Current limit',
    memberKeys: [pinMemberKey('a', 'session'), pinMemberKey('b', 'session')],
  })

  test("names every member under its group's name, not just the rolled-up figure", () => {
    expect(buildStatusItemTooltip(subs, [currentLimit])).toBe(
      [
        'Quotos',
        'Current limit: Work — Session 99% · Home — Session (Fable) 47%',
        'Work: Weekly 12%',
      ].join('\n'),
    )
  })

  test("marks a group whose member is behind, in the row badge's words", () => {
    const stale = [subs[0], subscription({ ...subs[1], id: 'b', state: 'behind' })]
    expect(buildStatusItemTooltip(stale, [currentLimit])).toContain(
      'Current limit: Work — Session 99% · Home — Session (Fable) 47% — not current',
    )
  })

  test('says nothing about a group with no contributing member', () => {
    const empty = group({ id: 'g2', name: 'Money', memberKeys: [pinMemberKey('a', 'gone')] })
    expect(buildStatusItemTooltip(subs, [empty])).not.toContain('Money')
  })
})
