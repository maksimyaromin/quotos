import { useEffect, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Panel } from "./design-system/components/shell/Panel";
import { IconButton } from "./design-system/components/controls/IconButton";
import { Button } from "./design-system/components/controls/Button";
import { SubscriptionRow } from "./design-system/components/subscription/SubscriptionRow";
import { SubscriptionsScreen } from "./components/SubscriptionsScreen";
import { UndoRow } from "./components/UndoRow";
import { useSubscriptions } from "./hooks/useSubscriptions";
import { formatExactReset, formatRelativePast, formatClockTime } from "./lib/time";
import { RefreshIcon, PlusIcon, SnapBackIcon, BackIcon, DebugIcon } from "./components/icons";
import { hidePanel, setDetached as setDetachedIpc, debugRateLimitSnapshot, onPanelBeakOffset } from "./lib/tauriClient";
import "./app.css";

const NOW_TICK_MS = 30_000;
const STOP_TRACKING_UNDO_MS = 5_000;

// Beak center offset (px) from the panel's own left edge. B3/B5: the native
// side now computes and pushes the real value on every dock/re-dock (see
// `compute_docked_layout` in src-tauri/src/lib.rs — it depends on the tray
// icon's actual position and how much the panel got clamped off it, so it
// can't be a fixed constant there). This stays only as the fallback for the
// browser mock harness, which has no real tray glyph to measure.
const BEAK_LEFT_FALLBACK = 24;

const isTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

function isBlocked(rateLimitedUntil: string | null, now: number): boolean {
  return !!rateLimitedUntil && new Date(rateLimitedUntil).getTime() > now;
}

