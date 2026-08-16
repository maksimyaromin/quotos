import type * as React from "react";

const TONES: Record<
  "neutral" | "accent" | "warn" | "danger" | "info",
  { color: string; bg: string; border: string }
> = {
  neutral: {
    color: "var(--text-tertiary)",
    bg: "var(--bg-elevated)",
    border: "var(--border-default)",
  },
  accent: { color: "var(--text-accent)", bg: "var(--teal-muted)", border: "transparent" },
  warn: { color: "var(--amber)", bg: "var(--amber-muted)", border: "transparent" },
  danger: { color: "var(--red)", bg: "var(--red-muted)", border: "transparent" },
  info: { color: "var(--blue)", bg: "var(--blue-muted)", border: "transparent" },
};

export interface BadgeProps {
  /** Muted color family. `neutral` for scope tags, the semantic tones for states. */
  tone?: "neutral" | "accent" | "warn" | "danger" | "info";
  children?: React.ReactNode;
  style?: React.CSSProperties;
}

/** A small pill for a status word such as "Behind" or "Broken", or a window scope tag such as "Opus". */
export function Badge({ tone = "neutral", children, style }: BadgeProps) {
  const t = TONES[tone];
  return (
    <span
      style={{
        display: "inline-flex",
        alignItems: "center",
        height: "16px",
        padding: "0 var(--space-1-5)",
        fontFamily: "var(--font-sans)",
        fontSize: "var(--text-xs)",
        fontWeight: "var(--weight-medium)",
        lineHeight: 1,
        color: t.color,
        background: t.bg,
        border: `0.5px solid ${t.border}`,
        borderRadius: "var(--radius-xs)",
        whiteSpace: "nowrap",
        ...style,
      }}
    >
      {children}
    </span>
  );
}
