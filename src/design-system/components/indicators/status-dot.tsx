import type * as React from "react";
import { joinClassNames } from "../../join-class-names";
import styles from "./status-dot.module.css";

export type SubscriptionState = "idle" | "connecting" | "working" | "reading" | "behind" | "broken";

export interface StatusDotProps {
  state?: SubscriptionState;
  size?: number;
  className?: string;
  style?: React.CSSProperties;
}

export function StatusDot({ state = "working", size = 7, className, style }: StatusDotProps) {
  return (
    <span
      className={joinClassNames(styles.dot, className)}
      data-state={state}
      style={{ width: size, height: size, ...style }}
    />
  );
}
