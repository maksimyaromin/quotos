import { useEffect, useState } from "react";
import { Button } from "../design-system/components/controls/Button";
import type { AccountDescriptor, Subscription } from "../types/entities";
import { listAccounts, onPanelVisibility } from "../lib/tauriClient";
import { StatuslineControl } from "./StatuslineControl";

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
  displayLabelFor,
}: {
  tracked: Subscription[];
  onAdd: (account: AccountDescriptor) => void;
  onRemove: (id: string) => void;
  /** R4-4: how to name an account this screen lists but the panel isn't
   * showing. Supplied by `useSubscriptions` rather than derived here, so both
   * halves of this list — and the panel — spell one account the same way; see
   * that hook's `displayLabelFor`. */
  displayLabelFor: (account: AccountDescriptor) => string;
}) {
  const [discovered, setDiscovered] = useState<AccountDescriptor[] | null>(null);

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    const scan = () => {
      void listAccounts().then((found) => {
        if (!cancelled) setDiscovered(found);
      });
    };
    scan();
    // Signing in to a new account happens in a terminal, which blurs (and so
    // hides) the panel — by the time it is back on screen, the new account is
    // already discoverable, so a rescan on the hidden→visible transition
    // makes it simply appear. The transition guard also keeps the browser
    // harness's subscribe-time "visible" signal from double-scanning a fresh
    // mount.
    let wasHidden = false;
    void onPanelVisibility((visible) => {
      if (!visible) {
        wasHidden = true;
      } else if (wasHidden) {
        wasHidden = false;
        scan();
      }
    }).then((stop) => {
      if (cancelled) stop();
      else unlisten = stop;
    });
    return () => {
      cancelled = true;
      unlisten?.();
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
    .map((a) => ({ id: a.id, name: displayLabelFor(a), path: a.config_dir, tracked: false, account: a }));
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
          display: "flex", flexDirection: "column", gap: "var(--space-1-5)",
          padding: "var(--space-2)", borderRadius: "var(--radius-md)",
          border: "0.5px solid var(--border-subtle)",
          background: r.tracked ? "var(--bg-elevated)" : "transparent",
        }}>
          <div style={{ display: "flex", alignItems: "center", gap: "var(--space-2)" }}>
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
          {/* S2: the statusline opt-in offer — only for tracked accounts,
              right where "Add subscription" already lives (the captain's
              own example placement for the offer). */}
          {r.tracked ? <StatuslineControl configDir={r.path} /> : null}
        </div>
      ))}

      <div style={{
        fontFamily: "var(--font-sans)", fontSize: "var(--text-xs)",
        lineHeight: "var(--leading-snug)", color: "var(--text-quaternary)",
      }}>
        {/* Deviates from the handoff's string table on purpose: its "Quit and
            reopen Quotos to pick it up" assumed launch-only discovery, but
            discovery is a fresh scan on every mount (and on every panel
            re-show, above) — the advice was simply false of this build. */}
        Signed in to another account just now? Reopen this screen to pick it up.
      </div>
    </div>
  );
}
