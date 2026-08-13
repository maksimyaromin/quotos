import { useEffect, useState } from "react";
import { Button } from "../design-system/components/controls/Button";
import type { AccountDescriptor, Subscription } from "../types/entities";
import { listAccounts } from "../lib/tauriClient";
import { accountLabel } from "../hooks/useSubscriptions";

interface Row {
  id: string;
  name: string;
  path: string;
  tracked: boolean;
  account: AccountDescriptor;
}

/** The Subscriptions screen (replaces the old add-subscription sheet): one
 * list of every account Quotos can see on this Mac. Tracked ones sit
 * highlighted with a Remove button; the rest get an Add button — same size
 * and style, only the label color differs. There is no Done button; the
 * panel's header back-arrow (App.tsx) is the only way out. */
export function SubscriptionsScreen({
  tracked,
  onAdd,
  onRemove,
}: {
  tracked: Subscription[];
  onAdd: (account: AccountDescriptor) => void;
  onRemove: (id: string) => void;
}) {
  const [discovered, setDiscovered] = useState<AccountDescriptor[] | null>(null);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      const found = await listAccounts();
      if (!cancelled) setDiscovered(found);
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  const trackedRows: Row[] = tracked.map((s) => ({
    id: s.id,
    name: s.labelOverride ?? s.label,
    path: s.configDir,
    tracked: true,
    account: { id: s.id, provider: s.provider, config_dir: s.configDir },
  }));
  const trackedIds = new Set(trackedRows.map((r) => r.id));
  const untrackedRows: Row[] = (discovered ?? [])
    .filter((a) => !trackedIds.has(a.id))
    .map((a) => ({ id: a.id, name: accountLabel(a), path: a.config_dir, tracked: false, account: a }));
  const rows = [...trackedRows, ...untrackedRows];

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: "var(--space-2)", padding: "var(--space-1-5) var(--space-1-5) var(--space-1)" }}>
      <p style={{
        margin: 0, fontFamily: "var(--font-sans)", fontSize: "var(--text-sm)",
        color: "var(--text-tertiary)", lineHeight: "var(--leading-snug)",
      }}>
        Accounts Quotos can see on this Mac. Adding one reads it once, right away.
      </p>

      {rows.map((r) => (
        <div key={r.id} style={{
          display: "flex", alignItems: "center", gap: "var(--space-2)",
          padding: "var(--space-2)", borderRadius: "var(--radius-md)",
          border: "0.5px solid var(--border-subtle)",
          background: r.tracked ? "var(--bg-elevated)" : "transparent",
        }}>
          <div style={{ flex: 1, minWidth: 0 }}>
            <div style={{
              fontFamily: "var(--font-sans)", fontSize: "var(--text-sm)",
              fontWeight: "var(--weight-medium)",
              color: r.tracked ? "var(--text-primary)" : "var(--text-secondary)",
              overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap",
            }}>{r.name}</div>
            <div style={{
              fontFamily: "var(--font-mono)", fontSize: "var(--text-xs)", color: "var(--text-quaternary)",
              overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap",
            }}>{r.path}</div>
          </div>
          <Button
            size="sm"
            variant="secondary"
            style={{ color: r.tracked ? "var(--red)" : "var(--text-primary)" }}
            onClick={() => (r.tracked ? onRemove(r.id) : onAdd(r.account))}
          >
            {r.tracked ? "Remove" : "Add"}
          </Button>
        </div>
      ))}

      <div style={{
        fontFamily: "var(--font-sans)", fontSize: "var(--text-xs)",
        lineHeight: "var(--leading-snug)", color: "var(--text-quaternary)",
      }}>
        Signed in to another account just now? Quit and reopen Quotos to pick it up.
      </div>
    </div>
  );
}
