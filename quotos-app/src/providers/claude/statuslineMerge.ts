import type { StatuslineFeedWire } from "../../types/entities";

function isoFromEpochSeconds(sec: number | null | undefined): string | null {
  if (typeof sec !== "number" || !Number.isFinite(sec)) return null;
  return new Date(sec * 1000).toISOString();
}

/** S2: reconciles the Claude Code statusline's zero-cost feed with the API
 * read it's arriving alongside — "freshest wins" (write-mechanism contract
 * point 6). Patches the raw `/api/oauth/usage` shape *before* it reaches
 * `normalizeUsage`, rather than merging the already-normalized window list,
 * so every downstream rule (headline selection, severity, window naming)
 * stays exactly as tested — a fresher statusline reading looks, to the rest
 * of the pipeline, exactly like a fresher API response would have.
 *
 * Two things keep this from ever "double-counting" (contract point 6):
 *  - It only ever *refreshes* a window the API response already asserts
 *    exists (`five_hour`/`session` and `seven_day`/`weekly_all` are patched
 *    in place); it never synthesizes a window the API reported as absent
 *    (`null`) or omitted. The statusline feed can't tell Quotos a window
 *    exists that the account's own API read says it doesn't.
 *  - It's a no-op whenever the feed isn't strictly newer than this very API
 *    read (`feedTime <= apiTime`), which is also what makes "no interactive
 *    session is feeding it" degrade to exactly today's behavior with no
 *    special-casing: an empty/stale/never-installed feed is indistinguishable
 *    from "not fresher", so this always returns `usageRaw` unchanged. */
export function reconcileWithStatusline(
  usageRaw: unknown,
  apiFetchedAtIso: string,
  feed: StatuslineFeedWire | null | undefined,
): unknown {
  if (!feed || !feed.rate_limits) return usageRaw;
  if (usageRaw === null || typeof usageRaw !== "object") return usageRaw;

  const feedTime = Date.parse(feed.written_at);
  const apiTime = Date.parse(apiFetchedAtIso);
  if (!Number.isFinite(feedTime) || !Number.isFinite(apiTime) || feedTime <= apiTime) return usageRaw;

  const usage: Record<string, unknown> = { ...(usageRaw as Record<string, unknown>) };
  const fiveHour = feed.rate_limits.five_hour;
  const sevenDay = feed.rate_limits.seven_day;

  const rawLimits = usage.limits;
  if (Array.isArray(rawLimits) && rawLimits.length > 0) {
    usage.limits = rawLimits.map((entry) => {
      if (entry === null || typeof entry !== "object") return entry;
      const limit = entry as Record<string, unknown>;
      if (limit.kind === "session" && fiveHour) {
        return { ...limit, percent: fiveHour.used_percentage, resets_at: isoFromEpochSeconds(fiveHour.resets_at) };
      }
      if (limit.kind === "weekly_all" && sevenDay) {
        return { ...limit, percent: sevenDay.used_percentage, resets_at: isoFromEpochSeconds(sevenDay.resets_at) };
      }
      return limit;
    });
    return usage;
  }

  if (fiveHour && usage.five_hour && typeof usage.five_hour === "object") {
    usage.five_hour = {
      ...(usage.five_hour as Record<string, unknown>),
      utilization: fiveHour.used_percentage,
      resets_at: isoFromEpochSeconds(fiveHour.resets_at),
    };
  }
  if (sevenDay && usage.seven_day && typeof usage.seven_day === "object") {
    usage.seven_day = {
      ...(usage.seven_day as Record<string, unknown>),
      utilization: sevenDay.used_percentage,
      resets_at: isoFromEpochSeconds(sevenDay.resets_at),
    };
  }
  return usage;
}
