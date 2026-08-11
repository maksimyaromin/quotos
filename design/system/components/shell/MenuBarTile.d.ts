import * as React from "react";
import type { SubscriptionState } from "../indicators/StatusDot";

export interface PinnedFigure {
  /** % remaining for this pinned subscription. */
  remaining?: number | null;
  state?: SubscriptionState;
}

/**
 * The tray icon + pinned figures. Monochrome by convention; a figure only tints when it needs attention.
 * @startingPoint section="Shell" subtitle="Menu-bar tray icon with pinned figures" viewport="360x60"
 */
export interface MenuBarTileProps {
  /** The pinned subscriptions shown beside the glyph (typically 0–2). */
  pins?: PinnedFigure[];
  /** Click opens the panel. */
  onClick?: () => void;
  /** Render on a mock menu-bar strip (default) or bare (false) for embedding. */
  showStrip?: boolean;
  style?: React.CSSProperties;
}

/**
 * The tray icon + pinned figures. Monochrome by convention; a figure only tints
 * (amber/red) when it needs attention.
 */
export function MenuBarTile(props: MenuBarTileProps): React.ReactElement;

/** The placeholder quota-ring glyph (not a logo). Monochrome, currentColor. */
export function QuotaGlyph(props: { size?: number }): React.ReactElement;
