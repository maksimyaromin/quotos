import * as React from "react";

/**
 * The Quotos popover shell — floating vibrancy surface, header toolbar, scrolling body, footer.
 * @startingPoint section="Shell" subtitle="The popover container with header and footer" viewport="380x420"
 */
export interface PanelProps {
  /** Popover title. Defaults to "Quotos". */
  title?: string;
  /** Show the macOS top beak pointing at the menu bar. */
  beak?: boolean;
  /** Toolbar nodes on the right of the header (refresh, add, settings IconButtons). */
  headerActions?: React.ReactNode;
  /** Optional footer content (e.g. an "Add subscription" button). */
  footer?: React.ReactNode;
  /** Max scroll height of the body before it scrolls (default 460). */
  maxBodyHeight?: number;
  /** The subscription rows. */
  children?: React.ReactNode;
  style?: React.CSSProperties;
}

/**
 * The Quotos popover shell — floating vibrancy surface, header toolbar, scrolling body, footer.
 */
export function Panel(props: PanelProps): React.ReactElement;
