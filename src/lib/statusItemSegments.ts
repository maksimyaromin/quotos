import type { Subscription, TraySegment } from "../types/entities";

/** Bar and number color mirrors the row's own per-window rule in
 * `LimitWindow.jsx`'s `numberColor`: neutral below 75%, amber from 75%, red
 * from 90%. It is evaluated on that window's own `used`, not the
 * subscription's aggregate `severity`, so a pinned session figure reads by
 * its own number rather than by whatever else the same subscription is
 * doing. */
function pickBaseFigureColor(used: number): TraySegment["color"] {
  if (used >= 90) return "red";
  if (used >= 75) return "amber";
  return "neutral";
}

/** Once any contributing pinned window's subscription is stale, `state ===
 * "behind"`, every figure in the bar reads amber. Held-over numbers are
 * one fact the whole bar shares, not a per-account one. Scoped to
 * subscriptions that actually contribute a figure right now, so a stale
 * subscription with nothing pinned does not taint the bar. */
function pickFigureColor(used: number, anyContributingStale: boolean): TraySegment["color"] {
  return anyContributingStale ? "amber" : pickBaseFigureColor(used);
}

/** One segment per pinned window, in panel order, `subscriptions`' own
 * order, then provider order, each subscription's own `windows` order, the
 * same order the expanded row lists them in. `groupStart` marks the first
 * segment of a new subscription's group so the Rust side in
 * `tray_render.rs` knows where to draw the hairline. A pin whose window no
 * longer exists, or has no `used` value yet, contributes nothing: never a
 * placeholder character, only a real digit or an empty bar. */
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
      color: pickFigureColor(used, anyContributingStale),
      groupStart: lastSubId !== null && lastSubId !== sub.id,
    });
    lastSubId = sub.id;
  }
  return segments;
}

/** The tray digits carry no text VoiceOver can read, and with more than one
 * subscription pinned the bare figures do not say whose number is whose,
 * so the tooltip names every contributing figure, one line per
 * subscription, in the bar's own order. A stale subscription's line says
 * so in the row badge's own words, "Not current", since the bar-wide amber
 * rule stays a digits-only fact. With nothing contributing, the tooltip is
 * just the product name. Applied verbatim by the Rust side in `shell.rs`'s
 * `repaint_tray_icon`, which never composes tooltip text itself. */
export function buildTrayTooltip(subscriptions: Subscription[]): string {
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

/** The bare glyph's own arc is always filled to the worst active limit
 * across everything tracked, not just pinned windows and not inactive
 * ones. Deliberately narrower than `severity`, which counts every window.
 * Returns 0, an empty ring, when nothing tracked has any active, numeric
 * window yet. */
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
