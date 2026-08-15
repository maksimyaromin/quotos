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
  /** Headline % consumed — the account-wide weekly window, not simply the
   * most-consumed one. Null for no-data states. */
  used?: number | null;
  /** "healthy" | "warn" | "critical" — provider-computed from *every*
   * window, not just the headline one. Colors the headline number and bar;
   * a 20%-headline account with an 85%-used session still reads amber. */
  severity?: "healthy" | "warn" | "critical";
  /** Reset copy for the binding window, e.g. "Resets today at 4:05 PM". */
  resetLabel?: string | null;
  /** Relative age of the last successful read, e.g. "2 min ago". Always shown. */
  lastRead?: string | null;
  /** The variable detail list. Empty hides the expander. */
  windows?: LimitWindowProps[];
  /** Human reason for a no-data state (broken/idle/no-limits-yet). */
  reason?: string | null;
  /** State badge, classified by the caller — "Not current" for held-over
   * numbers, "Needs sign-in" only when signing in is genuinely the answer. */
  badge?: "Not current" | "Needs sign-in" | null;
  pinned?: boolean;
  expanded?: boolean;
  /** Whether this row's "…" menu is open — one row's menu open at a time, owned by the caller. */
  menuOpen?: boolean;
  /** Inline footer action label — "Try again" (behind) or "Open Claude Code" (broken). */
  actionLabel?: string | null;
  /** Visually inert but still labelled — e.g. mid rate-limit wait. */
  actionDisabled?: boolean;
  /** Overrides the computed "Read …" footer text, e.g. a rate-limit quiet note. */
  footerNote?: string | null;
  onAction?: () => void;
  onTogglePin?: () => void;
  onToggleExpand?: () => void;
  onToggleMenu?: () => void;
  /** Present enables the inline rename affordance (from the "…" menu); called with the new label, or `null` to clear back to the provider default. */
  onRename?: (nextLabel: string | null) => void;
  /** "Read now" menu item — an explicit one-off refresh, independent of the footer action. */
  onReadNow?: () => void;
  /** "Stop tracking" menu item. */
  onStopTracking?: () => void;
  /** R2-6: a `claude setup-token` session is running for this account —
   * shows the paste-code field in place of the reason text and hides the
   * caller's own action button. */
  signInInProgress?: boolean;
  /** Called with the pasted code on submit. */
  onSubmitSignInCode?: (code: string) => void;
  /** Cancels the in-progress sign-in. */
  onCancelSignIn?: () => void;
  style?: React.CSSProperties;
}

/**
 * The core Quotos panel row — one subscription, state-driven, age always visible.
 */
export function SubscriptionRow(props: SubscriptionRowProps): React.ReactElement;
