import { useCallback, useEffect, useRef, useState } from "react";
import type { AccountDescriptor, Subscription } from "../types/entities";
import { isFetchError } from "../types/entities";
import { fetchSnapshot, setTrayTitle } from "../lib/tauriClient";
import { normalizeFor, providerDisplayName } from "../providers/registry";
import { loadTracked, saveTracked, type TrackedAccount } from "../lib/persistence";

// B5: matches the measured provider budget (5 requests / 300s, shared with
// Claude Code itself — data/quotos-source-s1/report.md, "Rate limits —
// measured"). One automatic read every 5 minutes uses only 1 of those 5
// slots, leaving headroom for manual refreshes without ever approaching the
// limit through background polling alone.
const BACKGROUND_REFRESH_INTERVAL_MS = 5 * 60 * 1000;

/** The provider-derived default label for an account before any read has
 * come back (or before a custom rename) — shared with the add-subscription
 * flow so a candidate shows the same name there as it will once tracked. */
export function accountLabel(account: AccountDescriptor): string {
  const slug = account.id.split(":")[1] ?? account.id;
  return slug === "claude" ? "Claude" : slug.replace(/[-_]/g, " ");
}

function initialSubscription(account: AccountDescriptor, labelOverride: string | null, pinned: boolean): Subscription {
  return {
    id: account.id,
    provider: account.provider,
    providerName: providerDisplayName(account.provider),
    label: accountLabel(account),
    labelOverride,
    account: null,
    state: "connecting",
    used: null,
    resetsAt: null,
    lastReadAt: null,
    windows: [],
    reason: null,
    pinned,
    configDir: account.config_dir,
    rateLimitedUntil: null,
  };
}

function isBlocked(s: Subscription, now: number): boolean {
  return !!s.rateLimitedUntil && new Date(s.rateLimitedUntil).getTime() > now;
}

/** The single owner of refresh cadence, tracked-subscription membership, and
 * per-subscription state. Every failure is scoped to its own subscription —
 * one broken account is never allowed to blank or block the others.
 *
 * I6: nothing is tracked by default — the list here is exactly what the
 * user added, persisted via lib/persistence.ts. Discovery (tauriClient's
 * listAccounts) only ever feeds the add-subscription flow; it is never
 * rendered directly.
 *
 * B5: opening/closing the panel spends no budget at all — there is no
 * refresh tied to visibility. Only the initial read at launch, an explicit
 * manual refresh, and the 5-minute background interval ever call
 * fetchSnapshot, and all three skip any subscription currently inside its
 * own rate-limited window (see isBlocked) rather than spamming it. */
