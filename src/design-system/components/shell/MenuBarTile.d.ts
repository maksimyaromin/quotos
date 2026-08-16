import * as React from "react";
import type { SubscriptionState } from "../indicators/StatusDot";

export interface PinnedFigure {
  /** % consumed shown for this pinned figure. */
  used?: number | null;
  state?: SubscriptionState;
}

/**
 * The status item glyph plus pinned figures. Monochrome by convention; a figure only tints when it needs attention.
 * @startingPoint section="Shell" subtitle="Menu-bar status item with pinned figures" viewport="360x60"
 */
export interface MenuBarTileProps {
  /** The pinned subscriptions shown beside the glyph, typically 0 to 2. */
  pins?: PinnedFigure[];
  /** Click opens the panel. */
  onClick?: () => void;
  /** Render on a mock menu-bar strip by default, or bare when false, for embedding. */
  showStrip?: boolean;
  style?: React.CSSProperties;
}

/**
 * The status item glyph plus pinned figures. Monochrome by convention; a
 * figure only tints, amber or red, when it needs attention.
 */
export function MenuBarTile(props: MenuBarTileProps): React.ReactElement;

/** The placeholder quota-ring glyph. Not a logo. Monochrome, currentColor. */
export function QuotaGlyph(props: { size?: number }): React.ReactElement;
