import React from "react";

const base = {
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
  // biome-ignore format: reducedMotion.spec.jsx scans this line for a --dur token, so it must stay on one line.
  transition: "background var(--dur-fast) var(--ease-standard), border-color var(--dur-fast) var(--ease-standard), opacity var(--dur-fast), transform var(--dur-instant)",
};

const sizes = {
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

const variants = {
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

/** A labeled action button using sentence-case, verb-first labels. */
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
}) {
  const [hover, setHover] = React.useState(false);
  const [active, setActive] = React.useState(false);

  const hoverBg = {
    primary: "var(--teal-bright)",
    secondary: "var(--bg-row-hover)",
    ghost: "var(--bg-row-hover)",
    danger: "var(--red-muted)",
  }[variant];

  const composed = {
    ...base,
    ...sizes[size],
    ...variants[variant],
    ...(hover && !disabled ? { background: hoverBg } : null),
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
