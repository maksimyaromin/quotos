import type { GroupColor, PinGroup, StatusItemSegment, Subscription } from '@/types/entities'
import {
  collectPinnedEntries,
  layoutPinnedEntries,
  type PinnedEntry,
  rollUpUsed,
} from './pin-groups'

function pickBaseFigureColor(used: number): StatusItemSegment['color'] {
  if (used >= 90) return 'red'
  if (used >= 75) return 'amber'
  return 'neutral'
}

function pickFigureColor(used: number, anyContributingStale: boolean): StatusItemSegment['color'] {
  return anyContributingStale ? 'amber' : pickBaseFigureColor(used)
}

function withFigure(entries: PinnedEntry[]): PinnedEntry[] {
  return entries.filter((entry) => typeof entry.window.used === 'number')
}

function subscriptionLabel(entry: PinnedEntry): string {
  return entry.subscription.labelOverride ?? entry.subscription.label
}

function windowLabel(entry: PinnedEntry): string {
  const scope = entry.window.scope ? ` (${entry.window.scope})` : ''
  return `${entry.window.name}${scope} ${entry.window.used}%`
}

interface Figure {
  used: number
  groupId: string | null
  groupColor: GroupColor | null
  cluster: string
}

export function buildStatusItemSegments(
  subscriptions: Subscription[],
  groups: PinGroup[] = [],
): StatusItemSegment[] {
  const layout = layoutPinnedEntries(subscriptions, groups)
  const anyContributingStale = withFigure(collectPinnedEntries(subscriptions)).some(
    (entry) => entry.subscription.state === 'behind',
  )

  // A rolled-up group spends one figure's width for however many members
  // it holds; opened out, it spends each member's own. Either way the
  // group is one cluster, so a break falls between groups and again
  // before the standalone pins.
  const figures: Figure[] = []
  for (const { group, members } of layout.groups) {
    const cluster = `group:${group.id}`
    if (group.collapsed) {
      const used = rollUpUsed(members)
      if (used === null) continue
      figures.push({ used, groupId: group.id, groupColor: group.color, cluster })
      continue
    }
    for (const entry of withFigure(members)) {
      figures.push({
        used: entry.window.used as number,
        groupId: group.id,
        groupColor: group.color,
        cluster,
      })
    }
  }
  for (const entry of withFigure(layout.standalone)) {
    figures.push({
      used: entry.window.used as number,
      groupId: null,
      groupColor: null,
      cluster: `subscription:${entry.subscription.id}`,
    })
  }

  let lastCluster: string | null = null
  return figures.map(({ used, groupId, groupColor, cluster }) => {
    const segment: StatusItemSegment = {
      text: `${used}%`,
      color: pickFigureColor(used, anyContributingStale),
      groupStart: lastCluster !== null && lastCluster !== cluster,
      groupId,
      groupColor,
    }
    lastCluster = cluster
    return segment
  })
}

export function buildStatusItemTooltip(
  subscriptions: Subscription[],
  groups: PinGroup[] = [],
): string {
  const layout = layoutPinnedEntries(subscriptions, groups)
  const lines: string[] = []

  for (const { group, members } of layout.groups) {
    const contributing = withFigure(members)
    if (contributing.length === 0) continue
    const figures = contributing.map(
      (entry) => `${subscriptionLabel(entry)} — ${windowLabel(entry)}`,
    )
    const stale = contributing.some((entry) => entry.subscription.state === 'behind')
    lines.push(`${group.name}: ${figures.join(' · ')}${stale ? ' — not current' : ''}`)
  }

  for (const sub of subscriptions) {
    const figures = withFigure(layout.standalone)
      .filter((entry) => entry.subscription.id === sub.id)
      .map(windowLabel)
    if (figures.length === 0) continue
    const stale = sub.state === 'behind' ? ' — not current' : ''
    lines.push(`${sub.labelOverride ?? sub.label}: ${figures.join(' · ')}${stale}`)
  }

  return lines.length === 0 ? 'Quotos' : ['Quotos', ...lines].join('\n')
}

export function computeWorstActiveLimitPercent(subscriptions: Subscription[]): number {
  let worst = 0
  for (const sub of subscriptions) {
    for (const w of sub.windows) {
      if (!w.isActive || typeof w.used !== 'number') continue
      if (w.used > worst) worst = w.used
    }
  }
  return worst
}
