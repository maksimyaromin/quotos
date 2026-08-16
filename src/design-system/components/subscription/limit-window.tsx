import type * as React from "react";
import { Badge } from "../indicators/badge";
import { CapacityBar } from "../indicators/capacity-bar";
import styles from "./limit-window.module.css";

function numberLevel(used: number): "critical" | "warn" | "neutral" {
  if (used >= 90) return "critical";
  if (used >= 75) return "warn";
  return "neutral";
}

// Filled means pinned to the menu bar, ghost means not. Matches
// SubscriptionRow's own PinGlyph exactly, kept local here since these two
// files share no module of their own.
function PinGlyph() {
  return (
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
}

export interface LimitWindowProps {
  /** Stable id for this window, see `LimitWindowEntity.id`. Passed back to `onTogglePin`. */
  id?: string;
  /** The provider's own wording for this window, rendered verbatim and truncated if long. */
  name: string;
  /** Percent consumed 0 to 100, or null when the provider reports no percentage. */
  used?: number | null;
  /** Exact reset copy, for example "Resets today at 4:05 PM", or null. */
  resetLabel?: string | null;
  /** Optional scope tag when the window applies to part of the subscription, for example "Opus 4". */
  scope?: string | null;
  /** Dim when the parent subscription is behind. */
  stale?: boolean;
  /** Whether this window's own figure is in the menu bar. Always renders
   * its pin button, leading the row, regardless of this value. */
  pinned?: boolean;
  /** Called with this window's `id` when its pin button is clicked. */
  onTogglePin?: (id: string) => void;
  style?: React.CSSProperties;
}

/** One row in a subscription's variable detail list. Handles a missing percentage or reset time gracefully. */
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
}: LimitWindowProps) {
  const hasPct = typeof used === "number";
  return (
    <div className={styles.row} style={style}>
      <div className={styles.header}>
        <button
          type="button"
          title={pinned ? "Remove from menu bar" : "Show in menu bar"}
          aria-label={pinned ? "Remove from menu bar" : "Show in menu bar"}
          aria-pressed={pinned}
          onClick={(e) => {
            e.stopPropagation();
            if (id !== undefined) onTogglePin?.(id);
          }}
          data-pinned={pinned ? "true" : undefined}
          className={styles.pinButton}
        >
          <PinGlyph />
        </button>
        <span className={styles.name}>{name}</span>
        {scope ? (
          <Badge tone="neutral" className={styles.scopeBadge}>
            <span className={styles.scopeInner}>{scope}</span>
          </Badge>
        ) : null}
        <span className={styles.value} data-level={hasPct ? numberLevel(used) : undefined}>
          {hasPct ? `${used}%` : "—"}
        </span>
      </div>
      {hasPct ? <CapacityBar used={used} stale={stale} height="3px" /> : null}
      {resetLabel ? <span className={styles.resetLabel}>{resetLabel}</span> : null}
    </div>
  );
}
