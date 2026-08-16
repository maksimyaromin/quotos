import type * as React from "react";
import styles from "./capacity-bar.module.css";

type CapacityLevel = "healthy" | "warn" | "critical";

function capacityLevel(used: number): CapacityLevel {
  if (used >= 90) return "critical";
  if (used >= 75) return "warn";
  return "healthy";
}

function levelColor(level: CapacityLevel): string {
  if (level === "critical") return "var(--cap-critical)";
  if (level === "warn") return "var(--cap-warn)";
  return "var(--cap-healthy)";
}

/** Thresholds must match `providers/claude/normalize-usage.ts`'s severity
 *  calculation exactly. */
export function capacityColor(used: number): string {
  return levelColor(capacityLevel(used));
}

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
  severity?: CapacityLevel | null;
  /** Overrides the bar height. Defaults to `--cap-bar-height`, 4px. */
  height?: string;
  style?: React.CSSProperties;
}

/** `used`, 0 to 100 percent consumed, always sets fill width: more filled
 *  always means more used, never the reverse. `severity` overrides the
 *  fill color when given; otherwise color falls back to `used`'s own
 *  bracket. `reading` plays a shimmer over the held value without
 *  blanking it. `stale` dims the fill. */
export function CapacityBar({
  used = 0,
  reading = false,
  stale = false,
  severity = null,
  height,
  style,
}: CapacityBarProps) {
  const fill = Math.max(0, Math.min(100, used));
  const level = severity ?? capacityLevel(used);
  return (
    <div className={styles.track} style={{ height, ...style }}>
      <div
        className={styles.fill}
        data-color={level}
        data-stale={stale ? "true" : undefined}
        style={{ width: `${fill}%` }}
      />
      {reading ? <div data-quotos-shimmer="" className={styles.shimmer} /> : null}
    </div>
  );
}
