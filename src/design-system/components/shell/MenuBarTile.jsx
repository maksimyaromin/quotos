/** The placeholder menu-bar glyph, a quota ring. Not a logo, since Quotos
 *  has none. A functional macOS template mark, monochrome via
 *  currentColor. */
export function QuotaGlyph({ size = 15 }) {
  return (
    <svg width={size} height={size} viewBox="0 0 16 16" fill="none" style={{ flex: "0 0 auto" }}>
      <circle cx="8" cy="8" r="5.4" stroke="currentColor" strokeWidth="1.4" opacity="0.28" />
      <path
        d="M4.46 12.02a5.4 5.4 0 1 1 7.08 0"
        stroke="currentColor"
        strokeWidth="1.9"
        strokeLinecap="round"
      />
    </svg>
  );
}

const AttentionMark = () => (
  <svg
    width="11"
    height="11"
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    strokeWidth="2.2"
    strokeLinecap="round"
    strokeLinejoin="round"
  >
    <path d="M10.3 3.6 1.8 18a1.9 1.9 0 0 0 1.7 2.9h17a1.9 1.9 0 0 0 1.7-2.9L13.7 3.6a1.9 1.9 0 0 0-3.4 0Z" />
    <path d="M12 9v4M12 17h.01" />
  </svg>
);

/** The status item's representation: the glyph plus optional pinned
 *  figures. Monochrome by macOS convention, so a pinned figure only takes
 *  a warning tint, amber or red, when it actually needs attention. Broken
 *  pins show a small mark instead of a stale number. Renders on a mock
 *  menu-bar strip for preview. */
export function MenuBarTile({ pins = [], onClick, showStrip = true, style }) {
  const tintFor = (p) => {
    if (p.state === "broken" || p.state === "behind") return "var(--amber)";
    if (typeof p.used === "number" && p.used >= 90) return "var(--red)";
    if (typeof p.used === "number" && p.used >= 75) return "var(--amber)";
    return "inherit";
  };

  const tile = (
    <button
      type="button"
      onClick={onClick}
      style={{
        display: "inline-flex",
        alignItems: "center",
        gap: "var(--space-1-5)",
        height: 22,
        padding: "0 var(--space-1-5)",
        border: 0,
        borderRadius: "var(--radius-xs)",
        background: "transparent",
        cursor: "pointer",
        color: showStrip ? "rgba(255,255,255,0.92)" : "var(--text-primary)",
        fontFamily: "var(--font-mono)",
        fontSize: "12px",
        fontWeight: "var(--weight-medium)",
        fontVariantNumeric: "tabular-nums",
      }}
    >
      <QuotaGlyph />
      {pins.map((p, i) => (
        <span
          // biome-ignore lint/suspicious/noArrayIndexKey: pins carry no stable id yet.
          key={i}
          style={{ display: "inline-flex", alignItems: "center", gap: 3, color: tintFor(p) }}
        >
          {p.state === "broken" ? (
            <AttentionMark />
          ) : (
            <>
              {typeof p.used === "number" ? `${p.used}%` : "—"}
              {p.state === "behind" ? (
                <span style={{ opacity: 0.7 }}>
                  <AttentionMark />
                </span>
              ) : null}
            </>
          )}
        </span>
      ))}
    </button>
  );

  if (!showStrip) return <span style={style}>{tile}</span>;

  return (
    <div
      style={{
        display: "flex",
        alignItems: "center",
        gap: "var(--space-4)",
        height: 24,
        padding: "0 var(--space-2)",
        background: "rgba(30,32,36,0.72)",
        backdropFilter: "saturate(160%) blur(20px)",
        WebkitBackdropFilter: "saturate(160%) blur(20px)",
        borderRadius: "var(--radius-sm)",
        ...style,
      }}
    >
      {tile}
      {/* faint neighbors, to show it living among other menu-bar items */}
      <span
        style={{
          display: "inline-flex",
          alignItems: "center",
          gap: 3,
          color: "rgba(255,255,255,0.6)",
          fontFamily: "var(--font-sans)",
          fontSize: 12,
        }}
      >
        100%
        <svg width="22" height="12" viewBox="0 0 26 13" fill="none">
          <rect x="0.5" y="0.5" width="22" height="12" rx="3" stroke="currentColor" opacity="0.7" />
          <rect x="2" y="2" width="19" height="9" rx="1.5" fill="currentColor" />
          <rect x="23.5" y="4" width="2" height="5" rx="1" fill="currentColor" opacity="0.7" />
        </svg>
      </span>
      <span
        style={{ color: "rgba(255,255,255,0.82)", fontFamily: "var(--font-sans)", fontSize: 12 }}
      >
        Mon 9:41
      </span>
    </div>
  );
}
