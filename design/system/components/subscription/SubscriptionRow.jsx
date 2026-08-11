import React from "react";
import { CapacityBar, capacityColor } from "../indicators/CapacityBar.jsx";
import { StatusDot } from "../indicators/StatusDot.jsx";
import { Badge } from "../indicators/Badge.jsx";
import { LimitWindow } from "./LimitWindow.jsx";

const HAS_DATA = new Set(["working", "reading", "behind", "waiting", "repairing"]);

// Minimal default affordance glyphs (generic UI arrows/marks, not brand icons).
const Chevron = ({ open }) => (
  <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2"
    strokeLinecap="round" strokeLinejoin="round"
    style={{ transform: open ? "rotate(180deg)" : "none", transition: "transform var(--dur-base) var(--ease-standard)" }}>
    <path d="M6 9l6 6 6-6" />
  </svg>
);
const PinGlyph = () => (
  <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8"
    strokeLinecap="round" strokeLinejoin="round">
    <path d="M12 17v5M9 10.76V4a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v6.76a2 2 0 0 0 .59 1.41l1.3 1.3A1 1 0 0 1 17.18 15H6.82a1 1 0 0 1-.7-1.71l1.29-1.32A2 2 0 0 0 9 10.76Z" />
  </svg>
);

function TinyBtn({ label, active, onClick, children }) {
  const [h, setH] = React.useState(false);
  return (
    <button type="button" aria-label={label} title={label} onClick={onClick}
      onMouseEnter={() => setH(true)} onMouseLeave={() => setH(false)}
      style={{
        display: "inline-flex", alignItems: "center", justifyContent: "center",
        gap: "var(--space-1)", height: 20, padding: "0 4px", border: 0,
        borderRadius: "var(--radius-xs)", cursor: "pointer",
        background: h ? "var(--bg-row-hover)" : "transparent",
        color: active ? "var(--text-accent)" : "var(--text-tertiary)",
        fontFamily: "var(--font-sans)", fontSize: "var(--text-xs)",
        transition: "background var(--dur-fast), color var(--dur-fast)",
      }}>
      {children}
    </button>
  );
}

const STATE_META = {
  behind: { tone: "warn", label: "Behind" },
  waiting: { tone: "info", label: "Waiting on limits" },
  repairing: { tone: "info", label: "Repairing" },
  broken: { tone: "danger", label: "Broken" },
};

/** The core panel row: one subscription, scannable in a single pass. Renders a
 *  "% left" headline + capacity bar for data-bearing states, and a message +
 *  action for not-connected / connecting / broken. State-driven throughout;
 *  age is always shown, and stale data is visibly dimmed, never presented as
 *  current. Expands to the variable window list. */
