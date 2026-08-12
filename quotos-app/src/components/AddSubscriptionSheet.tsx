import { useEffect, useState } from "react";
import { Button } from "../design-system/components/controls/Button";
import type { AccountDescriptor, Subscription } from "../types/entities";
import { listAccounts } from "../lib/tauriClient";
import { providerDisplayName } from "../providers/registry";
import { accountLabel } from "../hooks/useSubscriptions";

/** I6: "the add-subscription flow must genuinely work for locally
 * discovered subscriptions — discovery feeds the list, it never is the
 * list." Re-scans on open (cheap: local filesystem + Keychain existence
 * checks, no network) so a newly-added Claude Code account shows up the
 * same way a future provider's would, without reinventing this per
 * provider. */
export function AddSubscriptionSheet({
  tracked,
  onAdd,
  onClose,
}: {
  tracked: Subscription[];
  onAdd: (account: AccountDescriptor) => void;
  onClose: () => void;
}) {
  const [candidates, setCandidates] = useState<AccountDescriptor[] | null>(null);
  const trackedIds = new Set(tracked.map((s) => s.id));

  useEffect(() => {
    let cancelled = false;
    (async () => {
      const found = await listAccounts();
      if (!cancelled) setCandidates(found);
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  const untracked = (candidates ?? []).filter((a) => !trackedIds.has(a.id));

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: "var(--space-3)", padding: "var(--space-2)" }}>
      <p style={{
        margin: 0, fontFamily: "var(--font-sans)", fontSize: "var(--text-sm)",
        color: "var(--text-tertiary)", lineHeight: "var(--leading-snug)",
      }}>
        Found on this Mac. Nothing is tracked until you add it.
      </p>

      {candidates === null ? (
        <div style={{ fontFamily: "var(--font-sans)", fontSize: "var(--text-sm)", color: "var(--text-tertiary)" }}>
          Scanning…
        </div>
      ) : untracked.length === 0 ? (
        <div style={{ fontFamily: "var(--font-sans)", fontSize: "var(--text-sm)", color: "var(--text-tertiary)", lineHeight: "var(--leading-snug)" }}>
          No new subscriptions found. If you've just signed in to another Claude
          Code account, quit and reopen Quotos, then try again.
        </div>
      ) : (
        <div style={{ display: "flex", flexDirection: "column", gap: "var(--space-1)" }}>
          {untracked.map((account) => (
            <div key={account.id} style={{
              display: "flex", alignItems: "center", gap: "var(--space-2)",
              padding: "var(--space-2)", borderRadius: "var(--radius-md)",
              border: "0.5px solid var(--border-subtle)",
            }}>
              <div style={{ flex: 1, minWidth: 0 }}>
                <div style={{
                  fontFamily: "var(--font-sans)", fontSize: "var(--text-sm)",
                  fontWeight: "var(--weight-medium)", color: "var(--text-primary)",
                  overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap",
                }}>{accountLabel(account)}</div>
                <div style={{ fontFamily: "var(--font-sans)", fontSize: "var(--text-xs)", color: "var(--text-tertiary)" }}>
                  {providerDisplayName(account.provider)}
                </div>
              </div>
              <Button size="sm" variant="secondary" onClick={() => onAdd(account)}>Add</Button>
            </div>
          ))}
        </div>
      )}

      <Button variant="ghost" size="sm" onClick={onClose} fullWidth>Done</Button>
    </div>
  );
}
