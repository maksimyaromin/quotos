import { describe, expect, test } from 'vitest'
import type { LimitWindowEntity, PinGroup, Subscription } from '@/types/entities'
import { pinMemberKey } from './pin-groups'
import {
  buildStatusItemSegments,
  buildStatusItemTooltip,
  computeIconFillPercent,
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

  test("orders figures in panel order, then each subscription's own window order", () => {
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
      { kind: 'figure', text: '61%', color: 'neutral' },
      { kind: 'figure', text: '74%', color: 'neutral' },
      { kind: 'figure', text: '52%', color: 'neutral' },
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
      { kind: 'figure', text: '20%', color: 'neutral' },
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
      { kind: 'figure', text: '10%', color: 'neutral' },
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

describe('computeIconFillPercent', () => {
  const twoHeadlines = [
    subscription({ id: 'a', used: 20, windows: [window({ id: 'w1', used: 61 })] }),
    subscription({ id: 'b', used: 50, windows: [window({ id: 'w2', used: 74 })] }),
  ]

  test('is 0 when nothing tracked reports a headline figure', () => {
    expect(computeIconFillPercent([])).toBe(0)
    expect(computeIconFillPercent([subscription({ id: 'a' })])).toBe(0)
  })

  test('defaults to the arithmetic mean of every headline figure', () => {
    expect(computeIconFillPercent(twoHeadlines)).toBe(35)
  })

  test('skips a subscription with no headline figure rather than counting it as zero', () => {
    expect(computeIconFillPercent([...twoHeadlines, subscription({ id: 'c' })])).toBe(35)
  })

  test('rounds the mean to a whole percent', () => {
    const subs = [
      subscription({ id: 'a', used: 10 }),
      subscription({ id: 'b', used: 10 }),
      subscription({ id: 'c', used: 11 }),
    ]
    expect(computeIconFillPercent(subs)).toBe(10)
  })

  test('a designated window overrides the mean', () => {
    expect(computeIconFillPercent(twoHeadlines, pinMemberKey('b', 'w2'))).toBe(74)
  })

  test('a designated window need not be pinned', () => {
    const subs = [subscription({ id: 'a', used: 20, windows: [window({ id: 'w1', used: 61 })] })]
    expect(computeIconFillPercent(subs, pinMemberKey('a', 'w1'))).toBe(61)
  })

  test('falls back to the mean when the designated window reports nothing', () => {
    expect(computeIconFillPercent(twoHeadlines, pinMemberKey('b', 'missing'))).toBe(35)
    expect(computeIconFillPercent(twoHeadlines, pinMemberKey('gone', 'w2'))).toBe(35)
    const noFigure = [
      subscription({ id: 'a', used: 20, windows: [window({ id: 'w1', used: null })] }),
    ]
    expect(computeIconFillPercent(noFigure, pinMemberKey('a', 'w1'))).toBe(20)
  })

  test('a malformed key falls back to the mean rather than throwing', () => {
    expect(computeIconFillPercent(twoHeadlines, 'not-a-member-key')).toBe(35)
  })

  // The shape a real choice has on disk: a subscription id with its
  // own colon, and a window id that carries a second one. The gauge
  // reads the window that was chosen, not the headline it sits under.
  test('reads a scoped window whose own id carries a colon', () => {
    const subs = [
      subscription({
        id: 'claude:claude',
        used: 58,
        pinnedWindowIds: ['session', 'weekly_scoped:Fable'],
        windows: [
          window({ id: 'session', name: 'Session', used: 12 }),
          window({ id: 'weekly_scoped:Fable', name: 'Weekly', scope: 'Fable', used: 99 }),
        ],
      }),
    ]
    const chosen = pinMemberKey('claude:claude', 'weekly_scoped:Fable')
    expect(chosen).toBe('claude%3Aclaude::weekly_scoped:Fable')
    expect(computeIconFillPercent(subs, chosen)).toBe(99)
    expect(computeIconFillPercent(subs)).toBe(58)
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
    color: 'red',
    memberKeys: [pinMemberKey('a', 'session'), pinMemberKey('b', 'session')],
  })

  test('a rolled-up group is its chip and nothing else — no figure of any kind', () => {
    expect(buildStatusItemSegments(twoSubscriptions, [currentLimit])).toEqual([
      { kind: 'chip', slug: 'CUR', color: 'red', groupId: 'g1' },
      { kind: 'figure', text: '12%', color: 'neutral' },
    ])
  })

  test("opened, the chip stays and every member's own figure follows it", () => {
    const opened = { ...currentLimit, collapsed: false }
    expect(buildStatusItemSegments(twoSubscriptions, [opened])).toEqual([
      { kind: 'chip', slug: 'CUR', color: 'red', groupId: 'g1' },
      { kind: 'figure', text: '99%', color: 'red' },
      { kind: 'figure', text: '47%', color: 'neutral' },
      { kind: 'figure', text: '12%', color: 'neutral' },
    ])
  })

  test('opening a group changes nothing about the chip that leads it', () => {
    const collapsed = buildStatusItemSegments(twoSubscriptions, [currentLimit])
    const opened = buildStatusItemSegments(twoSubscriptions, [
      { ...currentLimit, collapsed: false },
    ])
    expect(opened[0]).toEqual(collapsed[0])
  })

  test('a group is named once, by its chip, never by a member', () => {
    const opened = { ...currentLimit, collapsed: false }
    expect(
      buildStatusItemSegments(twoSubscriptions, [opened]).filter((s) => s.kind === 'chip'),
    ).toHaveLength(1)
  })

  test('the chip is the one thing carrying a group id, so only it folds the group', () => {
    const opened = { ...currentLimit, collapsed: false }
    const withGroup = buildStatusItemSegments(twoSubscriptions, [opened]).filter(
      (s) => s.kind === 'chip',
    )
    expect(withGroup.map((s) => s.groupId)).toEqual(['g1'])
  })

  test("a chip's slug is the group's name's first three letters, uppercased", () => {
    expect(buildStatusItemSegments(twoSubscriptions, [currentLimit])[0]).toMatchObject({
      slug: 'CUR',
    })
  })

  test('derives the slug from whatever the group is called now, never from a stored one', () => {
    const renamed = { ...currentLimit, name: 'Fable' }
    expect(buildStatusItemSegments(twoSubscriptions, [renamed])[0]).toMatchObject({ slug: 'FAB' })
  })

  test('a group named in one or two characters keeps the whole of its name', () => {
    const short = { ...currentLimit, name: 'Q' }
    expect(buildStatusItemSegments(twoSubscriptions, [short])[0]).toMatchObject({ slug: 'Q' })
  })

  test('a chip wears its own group colour', () => {
    const violet = { ...currentLimit, color: 'violet' as const }
    expect(buildStatusItemSegments(twoSubscriptions, [violet])[0]).toMatchObject({
      color: 'violet',
    })
  })

  test("an opened group's members come out in the order the group holds them", () => {
    const opened = {
      ...currentLimit,
      collapsed: false,
      memberKeys: [pinMemberKey('b', 'session'), pinMemberKey('a', 'session')],
    }
    expect(
      buildStatusItemSegments(twoSubscriptions, [opened])
        .filter((s) => s.kind === 'figure')
        .map((s) => s.text),
    ).toEqual(['47%', '99%', '12%'])
  })

  test('a standalone pin is a bare figure, carrying no group to fold', () => {
    const segments = buildStatusItemSegments(twoSubscriptions, [currentLimit])
    expect(segments[segments.length - 1]).toEqual({
      kind: 'figure',
      text: '12%',
      color: 'neutral',
    })
  })

  test('with no groups at all the row is nothing but figures', () => {
    const segments = buildStatusItemSegments(twoSubscriptions, [])
    expect(segments.every((s) => s.kind === 'figure')).toBe(true)
    expect(segments).toHaveLength(3)
  })

  test('emits nothing at all, chip included, for a group with nothing to report', () => {
    const empty = group({ id: 'g3', memberKeys: [pinMemberKey('a', 'gone')] })
    expect(buildStatusItemSegments(twoSubscriptions, [empty])).toHaveLength(3)
  })

  test('emits no chip for a group whose name derives no slug', () => {
    const nameless = { ...currentLimit, name: '   ' }
    expect(buildStatusItemSegments(twoSubscriptions, [nameless])).toEqual([
      { kind: 'figure', text: '12%', color: 'neutral' },
    ])
  })

  test('still turns every figure amber when a grouped member is behind', () => {
    const stale = [
      twoSubscriptions[0],
      subscription({ ...twoSubscriptions[1], id: 'b', state: 'behind' }),
    ]
    expect(
      buildStatusItemSegments(stale, [{ ...currentLimit, collapsed: false }])
        .filter((s) => s.kind === 'figure')
        .map((s) => s.color),
    ).toEqual(['amber', 'amber', 'amber'])
  })
})
