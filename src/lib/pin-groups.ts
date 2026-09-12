import {
  GROUP_COLORS,
  type GroupColor,
  type LimitWindowEntity,
  type PinGroup,
  type Subscription,
} from '@/types/entities'

const KEY_SEPARATOR = '::'

export interface PinnedEntry {
  key: string
  subscription: Subscription
  window: LimitWindowEntity
}

export interface PinnedGroupEntry {
  group: PinGroup
  members: PinnedEntry[]
}

export interface PinnedLayout {
  groups: PinnedGroupEntry[]
  standalone: PinnedEntry[]
}

// A subscription id is `provider:slug` and a window id can itself be
// `kind:scope`, so neither half is free of the separator on its own.
// Percent-encoding the left half is what makes the join reversible: no
// encoded subscription id can contain a colon, so the first `::` is
// always the real boundary however the window id is spelled.
export function pinMemberKey(subscriptionId: string, windowId: string): string {
  return `${encodeURIComponent(subscriptionId)}${KEY_SEPARATOR}${windowId}`
}

export function splitPinMemberKey(
  key: string,
): { subscriptionId: string; windowId: string } | null {
  const boundary = key.indexOf(KEY_SEPARATOR)
  if (boundary === -1) return null
  return {
    subscriptionId: decodeURIComponent(key.slice(0, boundary)),
    windowId: key.slice(boundary + KEY_SEPARATOR.length),
  }
}

export function isGroupColor(value: unknown): value is GroupColor {
  return typeof value === 'string' && (GROUP_COLORS as readonly string[]).includes(value)
}

// A new group takes the first palette colour no group is already
// wearing, so two groups only ever share one once the palette is used
// up, and then in palette order rather than at random.
export function nextGroupColor(groups: PinGroup[]): GroupColor {
  const taken = new Set(groups.map((group) => group.color))
  return (
    GROUP_COLORS.find((color) => !taken.has(color)) ??
    GROUP_COLORS[groups.length % GROUP_COLORS.length]
  )
}

export function collectPinnedEntries(subscriptions: Subscription[]): PinnedEntry[] {
  const entries: PinnedEntry[] = []
  for (const subscription of subscriptions) {
    for (const window of subscription.windows) {
      if (!subscription.pinnedWindowIds.includes(window.id)) continue
      entries.push({ key: pinMemberKey(subscription.id, window.id), subscription, window })
    }
  }
  return entries
}

export function sortPinGroups(groups: PinGroup[]): PinGroup[] {
  return groups
    .map((group, index) => ({ group, index }))
    .sort((a, b) => a.group.order - b.group.order || a.index - b.index)
    .map((entry) => entry.group)
}

// Every group is reported, members or not, so the panel can still show
// and dissolve one whose members were all unpinned.
export function layoutPinnedEntries(
  subscriptions: Subscription[],
  groups: PinGroup[],
): PinnedLayout {
  const entries = collectPinnedEntries(subscriptions)
  const byKey = new Map(entries.map((entry) => [entry.key, entry]))
  const ordered = sortPinGroups(groups)
  const claimed = new Set<string>()
  // Members come out in `memberKeys` order, which is the order the
  // customize screen dragged them into, not the order the
  // subscriptions happen to be listed in.
  const laidOut = ordered.map((group) => {
    const members = group.memberKeys
      .map((key) => byKey.get(key))
      .filter((entry): entry is PinnedEntry => entry !== undefined)
    for (const member of members) claimed.add(member.key)
    return { group, members }
  })
  return {
    groups: laidOut,
    standalone: entries.filter((entry) => !claimed.has(entry.key)),
  }
}

export function rollUpUsed(members: PinnedEntry[]): number | null {
  let worst: number | null = null
  for (const member of members) {
    const used = member.window.used
    if (typeof used !== 'number') continue
    if (worst === null || used > worst) worst = used
  }
  return worst
}
