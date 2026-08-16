import * as React from "react";
import type { SubscriptionState } from "../indicators/StatusDot";
import { Badge } from "../indicators/Badge";
import { CapacityBar } from "../indicators/CapacityBar";
import { StatusDot } from "../indicators/StatusDot";
import { LimitWindow, type LimitWindowProps } from "./LimitWindow";

// Minimal default affordance glyphs: generic UI arrows and marks, not brand icons.
function Chevron({ open }: { open: boolean }) {
  return (
    <svg
      width="12"
      height="12"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2.2"
      strokeLinecap="round"
      strokeLinejoin="round"
      style={{
        transform: open ? "rotate(180deg)" : "none",
        transition: "transform var(--dur-base) var(--ease-standard)",
      }}
    >
      <path d="M6 9l6 6 6-6" />
    </svg>
  );
}

function PinGlyph({ size = 12 }: { size?: number }) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.8"
      strokeLinecap="round"
      strokeLinejoin="round"
    >
      <path d="M12 17v5M9 10.76V4a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v6.76a2 2 0 0 0 .59 1.41l1.3 1.3A1 1 0 0 1 17.18 15H6.82a1 1 0 0 1-.7-1.71l1.29-1.32A2 2 0 0 0 9 10.76Z" />
    </svg>
  );
}

function MenuDotsGlyph() {
  return (
    <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor">
      <circle cx="5" cy="12" r="1.6" />
      <circle cx="12" cy="12" r="1.6" />
      <circle cx="19" cy="12" r="1.6" />
    </svg>
  );
}

// The row menu's geometry, in viewport coordinates. It is position: fixed
// rather than absolute inside the row, deliberately, see the
// useLayoutEffect below.
const MENU_GAP = 4; // between the "…" button's bottom edge and the menu's top
const MENU_VIEWPORT_MARGIN = 8; // never closer than this to the window's own edge
const MENU_MIN_WIDTH = 168;

function MenuItem({
  danger,
  disabled,
  onClick,
  children,
}: {
  danger?: boolean;
  disabled?: boolean;
  onClick?: () => void;
  children?: React.ReactNode;
}) {
  const [hover, setHover] = React.useState(false);
  return (
    <button
      type="button"
      role="menuitem"
      disabled={disabled}
      onClick={onClick}
      onMouseEnter={() => setHover(true)}
      onMouseLeave={() => setHover(false)}
      style={{
        display: "flex",
        alignItems: "center",
        height: 26,
        padding: "0 var(--space-2)",
        border: 0,
        borderRadius: "var(--radius-sm)",
        background: hover && !disabled ? "var(--bg-row-hover)" : "transparent",
        fontFamily: "var(--font-sans)",
        fontSize: "var(--text-base)",
        color: disabled ? "var(--text-quaternary)" : danger ? "var(--red)" : "var(--text-primary)",
        textAlign: "left",
        cursor: disabled ? "default" : "pointer",
      }}
    >
      {children}
    </button>
  );
}

