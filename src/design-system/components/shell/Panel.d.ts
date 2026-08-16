import * as React from "react";

/**
 * The Quotos popover shell — floating vibrancy surface, header toolbar, scrolling body, footer.
 * @startingPoint section="Shell" subtitle="The popover container with header and footer" viewport="380x420"
 */
export interface PanelProps {
  /** Popover title. Defaults to "Quotos". */
  title?: string;
  /** Docked under the tray icon (shows the beak) vs. torn off into a free-floating window. */
  docked?: boolean;
  /** Left edge (px, from the panel's own left edge) of the beak's 12px-wide base — dynamic, tracks the tray glyph's center. */
  beakLeft?: number;
  /** True while the header is being dragged (grabbing vs. grab cursor). */
  dragging?: boolean;
  /** Mousedown handler on the header — the drag/detach entry point; there is no detach button. */
  onHeaderPointerDown?: (event: React.MouseEvent) => void;
  /** Optional leading header node (e.g. the Subscriptions screen's back arrow). */
  leading?: React.ReactNode;
  /** Toolbar nodes on the right of the header (refresh, snap-back IconButtons). */
  headerActions?: React.ReactNode;
  /** Optional footer content (e.g. an "Add subscription" button). */
  footer?: React.ReactNode;
  /** Max scroll height of the body before it scrolls (default 452). */
  maxBodyHeight?: number;
  /** Fixed screen position while detached, for browser-harness dragging (native Tauri leaves this null — the OS window itself moves). */
  position?: { x: number; y: number } | null;
  /** The subscription rows. */
  children?: React.ReactNode;
  style?: React.CSSProperties;
}

/**
 * The Quotos popover shell — floating vibrancy surface, header toolbar, scrolling body, footer.
 */
export function Panel(props: PanelProps): React.ReactElement;
