/** Thresholds must match `providers/claude/normalizeUsage.ts`'s severity
 *  calculation exactly. */
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
          // biome-ignore format: reducedMotion.spec.jsx scans this line for a --dur token, so it must stay on one line.
          transition: "width var(--dur-slow) var(--ease-out), background var(--dur-base), opacity var(--dur-base)",
        }}
      />
      {reading ? (
        // data-quotos-shimmer lets tokens/elevation.css hide this overlay
        // under prefers-reduced-motion, where a stopped gradient would sit
        // as a static white stripe instead of a moving glint.
        <div
          data-quotos-shimmer=""
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
