import * as React from "react";
import type { SubscriptionState } from "../indicators/StatusDot";
import type { LimitWindowProps } from "./LimitWindow";

/**
 * The core Quotos panel row — one subscription, state-driven, age always visible.
 * @startingPoint section="Subscription" subtitle="A subscription row across states" viewport="360x260"
 */
export interface SubscriptionRowProps {
  /** Renameable label from the provider — two can look confusingly similar. */
  label: string;
  /** Who issued it (e.g. "Anthropic"). */
  provider?: string;
  /** Account/plan qualifier shown beside the provider (e.g. "Personal · Max"). */
  account?: string;
  state?: SubscriptionState;
  /** Headline % consumed (the most-consumed active limit). Null for no-data states. */
  used?: number | null;
  /** Reset copy for the binding window, e.g. "Resets today at 4:05 PM". */
  resetLabel?: string | null;
  /** Relative age of the last successful read, e.g. "2 min ago". Always shown. */
  lastRead?: string | null;
  /** The variable detail list. Empty hides the expander. */
  windows?: LimitWindowProps[];
  /** Human reason for the broken state + the fix it implies. */
  reason?: string | null;
  pinned?: boolean;
  expanded?: boolean;
  /** Inline action for idle/broken (e.g. "Finish setup", "Retry"). */
  actionLabel?: string | null;
  /** Visually inert but still labelled — e.g. mid rate-limit wait. */
  actionDisabled?: boolean;
  /** Overrides the computed "Last read …" footer text, e.g. a rate-limit quiet note. */
  footerNote?: string | null;
  onAction?: () => void;
  onTogglePin?: () => void;
  onToggleExpand?: () => void;
  /** Present enables the inline rename affordance; called with the new label, or `null` to clear back to the provider default. */
  onRename?: (nextLabel: string | null) => void;
  /** Present enables the (confirm-to-remove) delete affordance. */
  onDelete?: () => void;
  style?: React.CSSProperties;
}

/**
 * The core Quotos panel row — one subscription, state-driven, age always visible.
 */
export function SubscriptionRow(props: SubscriptionRowProps): React.ReactElement;
