import type { StatusItemSegment, Subscription } from "../types/entities";

/** Must match `LimitWindow.jsx`'s `numberColor` thresholds. */
function pickBaseFigureColor(used: number): StatusItemSegment["color"] {
  if (used >= 90) return "red";
  if (used >= 75) return "amber";
  return "neutral";
}

/** A stale subscription only taints the bar if it currently contributes a
 * pinned figure. */
function pickFigureColor(used: number, anyContributingStale: boolean): StatusItemSegment["color"] {
  return anyContributingStale ? "amber" : pickBaseFigureColor(used);
}

/** One segment per pinned window, in panel order matching the expanded
 * row. A pin whose window no longer exists, or has no numeric value yet,
 * contributes nothing. */
export function buildStatusItemSegments(subscriptions: Subscription[]): StatusItemSegment[] {
  const entries: { sub: Subscription; used: number }[] = [];
  for (const sub of subscriptions) {
    for (const w of sub.windows) {
      if (typeof w.used !== "number") continue;
      if (!sub.pinnedWindowIds.includes(w.id)) continue;
      entries.push({ sub, used: w.used });
    }
  }
  const anyContributingStale = entries.some((e) => e.sub.state === "behind");

  const segments: StatusItemSegment[] = [];
  let lastSubId: string | null = null;
  for (const { sub, used } of entries) {
    segments.push({
      text: `${used}%`,
      color: pickFigureColor(used, anyContributingStale),
      groupStart: lastSubId !== null && lastSubId !== sub.id,
    });
    lastSubId = sub.id;
  }
  return segments;
}

/** Names every contributing figure for VoiceOver, since the status item's
 * digits alone carry no accessible text. "Quotos" alone when nothing
 * contributes. Applied verbatim by the native side, which never composes
 * tooltip text itself. */
export function buildStatusItemTooltip(subscriptions: Subscription[]): string {
  const lines: string[] = [];
  for (const sub of subscriptions) {
    const figures = sub.windows
      .filter((w) => typeof w.used === "number" && sub.pinnedWindowIds.includes(w.id))
      .map((w) => `${w.name}${w.scope ? ` (${w.scope})` : ""} ${w.used}%`);
    if (figures.length === 0) continue;
    const stale = sub.state === "behind" ? " — not current" : "";
    lines.push(`${sub.labelOverride ?? sub.label}: ${figures.join(" · ")}${stale}`);
  }
  return lines.length === 0 ? "Quotos" : ["Quotos", ...lines].join("\n");
}

/** Narrower than `severity`: only active windows count, and pinning does
 * not matter. Returns 0, an empty ring, when nothing qualifies. */
export function computeWorstActiveLimitPercent(subscriptions: Subscription[]): number {
  let worst = 0;
  for (const sub of subscriptions) {
    for (const w of sub.windows) {
      if (!w.isActive || typeof w.used !== "number") continue;
      if (w.used > worst) worst = w.used;
    }
  }
  return worst;
}
