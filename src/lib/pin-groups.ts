import type { LimitWindowEntity, PinGroup, Subscription } from '@/types/entities'

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

export function findGroupForMember(groups: PinGroup[], key: string): PinGroup | null {
  return groups.find((group) => group.memberKeys.includes(key)) ?? null
}

// Every group is reported, members or not, so the panel can still show
// and dissolve one whose members were all unpinned.
export function layoutPinnedEntries(
  subscriptions: Subscription[],
  groups: PinGroup[],
): PinnedLayout {
  const entries = collectPinnedEntries(subscriptions)
  const ordered = sortPinGroups(groups)
  const claimed = new Set<string>()
  const laidOut = ordered.map((group) => {
    const members = entries.filter((entry) => group.memberKeys.includes(entry.key))
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
