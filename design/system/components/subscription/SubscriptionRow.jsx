import React from "react";
import { CapacityBar, capacityColor } from "../indicators/CapacityBar.jsx";
import { StatusDot } from "../indicators/StatusDot.jsx";
import { Badge } from "../indicators/Badge.jsx";
import { LimitWindow } from "./LimitWindow.jsx";

const HAS_DATA = new Set(["working", "reading", "behind", "repairing"]);

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
const PencilGlyph = () => (
  <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"
    strokeLinecap="round" strokeLinejoin="round">
    <path d="M17 3a2.85 2.85 0 1 1 4 4L7.5 20.5 2 22l1.5-5.5Z" />
  </svg>
);
const TrashGlyph = () => (
  <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"
    strokeLinecap="round" strokeLinejoin="round">
    <path d="M3 6h18M8 6V4a1 1 0 0 1 1-1h6a1 1 0 0 1 1 1v2m3 0-1 14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2L4 6" />
  </svg>
);

function TinyBtn({ label, active, danger, onClick, onMouseLeave, children }) {
  const [h, setH] = React.useState(false);
  return (
    <button type="button" aria-label={label} title={label} onClick={onClick}
      onMouseEnter={() => setH(true)}
      onMouseLeave={() => { setH(false); onMouseLeave?.(); }}
      style={{
        display: "inline-flex", alignItems: "center", justifyContent: "center",
        gap: "var(--space-1)", height: 20, padding: "0 4px", border: 0,
        borderRadius: "var(--radius-xs)", cursor: "pointer",
        background: h ? (danger ? "var(--red-muted)" : "var(--bg-row-hover)") : "transparent",
        color: danger ? "var(--red)" : active ? "var(--text-accent)" : "var(--text-tertiary)",
        fontFamily: "var(--font-sans)", fontSize: "var(--text-xs)",
        transition: "background var(--dur-fast), color var(--dur-fast)",
      }}>
      {children}
    </button>
  );
}

const STATE_META = {
  behind: { tone: "warn", label: "Behind" },
  repairing: { tone: "info", label: "Repairing" },
  broken: { tone: "danger", label: "Broken" },
};

/** The core panel row: one subscription, scannable in a single pass. Renders a
 *  "% used" headline + capacity bar for data-bearing states, and a message +
 *  action for not-connected / connecting / broken. State-driven throughout;
 *  age is always shown, and stale data is visibly dimmed, never presented as
 *  current. Expands to the variable window list without reflowing the card
 *  (the expand track is always present, animated between 0fr/1fr, and the
 *  scrollbar gutter is reserved in the parent Panel — see Panel.jsx). */