export interface SubscriptionRowProps {
  /** Renameable label from the provider. Two can look confusingly similar. */
  label: string;
  /** Who issued it, for example "Anthropic". */
  provider?: string;
  /** Account or plan qualifier shown beside the provider, for example "Personal · Max". */
  account?: string;
  state?: SubscriptionState;
  /** Headline percent consumed: the account-wide weekly window, not
   * simply the most-consumed one. Null for no-data states. */
  used?: number | null;
  /** Provider-computed from every window, not just the headline one.
   * Colors the headline number and bar; a 20%-headline account with an
   * 85%-used session still reads amber. */
  severity?: "healthy" | "warn" | "critical";
  /** Reset copy for the binding window, for example "Resets today at 4:05 PM". */
  resetLabel?: string | null;
  /** Relative age of the last successful read, for example "2 min ago". Always shown. */
  lastRead?: string | null;
  /** The variable detail list. Empty hides the expander. */
  windows?: LimitWindowProps[];
  /** Human reason for a no-data state such as broken, idle, or no limits yet. */
  reason?: string | null;
  /** State badge, classified by the caller: "Not current" for held-over
   * numbers, "Needs sign-in" only when signing in is genuinely the answer. */
  badge?: "Not current" | "Needs sign-in" | null;
  /** How many of this subscription's windows are currently pinned. An
   * indicator, not a control. Renders nothing at 0. */
  pinnedCount?: number;
  /** Whether the headline window specifically is pinned. Reflected and
   * toggled, via `onTogglePin`, by the "…" menu's "Show/Hide in menu bar". */
  headlinePinned?: boolean;
  expanded?: boolean;
  /** Whether this row's "…" menu is open. One row's menu open at a time, owned by the caller. */
  menuOpen?: boolean;
  /** Inline footer action label, for example "Try again" or "Open Claude Code". */
  actionLabel?: string | null;
  /** Visually inert but still labelled, for example mid rate-limit wait. */
  actionDisabled?: boolean;
  /** Overrides the computed "Read …" footer text, for example a rate-limit quiet note. */
  footerNote?: string | null;
  onAction?: () => void;
  /** Toggles the headline window's pin. The "…" menu item only. */
  onTogglePin?: () => void;
  /** Toggles a specific window's pin, called with that window's `id`.
   * Wired to each row in the expanded list's own pin button. */
  onToggleWindowPin?: (id: string) => void;
  onToggleExpand?: () => void;
  onToggleMenu?: () => void;
  /** Present enables the inline rename affordance from the "…" menu. Called with the new label, or `null` to clear back to the provider default. */
  onRename?: (nextLabel: string | null) => void;
  /** "Read now" menu item. An explicit one-off refresh, independent of the footer action. */
  onReadNow?: () => void;
  /** Whether "Move up" and "Move down" are possible for this row. An
   * impossible direction renders disabled, never hidden. */
  canMoveUp?: boolean;
  canMoveDown?: boolean;
  /** "Move up" and "Move down" menu items: reorder the panel's rows, and
   * with them the status item's digit order. */
  onMoveUp?: () => void;
  onMoveDown?: () => void;
  /** "Stop tracking" menu item. */
  onStopTracking?: () => void;
  /** A `claude setup-token` session is running for this account. Shows
   * the paste-code field in place of the reason text and hides the
   * caller's own action button. */
  signInInProgress?: boolean;
  /** Called with the pasted code on submit. */
  onSubmitSignInCode?: (code: string) => void;
  /** Cancels the in-progress sign-in. */
  onCancelSignIn?: () => void;
  style?: React.CSSProperties;
}

interface MenuPosition {
  top: number;
  left: number;
}

