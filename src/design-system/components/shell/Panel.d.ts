import * as React from "react";

/**
 * The Quotos popover shell: floating vibrancy surface, header toolbar, scrolling body, footer.
 * @startingPoint section="Shell" subtitle="The popover container with header and footer" viewport="380x420"
 */
export interface PanelProps {
  /** Popover title. Defaults to "Quotos". */
  title?: string;
  /** Docked under the status item, showing the beak, or torn off into a free-floating window. */
  docked?: boolean;
  /** Left edge in pixels from the panel's own left edge of the beak's 12px-wide base. Tracks the status item glyph's center. */
  beakLeft?: number;
  /** True while the header is being dragged. */
  dragging?: boolean;
  /** Mousedown handler on the header, the drag and detach entry point. There is no detach button. */
  onHeaderPointerDown?: (event: React.MouseEvent) => void;
  /** Optional leading header node, such as the Subscriptions screen's back arrow. */
  leading?: React.ReactNode;
  /** Toolbar nodes on the right of the header. */
  headerActions?: React.ReactNode;
  /** Optional footer content. */
  footer?: React.ReactNode;
  /** Max scroll height of the body before it scrolls. Default 452. */
  maxBodyHeight?: number;
  /** Fixed screen position while detached, for browser-harness dragging. Native Tauri leaves this null: the OS window itself moves. */
  position?: { x: number; y: number } | null;
  /** The subscription rows. */
  children?: React.ReactNode;
  style?: React.CSSProperties;
}

/**
 * The Quotos popover shell: floating vibrancy surface, header toolbar, scrolling body, footer.
 */
export function Panel(props: PanelProps): React.ReactElement;
