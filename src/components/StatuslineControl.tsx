import { useEffect, useState } from "react";
import { Button } from "../design-system/components/controls/Button";
import { statuslineInstall, statuslineRemove, statuslineStatus } from "../lib/tauriClient";
import { isStatuslineError, type StatuslineIntegrationStatus } from "../types/entities";

function errorMessage(err: unknown): string {
  if (isStatuslineError(err) && err.kind !== "conflict" && "message" in err) {
    return err.message;
  }
  return "Quotos couldn't do that. Nothing was changed.";
}

const noteStyle: React.CSSProperties = {
  fontFamily: "var(--font-sans)",
  fontSize: "var(--text-xs)",
  lineHeight: "var(--leading-snug)",
  color: "var(--text-quaternary)",
};

/** S2: the in-app opt-in offer for Claude Code's zero-cost statusline feed —
 * one row per tracked subscription, right where "Add subscription" already
 * lives. Per the write-mechanism contract: never installs without this
 * explicit click, shows what's already configured before offering to
 * replace it, and "Turn off" restores exactly what was there before. */
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
        setError(errorMessage(err));
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
      setError(errorMessage(err));
    } finally {
      setBusy(false);
    }
  };

  // Still checking — render nothing rather than a flash of "not installed"
  // for an account that turns out to already have it.
  if (!status) return null;

  if (status.kind === "installed") {
    return (
      <div style={{ display: "flex", alignItems: "center", gap: "var(--space-2)" }}>
        <span style={noteStyle}>Live updates from Claude Code — on, free</span>
        <Button size="sm" variant="ghost" disabled={busy} onClick={() => void turnOff()}>
          Turn off
        </Button>
        {error ? <span style={{ ...noteStyle, color: "var(--red)" }}>{error}</span> : null}
      </div>
    );
  }

  if (status.kind === "conflict") {
    return (
      <div style={{ display: "flex", flexDirection: "column", gap: "var(--space-1)" }}>
        <span style={noteStyle}>
          Claude Code already runs a different status line:{" "}
          <span style={{ fontFamily: "var(--font-mono)" }}>{status.existing_command}</span>
        </span>
        <div style={{ display: "flex", alignItems: "center", gap: "var(--space-2)" }}>
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
        {error ? <span style={{ ...noteStyle, color: "var(--red)" }}>{error}</span> : null}
      </div>
    );
  }

  return (
    <div style={{ display: "flex", alignItems: "center", gap: "var(--space-2)" }}>
      <Button size="sm" variant="ghost" disabled={busy} onClick={() => void enable(false)}>
        Enable live updates from Claude Code
      </Button>
      {error ? <span style={{ ...noteStyle, color: "var(--red)" }}>{error}</span> : null}
    </div>
  );
}
