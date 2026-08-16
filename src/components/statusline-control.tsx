import { useEffect, useState } from "react";
import { Button } from "@/design-system";
import { statuslineInstall, statuslineRemove, statuslineStatus } from "@/lib/tauri-client";
import { isStatuslineError, type StatuslineIntegrationStatus } from "@/types/entities";
import styles from "./statusline-control.module.css";

function describeError(err: unknown): string {
  if (isStatuslineError(err) && err.kind !== "conflict" && "message" in err) {
    return err.message;
  }
  return "Quotos couldn't do that. Nothing was changed.";
}

/** The in-app opt-in offer for Claude Code's zero-cost statusline feed, one
 * row per tracked subscription, right where "Add subscription" already
 * lives. Never installs without this explicit click, shows what is already
 * configured before offering to replace it, and "Turn off" restores
 * exactly what was there before. */
export function StatuslineControl({ configDir }: { configDir: string }) {
  const [status, setStatus] = useState<StatuslineIntegrationStatus | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    void statuslineStatus(configDir)
      .then((s) => {
        if (!cancelled) setStatus(s);
      })
      .catch(() => {
        if (!cancelled) setStatus({ kind: "not_installed" });
      });
    return () => {
      cancelled = true;
    };
  }, [configDir]);

  const enable = async (force: boolean) => {
    setBusy(true);
    setError(null);
    try {
      await statuslineInstall(configDir, force);
      setStatus({ kind: "installed" });
    } catch (err) {
      if (isStatuslineError(err) && err.kind === "conflict") {
        setStatus({ kind: "conflict", existing_command: err.existing_command });
      } else {
        setError(describeError(err));
      }
    } finally {
      setBusy(false);
    }
  };

  const turnOff = async () => {
    setBusy(true);
    setError(null);
    try {
      await statuslineRemove(configDir);
      setStatus({ kind: "not_installed" });
    } catch (err) {
      setError(describeError(err));
    } finally {
      setBusy(false);
    }
  };

  // Still checking. Render nothing rather than a flash of "not installed"
  // for an account that turns out to already have it.
  if (!status) return null;

  if (status.kind === "installed") {
    return (
      <div className={styles.row}>
        <span className={styles.note}>Live updates from Claude Code — on, free</span>
        <Button size="sm" variant="ghost" disabled={busy} onClick={() => void turnOff()}>
          Turn off
        </Button>
        {error ? (
          <span className={styles.note} data-tone="error">
            {error}
          </span>
        ) : null}
      </div>
    );
  }

  if (status.kind === "conflict") {
    return (
      <div className={styles.column}>
        <span className={styles.note}>
          Claude Code already runs a different status line:{" "}
          <span className={styles.code}>{status.existing_command}</span>
        </span>
        <div className={styles.row}>
          <Button size="sm" variant="secondary" disabled={busy} onClick={() => void enable(true)}>
            Replace with live updates
          </Button>
          <Button
            size="sm"
            variant="ghost"
            disabled={busy}
            onClick={() => setStatus({ kind: "not_installed" })}
          >
            Not now
          </Button>
        </div>
        {error ? (
          <span className={styles.note} data-tone="error">
            {error}
          </span>
        ) : null}
      </div>
    );
  }

  return (
    <div className={styles.row}>
      <Button size="sm" variant="ghost" disabled={busy} onClick={() => void enable(false)}>
        Enable live updates from Claude Code
      </Button>
      {error ? (
        <span className={styles.note} data-tone="error">
          {error}
        </span>
      ) : null}
    </div>
  );
}
