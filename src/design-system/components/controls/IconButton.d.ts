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
export function IconButton(props: IconButtonProps): React.ReactElement;
