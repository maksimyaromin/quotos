import type * as React from "react";
import { joinClassNames } from "../../join-class-names";
import styles from "./button.module.css";

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
  className,
  ...rest
}: ButtonProps) {
  return (
    <button
      type="button"
      disabled={disabled}
      onClick={onClick}
      data-variant={variant}
      data-size={size}
      className={joinClassNames(styles.button, fullWidth && styles.fullWidth, className)}
      {...rest}
    >
      {icon ? <span className={styles.icon}>{icon}</span> : null}
      {children}
    </button>
  );
}
