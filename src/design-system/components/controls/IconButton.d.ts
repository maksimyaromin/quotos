import * as React from "react";

export interface IconButtonProps
  extends Omit<React.ButtonHTMLAttributes<HTMLButtonElement>, "children"> {
  /** Outer square size in px (default 24). The glyph is scaled to ~60%. */
  size?: number;
  /** Toggle-on look (teal). Used for the pin control. */
  active?: boolean;
  disabled?: boolean;
  /** Accessible label + tooltip — required, since there is no visible text. */
  label: string;
  onClick?: React.MouseEventHandler<HTMLButtonElement>;
  /** The glyph, typically a Lucide SVG that inherits currentColor. */
  children?: React.ReactNode;
}

/** Borderless square icon control for toolbar actions (refresh, close, expand, pin). */
export function IconButton(props: IconButtonProps): React.ReactElement;
