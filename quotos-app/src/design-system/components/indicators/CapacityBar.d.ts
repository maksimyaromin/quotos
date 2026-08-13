import * as React from "react";

export interface CapacityBarProps {
  /** Consumed capacity 0–100. Sets fill width AND color (teal / amber / red). */
  used?: number;
  /** Play an indeterminate shimmer over the held value during a refresh. */
  reading?: boolean;
  /** Dim the fill when the data is behind (stale). */
  stale?: boolean;
  /** "healthy" | "warn" | "critical" — when given, overrides `used`-based
   * color (the subscription's own headline bar, colored by the worst of
   * every window). Omit for a single window's own bar, which colors from
   * its own `used` instead. */
  severity?: "healthy" | "warn" | "critical" | null;
  /** Override the bar height (default `--cap-bar-height`, 4px). */
  height?: string;
  style?: React.CSSProperties;
}

/** The thin filling capacity bar used in every subscription row. */
export function CapacityBar(props: CapacityBarProps): React.ReactElement;
/** Used → fill color. Exported for reuse (e.g. tinting the headline numeral). */
export function capacityColor(used: number): string;
