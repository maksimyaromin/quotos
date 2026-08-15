import type { Subscription, TraySegment } from "../types/entities";

/** v4 design/NOTES.md §1: bar/number colour mirrors the row's own per-window
 * rule (`LimitWindow.jsx`'s `numberColor`) — neutral below 75%, amber from
 * 75, red from 90 — evaluated on *that window's own* `used`, not the
 * subscription's aggregate `severity`. Pinning is per-window now, so a
 * pinned session figure must read by its own number, not by whatever else
 * the same subscription happens to be doing. */
function baseFigureColor(used: number): TraySegment["color"] {
  if (used >= 90) return "red";
  if (used >= 75) return "amber";
  return "neutral";
}

/** Followup-2 (unchanged by v4): once any *contributing* pinned window's
 * subscription is stale (`state === "behind"`), every figure in the bar
 * reads amber — held-over numbers are one fact the whole bar shares, not a
 * per-account one. Scoped to subscriptions that actually contribute a
 * figure right now; a stale subscription with nothing pinned doesn't taint
 * the bar. */
function figureColor(used: number, anyContributingStale: boolean): TraySegment["color"] {
  return anyContributingStale ? "amber" : baseFigureColor(used);
}

/** v4 design/NOTES.md §1/§4: one segment per pinned *window*, in panel order
 * (`subscriptions`' own order) then provider order (each subscription's own
 * `windows` order — the same order the expanded row lists them in), with
 * `groupStart` marking the first segment of a new subscription's group so
 * the Rust side (`tray_render.rs`) knows where to draw the hairline. A pin
 * whose window no longer exists, or has no `used` value yet, contributes
 * nothing — never "!", never "…" (design/NOTES.md's "либо цифра, либо
 * ничего"). */
export function buildTraySegments(subscriptions: Subscription[]): TraySegment[] {
  const entries: { sub: Subscription; used: number }[] = [];
  for (const sub of subscriptions) {
    for (const w of sub.windows) {
      if (typeof w.used !== "number") continue;
      if (!sub.pinnedWindowIds.includes(w.id)) continue;
      entries.push({ sub, used: w.used });
    }
  }
  const anyContributingStale = entries.some((e) => e.sub.state === "behind");

  const segments: TraySegment[] = [];
  let lastSubId: string | null = null;
  for (const { sub, used } of entries) {
    segments.push({
      text: `${used}%`,
      color: figureColor(used, anyContributingStale),
      groupStart: lastSubId !== null && lastSubId !== sub.id,
    });
    lastSubId = sub.id;
  }
  return segments;
}

/** v4 design/NOTES.md §1: the bare glyph's own arc — always filled to the
 * worst *active* limit across everything tracked (not just pinned windows,
 * and not inactive ones — deliberately narrower than `severity`, which
 * counts every window). Returns 0 (empty ring) when nothing tracked has any
 * active, numeric window yet. */
export function worstActiveLimitPercent(subscriptions: Subscription[]): number {
  let worst = 0;
  for (const sub of subscriptions) {
    for (const w of sub.windows) {
      if (!w.isActive || typeof w.used !== "number") continue;
      if (w.used > worst) worst = w.used;
    }
  }
  return worst;
}
