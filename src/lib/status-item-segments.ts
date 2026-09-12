import type { PinGroup, StatusItemSegment, Subscription } from '@/types/entities'
import {
  collectPinnedEntries,
  groupSlug,
  layoutPinnedEntries,
  type PinnedEntry,
  splitPinMemberKey,
} from './pin-groups'

type FigureColor = Extract<StatusItemSegment, { kind: 'figure' }>['color']

function pickBaseFigureColor(used: number): FigureColor {
  if (used >= 90) return 'red'
  if (used >= 75) return 'amber'
  return 'neutral'
}

function pickFigureColor(used: number, anyContributingStale: boolean): FigureColor {
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

export function buildStatusItemSegments(
  subscriptions: Subscription[],
  groups: PinGroup[] = [],
): StatusItemSegment[] {
  const layout = layoutPinnedEntries(subscriptions, groups)
  const anyContributingStale = withFigure(collectPinnedEntries(subscriptions)).some(
    (entry) => entry.subscription.state === 'behind',
  )

  const segments: StatusItemSegment[] = []
  // A group is its chip: rolled up, that chip is the whole of what it
  // draws, and the numbers live in the tooltip until it is opened. A
  // group with nothing to report draws nothing at all, chip included,
  // since there would be no figures behind it to open out to.
  for (const { group, members } of layout.groups) {
    const contributing = withFigure(members)
    const slug = groupSlug(group.name)
    if (contributing.length === 0 || slug.length === 0) continue
    segments.push({ kind: 'chip', slug, color: group.color, groupId: group.id })
    if (group.collapsed) continue
    for (const entry of contributing) {
      segments.push(figureSegment(entry.window.used as number, anyContributingStale))
    }
  }

  for (const entry of withFigure(layout.standalone)) {
    segments.push(figureSegment(entry.window.used as number, anyContributingStale))
  }

  return segments
}

function figureSegment(used: number, anyContributingStale: boolean): StatusItemSegment {
  return {
    kind: 'figure',
    text: `${used}%`,
    color: pickFigureColor(used, anyContributingStale),
  }
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

function chosenWindowPercent(subscriptions: Subscription[], source: string): number | null {
  const split = splitPinMemberKey(source)
  if (split === null) return null
  const used = subscriptions
    .find((sub) => sub.id === split.subscriptionId)
    ?.windows.find((window) => window.id === split.windowId)?.used
  return typeof used === 'number' ? used : null
}

// How full the menu bar mark's limit chevron is drawn. One designated
// window drives it when one is chosen; otherwise it is the arithmetic
// mean of every tracked subscription's own headline figure, which is a
// blunt summary but the one the mark defaults to. A chosen window that
// no longer reports a figure falls back to that mean rather than
// freezing on its last value.
export function computeIconFillPercent(
  subscriptions: Subscription[],
  source: string | null = null,
): number {
  const chosen = source === null ? null : chosenWindowPercent(subscriptions, source)
  if (chosen !== null) return chosen

  const headlines = subscriptions
    .map((sub) => sub.used)
    .filter((used): used is number => typeof used === 'number')
  if (headlines.length === 0) return 0
  return Math.round(headlines.reduce((sum, used) => sum + used, 0) / headlines.length)
}
