import React from "react";
import { CapacityBar } from "../indicators/CapacityBar.jsx";
import { Badge } from "../indicators/Badge.jsx";

/** One limit window inside a subscription's detail list. The list is variable:
 *  a window may lack a percentage or a reset time, and its name is the
 *  provider's own wording (rendered verbatim, possibly truncated). Renders
 *  gracefully whether there are 1 or 8 of these. Consumed, not remaining (I2). */
// D7/D8: "Bar/number colour: <75% healthy, 75%+ warn, 90%+ critical — the
// number itself is only tinted from 75% up, below that it's neutral." The
// bar (CapacityBar, below) already applied this per-window via its own
// `capacityColor`; the adjacent percent number didn't (CLAUDE.md's own note
// only covers the bar) and was always `--text-primary` regardless of value
// — this mirrors SubscriptionRow's identical headline-number rule (`amber`
// from warn, `red` from critical, neutral text below that) for this
// per-window case, where there's no subscription-wide `severity` to read
// from, only this window's own `used`.
function numberColor(used) {
  if (used >= 90) return "var(--red)";
  if (used >= 75) return "var(--amber)";
  return "var(--text-primary)";
}

export function LimitWindow({ name, used = null, resetLabel = null, scope = null, stale = false, style }) {
  const hasPct = typeof used === "number";
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: "var(--space-1)", padding: "var(--space-1-5) 0", ...style }}>
      <div style={{ display: "flex", alignItems: "center", gap: "var(--space-1-5)" }}>
        <span style={{
          flex: 1,
          minWidth: 0,
          fontFamily: "var(--font-sans)",
          fontSize: "var(--text-sm)",
          color: "var(--text-secondary)",
          overflow: "hidden",
          textOverflow: "ellipsis",
          whiteSpace: "nowrap",
        }}>{name}</span>
        {scope ? (
          <Badge tone="neutral" style={{ flex: "0 1 auto", minWidth: 0, maxWidth: 120, overflow: "hidden", textOverflow: "ellipsis" }}>
            {scope}
          </Badge>
        ) : null}
        <span style={{
          fontFamily: "var(--font-mono)",
          fontSize: "var(--numeral-sm)",
          fontWeight: "var(--weight-medium)",
          fontVariantNumeric: "tabular-nums",
          color: hasPct ? numberColor(used) : "var(--text-quaternary)",
          minWidth: 34,
          textAlign: "right",
        }}>{hasPct ? `${used}%` : "—"}</span>
      </div>
      {hasPct ? <CapacityBar used={used} stale={stale} height="3px" /> : null}
      {resetLabel ? (
        <span style={{ fontFamily: "var(--font-sans)", fontSize: "var(--text-xs)", color: "var(--text-tertiary)" }}>
          {resetLabel}
        </span>
      ) : null}
    </div>
  );
}
