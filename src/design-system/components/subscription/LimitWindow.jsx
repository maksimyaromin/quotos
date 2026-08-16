import { Badge } from "../indicators/Badge.jsx";
import { CapacityBar } from "../indicators/CapacityBar.jsx";

/** One limit window inside a subscription's detail list. A window may lack
 *  a percentage or a reset time, and its name is the provider's own
 *  wording, rendered verbatim and possibly truncated. Percentages mean
 *  consumed, never remaining. */
// Mirrors SubscriptionRow's headline-number rule: red from 90%, amber from
// 75%, neutral below, evaluated on this window's own `used` since there is
// no subscription-wide `severity` to read from here.
function numberColor(used) {
  if (used >= 90) return "var(--red)";
  if (used >= 75) return "var(--amber)";
  return "var(--text-primary)";
}

// Filled means pinned to the menu bar, ghost means not. Matches
// SubscriptionRow's own PinGlyph exactly, kept local here since these two
// files share no module of their own.
const PinGlyph = () => (
  <svg
    width="12"
    height="12"
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    strokeWidth="1.8"
    strokeLinecap="round"
    strokeLinejoin="round"
  >
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
    <div
      style={{
        display: "flex",
        flexDirection: "column",
        gap: "var(--space-1)",
        padding: "var(--space-1-5) 0",
        ...style,
      }}
    >
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
            flex: "0 0 auto",
            display: "inline-flex",
            alignItems: "center",
            justifyContent: "center",
            width: 20,
            height: 20,
            padding: 0,
            border: 0,
            borderRadius: "var(--radius-xs)",
            background: pinned ? "var(--bg-selected)" : "transparent",
            color: pinned ? "var(--text-accent)" : "var(--text-quaternary)",
            cursor: "pointer",
          }}
        >
          <PinGlyph />
        </button>
        <span
          style={{
            flex: 1,
            minWidth: 0,
            fontFamily: "var(--font-sans)",
            fontSize: "var(--text-sm)",
            color: "var(--text-secondary)",
            overflow: "hidden",
            textOverflow: "ellipsis",
            whiteSpace: "nowrap",
          }}
        >
          {name}
        </span>
        {scope ? (
          // text-overflow only applies to block containers, and Badge is
          // itself a flex container, so overflow:hidden on it hard-clips
          // the tag mid-character with no ellipsis. The badge keeps only
          // the layout constraints, and an inner block span owns the
          // truncation.
          <Badge tone="neutral" style={{ flex: "0 1 auto", minWidth: 0, maxWidth: 120 }}>
            <span
              style={{
                display: "block",
                minWidth: 0,
                overflow: "hidden",
                textOverflow: "ellipsis",
                whiteSpace: "nowrap",
              }}
            >
              {scope}
            </span>
          </Badge>
        ) : null}
        <span
          style={{
            fontFamily: "var(--font-mono)",
            fontSize: "var(--numeral-sm)",
            fontWeight: "var(--weight-medium)",
            fontVariantNumeric: "tabular-nums",
            color: hasPct ? numberColor(used) : "var(--text-quaternary)",
            minWidth: 34,
            textAlign: "right",
          }}
        >
          {hasPct ? `${used}%` : "—"}
        </span>
      </div>
      {hasPct ? <CapacityBar used={used} stale={stale} height="3px" /> : null}
      {resetLabel ? (
        <span
          style={{
            fontFamily: "var(--font-sans)",
            fontSize: "var(--text-xs)",
            color: "var(--text-tertiary)",
            paddingLeft: 26,
          }}
        >
          {resetLabel}
        </span>
      ) : null}
    </div>
  );
}
