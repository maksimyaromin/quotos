import { useCallback, useEffect, useRef, useState } from "react";
import type { AccountDescriptor, Subscription } from "../types/entities";
import { isFetchError } from "../types/entities";
import { fetchSnapshot, listAccounts, onPanelVisibility, setTrayTitle } from "../lib/tauriClient";
import { normalizeFor, providerDisplayName } from "../providers/registry";

const REFRESH_INTERVAL_MS = 2 * 60 * 1000;

function accountLabel(account: AccountDescriptor): string {
  const slug = account.id.split(":")[1] ?? account.id;
  return slug === "claude" ? "Claude" : slug.replace(/[-_]/g, " ");
}

function initialSubscription(account: AccountDescriptor): Subscription {
  return {
    id: account.id,
    provider: account.provider,
    providerName: providerDisplayName(account.provider),
    label: accountLabel(account),
    account: null,
    state: "connecting",
    remaining: null,
    resetsAt: null,
    lastReadAt: null,
    windows: [],
    reason: null,
    pinned: false,
    configDir: account.config_dir,
  };
}

/** The single owner of refresh cadence and per-subscription state. Every
 * failure is scoped to its own subscription — one broken account is never
 * allowed to blank or block the others. */
export function useSubscriptions() {
  const [subscriptions, setSubscriptions] = useState<Subscription[]>([]);
  const subscriptionsRef = useRef<Subscription[]>([]);
  subscriptionsRef.current = subscriptions;

  const patch = useCallback((id: string, changes: Partial<Subscription>) => {
    setSubscriptions((prev) => prev.map((s) => (s.id === id ? { ...s, ...changes } : s)));
  }, []);

  const refreshOne = useCallback(async (account: AccountDescriptor) => {
    const prior = subscriptionsRef.current.find((s) => s.id === account.id);
    const hadGoodRead = !!prior?.lastReadAt;
    patch(account.id, { state: hadGoodRead ? "reading" : "connecting" });

    try {
      const raw = await fetchSnapshot(account);
      const normalized = normalizeFor(raw.provider, raw.usage, raw.profile, accountLabel(account));
      patch(account.id, {
        state: "working",
        label: normalized.label,
        account: normalized.account,
        windows: normalized.windows,
        remaining: normalized.remaining,
        resetsAt: normalized.resetsAt,
        lastReadAt: raw.fetched_at,
        reason: null,
      });
    } catch (err) {
      if (!isFetchError(err)) {
        patch(account.id, {
          state: hadGoodRead ? "behind" : "broken",
          reason: "Something went wrong reading this subscription.",
        });
        return;
      }
      switch (err.kind) {
        case "rate_limited":
          patch(account.id, {
            state: "waiting",
            reason: `Reading too often — the provider asked us to wait ${err.retry_after_secs}s.`,
          });
          break;
        case "not_connected":
          patch(account.id, {
            state: hadGoodRead ? "behind" : "idle",
            reason: err.message,
          });
          break;
        case "unauthorized":
          patch(account.id, {
            state: hadGoodRead ? "behind" : "broken",
            reason: "Sign-in expired and could not be refreshed automatically.",
          });
          break;
        case "network":
          patch(account.id, {
            state: hadGoodRead ? "behind" : "broken",
            reason: err.message,
          });
          break;
        default:
          patch(account.id, {
            state: hadGoodRead ? "behind" : "broken",
            reason: err.message,
          });
      }
    }
  }, [patch]);

  const accountsRef = useRef<AccountDescriptor[]>([]);
  const refreshAll = useCallback(async () => {
    await Promise.allSettled(accountsRef.current.map((account) => refreshOne(account)));
  }, [refreshOne]);

  const togglePin = useCallback((id: string) => {
    setSubscriptions((prev) => prev.map((s) => (s.id === id ? { ...s, pinned: !s.pinned } : s)));
  }, []);

  const refreshAccountById = useCallback(
    async (id: string) => {
      const account = accountsRef.current.find((a) => a.id === id);
      if (account) await refreshOne(account);
    },
    [refreshOne],
  );

  // Discover accounts once, seed placeholder rows, do the first read.
  useEffect(() => {
    let cancelled = false;
    (async () => {
      const accounts = await listAccounts();
      if (cancelled) return;
      accountsRef.current = accounts;
      setSubscriptions(accounts.map(initialSubscription));
      await Promise.allSettled(accounts.map((account) => refreshOne(account)));
    })();
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Refresh cadence: every 2 minutes while the panel is open, nothing while
  // closed. This effect is the only place that owns a timer.
  useEffect(() => {
    let interval: ReturnType<typeof setInterval> | null = null;
    let disposed = false;

    const listenerPromise = onPanelVisibility((visible) => {
      if (visible) {
        refreshAll();
        if (interval) clearInterval(interval);
        interval = setInterval(refreshAll, REFRESH_INTERVAL_MS);
      } else if (interval) {
        clearInterval(interval);
        interval = null;
      }
    });

    return () => {
      disposed = true;
      if (interval) clearInterval(interval);
      listenerPromise.then((stop) => {
        // React StrictMode may run effect+cleanup synchronously before the
        // async `listen()` call resolves; unlisten as soon as it does.
        if (disposed) stop();
      });
    };
  }, [refreshAll]);

  // Stretch goal: mirror pinned subscriptions' headline figures beside the
  // tray glyph. A broken pin shows a mark instead of a stale number, never
  // a number presented as current.
  useEffect(() => {
    const pinned = subscriptions.filter((s) => s.pinned);
    const title = pinned
      .map((s) => {
        if (s.state === "broken") return "!";
        return typeof s.remaining === "number" ? `${s.remaining}%` : "…";
      })
      .join(" ");
    setTrayTitle(title);
  }, [subscriptions]);

  return { subscriptions, refreshAll, refreshAccountById, togglePin };
}
