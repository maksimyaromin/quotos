import React from "react";

/** Maps remaining capacity to a fill color: teal when healthy, amber when
 *  getting low, red when nearly out. This is the only place color changes
 *  meaning by value. */
export function capacityColor(remaining) {
  if (remaining <= 10) return "var(--cap-critical)";
  if (remaining <= 25) return "var(--cap-warn)";
  return "var(--cap-healthy)";
}

/** The thin depleting bar. `remaining` (0–100) sets fill width and color.
 *  When `reading`, an indeterminate shimmer plays over the held value —
 *  the number is never blanked. When `stale`, the fill dims. */
export function CapacityBar({
  remaining = 0,
  reading = false,
  stale = false,
  height,
  style,
}) {
  const fill = Math.max(0, Math.min(100, remaining));
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
          background: capacityColor(remaining),
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
