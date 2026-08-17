import * as React from "react";
import { Badge } from "../indicators/badge";
import { CapacityBar } from "../indicators/capacity-bar";
import type { SubscriptionState } from "../indicators/status-dot";
import { StatusDot } from "../indicators/status-dot";
import { LimitWindow, type LimitWindowProps } from "./limit-window";
import styles from "./subscription-row.module.css";

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

const MENU_GAP = 4;
const MENU_VIEWPORT_MARGIN = 8;
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
  label: string;
  provider?: string;
  account?: string;
  state?: SubscriptionState;
  used?: number | null;
  severity?: "healthy" | "warn" | "critical";
  resetLabel?: string | null;
  lastRead?: string | null;
  windows?: LimitWindowProps[];
  reason?: string | null;
  badge?: "Not current" | "Needs sign-in" | null;
  pinnedCount?: number;
  headlinePinned?: boolean;
  expanded?: boolean;
  menuOpen?: boolean;
  actionLabel?: string | null;
  actionDisabled?: boolean;
  footerNote?: string | null;
  onAction?: () => void;
  onTogglePin?: () => void;
  onToggleWindowPin?: (id: string) => void;
  onToggleExpand?: () => void;
  onToggleMenu?: () => void;
  onRename?: (nextLabel: string | null) => void;
  onReadNow?: () => void;
  canMoveUp?: boolean;
  canMoveDown?: boolean;
  onMoveUp?: () => void;
  onMoveDown?: () => void;
  onStopTracking?: () => void;
  signInInProgress?: boolean;
  onSubmitSignInCode?: (code: string) => void;
  onCancelSignIn?: () => void;
  style?: React.CSSProperties;
}

interface MenuPosition {
  top: number;
  left: number;
}

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
      const left = Math.min(
        Math.max(MENU_VIEWPORT_MARGIN, anchor.right - width),
        Math.max(MENU_VIEWPORT_MARGIN, window.innerWidth - width - MENU_VIEWPORT_MARGIN),
      );
      setMenuPos((prev) => (prev && prev.top === top && prev.left === left ? prev : { top, left }));
    };
    place();
    window.addEventListener("scroll", place, true);
    window.addEventListener("resize", place);
    return () => {
      window.removeEventListener("scroll", place, true);
      window.removeEventListener("resize", place);
    };
  }, [menuOpen]);

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
    if (trimmed === label) return;
    onRename?.(trimmed.length > 0 ? trimmed : null);
  };

  const stale = state === "behind";
  const reading = state === "reading" || state === "connecting";
  const hasData = typeof used === "number";
  const active = expanded || menuOpen;
  const hasWindows = windows.length > 0;

  const usedLevel = stale ? "stale" : severity !== "healthy" ? severity : undefined;

  const handleRowClick = () => {
    if (menuOpen) {
      onToggleMenu?.();
      return;
    }
    if (renaming) return;
    onToggleExpand?.();
  };

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
