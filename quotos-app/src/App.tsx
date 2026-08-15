import { useEffect, useState } from "react";
import { Panel } from "./design-system/components/shell/Panel";
import { IconButton } from "./design-system/components/controls/IconButton";
import { Button } from "./design-system/components/controls/Button";
import { SubscriptionRow } from "./design-system/components/subscription/SubscriptionRow";
import { SubscriptionsScreen } from "./components/SubscriptionsScreen";
import { UndoRow } from "./components/UndoRow";
import { useSubscriptions } from "./hooks/useSubscriptions";
import { formatExactReset, formatRelativePast, formatClockTime } from "./lib/time";
import { rowPresentation } from "./lib/rowPresentation";
import { RefreshIcon, PlusIcon, SnapBackIcon, BackIcon, DebugIcon } from "./components/icons";
import {
  hidePanel,
  setDetached as setDetachedIpc,
  debugRateLimitSnapshot,
  onPanelBeakOffset,
  onPanelVisibility,
  dragWindowStep,
  endWindowDrag,
} from "./lib/tauriClient";
import "./app.css";

const NOW_TICK_MS = 30_000;

// Beak offset (px) from the panel's own left edge. B3/B5: the native side
// computes and pushes the real value on every dock/re-dock (see
// `compute_docked_layout` in src-tauri/src/lib.rs — it depends on the tray
// icon's actual position and how much the panel got clamped off it, so it
// can't be a fixed constant there).
//
// This exists *only* for the browser mock harness, which has no real tray
// glyph to measure. The native build deliberately starts at `null` instead:
// if the `panel-beak-offset` event were ever missed, falling back to a
// hardcoded number would draw the beak confidently in a place that is right
// on nobody's screen. Drawing no beak for a frame is the honest failure.
const BEAK_LEFT_MOCK_FALLBACK = 24;

const isTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

function isBlocked(rateLimitedUntil: string | null, now: number): boolean {
  return !!rateLimitedUntil && new Date(rateLimitedUntil).getTime() > now;
}

