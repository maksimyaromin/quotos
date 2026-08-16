import type * as React from "react";
import { joinClassNames } from "../../join-class-names";
import styles from "./icon-button.module.css";

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
  className,
  style,
  ...rest
}: IconButtonProps) {
  const glyphSize = Math.round(size * 0.6);
  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      disabled={disabled}
      onClick={onClick}
      data-active={active ? "true" : undefined}
      className={joinClassNames(styles.button, className)}
      style={{ width: size, height: size, ...style }}
      {...rest}
    >
      <span className={styles.glyph} style={{ width: glyphSize, height: glyphSize }}>
        {children}
      </span>
    </button>
  );
}
