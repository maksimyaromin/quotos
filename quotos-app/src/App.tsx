import { useEffect, useState } from "react";
import { Panel } from "./design-system/components/shell/Panel";
import { IconButton } from "./design-system/components/controls/IconButton";
import { Button } from "./design-system/components/controls/Button";
import { SubscriptionRow } from "./design-system/components/subscription/SubscriptionRow";
import { useSubscriptions } from "./hooks/useSubscriptions";
import { formatRelativeFuture, formatRelativePast } from "./lib/time";
import { RefreshIcon, PlusIcon } from "./components/icons";
import { hidePanel } from "./lib/tauriClient";
import "./app.css";

const ACTIONABLE_STATES = new Set(["idle", "broken"]);

export default function App() {
  const { subscriptions, refreshAll, refreshAccountById, togglePin } = useSubscriptions();
  const [expanded, setExpanded] = useState<Set<string>>(new Set());
  const [refreshing, setRefreshing] = useState(false);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") hidePanel();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
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

  return (
    <Panel
      title="Quotos"
      headerActions={
        <IconButton label="Refresh all" onClick={handleManualRefresh} disabled={refreshing}>
          <RefreshIcon spinning={refreshing} />
        </IconButton>
      }
      footer={
        <Button variant="ghost" size="sm" icon={<PlusIcon />} disabled title="Add subscription — coming soon">
          Add subscription
        </Button>
      }
    >
      {subscriptions.length === 0 ? (
        <div className="quotos-empty">No subscriptions found yet.</div>
      ) : (
        subscriptions.map((sub) => (
          <SubscriptionRow
            key={sub.id}
            label={sub.label}
            provider={sub.providerName}
            account={sub.account ?? undefined}
            state={sub.state}
            remaining={sub.remaining}
            resetLabel={formatRelativeFuture(sub.resetsAt) ?? undefined}
            lastRead={formatRelativePast(sub.lastReadAt) ?? undefined}
            windows={sub.windows.map((w) => ({
              name: w.name,
              remaining: w.remaining,
              resetLabel: formatRelativeFuture(w.resetsAt),
              scope: w.scope,
            }))}
            reason={sub.reason ?? undefined}
            pinned={sub.pinned}
            expanded={expanded.has(sub.id)}
            actionLabel={ACTIONABLE_STATES.has(sub.state) ? "Retry" : null}
            onAction={() => refreshAccountById(sub.id)}
            onTogglePin={() => togglePin(sub.id)}
            onToggleExpand={() => toggleExpand(sub.id)}
          />
        ))
      )}
    </Panel>
  );
}
