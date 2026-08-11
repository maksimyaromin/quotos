import * as React from "react";

export interface CapacityBarProps {
  /** Remaining capacity 0–100. Sets fill width AND color (teal / amber / red). */
  remaining?: number;
  /** Play an indeterminate shimmer over the held value during a refresh. */
  reading?: boolean;
  /** Dim the fill when the data is behind (stale). */
  stale?: boolean;
  /** Override the bar height (default `--cap-bar-height`, 4px). */
  height?: string;
  style?: React.CSSProperties;
}

/** The thin depleting capacity bar used in every subscription row. */
export function CapacityBar(props: CapacityBarProps): React.ReactElement;
/** Remaining → fill color. Exported for reuse (e.g. tinting the headline numeral). */
export function capacityColor(remaining: number): string;