/**
 * The core panel row: one subscription, scannable in a single pass. Header
 * order is fixed: dot, name and subtitle, pin when pinned, state badge,
 * then the always-visible "…" menu. Nothing shifts, appears, or
 * disappears on hover. Clicking anywhere on the row expands it. An
 * expanded or menu-open row keeps --bg-row-hover so it visibly reads as
 * open. Renders a percent-used headline and capacity bar for data-bearing
 * states, or a message otherwise. Mirrors docs/design/prototype.html's
 * row logic.
 */
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
  badge = null,
  pinnedCount = 0,
  headlinePinned = false,
  expanded = false,
  menuOpen = false,
  actionLabel = null,
  actionDisabled = false,
  footerNote = null,
  onAction,
  onTogglePin,
  onToggleWindowPin,
  onToggleExpand,
  onToggleMenu,
  onRename,
  onReadNow,
  canMoveUp = false,
  canMoveDown = false,
  onMoveUp,
  onMoveDown,
  onStopTracking,
  signInInProgress = false,
  onSubmitSignInCode,
  onCancelSignIn,
  style,
}: SubscriptionRowProps) {
  const [renaming, setRenaming] = React.useState(false);
  const [draft, setDraft] = React.useState(label);
  const inputRef = React.useRef<HTMLInputElement>(null);
  const [codeDraft, setCodeDraft] = React.useState("");
  const codeInputRef = React.useRef<HTMLInputElement>(null);
  const menuButtonRef = React.useRef<HTMLButtonElement>(null);
  const menuRef = React.useRef<HTMLDivElement>(null);
  const [menuPos, setMenuPos] = React.useState<MenuPosition | null>(null);

  // Places the open menu in viewport coordinates, under its own "…" button.
  // Measured after mount rather than computed from constants, since
  // whether the menu opens downward or flips above depends on where the
  // row sits in the window. useLayoutEffect, not useEffect, flushes the
  // resulting state update before paint, so the menu never renders visible
  // at 0,0 for a frame.
  //
  // This depends on nothing above the row establishing a containing block
  // for fixed descendants: no transform, filter, backdrop-filter,
  // perspective, will-change or contain on the panel's own wrappers.
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

  // When the menu closes, focus is often standing on an item that just
  // unmounted, which drops it to <body>. Hand it back to the "…" trigger,
  // but only when focus was genuinely lost: a click that closed the menu
  // by landing somewhere else keeps its own target.
  const wasMenuOpen = React.useRef(false);
  React.useEffect(() => {
    if (wasMenuOpen.current && !menuOpen && document.activeElement === document.body) {
      menuButtonRef.current?.focus();
    }
    wasMenuOpen.current = menuOpen;
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
    // `label` is the composed name, the custom override when one exists,
    // so an unchanged draft must be a no-op, never a clear. Only an
    // explicitly emptied field clears the custom name.
    if (trimmed === label) return;
    onRename?.(trimmed.length > 0 ? trimmed : null);
  };

  const stale = state === "behind";
  const reading = state === "reading" || state === "connecting";
  const hasData = typeof used === "number";

  // The headline number and bar take their color from the
  // provider-computed severity, the worst of every window, not from the
  // headline percentage's own magnitude. Tints only from warn upward,
  // stays neutral below it.
  const numColor = stale
    ? "var(--amber)"
    : severity === "critical"
      ? "var(--red)"
      : severity === "warn"
        ? "var(--amber)"
        : "var(--text-primary)";

  const handleRowClick = () => {
    if (menuOpen) {
      onToggleMenu?.();
      return;
    }
    if (renaming) return;
    onToggleExpand?.();
  };

  // ArrowUp and ArrowDown walk the enabled items, wrapping like a native
  // NSMenu. Home and End jump to the edges. Lives on the row div because
  // the fixed-position dropdown is still this row's DOM child, so keydowns
  // bubble through here. Dismissal is not handled here: Escape and
  // click-away stay with the window-level layering in App.tsx. The sign-in
  // code field is excluded so its caret keeps the arrow keys.
  const handleMenuKeyDown = (e: React.KeyboardEvent<HTMLDivElement>) => {
    if (!menuOpen || !menuRef.current) return;
    if ((e.target as HTMLElement).tagName === "INPUT") return;
    if (e.key !== "ArrowDown" && e.key !== "ArrowUp" && e.key !== "Home" && e.key !== "End") return;
    const items = Array.from(menuRef.current.querySelectorAll("button")).filter((b) => !b.disabled);
    if (items.length === 0) return;
    e.preventDefault();
    const current = items.indexOf(document.activeElement as HTMLButtonElement);
    const next =
      e.key === "Home"
        ? 0
        : e.key === "End"
          ? items.length - 1
          : e.key === "ArrowDown"
            ? current < 0
              ? 0
              : (current + 1) % items.length
            : current < 0
              ? items.length - 1
              : (current - 1 + items.length) % items.length;
    items[next].focus();
  };

  return (
    <div
      onClick={handleRowClick}
      onKeyDown={handleMenuKeyDown}
      style={{
        position: "relative",
        display: "flex",
        flexDirection: "column",
        gap: "var(--space-2)",
        padding: "var(--space-3)",
        borderRadius: "var(--radius-md)",
        background: expanded || menuOpen ? "var(--bg-row-hover)" : "transparent",
        cursor: "pointer",
        transition: "background var(--dur-fast) var(--ease-standard)",
        ...style,
      }}
    >
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
                width: "100%",
                font: "inherit",
                fontFamily: "var(--font-sans)",
                fontSize: "var(--text-md)",
                fontWeight: "var(--weight-semibold)",
                color: "var(--text-primary)",
                letterSpacing: "var(--tracking-tight)",
                background: "var(--bg-input)",
                border: "0.5px solid var(--border-focus)",
                borderRadius: "var(--radius-xs)",
                padding: "1px 4px",
                margin: "-1px -4px",
                outline: "none",
              }}
            />
          ) : (
            <div
              style={{
                fontFamily: "var(--font-sans)",
                fontSize: "var(--text-md)",
                fontWeight: "var(--weight-semibold)",
                color: "var(--text-primary)",
                letterSpacing: "var(--tracking-tight)",
                overflow: "hidden",
                textOverflow: "ellipsis",
                whiteSpace: "nowrap",
              }}
            >
              {label}
            </div>
          )}
          {provider || account ? (
            <div
              style={{
                fontFamily: "var(--font-sans)",
                fontSize: "var(--text-sm)",
                color: "var(--text-tertiary)",
                overflow: "hidden",
                textOverflow: "ellipsis",
                whiteSpace: "nowrap",
              }}
            >
              {[account, provider].filter(Boolean).join(" · ")}
            </div>
          ) : null}
        </div>
        {pinnedCount > 0 ? (
          <Badge tone="accent" style={{ flex: "0 0 auto", marginTop: 2, gap: 3 }}>
            <PinGlyph size={10} />
            {pinnedCount}
          </Badge>
        ) : null}
        {badge ? (
          <Badge tone={stale ? "warn" : "danger"} style={{ marginTop: 2, flex: "0 0 auto" }}>
            {badge}
          </Badge>
        ) : null}
        <button
          type="button"
          title="More"
          aria-label="More"
          aria-haspopup="menu"
          aria-expanded={menuOpen}
          data-quotos-menu-scope="true"
          ref={menuButtonRef}
          onClick={(e) => {
            e.stopPropagation();
            onToggleMenu?.();
          }}
          style={{
            flex: "0 0 auto",
            display: "inline-flex",
            alignItems: "center",
            justifyContent: "center",
            width: 20,
            height: 20,
            margin: "1px -4px 0 0",
            padding: 0,
            border: 0,
            borderRadius: "var(--radius-xs)",
            background: menuOpen ? "var(--bg-row-hover)" : "transparent",
            color: menuOpen ? "var(--text-primary)" : "var(--text-quaternary)",
            cursor: "pointer",
          }}
        >
          <MenuDotsGlyph />
        </button>
      </div>

      {signInInProgress ? (
        // Claude Code's own sign-in is running for this account, see
        // signin.rs. It opens the browser itself, so Quotos only relays
        // whatever code comes back. Replaces the reason text while active.
        <div style={{ display: "flex", flexDirection: "column", gap: "var(--space-2)" }}>
          <div
            style={{
              fontFamily: "var(--font-sans)",
              fontSize: "var(--text-sm)",
              lineHeight: "var(--leading-snug)",
              color: "var(--text-secondary)",
            }}
          >
            Finish signing in in the browser, then paste the code here.
          </div>
          <div
            style={{ display: "flex", gap: "var(--space-2)" }}
            onClick={(e) => e.stopPropagation()}
          >
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
                flex: 1,
                font: "inherit",
                fontFamily: "var(--font-mono)",
                fontSize: "var(--text-sm)",
                color: "var(--text-primary)",
                background: "var(--bg-input)",
                border: "0.5px solid var(--border-focus)",
                borderRadius: "var(--radius-xs)",
                padding: "3px 6px",
                outline: "none",
              }}
            />
            <button
              type="button"
              onClick={submitCode}
              disabled={codeDraft.trim().length === 0}
              style={{
                padding: "0 10px",
                border: 0,
                borderRadius: "var(--radius-xs)",
                background: "var(--bg-selected)",
                color:
                  codeDraft.trim().length === 0 ? "var(--text-quaternary)" : "var(--text-accent)",
                fontFamily: "var(--font-sans)",
                fontSize: "var(--text-sm)",
                fontWeight: "var(--weight-medium)",
                cursor: codeDraft.trim().length === 0 ? "default" : "pointer",
              }}
            >
              Submit
            </button>
            <button
              type="button"
              onClick={() => onCancelSignIn?.()}
              style={{
                padding: "0 8px",
                border: 0,
                borderRadius: "var(--radius-xs)",
                background: "transparent",
                color: "var(--text-tertiary)",
                fontFamily: "var(--font-sans)",
                fontSize: "var(--text-sm)",
                cursor: "pointer",
              }}
            >
              Cancel
            </button>
          </div>
        </div>
      ) : hasData ? (
        <>
          <div style={{ display: "flex", alignItems: "flex-end", gap: "var(--space-2)" }}>
            <div
              style={{ display: "flex", alignItems: "baseline", gap: "var(--space-1)", flex: 1 }}
            >
              <span
                style={{
                  fontFamily: "var(--font-mono)",
                  fontSize: "var(--numeral-lg)",
                  fontWeight: "var(--weight-medium)",
                  fontVariantNumeric: "tabular-nums",
                  lineHeight: 1,
                  color: numColor,
                  letterSpacing: "var(--tracking-tighter)",
                }}
              >
                {used}
                <span style={{ fontSize: "18px" }}>%</span>
              </span>
              <span
                style={{
                  fontFamily: "var(--font-sans)",
                  fontSize: "var(--text-sm)",
                  color: "var(--text-tertiary)",
                }}
              >
                used
              </span>
            </div>
            {resetLabel ? (
              <span
                style={{
                  fontFamily: "var(--font-sans)",
                  fontSize: "var(--text-xs)",
                  color: "var(--text-tertiary)",
                  paddingBottom: 3,
                }}
              >
                {resetLabel}
              </span>
            ) : null}
          </div>
          <CapacityBar used={used} reading={reading} stale={stale} severity={severity} />
        </>
      ) : (
        <div
          style={{
            fontFamily: "var(--font-sans)",
            fontSize: "var(--text-sm)",
            lineHeight: "var(--leading-snug)",
            color: "var(--text-secondary)",
          }}
        >
          {reason || "No limits reported yet."}
        </div>
      )}

      <div style={{ display: "flex", alignItems: "center", gap: "var(--space-2)", minHeight: 20 }}>
        <span
          style={{
            flex: 1,
            fontFamily: "var(--font-sans)",
            fontSize: "var(--text-xs)",
            color: stale ? "var(--amber)" : "var(--text-tertiary)",
          }}
        >
          {reading
            ? "Reading…"
            : footerNote
              ? footerNote
              : lastRead
                ? `Read ${lastRead}`
                : "Not read yet"}
        </span>
        {actionLabel ? (
          <button
            type="button"
            title={actionLabel}
            onClick={(e) => {
              e.stopPropagation();
              if (!actionDisabled) onAction?.();
            }}
            style={{
              height: 20,
              padding: "0 6px",
              border: 0,
              borderRadius: "var(--radius-xs)",
              background: "transparent",
              fontFamily: "var(--font-sans)",
              fontSize: "var(--text-xs)",
              fontWeight: "var(--weight-medium)",
              cursor: actionDisabled ? "default" : "pointer",
              color: actionDisabled ? "var(--text-quaternary)" : "var(--text-accent)",
            }}
          >
            {actionLabel}
          </button>
        ) : null}
        {windows && windows.length > 0 ? (
          // A real button, not a span, so this stays reachable by keyboard
          // and visible to the accessibility tree even though the row
          // div's own onClick already handles pointer clicks.
          // stopPropagation keeps the row's click from toggling it
          // straight back.
          <button
            type="button"
            aria-expanded={expanded}
            onClick={(e) => {
              e.stopPropagation();
              onToggleExpand?.();
            }}
            style={{
              display: "inline-flex",
              alignItems: "center",
              gap: 4,
              margin: 0,
              padding: 0,
              border: 0,
              background: "transparent",
              fontFamily: "var(--font-sans)",
              fontSize: "var(--text-xs)",
              color: "var(--text-tertiary)",
              cursor: "pointer",
            }}
          >
            {windows.length} {windows.length === 1 ? "limit" : "limits"}
            <Chevron open={expanded} />
          </button>
        ) : null}
      </div>

      {/* Always mounted, animated via grid-template-rows so expand and
          collapse are a deliberate motion. Collapsed, it must also be
          visibility-hidden, not merely clipped: overflow alone leaves the
          per-window pin buttons in the tab order, reachable by keyboard
          with nothing on screen. visibility transitions on the same
          token, so it animates discretely, staying visible while the row
          closes and only then dropping out of the tab order. */}
      <div
        aria-hidden={!expanded}
        style={{
          display: "grid",
          gridTemplateRows: expanded && windows && windows.length > 0 ? "1fr" : "0fr",
          visibility: expanded && windows && windows.length > 0 ? "visible" : "hidden",
          // biome-ignore format: reducedMotion.spec.tsx scans this line for a --dur token; keep it on one line.
          transition: "grid-template-rows var(--dur-base) var(--ease-standard), visibility var(--dur-base) var(--ease-standard)",
        }}
      >
        <div style={{ overflow: "hidden", minHeight: 0 }}>
          {windows && windows.length > 0 ? (
            <div
              style={{
                borderTop: "0.5px solid var(--border-subtle)",
                paddingTop: "var(--space-1)",
                marginTop: "var(--space-0-5)",
              }}
            >
              {windows.map((w, i) => (
                <LimitWindow
                  key={w.id ?? i}
                  {...w}
                  stale={stale}
                  onTogglePin={onToggleWindowPin}
                  style={
                    i < windows.length - 1
                      ? { borderBottom: "0.5px solid var(--border-subtle)" }
                      : undefined
                  }
                />
              ))}
            </div>
          ) : null}
        </div>
      </div>

      {/* One fixed set of actions, always the same place. Overlays
          everything: never clipped by the panel body, never part of its
          scroll extent. */}
      {menuOpen ? (
        <div
          ref={menuRef}
          role="menu"
          aria-label="Subscription actions"
          data-quotos-menu-scope="true"
          onClick={(e) => e.stopPropagation()}
          style={{
            position: "fixed",
            top: menuPos ? menuPos.top : 0,
            left: menuPos ? menuPos.left : 0,
            visibility: menuPos ? "visible" : "hidden",
            zIndex: 30,
            minWidth: MENU_MIN_WIDTH,
            padding: "var(--space-1)",
            borderRadius: "var(--radius-lg)",
            background: "var(--bg-elevated)",
            border: "0.5px solid var(--border-default)",
            boxShadow: "var(--shadow-menu)",
            display: "flex",
            flexDirection: "column",
          }}
        >
          <MenuItem
            onClick={() => {
              onToggleMenu?.();
              onReadNow?.();
            }}
          >
            Read now
          </MenuItem>
          <MenuItem
            onClick={() => {
              onToggleMenu?.();
              setRenaming(true);
            }}
          >
            Rename
          </MenuItem>
          <MenuItem
            onClick={() => {
              onToggleMenu?.();
              onTogglePin?.();
            }}
          >
            {headlinePinned ? "Hide from menu bar" : "Show in menu bar"}
          </MenuItem>
          <MenuItem
            disabled={!canMoveUp}
            onClick={() => {
              onToggleMenu?.();
              onMoveUp?.();
            }}
          >
            Move up
          </MenuItem>
          <MenuItem
            disabled={!canMoveDown}
            onClick={() => {
              onToggleMenu?.();
              onMoveDown?.();
            }}
          >
            Move down
          </MenuItem>
          <div
            role="separator"
            style={{
              height: "0.5px",
              margin: "var(--space-1) var(--space-2)",
              background: "var(--border-default)",
            }}
          />
          <MenuItem
            danger
            onClick={() => {
              onToggleMenu?.();
              onStopTracking?.();
            }}
          >
            Stop tracking
          </MenuItem>
        </div>
      ) : null}
    </div>
  );
}
