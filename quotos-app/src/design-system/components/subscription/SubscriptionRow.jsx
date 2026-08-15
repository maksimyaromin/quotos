import React from "react";
import { CapacityBar } from "../indicators/CapacityBar.jsx";
import { StatusDot } from "../indicators/StatusDot.jsx";
import { Badge } from "../indicators/Badge.jsx";
import { LimitWindow } from "./LimitWindow.jsx";

// Minimal default affordance glyphs (generic UI arrows/marks, not brand icons).
const Chevron = ({ open }) => (
  <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2"
    strokeLinecap="round" strokeLinejoin="round"
    style={{ transform: open ? "rotate(180deg)" : "none", transition: "transform var(--dur-base) var(--ease-standard)" }}>
    <path d="M6 9l6 6 6-6" />
  </svg>
);
const PinGlyph = () => (
  <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8"
    strokeLinecap="round" strokeLinejoin="round">
    <path d="M12 17v5M9 10.76V4a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v6.76a2 2 0 0 0 .59 1.41l1.3 1.3A1 1 0 0 1 17.18 15H6.82a1 1 0 0 1-.7-1.71l1.29-1.32A2 2 0 0 0 9 10.76Z" />
  </svg>
);
const MenuDotsGlyph = () => (
  <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor">
    <circle cx="5" cy="12" r="1.6" />
    <circle cx="12" cy="12" r="1.6" />
    <circle cx="19" cy="12" r="1.6" />
  </svg>
);

const STATE_DOT_COLOR = {
  working: "var(--status-working)",
  behind: "var(--status-behind)",
  broken: "var(--status-broken)",
};

// R4-5: the row menu's geometry, in viewport coordinates. It is `position:
// fixed` rather than `position: absolute` inside the row, and that is a fix,
// not a style choice — see the `useLayoutEffect` below.
const MENU_GAP = 4; // between the "…" button's bottom edge and the menu's top
const MENU_VIEWPORT_MARGIN = 8; // never closer than this to the window's own edge
const MENU_MIN_WIDTH = 168;

function MenuItem({ danger, onClick, children }) {
  const [hover, setHover] = React.useState(false);
  return (
    <button type="button" onClick={onClick}
      onMouseEnter={() => setHover(true)} onMouseLeave={() => setHover(false)}
      style={{
        display: "flex", alignItems: "center", height: 26, padding: "0 var(--space-2)",
        border: 0, borderRadius: "var(--radius-sm)",
        background: hover ? "var(--bg-row-hover)" : "transparent",
        fontFamily: "var(--font-sans)", fontSize: "var(--text-base)",
        color: danger ? "var(--red)" : "var(--text-primary)",
        textAlign: "left", cursor: "pointer",
      }}>
      {children}
    </button>
  );
}

/** The core panel row: one subscription, scannable in a single pass. Header
 *  order is fixed — dot, name + subtitle, pin (only if pinned), state badge,
 *  the always-visible "…" menu — nothing shifts, appears, or disappears on
 *  hover. Clicking anywhere on the row expands it; an expanded (or
 *  menu-open) row keeps --bg-row-hover so it visibly reads as open. Renders
 *  a "% used" headline + capacity bar for data-bearing states, and a
 *  message for not-connected / connecting / broken / no-limits-yet. Mirrors
 *  quotos-prototype.html's row logic exactly — see design-notes. */
