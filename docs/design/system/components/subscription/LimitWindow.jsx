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

// v4 docs/design/NOTES.md §2/§4: each window gets its own leading pin button —
// same 12x12-in-20x20 glyph/button the row header used to show only when
// pinned, now always present so nothing appears or shifts on hover (the
// existing SubscriptionRow rule). Filled (teal) = in the menu bar; ghost
// (quaternary) = not. Matches the row header's own PinGlyph exactly — kept
// local here (not imported) since these two files have no shared module of
// their own and each is a design-system leaf.
const PinGlyph = () => (
  <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8"
    strokeLinecap="round" strokeLinejoin="round">
    <path d="M12 17v5M9 10.76V4a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v6.76a2 2 0 0 0 .59 1.41l1.3 1.3A1 1 0 0 1 17.18 15H6.82a1 1 0 0 1-.7-1.71l1.29-1.32A2 2 0 0 0 9 10.76Z" />
  </svg>
);

export function LimitWindow({
  id,
  name,
  used = null,
  resetLabel = null,
  scope = null,
  stale = false,
  pinned = false,
  onTogglePin,
  style,
}) {
  const hasPct = typeof used === "number";
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: "var(--space-1)", padding: "var(--space-1-5) 0", ...style }}>
      <div style={{ display: "flex", alignItems: "center", gap: "var(--space-1-5)" }}>
        <button
          type="button"
          title={pinned ? "Remove from menu bar" : "Show in menu bar"}
          aria-label={pinned ? "Remove from menu bar" : "Show in menu bar"}
          aria-pressed={pinned}
          onClick={(e) => {
            e.stopPropagation();
            onTogglePin?.(id);
          }}
          style={{
            flex: "0 0 auto", display: "inline-flex", alignItems: "center", justifyContent: "center",
            width: 20, height: 20, padding: 0, border: 0,
            borderRadius: "var(--radius-xs)",
            background: pinned ? "var(--bg-selected)" : "transparent",
            color: pinned ? "var(--text-accent)" : "var(--text-quaternary)",
            cursor: "pointer",
          }}
        >
          <PinGlyph />
        </button>
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
          // `text-overflow` only applies to block containers, and the Badge is
          // itself a flex container — on it, overflow:hidden hard-clips the tag
          // mid-character with no "…" ever drawn (verified live; R3-2's note
          // claiming otherwise mistook the clip for an ellipsis). So the badge
          // keeps only the layout constraints and an inner block span owns the
          // truncation.
          <Badge tone="neutral" style={{ flex: "0 1 auto", minWidth: 0, maxWidth: 120 }}>
            <span style={{ display: "block", minWidth: 0, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
              {scope}
            </span>
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
        <span style={{ fontFamily: "var(--font-sans)", fontSize: "var(--text-xs)", color: "var(--text-tertiary)", paddingLeft: 26 }}>
          {resetLabel}
        </span>
      ) : null}
    </div>
  );
}
