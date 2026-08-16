import type * as React from "react";
import { joinClassNames } from "../../joinClassNames";
import styles from "./badge.module.css";

export interface BadgeProps {
  /** Muted color family. `neutral` for scope tags, the semantic tones for states. */
  tone?: "neutral" | "accent" | "warn" | "danger" | "info";
  children?: React.ReactNode;
  className?: string;
  style?: React.CSSProperties;
}

/** A small pill for a status word such as "Behind" or "Broken", or a window scope tag such as "Opus". */
export function Badge({ tone = "neutral", children, className, style }: BadgeProps) {
  return (
    <span className={joinClassNames(styles.badge, className)} data-tone={tone} style={style}>
      {children}
    </span>
  );
}