export function useSubscriptions() {
  // Lazily seeded straight from storage on the very first render — not in a
  // useEffect. An effect-based load leaves one render where `subscriptions`
  // is genuinely `[]` before the load resolves; the persistence effect
  // below would see that empty array first and clobber real, previously
  // saved data with it before the loaded value ever committed.
  const [subscriptions, setSubscriptions] = useState<Subscription[]>(() =>
    loadTracked().map((t) =>
      initialSubscription({ id: t.id, provider: t.provider, config_dir: t.config_dir }, t.label, t.pinned),
    ),
  );
  const subscriptionsRef = useRef<Subscription[]>(subscriptions);
  subscriptionsRef.current = subscriptions;

  const patch = useCallback((id: string, changes: Partial<Subscription>) => {
    setSubscriptions((prev) => prev.map((s) => (s.id === id ? { ...s, ...changes } : s)));
  }, []);

  const refreshOne = useCallback(async (account: AccountDescriptor) => {
    const prior = subscriptionsRef.current.find((s) => s.id === account.id);
    const hadGoodRead = !!prior?.lastReadAt;
    // Captured so a rate-limited outcome (see below) can restore exactly
    // this — otherwise the optimistic "reading"/"connecting" patch on the
    // next line would itself be the thing that erases a real diagnosis
    // (e.g. Broken/expired login) while the retry is in flight.
    const priorState = prior?.state ?? "connecting";
    const priorReason = prior?.reason ?? null;
    patch(account.id, { state: hadGoodRead ? "reading" : "connecting" });

    try {
      const raw = await fetchSnapshot(account);
      const normalized = normalizeFor(raw.provider, raw.usage, raw.profile, accountLabel(account));
      patch(account.id, {
        state: "working",
        label: normalized.label,
        account: normalized.account,
        windows: normalized.windows,
        used: normalized.used,
        resetsAt: normalized.resetsAt,
        lastReadAt: raw.fetched_at,
        reason: null,
        rateLimitedUntil: null,
      });
    } catch (err) {
      if (!isFetchError(err)) {
        patch(account.id, {
          state: hadGoodRead ? "behind" : "broken",
          reason: "Something went wrong reading this subscription.",
          rateLimitedUntil: null,
        });
        return;
      }
      switch (err.kind) {
        case "rate_limited": {
          // B5/B6: a self-imposed wait is a separate fact, not a health
          // state. Restore exactly the health state/reason this account had
          // before this attempt started — a real diagnosis (e.g.
          // Broken/expired login) must never decay into a generic wait, not
          // even transiently through the optimistic "connecting" patch above.
          const until = new Date(Date.now() + err.retry_after_secs * 1000).toISOString();
          patch(account.id, { state: priorState, reason: priorReason, rateLimitedUntil: until });
          break;
        }
        case "not_connected":
          patch(account.id, {
            state: hadGoodRead ? "behind" : "idle",
            reason: err.message,
            rateLimitedUntil: null,
          });
          break;
        case "unauthorized":
          patch(account.id, {
            state: hadGoodRead ? "behind" : "broken",
            reason: "Sign-in expired and could not be refreshed automatically.",
            rateLimitedUntil: null,
          });
          break;
        case "network":
          patch(account.id, {
            state: hadGoodRead ? "behind" : "broken",
            reason: err.message,
            rateLimitedUntil: null,
          });
          break;
        default:
          patch(account.id, {
            state: hadGoodRead ? "behind" : "broken",
            reason: err.message,
            rateLimitedUntil: null,
          });
      }
    }
  }, [patch]);

  const refreshAll = useCallback(async () => {
    const now = Date.now();
    const targets = subscriptionsRef.current.filter((s) => !isBlocked(s, now));
    await Promise.allSettled(
      targets.map((s) => refreshOne({ id: s.id, provider: s.provider, config_dir: s.configDir })),
    );
  }, [refreshOne]);

  const refreshAccountById = useCallback(
    async (id: string) => {
      const sub = subscriptionsRef.current.find((s) => s.id === id);
      if (!sub || isBlocked(sub, Date.now())) return;
      await refreshOne({ id: sub.id, provider: sub.provider, config_dir: sub.configDir });
    },
    [refreshOne],
  );

  const togglePin = useCallback((id: string) => {
    setSubscriptions((prev) => prev.map((s) => (s.id === id ? { ...s, pinned: !s.pinned } : s)));
  }, []);

  const renameSubscription = useCallback((id: string, label: string | null) => {
    setSubscriptions((prev) => prev.map((s) => (s.id === id ? { ...s, labelOverride: label } : s)));
  }, []);

  const addSubscription = useCallback(
    (account: AccountDescriptor) => {
      setSubscriptions((prev) => {
        if (prev.some((s) => s.id === account.id)) return prev;
        return [...prev, initialSubscription(account, null, false)];
      });
      // Brief §5.3 step 4: verify by reading once immediately, so the
      // person sees what came back rather than a cold placeholder.
      void refreshOne(account);
    },
    [refreshOne],
  );

  const removeSubscription = useCallback((id: string) => {
    setSubscriptions((prev) => prev.filter((s) => s.id !== id));
  }, []);

  // The one-time initial read at launch for whatever was already loaded
  // into `subscriptions` above — this is the "scheduled" read at startup,
  // not something opening the panel triggers. Rows themselves are seeded
  // by the lazy useState initializer, not here. Guarded so it truly fires
  // once even if the effect itself runs twice (React StrictMode's dev-only
  // double-invoke) — otherwise two concurrent reads for the same account
  // both capture the same stale "prior" state and can race each other.
  const initialReadFired = useRef(false);
  useEffect(() => {
    if (initialReadFired.current) return;
    initialReadFired.current = true;
    void Promise.allSettled(
      subscriptionsRef.current.map((s) => refreshOne({ id: s.id, provider: s.provider, config_dir: s.configDir })),
    );
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Persist the user-owned parts of the list (membership, custom labels,
  // pins) on every change. Ephemeral read state (windows, used %, reason)
  // is deliberately not persisted — a stale number must never be shown as
  // current after a restart; every subscription re-verifies on launch.
  useEffect(() => {
    const tracked: TrackedAccount[] = subscriptions.map((s) => ({
      id: s.id,
      provider: s.provider,
      config_dir: s.configDir,
      label: s.labelOverride,
      pinned: s.pinned,
    }));
    saveTracked(tracked);
  }, [subscriptions]);

  // The only background timer in the app: every 5 minutes, for the
  // lifetime of the panel process — independent of whether the panel is
  // currently open, so pinned tray figures and "last read" stay reasonably
  // fresh even while closed.
  useEffect(() => {
    const interval = setInterval(refreshAll, BACKGROUND_REFRESH_INTERVAL_MS);
    return () => clearInterval(interval);
  }, [refreshAll]);

  // Mirror pinned subscriptions' headline figures beside the tray glyph
  // (I2: consumed, not remaining). A broken pin shows a mark instead of a
  // stale number, never a number presented as current. Unpinned means gone
  // — this effect is a pure function of current state every time it runs.
  useEffect(() => {
    const pinned = subscriptions.filter((s) => s.pinned);
    const title = pinned
      .map((s) => {
        if (s.state === "broken") return "!";
        return typeof s.used === "number" ? `${s.used}%` : "…";
      })
      .join(" ");
    setTrayTitle(title);
  }, [subscriptions]);

  return {
    subscriptions,
    refreshAll,
    refreshAccountById,
    togglePin,
    renameSubscription,
    addSubscription,
    removeSubscription,
  };
}