export function SubscriptionRow({
  label,
  provider,
  account,
  state = "working",
  used = null,
  resetLabel = null,
  lastRead = null,
  windows = [],
  reason = null,
  pinned = false,
  expanded = false,
  actionLabel = null,
  actionDisabled = false,
  footerNote = null,
  onAction,
  onTogglePin,
  onToggleExpand,
  onRename,
  onDelete,
  style,
}) {
  const [hover, setHover] = React.useState(false);
  const [renaming, setRenaming] = React.useState(false);
  const [draft, setDraft] = React.useState(label);
  const [confirmingDelete, setConfirmingDelete] = React.useState(false);
  const inputRef = React.useRef(null);

  React.useEffect(() => {
    if (renaming) {
      setDraft(label);
      requestAnimationFrame(() => inputRef.current?.select());
    }
  }, [renaming, label]);

  const commitRename = () => {
    setRenaming(false);
    const trimmed = draft.trim();
    onRename?.(trimmed.length > 0 && trimmed !== label ? trimmed : null);
  };

  const connected = HAS_DATA.has(state);
  const hasData = connected && typeof used === "number";
  const noLimits = connected && typeof used !== "number"; // read fine, provider reports nothing useful
  const stale = state === "behind";
  const reading = state === "reading";
  const meta = STATE_META[state];

  const numeralColor = stale
    ? "var(--text-tertiary)"
    : used !== null && used >= 75
    ? capacityColor(used)
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
          {renaming ? (
            <input
              ref={inputRef}
              value={draft}
              onChange={(e) => setDraft(e.target.value)}
              onBlur={commitRename}
              onKeyDown={(e) => {
                if (e.key === "Enter") commitRename();
                if (e.key === "Escape") { setDraft(label); setRenaming(false); }
              }}
              style={{
                width: "100%", font: "inherit",
                fontFamily: "var(--font-sans)", fontSize: "var(--text-md)",
                fontWeight: "var(--weight-semibold)", color: "var(--text-primary)",
                letterSpacing: "var(--tracking-tight)",
                background: "var(--bg-input)", border: "0.5px solid var(--border-focus)",
                borderRadius: "var(--radius-xs)", padding: "1px 4px", margin: "-1px -4px",
                outline: "none",
              }}
            />
          ) : (
            <div style={{
              fontFamily: "var(--font-sans)", fontSize: "var(--text-md)",
              fontWeight: "var(--weight-semibold)", color: "var(--text-primary)",
              letterSpacing: "var(--tracking-tight)",
              overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap",
            }}>{label}</div>
          )}
          {(provider || account) ? (
            <div style={{
              fontFamily: "var(--font-sans)", fontSize: "var(--text-sm)",
              color: "var(--text-tertiary)",
              overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap",
            }}>{[account, provider].filter(Boolean).join(" · ")}</div>
          ) : null}
        </div>
        {meta ? <Badge tone={meta.tone} style={{ marginTop: 1, flex: "0 0 auto" }}>{meta.label}</Badge> : null}
        <div style={{ display: "flex", alignItems: "center", flex: "0 0 auto", opacity: pinned || hover ? 1 : 0, transition: "opacity var(--dur-fast)" }}>
          {onRename ? (
            <TinyBtn label="Rename" onClick={() => setRenaming(true)}>
              <PencilGlyph />
            </TinyBtn>
          ) : null}
          {onDelete ? (
            <TinyBtn
              label={confirmingDelete ? "Click again to remove" : "Remove subscription"}
              danger
              onClick={() => (confirmingDelete ? onDelete() : setConfirmingDelete(true))}
              onMouseLeave={() => setConfirmingDelete(false)}
            >
              {confirmingDelete ? "Remove?" : <TrashGlyph />}
            </TinyBtn>
          ) : null}
          <TinyBtn label={pinned ? "Unpin from menu bar" : "Pin to menu bar"} active={pinned} onClick={onTogglePin}>
            <PinGlyph />
          </TinyBtn>
        </div>
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
              }}>{used}<span style={{ fontSize: "18px" }}>%</span></span>
              <span style={{ fontFamily: "var(--font-sans)", fontSize: "var(--text-sm)", color: "var(--text-tertiary)" }}>used</span>
            </div>
            {resetLabel ? (
              <span style={{ fontFamily: "var(--font-sans)", fontSize: "var(--text-xs)", color: "var(--text-tertiary)", paddingBottom: 3 }}>
                {resetLabel}
              </span>
            ) : null}
          </div>
          <CapacityBar used={used} reading={reading} stale={stale} />
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
            : footerNote ? footerNote
            : state === "connecting" ? "This can take a few seconds"
            : lastRead ? `Last read ${lastRead}` : ""}
        </span>
        {actionLabel ? (
          <TinyBtn label={actionLabel} onClick={actionDisabled ? undefined : onAction}>
            <span style={{ color: actionDisabled ? "var(--text-quaternary)" : "var(--text-accent)", fontWeight: "var(--weight-medium)" }}>{actionLabel}</span>
          </TinyBtn>
        ) : null}
        {windows && windows.length > 0 ? (
          <TinyBtn label={expanded ? "Hide limits" : "Show limits"} onClick={onToggleExpand}>
            {windows.length} {windows.length === 1 ? "limit" : "limits"} <Chevron open={expanded} />
          </TinyBtn>
        ) : null}
      </div>

      {/* expanded detail — always mounted, animated via grid-template-rows so
          expand/collapse is a deliberate motion rather than a pop, and never
          shifts layout by itself (I4). */}
      <div
        aria-hidden={!expanded}
        style={{
          display: "grid",
          gridTemplateRows: expanded && windows && windows.length > 0 ? "1fr" : "0fr",
          transition: "grid-template-rows var(--dur-base) var(--ease-standard)",
        }}
      >
        <div style={{ overflow: "hidden", minHeight: 0 }}>
          {windows && windows.length > 0 ? (
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
      </div>
    </div>
  );
}