export default function App() {
  const {
    subscriptions,
    trackedSubscriptions,
    refreshAll,
    refreshAccountById,
    togglePin,
    renameSubscription,
    addSubscription,
    removeSubscription,
    stopTracking,
    undoStopTracking,
    displayLabelFor,
    startSignIn,
    submitSignInCode,
    cancelSignIn,
  } = useSubscriptions();
  const [expanded, setExpanded] = useState<Set<string>>(new Set());
  const [openMenuId, setOpenMenuId] = useState<string | null>(null);
  const [refreshing, setRefreshing] = useState(false);
  const [screen, setScreen] = useState<"list" | "manage">("list");
  const [detached, setDetached] = useState(false);
  const [dragging, setDragging] = useState(false);
  const [position, setPosition] = useState<{ x: number; y: number } | null>(null);
  const [now, setNow] = useState(() => Date.now());
  const [beakLeft, setBeakLeft] = useState<number | null>(isTauri ? null : BEAK_LEFT_MOCK_FALLBACK);

  // B3/B5: keep the beak centered under the real tray glyph position.
  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    void onPanelBeakOffset(setBeakLeft).then((fn) => {
      if (cancelled) fn();
      else unlisten = fn;
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  // Escape dismisses the innermost transient layer first: an open "…" menu
  // closes and the panel stays up; only a bare Escape hides the panel. The
  // rename and sign-in fields own their layer the same way — their handlers
  // stopPropagation, so the key never reaches this window listener.
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      if (openMenuId) {
        setOpenMenuId(null);
        return;
      }
      hidePanel();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [openMenuId]);

  // The menu must not still be hanging open when the panel comes back after
  // a hide — the prototype resets it on every open for the same reason.
  // Only visible=false is acted on: the mock harness fires an initial
  // visible=true at subscribe time.
  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    void onPanelVisibility((visible) => {
      if (!visible) setOpenMenuId(null);
    }).then((fn) => {
      if (cancelled) fn();
      else unlisten = fn;
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  // Keeps relative "ago" text, exact reset copy, and rate-limit availability
  // fresh while the panel sits open — otherwise these would only update on
  // the next data refresh, minutes later.
  useEffect(() => {
    const interval = setInterval(() => setNow(Date.now()), NOW_TICK_MS);
    return () => clearInterval(interval);
  }, []);

  // The "…" menu closes on any interaction outside itself or its trigger —
  // never on interaction with them (see data-quotos-menu-scope in
  // SubscriptionRow.jsx). Only ever closes, never (re)opens, so it can't
  // race the trigger button's own toggle.
  useEffect(() => {
    if (!openMenuId) return;
    const onMouseDown = (event: MouseEvent) => {
      const target = event.target as HTMLElement | null;
      if (target?.closest("[data-quotos-menu-scope]")) return;
      setOpenMenuId(null);
    };
    window.addEventListener("mousedown", onMouseDown);
    return () => window.removeEventListener("mousedown", onMouseDown);
  }, [openMenuId]);

  const toggleExpand = (id: string) => {
    setExpanded((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  const handleManualRefresh = async () => {
    setRefreshing(true);
    try {
      await refreshAll();
    } finally {
      setRefreshing(false);
    }
  };

  // Dragging the header is the only way to detach — there is no detach
  // button. The *visible* detach (losing the beak, gaining the snap-back
  // arrow, no longer closing on click-away) still only happens on the first
  // real movement, not the mousedown itself, so a plain click on the header
  // does nothing — that part is unchanged.
  //
  // R3-3 tried firing Tauri's own `startDragging()` (native
  // `performWindowDragWithEvent:`) synchronously on mousedown, reasoning
  // that the earlier `mousemove`-deferred version missed the live event.
  // That reasoning was sound but the fix wasn't enough on its own — a real
  // hand-drag still did nothing, because `startDragging()`'s IPC call was
  // silently denied by Tauri's ACL (no `core:window:allow-start-dragging`
  // capability was ever granted). That part is a real, closed bug fix.
  //
  // Dragging now moves the window by hand instead of through
  // `startDragging()`: `dragWindowStep()` is invoked on every `mousemove`
  // once a gesture has started, and the Rust side sets the window's frame
  // directly from the live cursor delta (see `drag_window_step`'s doc
  // comment in `src-tauri/src/lib.rs`). That doc comment also records a
  // deliberate, captain-approved tradeoff: live-following the cursor this
  // way reactivates the app for as long as the mouse stays down, same as
  // `performWindowDragWithEvent:` did — measured to be a property of
  // relocating the window's frame *at all* while a mouse-down gesture is
  // live over it, not specific to either API, and not something the R4-1
  // non-activating panel can be asked to fix by itself. Escalated rather
  // than shipped silently; the captain chose live-follow, reactivation
  // scoped to the physical gesture only, over the alternatives on offer.
  //
  // Calling this only after the first real movement (not on the mousedown
  // itself) is what keeps C3 ("a plain click does not detach") true —
  // unchanged from before.
  const handleHeaderPointerDown = (event: React.MouseEvent) => {
    if (event.button !== 0) return;
    if ((event.target as HTMLElement).closest("button, input")) return;
    const startX = event.clientX;
    const startY = event.clientY;
    const panelEl = document.querySelector("[data-quotos-panel]");
    const startRect = panelEl?.getBoundingClientRect() ?? null;
    let started = false;

    const cleanup = () => {
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", onUp);
    };
    const onMove = (moveEvent: MouseEvent) => {
      if (!started) {
        started = true;
        setDragging(true);
        setDetached(true);
        void setDetachedIpc(true);
      }
      if (isTauri) {
        void dragWindowStep();
        return;
      }
      if (startRect) {
        setPosition({
          x: Math.max(8, startRect.left + (moveEvent.clientX - startX)),
          y: Math.max(8, startRect.top + (moveEvent.clientY - startY)),
        });
      }
    };
    const onUp = () => {
      setDragging(false);
      if (isTauri) {
        void endWindowDrag();
      }
      cleanup();
    };
    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
  };

  const handleSnapBack = async () => {
    setDetached(false);
    setDragging(false);
    setPosition(null);
    await setDetachedIpc(false);
  };

  const goManage = () => {
    setOpenMenuId(null);
    setScreen("manage");
  };
  const goList = () => setScreen("list");

  // P7 stretch: a dev-only way to print the app's resolved state as JSON —
  // discovered/tracked subscriptions, parsed windows, last-read times, and
  // the rate budget — without needing screenshots. Gated on Vite's DEV flag
  // so it never appears in a production build's UI.
  const handleDebugDump = async () => {
    const rateLimit = await debugRateLimitSnapshot();
    const dump = {
      dumpedAt: new Date().toISOString(),
      subscriptions,
      rateLimit,
    };
    const json = JSON.stringify(dump, null, 2);
    console.log("[quotos debug state]", dump);
    try {
      await navigator.clipboard.writeText(json);
    } catch {
      // Clipboard permission can be finicky in a dev webview; the console
      // log above is the fallback, not this.
    }
  };

  const nowDate = new Date(now);
  // Same exclusion the rows use (lib/rowPresentation.ts): a subscription
  // whose answer is "sign in" is not waiting on the rate budget, so it must
  // not make the header claim everything is. R4-3: and the summaries describe
  // what is *tracked*, so a row inside its undo window is out of all of them.
  const blockedSubs = trackedSubscriptions.filter((s) => !s.needsSignIn && isBlocked(s.rateLimitedUntil, now));
  const allBlocked = trackedSubscriptions.length > 0 && blockedSubs.length === trackedSubscriptions.length;
  const earliestAvailable = blockedSubs.length
    ? blockedSubs.reduce((min, s) =>
        new Date(s.rateLimitedUntil as string).getTime() < new Date(min.rateLimitedUntil as string).getTime() ? s : min,
      ).rateLimitedUntil
    : null;
  // R3-4: the wait is a tooltip, never a disabled control. Disabling this
  // button while a wait was pending removed the last way to re-test a wrong
  // diagnosis — and the wait itself was a consequence of the wrong
  // diagnosis, so the captain had no way out of the loop at all. Pressing it
  // during a wait is harmless: the Rust limiter refuses without spending
  // anything, and the first press after the budget frees up reads for real.
  const refreshLabel = refreshing
    ? "Reading…"
    : allBlocked
    ? `Waiting for the rate budget — retry at ${formatClockTime(earliestAvailable)}`
    : "Read all now";
  const mostRecentRead = trackedSubscriptions.reduce<string | null>((latest, s) => {
    if (!s.lastReadAt) return latest;
    if (!latest || new Date(s.lastReadAt).getTime() > new Date(latest).getTime()) return s.lastReadAt;
    return latest;
  }, null);
  const footerSummary = refreshing
    ? "Reading…"
    : mostRecentRead
    ? `Last read ${formatRelativePast(mostRecentRead, nowDate)}`
    : "";

  return (
    <Panel
      title={screen === "manage" ? "Subscriptions" : "Quotos"}
      docked={!detached}
      beakLeft={beakLeft ?? undefined}
      dragging={dragging}
      onHeaderPointerDown={handleHeaderPointerDown}
      position={position}
      leading={
        screen === "manage" ? (
          <IconButton label="Back" onClick={goList} style={{ marginLeft: -6 }}>
            <BackIcon />
          </IconButton>
        ) : null
      }
      headerActions={
        <>
          {import.meta.env.DEV && screen === "list" ? (
            <IconButton label="Copy debug state (dev only)" onClick={handleDebugDump}>
              <DebugIcon />
            </IconButton>
          ) : null}
          {screen === "list" ? (
            <IconButton label={refreshLabel} onClick={handleManualRefresh} disabled={refreshing}>
              <RefreshIcon spinning={refreshing} />
            </IconButton>
          ) : null}
          {detached ? (
            <IconButton label="Snap back to the menu bar" onClick={handleSnapBack}>
              <SnapBackIcon />
            </IconButton>
          ) : null}
        </>
      }
      footer={
        screen === "list" ? (
          <>
            <Button variant="ghost" size="sm" icon={<PlusIcon />} onClick={goManage}>
              Add subscription
            </Button>
            <div style={{ flex: 1 }} />
            <span style={{ fontFamily: "var(--font-sans)", fontSize: "var(--text-xs)", color: "var(--text-quaternary)" }}>
              {footerSummary}
            </span>
          </>
        ) : null
      }
    >
      {screen === "manage" ? (
        // R4-3: the *tracked* list, not the panel's row list — a row inside
        // its "Stop tracking" undo window is already untracked, and this
        // screen must not go on offering [Remove] for it (the desync in the
        // captain's 2026-08-15 screencast).
        <SubscriptionsScreen
          tracked={trackedSubscriptions}
          onAdd={addSubscription}
          onRemove={removeSubscription}
          displayLabelFor={displayLabelFor}
        />
      ) : subscriptions.length === 0 ? (
        <div className="quotos-empty">
          <p>Nothing tracked yet. Add a subscription and you'll see what's left on it here.</p>
        </div>
      ) : (
        subscriptions.map((sub) => {
          if (sub.pendingRemoval) {
            return (
              <UndoRow
                key={sub.id}
                label={sub.labelOverride ?? sub.label}
                onUndo={() => undoStopTracking(sub.id)}
              />
            );
          }
          // R2-6: a row that needs signing in offers Claude Code's own
          // sign-in (signin.rs) instead of retrying the same failed read —
          // once a session is running, the row shows the paste-code field
          // instead of this button (see SubscriptionRow's signInInProgress).
          // R3-4: badge, action and footer note are decided together — see
          // lib/rowPresentation.ts for the two rules that keep them from
          // contradicting each other.
          const presentation = rowPresentation(sub, now);
          const label = sub.labelOverride ?? sub.label;
          const windows = sub.windows.map((w) => ({
            id: w.id,
            name: w.name,
            used: w.used,
            resetLabel: formatExactReset(w.resetsAt, nowDate),
            scope: w.scope,
            pinned: sub.pinnedWindowIds.includes(w.id),
          }));
          return (
            <SubscriptionRow
              key={sub.id}
              label={label}
              provider={sub.providerName}
              account={sub.account ?? undefined}
              state={sub.state}
              used={sub.used}
              severity={sub.severity}
              resetLabel={formatExactReset(sub.resetsAt, nowDate) ?? undefined}
              lastRead={formatRelativePast(sub.lastReadAt, nowDate) ?? undefined}
              windows={windows}
              reason={sub.reason ?? undefined}
              badge={presentation.badge}
              pinnedCount={windows.filter((w) => w.pinned).length}
              headlinePinned={sub.headlineWindowId !== null && sub.pinnedWindowIds.includes(sub.headlineWindowId)}
              expanded={expanded.has(sub.id)}
              menuOpen={openMenuId === sub.id}
              actionLabel={presentation.actionLabel}
              footerNote={presentation.footerNote}
              onAction={() => (sub.needsSignIn ? startSignIn(sub.id) : refreshAccountById(sub.id))}
              onReadNow={() => refreshAccountById(sub.id)}
              onTogglePin={() => togglePin(sub.id, sub.headlineWindowId)}
              onToggleWindowPin={(windowId: string) => togglePin(sub.id, windowId)}
              onToggleExpand={() => toggleExpand(sub.id)}
              onToggleMenu={() => setOpenMenuId((prev) => (prev === sub.id ? null : sub.id))}
              onRename={(next: string | null) => renameSubscription(sub.id, next)}
              onStopTracking={() => stopTracking(sub.id)}
              signInInProgress={sub.signInInProgress}
              onSubmitSignInCode={(code: string) => submitSignInCode(sub.id, code)}
              onCancelSignIn={() => cancelSignIn(sub.id)}
            />
          );
        })
      )}
    </Panel>
  );
}
