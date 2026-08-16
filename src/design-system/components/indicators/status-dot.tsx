import type * as React from "react";
import { joinClassNames } from "../../join-class-names";
import styles from "./status-dot.module.css";

export type SubscriptionState =
  | "idle" // not connected
  | "connecting"
  | "working"
  | "reading" // refresh in flight
  | "behind" // stale data held
  | "broken";
// "Waiting on the rate budget" is deliberately not a health state. It is
// tracked separately as a rate-limit fact that never overrides the
// subscription's own health.

export interface StatusDotProps {
  state?: SubscriptionState;
  /** Diameter in pixels. Defaults to 7. */
  size?: number;
  className?: string;
  style?: React.CSSProperties;
}

/** A small colored state dot. In-progress states pulse. "working" is calm teal, not a loud green. */
export function StatusDot({ state = "working", size = 7, className, style }: StatusDotProps) {
  return (
    <span
      className={joinClassNames(styles.dot, className)}
      data-state={state}
      style={{ width: size, height: size, ...style }}
    />
  );
}
