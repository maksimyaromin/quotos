import * as React from "react";

/**
 * The standard Quotos action button. Sentence-case, verb-first labels.
 * @startingPoint section="Controls" subtitle="Buttons in every variant and size" viewport="700x150"
 */
export interface ButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  /** Visual weight. `primary` for the one main action; `secondary` default; `ghost` for low-emphasis; `danger` for destructive. */
  variant?: "primary" | "secondary" | "ghost" | "danger";
  /** Control height. `base` = macOS small control (26px). */
  size?: "sm" | "base" | "lg";
  disabled?: boolean;
  /** Stretch to the container width — used for the primary action in the add-subscription sheet. */
  fullWidth?: boolean;
  /** Optional 14px leading icon node (e.g. a Lucide SVG). */
  icon?: React.ReactNode;
  onClick?: React.MouseEventHandler<HTMLButtonElement>;
  children?: React.ReactNode;
}

/**
 * The standard Quotos action button. Sentence-case, verb-first labels.
 */
export function Button(props: ButtonProps): React.ReactElement;
