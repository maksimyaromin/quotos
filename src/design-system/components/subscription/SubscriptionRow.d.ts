import * as React from "react";
import type { SubscriptionState } from "../indicators/StatusDot";
import type { LimitWindowProps } from "./LimitWindow";

/**
 * The core Quotos panel row: one subscription, state-driven, age always visible.
 * @startingPoint section="Subscription" subtitle="A subscription row across states" viewport="360x260"
 */
export interface SubscriptionRowProps {
  /** Renameable label from the provider. Two can look confusingly similar. */
  label: string;
  /** Who issued it, for example "Anthropic". */
  provider?: string;
  /** Account or plan qualifier shown beside the provider, for example "Personal · Max". */
  account?: string;
  state?: SubscriptionState;
  /** Headline percent consumed: the account-wide weekly window, not
   * simply the most-consumed one. Null for no-data states. */
  used?: number | null;
  /** Provider-computed from every window, not just the headline one.
   * Colors the headline number and bar; a 20%-headline account with an
   * 85%-used session still reads amber. */
  severity?: "healthy" | "warn" | "critical";
  /** Reset copy for the binding window, for example "Resets today at 4:05 PM". */
  resetLabel?: string | null;
  /** Relative age of the last successful read, for example "2 min ago". Always shown. */
  lastRead?: string | null;
  /** The variable detail list. Empty hides the expander. */
  windows?: LimitWindowProps[];
  /** Human reason for a no-data state such as broken, idle, or no limits yet. */
  reason?: string | null;
  /** State badge, classified by the caller: "Not current" for held-over
   * numbers, "Needs sign-in" only when signing in is genuinely the answer. */
  badge?: "Not current" | "Needs sign-in" | null;
  /** How many of this subscription's windows are currently pinned. An
   * indicator, not a control. Renders nothing at 0. */
  pinnedCount?: number;
  /** Whether the headline window specifically is pinned. Reflected and
   * toggled, via `onTogglePin`, by the "…" menu's "Show/Hide in menu bar". */
  headlinePinned?: boolean;
  expanded?: boolean;
  /** Whether this row's "…" menu is open. One row's menu open at a time, owned by the caller. */
  menuOpen?: boolean;
  /** Inline footer action label, for example "Try again" or "Open Claude Code". */
  actionLabel?: string | null;
  /** Visually inert but still labelled, for example mid rate-limit wait. */
  actionDisabled?: boolean;
  /** Overrides the computed "Read …" footer text, for example a rate-limit quiet note. */
  footerNote?: string | null;
  onAction?: () => void;
  /** Toggles the headline window's pin. The "…" menu item only. */
  onTogglePin?: () => void;
  /** Toggles a specific window's pin, called with that window's `id`.
   * Wired to each row in the expanded list's own pin button. */
  onToggleWindowPin?: (id: string) => void;
  onToggleExpand?: () => void;
  onToggleMenu?: () => void;
  /** Present enables the inline rename affordance from the "…" menu. Called with the new label, or `null` to clear back to the provider default. */
  onRename?: (nextLabel: string | null) => void;
  /** "Read now" menu item. An explicit one-off refresh, independent of the footer action. */
  onReadNow?: () => void;
  /** Whether "Move up" and "Move down" are possible for this row. An
   * impossible direction renders disabled, never hidden. */
  canMoveUp?: boolean;
  canMoveDown?: boolean;
  /** "Move up" and "Move down" menu items: reorder the panel's rows, and
   * with them the status item's digit order. */
  onMoveUp?: () => void;
  onMoveDown?: () => void;
  /** "Stop tracking" menu item. */
  onStopTracking?: () => void;
  /** A `claude setup-token` session is running for this account. Shows
   * the paste-code field in place of the reason text and hides the
   * caller's own action button. */
  signInInProgress?: boolean;
  /** Called with the pasted code on submit. */
  onSubmitSignInCode?: (code: string) => void;
  /** Cancels the in-progress sign-in. */
  onCancelSignIn?: () => void;
  style?: React.CSSProperties;
}

/**
 * The core Quotos panel row: one subscription, state-driven, age always visible.
 */
export function SubscriptionRow(props: SubscriptionRowProps): React.ReactElement;
