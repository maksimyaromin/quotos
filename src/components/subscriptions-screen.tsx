import { useEffect, useState } from "react";
import { Button } from "@/design-system";
import { listAccounts, onPanelVisibility } from "@/lib/tauri-client";
import type { AccountDescriptor, Subscription } from "@/types/entities";
import { StatuslineControl } from "./statusline-control";
import styles from "./subscriptions-screen.module.css";

interface Row {
  id: string;
  name: string;
  path: string;
  tracked: boolean;
  account: AccountDescriptor;
}

/** The Subscriptions screen: one list of every account Quotos can see on
 * this Mac. Tracked ones sit highlighted with a Remove button. The rest
 * get an Add button of the same size and style, with only the label color
 * differing. There is no Done button. The panel's header back arrow in
 * app.tsx is the only way out. */
export function SubscriptionsScreen({
  tracked,
  onAdd,
  onRemove,
  displayLabelFor,
}: {
  tracked: Subscription[];
  onAdd: (account: AccountDescriptor) => void;
  onRemove: (id: string) => void;
  /** How to name an account this screen lists but the panel is not
   * showing. Supplied by `useSubscriptions` rather than derived here, so
   * both halves of this list and the panel spell one account the same
   * way. See that hook's `displayLabelFor`. */
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
    // Signing in happens in a terminal, which blurs and hides the panel, so
    // rescanning on the hidden-to-visible transition makes a new account
    // simply appear. The `wasHidden` guard also keeps the browser harness's
    // subscribe-time "visible" signal from double-scanning a fresh mount.
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
    .map((a) => ({
      id: a.id,
      name: displayLabelFor(a),
      path: a.config_dir,
      tracked: false,
      account: a,
    }));
  const rows = [...trackedRows, ...untrackedRows];

  return (
    <div className={styles.list}>
      <p className={styles.description}>
        Accounts Quotos can see on this Mac. Adding one reads it once, right away.
      </p>

      {rows.map((r) => (
        <div key={r.id} className={styles.row} data-tracked={r.tracked ? "true" : undefined}>
          <div className={styles.rowHeader}>
            <div className={styles.rowInfo}>
              <div className={styles.rowName}>{r.name}</div>
              <div className={styles.rowPath}>{r.path}</div>
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
          {r.tracked ? <StatuslineControl configDir={r.path} /> : null}
        </div>
      ))}

      <div className={styles.footerNote}>
        {/* Deliberately does not say "Quit and reopen Quotos to pick it
            up": discovery reruns on every mount and on every panel
            re-show above, so quitting is not required to see a new
            sign-in. */}
        Signed in to another account just now? Reopen this screen to pick it up.
      </div>
    </div>
  );
}