export default function App() {
  const {
    subscriptions,
    refreshAll,
    refreshAccountById,
    togglePin,
    renameSubscription,
    addSubscription,
    removeSubscription,
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
  const [pendingRemovals, setPendingRemovals] = useState<Record<string, { label: string }>>({});
  const removalTimers = useRef<Record<string, number>>({});
  const [beakLeft, setBeakLeft] = useState(BEAK_LEFT_FALLBACK);

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

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") hidePanel();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
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

  useEffect(() => {
    return () => {
      Object.values(removalTimers.current).forEach((id) => clearTimeout(id));
    };
  }, []);

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

  const handleStopTracking = (id: string, label: string) => {
    setPendingRemovals((prev) => ({ ...prev, [id]: { label } }));
    const timerId = window.setTimeout(() => {
      removeSubscription(id);
      setPendingRemovals((prev) => {
        const next = { ...prev };
        delete next[id];
        return next;
      });
      delete removalTimers.current[id];
    }, STOP_TRACKING_UNDO_MS);
    removalTimers.current[id] = timerId;
  };

  const handleUndo = (id: string) => {
    const timerId = removalTimers.current[id];
    if (timerId) {
      clearTimeout(timerId);
      delete removalTimers.current[id];
    }
    setPendingRemovals((prev) => {
      const next = { ...prev };
      delete next[id];
      return next;
    });
  };

  // Dragging the header is the only way to detach — there is no detach
  // button. The *visible* detach (losing the beak, gaining the snap-back
  // arrow, no longer closing on click-away) still only happens on the first
  // real movement, not the mousedown itself, so a plain click on the header
  // does nothing — that part is unchanged.
  //
  // R3-3 fix: starting the *native* drag itself used to wait for that same
  // first-movement signal, calling `startDragging()` from the `mousemove`
  // handler. On macOS that is what silently broke dragging entirely — the
  // captain's "the window doesn't drag at all." `startDragging()` calls into
  // tao's `drag_window()`, which hands `NSWindow.performWindowDragWithEvent`
  // whatever `NSApp.currentEvent()` happens to be *when the Rust side gets
  // around to running it* (it only substitutes a synthesized event for one
  // narrow stale-event case, not the general one) — and that call arrives
  // over an async `invoke()` IPC round-trip, so by the time it lands,
  // `currentEvent` is essentially never still the original mouseDown.
  // Apple's own docs say to call `performWindowDragWithEvent` from
  // `mouseDown:` itself; that only works here if `startDragging()` is fired
  // synchronously on the real mousedown, not deferred to a later movement
  // event — confirmed live: deferring it (the old code) left the window
  // stationary through an entire synthetic drag even though this component's
  // own movement tracking correctly saw it and flipped to detached; calling
  // it immediately here is what actually moves the window (see RESULT.md).
  // Calling it on a click that never moves is harmless — AppKit's own
  // tracking loop treats a mouseDown immediately followed by mouseUp with no
  // movement as a no-op, so C3 ("a plain click does not detach") still holds
  // for the *visible* state below, which is untouched by this.
  const handleHeaderPointerDown = (event: React.MouseEvent) => {
    if (event.button !== 0) return;
    if ((event.target as HTMLElement).closest("button, input")) return;
    if (isTauri) {
      void getCurrentWindow().startDragging();
    }
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
        if (isTauri) {
          cleanup();
          return;
        }
      }
      if (!isTauri && startRect) {
        setPosition({
          x: Math.max(8, startRect.left + (moveEvent.clientX - startX)),
          y: Math.max(8, startRect.top + (moveEvent.clientY - startY)),
        });
      }
    };
    const onUp = () => {
      setDragging(false);
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
  const blockedSubs = subscriptions.filter((s) => isBlocked(s.rateLimitedUntil, now));
  const allBlocked = subscriptions.length > 0 && blockedSubs.length === subscriptions.length;
  const earliestAvailable = blockedSubs.length
    ? blockedSubs.reduce((min, s) =>
        new Date(s.rateLimitedUntil as string).getTime() < new Date(min.rateLimitedUntil as string).getTime() ? s : min,
      ).rateLimitedUntil
    : null;
  const refreshLabel = refreshing
    ? "Reading…"
    : allBlocked
    ? `Waiting for the rate budget — available ${formatClockTime(earliestAvailable)}`
    : "Read all now";
  const mostRecentRead = subscriptions.reduce<string | null>((latest, s) => {
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
      beakLeft={beakLeft}
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
            <IconButton label={refreshLabel} onClick={handleManualRefresh} disabled={refreshing || allBlocked}>
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
        <SubscriptionsScreen tracked={subscriptions} onAdd={addSubscription} onRemove={removeSubscription} />
      ) : subscriptions.length === 0 ? (
        <div className="quotos-empty">
          <p>Nothing tracked yet. Add a subscription and you'll see what's left on it here.</p>
        </div>
      ) : (
        subscriptions.map((sub) => {
          if (pendingRemovals[sub.id]) {
            return <UndoRow key={sub.id} label={pendingRemovals[sub.id].label} onUndo={() => handleUndo(sub.id)} />;
          }
          const blocked = isBlocked(sub.rateLimitedUntil, now);
          // R2-6: the broken row's action starts Claude Code's own sign-in
          // (signin.rs) instead of just retrying the same failed read —
          // once a session is running, the row shows the paste-code field
          // instead of this button (see SubscriptionRow's signInInProgress).
          const baseAction =
            sub.state === "broken" && !sub.signInInProgress
              ? "Open Claude Code"
              : sub.state === "behind"
              ? "Try again"
              : null;
          const label = sub.labelOverride ?? sub.label;
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
              windows={sub.windows.map((w) => ({
                name: w.name,
                used: w.used,
                resetLabel: formatExactReset(w.resetsAt, nowDate),
                scope: w.scope,
              }))}
              reason={sub.reason ?? undefined}
              pinned={sub.pinned}
              expanded={expanded.has(sub.id)}
              menuOpen={openMenuId === sub.id}
              actionLabel={baseAction ? (blocked ? `Retry at ${formatClockTime(sub.rateLimitedUntil)}` : baseAction) : null}
              actionDisabled={blocked}
              footerNote={blocked && !sub.lastReadAt ? `Waiting for the rate budget — available ${formatClockTime(sub.rateLimitedUntil)}` : null}
              onAction={() => (sub.state === "broken" ? startSignIn(sub.id) : refreshAccountById(sub.id))}
              onReadNow={() => refreshAccountById(sub.id)}
              onTogglePin={() => togglePin(sub.id)}
              onToggleExpand={() => toggleExpand(sub.id)}
              onToggleMenu={() => setOpenMenuId((prev) => (prev === sub.id ? null : sub.id))}
              onRename={(next: string | null) => renameSubscription(sub.id, next)}
              onStopTracking={() => handleStopTracking(sub.id, label)}
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
