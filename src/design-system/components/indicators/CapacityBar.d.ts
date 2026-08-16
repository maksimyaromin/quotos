import * as React from "react";

export interface CapacityBarProps {
  /** Consumed capacity, 0 to 100. Sets both fill width and color: teal, amber or red. */
  used?: number;
  /** Play an indeterminate shimmer over the held value during a refresh. */
  reading?: boolean;
  /** Dim the fill when the data is stale. */
  stale?: boolean;
  /** When given, overrides `used`-based color for the subscription's own
   * headline bar, colored by the worst of every window. Omit for a single
   * window's own bar, which colors from its own `used` instead. */
  severity?: "healthy" | "warn" | "critical" | null;
  /** Overrides the bar height. Defaults to `--cap-bar-height`, 4px. */
  height?: string;
  style?: React.CSSProperties;
}

/** The thin filling capacity bar used in every subscription row. */
export function CapacityBar(props: CapacityBarProps): React.ReactElement;
/** Used percent to fill color. Exported for reuse, such as tinting the headline numeral. */
export function capacityColor(used: number): string;
