import type * as React from "react";

export type SubscriptionState =
  | "idle" // not connected
  | "connecting"
  | "working"
  | "reading" // refresh in flight
  | "behind" // stale data held
  | "broken";
// "Waiting on the rate budget" is deliberately not a health state. It is
// tracked separately as a rate-limit fact that never overrides the
// subscription's own health.

const STATE_COLOR: Record<SubscriptionState, string> = {
  idle: "var(--status-idle)",
  connecting: "var(--status-progress)",
  working: "var(--status-working)",
  reading: "var(--status-progress)",
  behind: "var(--status-behind)",
  broken: "var(--status-broken)",
};

const PULSING = new Set<SubscriptionState>(["connecting", "reading"]);

export interface StatusDotProps {
  state?: SubscriptionState;
  /** Diameter in pixels. Defaults to 7. */
  size?: number;
  style?: React.CSSProperties;
}

/** A small colored state dot. In-progress states pulse. "working" is calm teal, not a loud green. */
export function StatusDot({ state = "working", size = 7, style }: StatusDotProps) {
  const pulsing = PULSING.has(state);
  return (
    <span
      style={{
        display: "inline-block",
        width: size,
        height: size,
        borderRadius: "var(--radius-pill)",
        backgroundColor: STATE_COLOR[state],
        boxShadow: state === "idle" ? "inset 0 0 0 1.5px var(--status-idle)" : "none",
        backgroundClip: state === "idle" ? "content-box" : "border-box",
        opacity: state === "idle" ? 0.9 : 1,
        animation: pulsing ? "quotos-pulse 1.4s var(--ease-standard) infinite" : "none",
        ...style,
      }}
    >
      <style>{`@keyframes quotos-pulse{0%,100%{opacity:1}50%{opacity:0.35}}`}</style>
    </span>
  );
}
