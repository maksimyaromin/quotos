import * as React from "react";

export interface LimitWindowProps {
  /** The provider's own wording for this window — rendered verbatim, truncated if long. */
  name: string;
  /** Percent remaining 0–100, or null when the provider reports no percentage. */
  remaining?: number | null;
  /** Relative reset copy, e.g. "resets in 3h" — or null. */
  resetLabel?: string | null;
  /** Optional scope tag when the window applies to part of the subscription (e.g. "Opus 4"). */
  scope?: string | null;
  /** Dim when the parent subscription is behind. */
  stale?: boolean;
  style?: React.CSSProperties;
}

/** One row in a subscription's variable detail list. Handles missing percentage / reset gracefully. */
export function LimitWindow(props: LimitWindowProps): React.ReactElement;
