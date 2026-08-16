import * as React from "react";
import type { SubscriptionState } from "../indicators/StatusDot";
import { Badge } from "../indicators/Badge";
import { CapacityBar } from "../indicators/CapacityBar";
import { StatusDot } from "../indicators/StatusDot";
import { LimitWindow, type LimitWindowProps } from "./LimitWindow";
import styles from "./SubscriptionRow.module.css";

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
      className={styles.chevron}
      data-open={open ? "true" : undefined}
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

function nextMenuIndex(key: string, current: number, length: number): number {
  if (key === "Home") return 0;
  if (key === "End") return length - 1;
  if (key === "ArrowDown") return current < 0 ? 0 : (current + 1) % length;
  return current < 0 ? length - 1 : (current - 1 + length) % length;
}

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
  return (
    <button
      type="button"
      role="menuitem"
      disabled={disabled}
      onClick={onClick}
      data-danger={danger ? "true" : undefined}
      className={styles.menuItem}
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
  const active = expanded || menuOpen;
  const hasWindows = windows.length > 0;

  // The headline number and bar take their color from the
  // provider-computed severity, the worst of every window, not from the
  // headline percentage's own magnitude. Tints only from warn upward,
  // stays neutral below it.
  const usedLevel = stale ? "stale" : severity !== "healthy" ? severity : undefined;

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
    items[nextMenuIndex(e.key, current, items.length)].focus();
  };

  return (
    <div
      onClick={handleRowClick}
      onKeyDown={handleMenuKeyDown}
      data-active={active ? "true" : undefined}
      className={styles.row}
      style={style}
    >
      <div className={styles.header}>
        <StatusDot state={state} className={styles.statusDot} />
        <div className={styles.titleArea}>
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
              className={styles.renameInput}
            />
          ) : (
            <div className={styles.title}>{label}</div>
          )}
          {provider || account ? (
            <div className={styles.subtitle}>{[account, provider].filter(Boolean).join(" · ")}</div>
          ) : null}
        </div>
        {pinnedCount > 0 ? (
          <Badge tone="accent" className={styles.pinnedBadge}>
            <PinGlyph size={10} />
            {pinnedCount}
          </Badge>
        ) : null}
        {badge ? (
          <Badge tone={stale ? "warn" : "danger"} className={styles.stateBadge}>
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
          data-open={menuOpen ? "true" : undefined}
          ref={menuButtonRef}
          onClick={(e) => {
            e.stopPropagation();
            onToggleMenu?.();
          }}
          className={styles.menuButton}
        >
          <MenuDotsGlyph />
        </button>
      </div>

      {signInInProgress ? (
        // Claude Code's own sign-in is running for this account, see
        // signin.rs. It opens the browser itself, so Quotos only relays
        // whatever code comes back. Replaces the reason text while active.
        <div className={styles.signInBlock}>
          <div className={styles.signInText}>
            Finish signing in in the browser, then paste the code here.
          </div>
          <div className={styles.codeRow} onClick={(e) => e.stopPropagation()}>
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
              className={styles.codeInput}
            />
            <button
              type="button"
              onClick={submitCode}
              disabled={codeDraft.trim().length === 0}
              className={styles.submitButton}
            >
              Submit
            </button>
            <button
              type="button"
              onClick={() => onCancelSignIn?.()}
              className={styles.cancelButton}
            >
              Cancel
            </button>
          </div>
        </div>
      ) : hasData ? (
        <>
          <div className={styles.dataRow}>
            <div className={styles.usedGroup}>
              <span className={styles.usedNumber} data-level={usedLevel}>
                {used}
                <span className={styles.percentSign}>%</span>
              </span>
              <span className={styles.usedLabel}>used</span>
            </div>
            {resetLabel ? <span className={styles.resetLabel}>{resetLabel}</span> : null}
          </div>
          <CapacityBar used={used} reading={reading} stale={stale} severity={severity} />
        </>
      ) : (
        <div className={styles.reasonText}>{reason || "No limits reported yet."}</div>
      )}

      <div className={styles.footer}>
        <span className={styles.footerNote} data-stale={stale ? "true" : undefined}>
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
            disabled={actionDisabled}
            onClick={(e) => {
              e.stopPropagation();
              onAction?.();
            }}
            className={styles.actionButton}
          >
            {actionLabel}
          </button>
        ) : null}
        {hasWindows ? (
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
            className={styles.disclosureButton}
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
        data-expanded={expanded && hasWindows ? "true" : undefined}
        className={styles.detailWrapper}
      >
        <div className={styles.detailInner}>
          {hasWindows ? (
            <div className={styles.windowsList}>
              {windows.map((w, i) => (
                <LimitWindow key={w.id ?? i} {...w} stale={stale} onTogglePin={onToggleWindowPin} />
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
          className={styles.rowMenu}
          style={{
            top: menuPos ? menuPos.top : 0,
            left: menuPos ? menuPos.left : 0,
            visibility: menuPos ? "visible" : "hidden",
            minWidth: MENU_MIN_WIDTH,
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
          <div role="separator" className={styles.separator} />
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
