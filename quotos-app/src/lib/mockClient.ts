/** Browser-only demo data, used when the app is opened in a plain browser
 * (e.g. `npm run dev` + Chrome) rather than inside the Tauri shell, where
 * `invoke()` has nothing to talk to. This exists purely so every state in
 * the feedback can be driven and screenshotted without packaging the app —
 * it never runs inside the real Tauri build. See RESULT.md.
 *
 * `listAccounts()` here stands in for discovery (B4: only ever returns
 * accounts that "have a credential" — no phantom folders) and is
 * deliberately decoupled from what's tracked/shown (I6: the tracked list,
 * persisted in localStorage via lib/persistence.ts, starts empty and is the
 * only thing the panel renders). */
import type { AccountDescriptor, FetchError, RawSnapshot } from "../types/entities";

const DELAY_MS = 500;
const callCounts = new Map<string, number>();

function delay<T>(value: T): Promise<T> {
  return new Promise((resolve) => setTimeout(() => resolve(value), DELAY_MS));
}

function usagePayload(fiveHourPct: number, sevenDayPct: number, fablePct: number) {
  return {
    limits: [
      {
        kind: "session",
        group: "session",
        percent: fiveHourPct,
        severity: "normal",
        resets_at: new Date(Date.now() + 3 * 3600_000).toISOString(),
        scope: null,
        is_active: true,
      },
      {
        kind: "weekly_all",
        group: "weekly",
        percent: sevenDayPct,
        severity: "normal",
        resets_at: new Date(Date.now() + 5 * 86400_000).toISOString(),
        scope: null,
        is_active: false,
      },
      {
        kind: "weekly_scoped",
        group: "weekly",
        percent: fablePct,
        severity: "normal",
        resets_at: new Date(Date.now() + 5 * 86400_000).toISOString(),
        scope: { model: { id: null, display_name: "Fable" }, surface: null },
        is_active: false,
      },
    ],
  };
}

function profilePayload(name: string, orgType: string) {
  return { organization: { name, organization_type: orgType, subscription_status: "active" } };
}

// What discovery finds on this (fake) machine — B4: every entry here would
// have resolved a real Keychain credential; a folder-only phantom like the
// captain's "claude-shared" never reaches this list at all.
const MOCK_ACCOUNTS: AccountDescriptor[] = [
  { id: "claude:claude", provider: "claude", config_dir: "~/.claude" },
  { id: "claude:claude-team", provider: "claude", config_dir: "~/.claude-team" },
  { id: "claude:demo-critical", provider: "claude", config_dir: "~/.claude-demo-critical" },
  { id: "claude:demo-idle", provider: "claude", config_dir: "~/.claude-demo-idle" },
  { id: "claude:demo-broken", provider: "claude", config_dir: "~/.claude-demo-broken" },
  { id: "claude:demo-waiting", provider: "claude", config_dir: "~/.claude-demo-waiting" },
  { id: "claude:demo-behind", provider: "claude", config_dir: "~/.claude-demo-behind" },
  { id: "claude:demo-nolimits", provider: "claude", config_dir: "~/.claude-demo-nolimits" },
  { id: "claude:demo-longnames", provider: "claude", config_dir: "~/.claude-demo-longnames" },
];

export async function listAccounts(): Promise<AccountDescriptor[]> {
  return delay(MOCK_ACCOUNTS);
}

