import React from "react";

const TONES = {
  neutral: { color: "var(--text-tertiary)", bg: "var(--bg-elevated)", border: "var(--border-default)" },
  accent: { color: "var(--text-accent)", bg: "var(--teal-muted)", border: "transparent" },
  warn: { color: "var(--amber)", bg: "var(--amber-muted)", border: "transparent" },
  danger: { color: "var(--red)", bg: "var(--red-muted)", border: "transparent" },
  info: { color: "var(--blue)", bg: "var(--blue-muted)", border: "transparent" },
};

/** A small pill for a short status word or a window scope tag. Sentence-case,
 *  never louder than it needs to be. */
export function Badge({ tone = "neutral", children, style }) {
  const t = TONES[tone] || TONES.neutral;
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
