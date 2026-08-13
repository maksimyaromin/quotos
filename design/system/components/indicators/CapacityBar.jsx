import React from "react";

/** Maps consumed capacity to a fill color: teal when little is used, amber
 *  approaching the limit, red near or at it. This is the only place color
 *  changes meaning by value. I2: consumed, not remaining — a full bar and a
 *  red bar both mean the same thing everywhere the bar appears.
 *
 *  Used directly by `LimitWindow`'s per-window bars, where each window's own
 *  percentage is the only thing that should color it. The subscription's
 *  own headline bar instead takes a `severity` prop (below) — R2-2/
 *  followup-3: the captain's own request is that the headline number and
 *  bar read from the *worst of every window*, not just the headline's own
 *  percentage, so a 20%-weekly account with an 85%-used session still shows
 *  amber. */
export function capacityColor(used) {
  if (used >= 90) return "var(--cap-critical)";
  if (used >= 75) return "var(--cap-warn)";
  return "var(--cap-healthy)";
}

function severityColor(severity) {
  if (severity === "critical") return "var(--cap-critical)";
  if (severity === "warn") return "var(--cap-warn)";
  return "var(--cap-healthy)";
}

/** The thin filling bar. `used` (0–100, percent consumed) always sets fill
 *  width — more filled always means more used, never the reverse. Color
 *  comes from `severity` when given (the subscription's own headline bar);
 *  otherwise it falls back to `used`'s own bracket (a single window's bar,
 *  which has no separate severity of its own). When `reading`, an
 *  indeterminate shimmer plays over the held value — the number is never
 *  blanked. When `stale`, the fill dims. */
export function CapacityBar({
  used = 0,
  reading = false,
  stale = false,
  severity = null,
  height,
  style,
}) {
  const fill = Math.max(0, Math.min(100, used));
  const color = severity ? severityColor(severity) : capacityColor(used);
  return (
    <div
      style={{
        position: "relative",
        width: "100%",
        height: height || "var(--cap-bar-height)",
        background: "var(--cap-track)",
        borderRadius: "var(--radius-pill)",
        overflow: "hidden",
        ...style,
      }}
    >
      <div
        style={{
          position: "absolute",
          inset: 0,
          width: `${fill}%`,
          background: color,
          borderRadius: "var(--radius-pill)",
          opacity: stale ? 0.4 : 1,
          transition: "width var(--dur-slow) var(--ease-out), background var(--dur-base), opacity var(--dur-base)",
        }}
      />
      {reading ? (
        <div
          style={{
            position: "absolute",
            inset: 0,
            background: "linear-gradient(90deg, transparent, rgba(255,255,255,0.28), transparent)",
            backgroundSize: "40% 100%",
            backgroundRepeat: "no-repeat",
            animation: "quotos-shimmer 1.1s var(--ease-standard) infinite",
          }}
        />
      ) : null}
      <style>{`@keyframes quotos-shimmer{0%{background-position:-40% 0}100%{background-position:140% 0}}`}</style>
    </div>
  );
}
