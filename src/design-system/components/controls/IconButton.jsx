import React from "react";

/** A square, borderless icon control for toolbar-style actions (refresh, close,
 *  settings, expand). Monochrome; inherits currentColor. */
export function IconButton({
  size = 24,
  active = false,
  disabled = false,
  label,
  onClick,
  children,
  style,
  ...rest
}) {
  const [hover, setHover] = React.useState(false);
  const composed = {
    display: "inline-flex",
    alignItems: "center",
    justifyContent: "center",
    width: size,
    height: size,
    padding: 0,
    border: 0,
    borderRadius: "var(--radius-sm)",
    background: hover && !disabled ? "var(--bg-row-hover)" : "transparent",
    color: active ? "var(--text-accent)" : "var(--text-secondary)",
    cursor: disabled ? "default" : "pointer",
    opacity: disabled ? 0.35 : 1,
    transition: "background var(--dur-fast) var(--ease-standard), color var(--dur-fast)",
    ...style,
  };
  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      disabled={disabled}
      onClick={disabled ? undefined : onClick}
      onMouseEnter={() => setHover(true)}
      onMouseLeave={() => setHover(false)}
      style={composed}
      {...rest}
    >
      <span
        style={{
          display: "inline-flex",
          width: Math.round(size * 0.6),
          height: Math.round(size * 0.6),
        }}
      >
        {children}
      </span>
    </button>
  );
}
