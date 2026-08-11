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
  /** Headline % remaining (100 − most-consumed active limit). Null for no-data states. */
  remaining?: number | null;
  /** Reset copy for the binding window, e.g. "resets in 3h". */
  resetLabel?: string | null;
  /** Relative age of the last successful read, e.g. "2 min ago". Always shown. */
  lastRead?: string | null;
  /** The variable detail list. Empty hides the expander. */
  windows?: LimitWindowProps[];
  /** Human reason for the broken state + the fix it implies. */
  reason?: string | null;
  pinned?: boolean;
  expanded?: boolean;
  /** Inline action for idle/broken (e.g. "Finish setup", "Reconnect"). */
  actionLabel?: string | null;
  onAction?: () => void;
  onTogglePin?: () => void;
  onToggleExpand?: () => void;
  style?: React.CSSProperties;
}

/**
 * The core Quotos panel row — one subscription, state-driven, age always visible.
 */
export function SubscriptionRow(props: SubscriptionRowProps): React.ReactElement;
