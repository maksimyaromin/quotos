const STATE_COLOR = {
  idle: "var(--status-idle)",
  connecting: "var(--status-progress)",
  working: "var(--status-working)",
  reading: "var(--status-progress)",
  behind: "var(--status-behind)",
  broken: "var(--status-broken)",
};

const PULSING = new Set(["connecting", "reading"]);

/** A small state dot. Color maps to subscription state, and in-progress
 *  states pulse gently. "working" reads as calm teal, not a loud green "OK". */
export function StatusDot({ state = "working", size = 7, style }) {
  const pulsing = PULSING.has(state);
  return (
    <span
      style={{
        display: "inline-block",
        width: size,
        height: size,
        borderRadius: "var(--radius-pill)",
        backgroundColor: STATE_COLOR[state] || "var(--status-idle)",
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
