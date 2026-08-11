/** Browser-only demo data, used when the app is opened in a plain browser
 * (e.g. `npm run dev` + Chrome) rather than inside the Tauri shell, where
 * `invoke()` has nothing to talk to. This exists purely so every state in
 * the brief can be driven and screenshotted without packaging the app —
 * it never runs inside the real Tauri build. See RESULT.md. */
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

const MOCK_ACCOUNTS: AccountDescriptor[] = [
  { id: "claude:claude", provider: "claude", config_dir: "~/.claude" },
  { id: "claude:claude-team", provider: "claude", config_dir: "~/.claude-team" },
  { id: "claude:demo-critical", provider: "claude", config_dir: "~/.claude-demo-critical" },
  { id: "claude:demo-idle", provider: "claude", config_dir: "~/.claude-demo-idle" },
  { id: "claude:demo-broken", provider: "claude", config_dir: "~/.claude-demo-broken" },
  { id: "claude:demo-waiting", provider: "claude", config_dir: "~/.claude-demo-waiting" },
  { id: "claude:demo-behind", provider: "claude", config_dir: "~/.claude-demo-behind" },
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
      return fail({ kind: "unauthorized", message: "still unauthorized after refreshing the credential" });
    case "claude:demo-waiting":
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
