import * as React from "react";

export interface BadgeProps {
  /** Muted color family. `neutral` for scope tags, the semantic tones for states. */
  tone?: "neutral" | "accent" | "warn" | "danger" | "info";
  children?: React.ReactNode;
  style?: React.CSSProperties;
}

/** A small pill for a status word such as "Behind" or "Broken", or a window scope tag such as "Opus". */
export function Badge(props: BadgeProps): React.ReactElement;
