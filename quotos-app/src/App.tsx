import { useEffect, useState } from "react";
import { Panel } from "./design-system/components/shell/Panel";
import { IconButton } from "./design-system/components/controls/IconButton";
import { Button } from "./design-system/components/controls/Button";
import { SubscriptionRow } from "./design-system/components/subscription/SubscriptionRow";
import { AddSubscriptionSheet } from "./components/AddSubscriptionSheet";
import { useSubscriptions } from "./hooks/useSubscriptions";
import { formatExactReset, formatRelativePast, formatClockTime } from "./lib/time";
import { RefreshIcon, PlusIcon, DetachIcon, AttachIcon, CloseIcon, DebugIcon } from "./components/icons";
import { hidePanel, setDetached as setDetachedIpc, debugRateLimitSnapshot } from "./lib/tauriClient";
import "./app.css";

const ACTIONABLE_STATES = new Set(["idle", "broken"]);
const APP_LABEL = "Quotos";
const NOW_TICK_MS = 30_000;

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
  } = useSubscriptions();
  const [expanded, setExpanded] = useState<Set<string>>(new Set());
  const [refreshing, setRefreshing] = useState(false);
  const [addingOpen, setAddingOpen] = useState(false);
  const [detached, setDetached] = useState(false);
  const [now, setNow] = useState(() => Date.now());

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

  const handleToggleDetach = async () => {
    const next = !detached;
    setDetached(next);
    await setDetachedIpc(next);
  };

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
  const refreshLabel = allBlocked
    ? `Waiting for the rate budget — available ${formatClockTime(earliestAvailable)}`
    : "Refresh all";

  return (
    <Panel
      title={addingOpen ? "Add subscription" : APP_LABEL}
      headerActions={
        addingOpen ? (
          <IconButton label="Close" onClick={() => setAddingOpen(false)}>
            <CloseIcon />
          </IconButton>
        ) : (
          <>
            {import.meta.env.DEV ? (
              <IconButton label="Copy debug state (dev only)" onClick={handleDebugDump}>
                <DebugIcon />
              </IconButton>
            ) : null}
            <IconButton
              label={detached ? "Dock back to the menu bar" : "Detach into its own window"}
              active={detached}
              onClick={handleToggleDetach}
            >
              {detached ? <AttachIcon /> : <DetachIcon />}
            </IconButton>
            <IconButton
              label={refreshLabel}
              onClick={handleManualRefresh}
              disabled={refreshing || allBlocked}
            >
              <RefreshIcon spinning={refreshing} />
            </IconButton>
          </>
        )
      }
      footer={
        addingOpen ? null : (
          <Button variant="ghost" size="sm" icon={<PlusIcon />} onClick={() => setAddingOpen(true)}>
            Add subscription
          </Button>
        )
      }
    >
      {addingOpen ? (
        <AddSubscriptionSheet
          tracked={subscriptions}
          onAdd={addSubscription}
          onClose={() => setAddingOpen(false)}
        />
      ) : subscriptions.length === 0 ? (
        <div className="quotos-empty">
          <p>No subscriptions tracked yet.</p>
          <Button variant="secondary" size="sm" icon={<PlusIcon />} onClick={() => setAddingOpen(true)}>
            Add subscription
          </Button>
        </div>
      ) : (
        subscriptions.map((sub) => {
          const blocked = isBlocked(sub.rateLimitedUntil, now);
          const actionable = ACTIONABLE_STATES.has(sub.state);
          const label = sub.labelOverride ?? sub.label;
          return (
            <SubscriptionRow
              key={sub.id}
              label={label}
              provider={sub.providerName}
              account={sub.account ?? undefined}
              state={sub.state}
              used={sub.used}
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
              actionLabel={actionable ? (blocked ? `Retry at ${formatClockTime(sub.rateLimitedUntil)}` : "Retry") : null}
              actionDisabled={blocked}
              footerNote={blocked && !sub.lastReadAt ? `Waiting for the rate budget — available ${formatClockTime(sub.rateLimitedUntil)}` : null}
              onAction={() => refreshAccountById(sub.id)}
              onTogglePin={() => togglePin(sub.id)}
              onToggleExpand={() => toggleExpand(sub.id)}
              onRename={(next: string | null) => renameSubscription(sub.id, next)}
              onDelete={() => removeSubscription(sub.id)}
            />
          );
        })
      )}
    </Panel>
  );
}
