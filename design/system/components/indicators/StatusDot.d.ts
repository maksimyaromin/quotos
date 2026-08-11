import * as React from "react";

export type SubscriptionState =
  | "idle"        // not connected
  | "connecting"
  | "working"
  | "reading"     // refresh in flight
  | "behind"      // stale data held
  | "repairing"
  | "broken"
  | "waiting";    // waiting on limits (provider rate-limited us)

export interface StatusDotProps {
  state?: SubscriptionState;
  /** Diameter in px (default 7). */
  size?: number;
  style?: React.CSSProperties;
}

/** A small colored state dot. In-progress states pulse; "working" is calm teal, not a loud green. */
export function StatusDot(props: StatusDotProps): React.ReactElement;
