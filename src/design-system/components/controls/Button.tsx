import * as React from "react";

const base: React.CSSProperties = {
  display: "inline-flex",
  alignItems: "center",
  justifyContent: "center",
  gap: "var(--space-1-5)",
  fontFamily: "var(--font-sans)",
  fontWeight: "var(--weight-medium)",
  lineHeight: 1,
  border: "0.5px solid transparent",
  borderRadius: "var(--radius-sm)",
  cursor: "pointer",
  userSelect: "none",
  whiteSpace: "nowrap",
  // biome-ignore format: reducedMotion.spec.tsx scans this line for a --dur token, so it must stay on one line.
  transition: "background var(--dur-fast) var(--ease-standard), border-color var(--dur-fast) var(--ease-standard), opacity var(--dur-fast), transform var(--dur-instant)",
};

const sizes: Record<"sm" | "base" | "lg", React.CSSProperties> = {
  sm: { height: "22px", padding: "0 var(--space-2)", fontSize: "var(--text-sm)" },
  base: {
    height: "var(--control-height)",
    padding: "0 var(--space-3)",
    fontSize: "var(--text-base)",
  },
  lg: {
    height: "var(--control-height-lg)",
    padding: "0 var(--space-4)",
    fontSize: "var(--text-md)",
  },
};

const variants: Record<"primary" | "secondary" | "ghost" | "danger", React.CSSProperties> = {
  primary: {
    background: "var(--teal)",
    color: "var(--text-on-accent)",
    borderColor: "color-mix(in oklch, var(--teal) 70%, black)",
  },
  secondary: {
    background: "var(--bg-input)",
    color: "var(--text-primary)",
    borderColor: "var(--border-default)",
  },
  ghost: {
    background: "transparent",
    color: "var(--text-secondary)",
    borderColor: "transparent",
  },
  danger: {
    background: "transparent",
    color: "var(--red)",
    borderColor: "var(--border-default)",
  },
};

const hoverBackground: Record<"primary" | "secondary" | "ghost" | "danger", string> = {
  primary: "var(--teal-bright)",
  secondary: "var(--bg-row-hover)",
  ghost: "var(--bg-row-hover)",
  danger: "var(--red-muted)",
};

export interface ButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  /** Visual weight. `primary` for the one main action; `secondary` default; `ghost` for low-emphasis; `danger` for destructive. */
  variant?: "primary" | "secondary" | "ghost" | "danger";
  /** Control height. `base` = macOS small control (26px). */
  size?: "sm" | "base" | "lg";
  disabled?: boolean;
  /** Stretch to the container width, used for the primary action in the add-subscription sheet. */
  fullWidth?: boolean;
  /** Optional 14px leading icon node, typically a Lucide SVG. */
  icon?: React.ReactNode;
  onClick?: React.MouseEventHandler<HTMLButtonElement>;
  children?: React.ReactNode;
}

/** The standard Quotos action button. Sentence-case, verb-first labels. */
export function Button({
  variant = "secondary",
  size = "base",
  disabled = false,
  fullWidth = false,
  icon = null,
  onClick,
  children,
  style,
  ...rest
}: ButtonProps) {
  const [hover, setHover] = React.useState(false);
  const [active, setActive] = React.useState(false);

  const composed: React.CSSProperties = {
    ...base,
    ...sizes[size],
    ...variants[variant],
    ...(hover && !disabled ? { background: hoverBackground[variant] } : null),
    ...(active && !disabled ? { transform: "translateY(0.5px)", opacity: 0.9 } : null),
    ...(disabled ? { opacity: 0.4, cursor: "default" } : null),
    ...(fullWidth ? { width: "100%" } : null),
    ...style,
  };

  return (
    <button
      type="button"
      disabled={disabled}
      onClick={disabled ? undefined : onClick}
      onMouseEnter={() => setHover(true)}
      onMouseLeave={() => {
        setHover(false);
        setActive(false);
      }}
      onMouseDown={() => setActive(true)}
      onMouseUp={() => setActive(false)}
      style={composed}
      {...rest}
    >
      {icon ? <span style={{ display: "inline-flex", width: 14, height: 14 }}>{icon}</span> : null}
      {children}
    </button>
  );
}
