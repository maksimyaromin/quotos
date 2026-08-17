import type * as React from "react";
import { joinClassNames } from "../../join-class-names";
import styles from "./badge.module.css";

export interface BadgeProps {
  tone?: "neutral" | "accent" | "warn" | "danger" | "info";
  children?: React.ReactNode;
  className?: string;
  style?: React.CSSProperties;
}

export function Badge({ tone = "neutral", children, className, style }: BadgeProps) {
  return (
    <span className={joinClassNames(styles.badge, className)} data-tone={tone} style={style}>
      {children}
    </span>
  );
}