export function SubscriptionRow({
  label,
  provider,
  account,
  state = "working",
  remaining = null,
  resetLabel = null,
  lastRead = null,
  windows = [],
  reason = null,
  pinned = false,
  expanded = false,
  actionLabel = null,
  onAction,
  onTogglePin,
  onToggleExpand,
  style,
}) {
  const [hover, setHover] = React.useState(false);
  const connected = HAS_DATA.has(state);
  const hasData = connected && typeof remaining === "number";
  const noLimits = connected && typeof remaining !== "number"; // read fine, provider reports nothing useful
  const stale = state === "behind";
  const reading = state === "reading";
  const meta = STATE_META[state];

  const numeralColor = stale
    ? "var(--text-tertiary)"
    : remaining !== null && remaining <= 25
    ? capacityColor(remaining)
    : "var(--text-primary)";

  return (
    <div
      onMouseEnter={() => setHover(true)}
      onMouseLeave={() => setHover(false)}
      style={{
        display: "flex", flexDirection: "column", gap: "var(--space-2)",
        padding: "var(--space-3)",
        borderRadius: "var(--radius-md)",
        background: hover ? "var(--bg-row-hover)" : "transparent",
        transition: "background var(--dur-fast) var(--ease-standard)",
        ...style,
      }}
    >
      {/* header */}
      <div style={{ display: "flex", alignItems: "flex-start", gap: "var(--space-2)" }}>
        <StatusDot state={state} style={{ marginTop: 4, flex: "0 0 auto" }} />
        <div style={{ flex: 1, minWidth: 0 }}>
          <div style={{
            fontFamily: "var(--font-sans)", fontSize: "var(--text-md)",
            fontWeight: "var(--weight-semibold)", color: "var(--text-primary)",
            letterSpacing: "var(--tracking-tight)",
            overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap",
          }}>{label}</div>
          {(provider || account) ? (
            <div style={{
              fontFamily: "var(--font-sans)", fontSize: "var(--text-sm)",
              color: "var(--text-tertiary)",
              overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap",
            }}>{[account, provider].filter(Boolean).join(" · ")}</div>
          ) : null}
        </div>
        {meta ? <Badge tone={meta.tone} style={{ marginTop: 1 }}>{meta.label}</Badge> : null}
        <TinyBtn label={pinned ? "Unpin from menu bar" : "Pin to menu bar"} active={pinned}
          onClick={onTogglePin} style={{ opacity: pinned || hover ? 1 : 0 }}>
          <span style={{ opacity: pinned || hover ? 1 : 0, transition: "opacity var(--dur-fast)" }}><PinGlyph /></span>
        </TinyBtn>
      </div>

      {/* body */}
      {hasData ? (
        <>
          <div style={{ display: "flex", alignItems: "flex-end", gap: "var(--space-2)" }}>
            <div style={{ display: "flex", alignItems: "baseline", gap: "var(--space-1)", flex: 1 }}>
              <span style={{
                fontFamily: "var(--font-mono)", fontSize: "var(--numeral-lg)",
                fontWeight: "var(--weight-medium)", fontVariantNumeric: "tabular-nums",
                lineHeight: 1, color: numeralColor, letterSpacing: "var(--tracking-tighter)",
              }}>{remaining}<span style={{ fontSize: "18px" }}>%</span></span>
              <span style={{ fontFamily: "var(--font-sans)", fontSize: "var(--text-sm)", color: "var(--text-tertiary)" }}>left</span>
            </div>
            {resetLabel ? (
              <span style={{ fontFamily: "var(--font-sans)", fontSize: "var(--text-xs)", color: "var(--text-tertiary)", paddingBottom: 3 }}>
                {resetLabel}
              </span>
            ) : null}
          </div>
          <CapacityBar remaining={remaining} reading={reading} stale={stale} />
        </>
      ) : (
        <div style={{
          fontFamily: "var(--font-sans)", fontSize: "var(--text-sm)",
          color: state === "broken" ? "var(--text-secondary)" : "var(--text-tertiary)",
          lineHeight: "var(--leading-snug)",
        }}>
          {state === "connecting" ? "Connecting…"
            : state === "idle" ? "Not connected — needs one more step."
            : noLimits ? "No limits reported — nothing to show yet."
            : reason || "Can’t read this subscription."}
        </div>
      )}

      {/* footer */}
      <div style={{ display: "flex", alignItems: "center", gap: "var(--space-2)", minHeight: 20 }}>
        <span style={{
          flex: 1, fontFamily: "var(--font-sans)", fontSize: "var(--text-xs)",
          color: stale ? "var(--amber)" : "var(--text-tertiary)",
        }}>
          {state === "reading" ? "Reading…"
            : state === "connecting" ? "This can take a few seconds"
            : lastRead ? `Last read ${lastRead}` : ""}
        </span>
        {actionLabel ? (
          <TinyBtn label={actionLabel} onClick={onAction}>
            <span style={{ color: "var(--text-accent)", fontWeight: "var(--weight-medium)" }}>{actionLabel}</span>
          </TinyBtn>
        ) : null}
        {windows && windows.length > 0 ? (
          <TinyBtn label={expanded ? "Hide limits" : "Show limits"} onClick={onToggleExpand}>
            {windows.length} {windows.length === 1 ? "limit" : "limits"} <Chevron open={expanded} />
          </TinyBtn>
        ) : null}
      </div>

      {/* expanded detail */}
      {expanded && windows && windows.length > 0 ? (
        <div style={{
          borderTop: "0.5px solid var(--border-subtle)",
          paddingTop: "var(--space-1)", marginTop: "var(--space-0-5)",
        }}>
          {windows.map((w, i) => (
            <LimitWindow key={i} {...w} stale={stale}
              style={i < windows.length - 1 ? { borderBottom: "0.5px solid var(--border-subtle)" } : null} />
          ))}
        </div>
      ) : null}
    </div>
  );
}