export function SubscriptionRow({
  label,
  provider,
  account,
  state = "working",
  used = null,
  severity = "healthy",
  resetLabel = null,
  lastRead = null,
  windows = [],
  reason = null,
  /** R3-4: classified by the caller (see lib/rowPresentation.ts), not
   *  inferred from `state` here. "Needs sign-in" used to be shown for every
   *  `broken` row, so an offline launch or an HTTP 403 told the captain his
   *  working account was signed out. */
  badge = null,
  pinned = false,
  expanded = false,
  menuOpen = false,
  actionLabel = null,
  actionDisabled = false,
  footerNote = null,
  onAction,
  onTogglePin,
  onToggleExpand,
  onToggleMenu,
  onRename,
  onReadNow,
  onStopTracking,
  signInInProgress = false,
  onSubmitSignInCode,
  onCancelSignIn,
  style,
}) {
  const [renaming, setRenaming] = React.useState(false);
  const [draft, setDraft] = React.useState(label);
  const inputRef = React.useRef(null);
  const [codeDraft, setCodeDraft] = React.useState("");
  const codeInputRef = React.useRef(null);
  const menuButtonRef = React.useRef(null);
  const menuRef = React.useRef(null);
  const [menuPos, setMenuPos] = React.useState(null);

  // R4-5: place the open menu in *viewport* coordinates, under its own "…"
  // button.
  //
  // It used to be `position: absolute` inside the row, which put it inside the
  // panel body's `overflow-y: auto` box. Two separate consequences, both in
  // the captain's 2026-08-15 screencast: the menu was clipped by the panel's
  // bottom edge (its last item, "Stop tracking", cut in half), and — because
  // an absolutely-positioned element *does* count toward its scroll
  // container's scrollable area — merely opening it made a two-row panel
  // scrollable, so reaching the clipped item meant scrolling the rows out from
  // under the cursor first. `position: fixed` fixes both at once and for the
  // same reason: its containing block is the viewport, so no ancestor's
  // `overflow` can clip it and it adds nothing to any scroll extent. (It works
  // here only because nothing above this row establishes a containing block
  // for fixed descendants — no `transform`, `filter`, `backdrop-filter`,
  // `perspective`, `will-change` or `contain` on the panel's own wrappers. The
  // panel's blurred backdrop layer is a *sibling*, not an ancestor.)
  //
  // Measured after mount rather than computed from constants, because whether
  // the menu opens downward or flips above depends on where the row happens to
  // sit in a 560px-tall window. `useLayoutEffect` (not `useEffect`) so the
  // resulting state update is flushed before paint — the menu is rendered
  // hidden for exactly one layout pass, never a visible frame at 0,0.
  React.useLayoutEffect(() => {
    if (!menuOpen) {
      setMenuPos(null);
      return undefined;
    }
    const place = () => {
      const trigger = menuButtonRef.current;
      const menu = menuRef.current;
      if (!trigger || !menu) return;
      const anchor = trigger.getBoundingClientRect();
      const width = menu.offsetWidth || MENU_MIN_WIDTH;
      const height = menu.offsetHeight;
      const below = anchor.bottom + MENU_GAP;
      const top =
        below + height <= window.innerHeight - MENU_VIEWPORT_MARGIN
          ? below
          : Math.max(MENU_VIEWPORT_MARGIN, anchor.top - MENU_GAP - height);
      // Right-aligned with the button, then held inside the window.
      const left = Math.min(
        Math.max(MENU_VIEWPORT_MARGIN, anchor.right - width),
        Math.max(MENU_VIEWPORT_MARGIN, window.innerWidth - width - MENU_VIEWPORT_MARGIN),
      );
      setMenuPos((prev) => (prev && prev.top === top && prev.left === left ? prev : { top, left }));
    };
    place();
    // Capture phase: the panel body is the scroller, not the window.
    window.addEventListener("scroll", place, true);
    window.addEventListener("resize", place);
    return () => {
      window.removeEventListener("scroll", place, true);
      window.removeEventListener("resize", place);
    };
  }, [menuOpen]);

  React.useEffect(() => {
    if (signInInProgress) {
      setCodeDraft("");
      requestAnimationFrame(() => codeInputRef.current?.focus());
    }
  }, [signInInProgress]);

  const submitCode = () => {
    const trimmed = codeDraft.trim();
    if (trimmed.length === 0) return;
    onSubmitSignInCode?.(trimmed);
    setCodeDraft("");
  };

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

  const stale = state === "behind";
  const reading = state === "reading" || state === "connecting";
  const hasData = typeof used === "number";

  // R2-2/followup-3: the headline number and bar take their color from the
  // provider-computed severity (the worst of *every* window), not from the
  // headline percentage's own magnitude — the captain's own example is a
  // 20%-weekly account whose session is nearly out, which must still read
  // amber. The handoff's own rule still applies on top: tint only from warn
  // upward, stay neutral below it.
  const numColor = stale
    ? "var(--amber)"
    : severity === "critical"
    ? "var(--red)"
    : severity === "warn"
    ? "var(--amber)"
    : "var(--text-primary)";

  const dotColor = STATE_DOT_COLOR[stale ? "behind" : state] || "var(--status-progress)";

  const handleRowClick = () => {
    if (menuOpen) { onToggleMenu?.(); return; }
    if (renaming) return;
    onToggleExpand?.();
  };

  return (
    <div
      onClick={handleRowClick}
      style={{
        position: "relative",
        display: "flex", flexDirection: "column", gap: "var(--space-2)",
        padding: "var(--space-3)",
        borderRadius: "var(--radius-md)",
        background: expanded || menuOpen ? "var(--bg-row-hover)" : "transparent",
        cursor: "pointer",
        transition: "background var(--dur-fast) var(--ease-standard)",
        ...style,
      }}
    >
      {/* header — fixed order: dot, name+subtitle, pin (if pinned), badge, "…" */}
      <div style={{ display: "flex", alignItems: "flex-start", gap: "var(--space-2)" }}>
        <StatusDot state={state} style={{ marginTop: 5, flex: "0 0 auto" }} />
        <div style={{ flex: 1, minWidth: 0 }}>
          {renaming ? (
            <input
              ref={inputRef}
              value={draft}
              onClick={(e) => e.stopPropagation()}
              onChange={(e) => setDraft(e.target.value)}
              onBlur={commitRename}
              onKeyDown={(e) => {
                if (e.key === "Enter") commitRename();
                if (e.key === "Escape") {
                  e.stopPropagation();
                  setDraft(label);
                  setRenaming(false);
                }
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
        {pinned ? (
          <button type="button" title="Hide from the menu bar" aria-label="Hide from the menu bar"
            onClick={(e) => { e.stopPropagation(); onTogglePin?.(); }}
            style={{
              flex: "0 0 auto", display: "inline-flex", alignItems: "center", justifyContent: "center",
              width: 20, height: 20, margin: "1px 0 0 0", padding: 0, border: 0,
              borderRadius: "var(--radius-xs)", background: "transparent", color: "var(--text-accent)",
              cursor: "pointer",
            }}>
            <PinGlyph />
          </button>
        ) : null}
        {badge ? (
          <Badge tone={stale ? "warn" : "danger"} style={{ marginTop: 2, flex: "0 0 auto" }}>{badge}</Badge>
        ) : null}
        <button type="button" title="More" aria-label="More" data-quotos-menu-scope="true"
          ref={menuButtonRef}
          onClick={(e) => { e.stopPropagation(); onToggleMenu?.(); }}
          style={{
            flex: "0 0 auto", display: "inline-flex", alignItems: "center", justifyContent: "center",
            width: 20, height: 20, margin: "1px -4px 0 0", padding: 0, border: 0,
            borderRadius: "var(--radius-xs)",
            background: menuOpen ? "var(--bg-row-hover)" : "transparent",
            color: menuOpen ? "var(--text-primary)" : "var(--text-quaternary)",
            cursor: "pointer",
          }}>
          <MenuDotsGlyph />
        </button>
      </div>

      {/* body */}
      {signInInProgress ? (
        // R2-6: Claude Code's own sign-in is running for this account (see
        // signin.rs) — it opens the browser itself, so Quotos only needs to
        // relay whatever code comes back. Replaces the reason text while
        // active; the row's own action button is hidden by the caller.
        <div style={{ display: "flex", flexDirection: "column", gap: "var(--space-2)" }}>
          <div style={{
            fontFamily: "var(--font-sans)", fontSize: "var(--text-sm)",
            lineHeight: "var(--leading-snug)", color: "var(--text-secondary)",
          }}>
            Finish signing in in the browser, then paste the code here.
          </div>
          <div style={{ display: "flex", gap: "var(--space-2)" }} onClick={(e) => e.stopPropagation()}>
            <input
              ref={codeInputRef}
              value={codeDraft}
              onChange={(e) => setCodeDraft(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") submitCode();
                if (e.key === "Escape") {
                  e.stopPropagation();
                  setCodeDraft("");
                  onCancelSignIn?.();
                }
              }}
              placeholder="Paste code"
              style={{
                flex: 1, font: "inherit",
                fontFamily: "var(--font-mono)", fontSize: "var(--text-sm)",
                color: "var(--text-primary)",
                background: "var(--bg-input)", border: "0.5px solid var(--border-focus)",
                borderRadius: "var(--radius-xs)", padding: "3px 6px",
                outline: "none",
              }}
            />
            <button type="button" onClick={submitCode} disabled={codeDraft.trim().length === 0}
              style={{
                padding: "0 10px", border: 0, borderRadius: "var(--radius-xs)",
                background: "var(--bg-selected)",
                color: codeDraft.trim().length === 0 ? "var(--text-quaternary)" : "var(--text-accent)",
                fontFamily: "var(--font-sans)", fontSize: "var(--text-sm)", fontWeight: "var(--weight-medium)",
                cursor: codeDraft.trim().length === 0 ? "default" : "pointer",
              }}>Submit</button>
            <button type="button" onClick={() => onCancelSignIn?.()}
              style={{
                padding: "0 8px", border: 0, borderRadius: "var(--radius-xs)",
                background: "transparent", color: "var(--text-tertiary)",
                fontFamily: "var(--font-sans)", fontSize: "var(--text-sm)", cursor: "pointer",
              }}>Cancel</button>
          </div>
        </div>
      ) : hasData ? (
        <>
          <div style={{ display: "flex", alignItems: "flex-end", gap: "var(--space-2)" }}>
            <div style={{ display: "flex", alignItems: "baseline", gap: "var(--space-1)", flex: 1 }}>
              <span style={{
                fontFamily: "var(--font-mono)", fontSize: "var(--numeral-lg)",
                fontWeight: "var(--weight-medium)", fontVariantNumeric: "tabular-nums",
                lineHeight: 1, color: numColor, letterSpacing: "var(--tracking-tighter)",
              }}>{used}<span style={{ fontSize: "18px" }}>%</span></span>
              <span style={{ fontFamily: "var(--font-sans)", fontSize: "var(--text-sm)", color: "var(--text-tertiary)" }}>used</span>
            </div>
            {resetLabel ? (
              <span style={{ fontFamily: "var(--font-sans)", fontSize: "var(--text-xs)", color: "var(--text-tertiary)", paddingBottom: 3 }}>
                {resetLabel}
              </span>
            ) : null}
          </div>
          <CapacityBar used={used} reading={reading} stale={stale} severity={severity} />
        </>
      ) : (
        <div style={{
          fontFamily: "var(--font-sans)", fontSize: "var(--text-sm)",
          lineHeight: "var(--leading-snug)", color: "var(--text-secondary)",
        }}>
          {reason || "No limits reported yet."}
        </div>
      )}

      {/* footer */}
      <div style={{ display: "flex", alignItems: "center", gap: "var(--space-2)", minHeight: 20 }}>
        <span style={{
          flex: 1, fontFamily: "var(--font-sans)", fontSize: "var(--text-xs)",
          color: stale ? "var(--amber)" : "var(--text-tertiary)",
        }}>
          {reading ? "Reading…" : footerNote ? footerNote : lastRead ? `Read ${lastRead}` : "Not read yet"}
        </span>
        {actionLabel ? (
          <button type="button" title={actionLabel}
            onClick={(e) => { e.stopPropagation(); if (!actionDisabled) onAction?.(); }}
            style={{
              height: 20, padding: "0 6px", border: 0, borderRadius: "var(--radius-xs)",
              background: "transparent", fontFamily: "var(--font-sans)", fontSize: "var(--text-xs)",
              fontWeight: "var(--weight-medium)", cursor: actionDisabled ? "default" : "pointer",
              color: actionDisabled ? "var(--text-quaternary)" : "var(--text-accent)",
            }}>{actionLabel}</button>
        ) : null}
        {windows && windows.length > 0 ? (
          <span style={{
            display: "inline-flex", alignItems: "center", gap: 4,
            fontFamily: "var(--font-sans)", fontSize: "var(--text-xs)", color: "var(--text-tertiary)",
          }}>
            {windows.length} {windows.length === 1 ? "limit" : "limits"}
            <Chevron open={expanded} />
          </span>
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

      {/* the "…" menu — one fixed set of actions, always the same place.
          Overlays everything (see the placement effect above): never clipped
          by the panel body, never part of its scroll extent. */}
      {menuOpen ? (
        <div
          ref={menuRef}
          data-quotos-menu-scope="true"
          onClick={(e) => e.stopPropagation()}
          style={{
            position: "fixed",
            top: menuPos ? menuPos.top : 0,
            left: menuPos ? menuPos.left : 0,
            visibility: menuPos ? "visible" : "hidden",
            zIndex: 30, minWidth: MENU_MIN_WIDTH,
            padding: "var(--space-1)", borderRadius: "var(--radius-lg)",
            background: "var(--bg-elevated)", border: "0.5px solid var(--border-default)",
            boxShadow: "var(--shadow-menu)", display: "flex", flexDirection: "column",
          }}
        >
          <MenuItem onClick={() => { onToggleMenu?.(); onReadNow?.(); }}>Read now</MenuItem>
          <MenuItem onClick={() => { onToggleMenu?.(); setRenaming(true); }}>Rename</MenuItem>
          <MenuItem onClick={() => { onToggleMenu?.(); onTogglePin?.(); }}>
            {pinned ? "Hide from menu bar" : "Show in menu bar"}
          </MenuItem>
          <div style={{ height: "0.5px", margin: "var(--space-1) var(--space-2)", background: "var(--border-default)" }} />
          <MenuItem danger onClick={() => { onToggleMenu?.(); onStopTracking?.(); }}>Stop tracking</MenuItem>
        </div>
      ) : null}
    </div>
  );
}