export async function fetchSnapshot(account: AccountDescriptor): Promise<RawSnapshot> {
  const n = (callCounts.get(account.id) ?? 0) + 1;
  callCounts.set(account.id, n);

  const fail = (error: FetchError): Promise<never> => {
    const rejection = delay(error).then((e) => {
      throw e;
    });
    return rejection as Promise<never>;
  };

  switch (account.id) {
    case "claude:claude":
      return delay({
        account_id: account.id,
        provider: "claude",
        config_dir: account.config_dir,
        fetched_at: new Date().toISOString(),
        usage: usagePayload(2, 3, 3),
        profile: profilePayload("Personal", "claude_max"),
      });
    case "claude:claude-team":
      return delay({
        account_id: account.id,
        provider: "claude",
        config_dir: account.config_dir,
        fetched_at: new Date().toISOString(),
        usage: usagePayload(78, 82, 45),
        profile: profilePayload("Scompler", "claude_team"),
      });
    case "claude:demo-critical":
      return delay({
        account_id: account.id,
        provider: "claude",
        config_dir: account.config_dir,
        fetched_at: new Date().toISOString(),
        usage: usagePayload(94, 91, 20),
        profile: profilePayload("Critical demo", "claude_pro"),
      });
    case "claude:demo-idle":
      return fail({ kind: "not_connected", message: "No Claude Code credentials in the Keychain for this account." });
    case "claude:demo-broken":
      // B6: the first read surfaces the real, diagnosable failure (an
      // expired login). Every read after that — simulating the shared
      // rate budget running dry from repeated automatic retries — comes
      // back rate_limited instead. The health state must NOT decay into
      // "waiting"; it must still read Broken/expired-login on the 2nd,
      // 3rd, ... call.
      if (n === 1) {
        return fail({ kind: "unauthorized", message: "still unauthorized after refreshing the credential" });
      }
      return fail({ kind: "rate_limited", retry_after_secs: 214 });
    case "claude:demo-waiting":
      // Never successfully read even once, and rate-limited from the very
      // first attempt — the "no health data yet, only a budget wait"
      // edge case.
      return fail({ kind: "rate_limited", retry_after_secs: 214 });
    case "claude:demo-behind":
      if (n === 1) {
        return delay({
          account_id: account.id,
          provider: "claude",
          config_dir: account.config_dir,
          fetched_at: new Date().toISOString(),
          usage: usagePayload(12, 24, 8),
          profile: profilePayload("Flaky demo", "claude_pro"),
        });
      }
      return fail({ kind: "network", message: "the connection timed out" });
    case "claude:demo-nolimits":
      // A clean read that simply has nothing to report yet — "working" with
      // no windows at all, not a failure. Exercises the "No limits reported
      // yet." message with a teal (not amber/red) dot.
      return delay({
        account_id: account.id,
        provider: "claude",
        config_dir: account.config_dir,
        fetched_at: new Date().toISOString(),
        usage: { limits: [] },
        profile: profilePayload("No limits demo", "claude_pro"),
      });
    case "claude:demo-longnames":
      // A provider-supplied window name long enough to force the ellipsis
      // truncation in LimitWindow.jsx — names are rendered verbatim, never
      // translated or shortened by us.
      return delay({
        account_id: account.id,
        provider: "claude",
        config_dir: account.config_dir,
        fetched_at: new Date().toISOString(),
        usage: {
          limits: [
            {
              kind: "session",
              group: "session",
              percent: 34,
              severity: "normal",
              resets_at: new Date(Date.now() + 3 * 3600_000).toISOString(),
              scope: null,
              is_active: true,
            },
            {
              kind: "extended_thinking_weekly_combined_all_models_quota",
              group: "weekly",
              percent: 61,
              severity: "normal",
              resets_at: new Date(Date.now() + 5 * 86400_000).toISOString(),
              scope: { model: { id: null, display_name: "Claude Opus 4.5 (extended thinking, research preview)" }, surface: null },
              is_active: false,
            },
          ],
        },
        profile: profilePayload("Long names demo", "claude_pro"),
      });
    default:
      return fail({ kind: "other", message: "unknown demo account" });
  }
}

export async function hidePanel(): Promise<void> {
  // no-op in the browser
}

export function onPanelVisibility(callback: (visible: boolean) => void): Promise<() => void> {
  callback(true);
  return Promise.resolve(() => {});
}

export async function setTrayTitle(_title: string): Promise<void> {
  // no-op in the browser — there is no real tray to update
}

export async function setDetached(_detached: boolean): Promise<void> {
  // no-op in the browser — there is no real window chrome to change
}

export async function debugRateLimitSnapshot(): Promise<Record<string, unknown>> {
  return Object.fromEntries(
    Array.from(callCounts.entries()).map(([id, n]) => [id, { used: Math.min(n, 5), max: 5, retry_after_secs: null }]),
  );
}
