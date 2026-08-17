import type { StatuslineFeedWire, StatuslineWindowWire } from '@/types/entities'

function isoFromEpochSeconds(sec: number | null | undefined): string | null {
  if (typeof sec !== 'number' || !Number.isFinite(sec)) return null
  return new Date(sec * 1000).toISOString()
}

function patchWindow(
  window: Record<string, unknown>,
  percentKey: 'percent' | 'utilization',
  feedWindow: StatuslineWindowWire,
): Record<string, unknown> {
  const patched = { ...window, [percentKey]: feedWindow.used_percentage }
  const resets = isoFromEpochSeconds(feedWindow.resets_at)
  // Only overwritten when present, so a feed entry missing a reset time
  // keeps the API's own reset time instead of blanking it.
  if (resets !== null) patched.resets_at = resets
  return patched
}

export function reconcileWithStatusline(
  usageRaw: unknown,
  apiFetchedAtIso: string,
  feed: StatuslineFeedWire | null | undefined,
): unknown {
  if (!feed?.rate_limits) return usageRaw
  if (usageRaw === null || typeof usageRaw !== 'object') return usageRaw

  const feedTime = Date.parse(feed.written_at)
  const apiTime = Date.parse(apiFetchedAtIso)
  if (!Number.isFinite(feedTime) || !Number.isFinite(apiTime) || feedTime <= apiTime)
    return usageRaw

  const usage: Record<string, unknown> = { ...(usageRaw as Record<string, unknown>) }
  const fiveHour = feed.rate_limits.five_hour
  const sevenDay = feed.rate_limits.seven_day

  const rawLimits = usage.limits
  if (Array.isArray(rawLimits) && rawLimits.length > 0) {
    usage.limits = rawLimits.map((entry) => {
      if (entry === null || typeof entry !== 'object') return entry
      const limit = entry as Record<string, unknown>
      if (limit.kind === 'session' && fiveHour) {
        return patchWindow(limit, 'percent', fiveHour)
      }
      if (limit.kind === 'weekly_all' && sevenDay) {
        return patchWindow(limit, 'percent', sevenDay)
      }
      return limit
    })
    return usage
  }

  if (fiveHour && usage.five_hour && typeof usage.five_hour === 'object') {
    usage.five_hour = patchWindow(
      usage.five_hour as Record<string, unknown>,
      'utilization',
      fiveHour,
    )
  }
  if (sevenDay && usage.seven_day && typeof usage.seven_day === 'object') {
    usage.seven_day = patchWindow(
      usage.seven_day as Record<string, unknown>,
      'utilization',
      sevenDay,
    )
  }
  return usage
}
