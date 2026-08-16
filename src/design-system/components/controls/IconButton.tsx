import * as React from "react";

export interface IconButtonProps
  extends Omit<React.ButtonHTMLAttributes<HTMLButtonElement>, "children"> {
  /** Outer square size in pixels. Defaults to 24. The glyph is scaled to about 60% of it. */
  size?: number;
  /** Toggle-on look, teal, used for the pin control. */
  active?: boolean;
  disabled?: boolean;
  /** Accessible label and tooltip, required since there is no visible text. */
  label: string;
  onClick?: React.MouseEventHandler<HTMLButtonElement>;
  /** The glyph, typically a Lucide SVG that inherits currentColor. */
  children?: React.ReactNode;
}

/** Borderless square icon control for toolbar actions such as refresh, close, expand and pin. */
export function IconButton({
  size = 24,
  active = false,
  disabled = false,
  label,
  onClick,
  children,
  style,
  ...rest
}: IconButtonProps) {
  const [hover, setHover] = React.useState(false);
  const composed: React.CSSProperties = {
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
