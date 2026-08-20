import type {
  AccountDescriptor,
  FetchError,
  RawSnapshot,
  SignInFinishedEvent,
  StatuslineIntegrationStatus,
} from '@/types/entities'

const DELAY_MS = 500
const callCounts = new Map<string, number>()
const signInSessions = new Set<string>()
const recoveredAccounts = new Set<string>()
let signInListeners: Array<(event: SignInFinishedEvent) => void> = []

function delay<T>(value: T): Promise<T> {
  return new Promise((resolve) => setTimeout(() => resolve(value), DELAY_MS))
}

function buildUsagePayload(fiveHourPct: number, sevenDayPct: number, fablePct: number) {
  return {
    limits: [
      {
        kind: 'session',
        group: 'session',
        percent: fiveHourPct,
        severity: 'normal',
        resets_at: new Date(Date.now() + 3 * 3600_000).toISOString(),
        scope: null,
        is_active: true,
      },
      {
        kind: 'weekly_all',
        group: 'weekly',
        percent: sevenDayPct,
        severity: 'normal',
        resets_at: new Date(Date.now() + 5 * 86400_000).toISOString(),
        scope: null,
        is_active: false,
      },
      {
        kind: 'weekly_scoped',
        group: 'weekly',
        percent: fablePct,
        severity: 'normal',
        resets_at: new Date(Date.now() + 5 * 86400_000).toISOString(),
        scope: { model: { id: null, display_name: 'Fable' }, surface: null },
        is_active: false,
      },
    ],
  }
}

function buildProfilePayload(name: string, orgType: string) {
  return { organization: { name, organization_type: orgType, subscription_status: 'active' } }
}

function snapshotEnvelope(account: AccountDescriptor, fetchedAt = new Date().toISOString()) {
  return {
    account_id: account.id,
    provider: 'claude',
    config_dir: account.config_dir,
    fetched_at: fetchedAt,
  }
}

const MOCK_ACCOUNTS: AccountDescriptor[] = [
  { id: 'claude:claude', provider: 'claude', config_dir: '~/.claude' },
  { id: 'claude:claude-team', provider: 'claude', config_dir: '~/.claude-team' },
  { id: 'claude:demo-critical', provider: 'claude', config_dir: '~/.claude-demo-critical' },
  { id: 'claude:demo-idle', provider: 'claude', config_dir: '~/.claude-demo-idle' },
  { id: 'claude:demo-broken', provider: 'claude', config_dir: '~/.claude-demo-broken' },
  {
    id: 'claude:demo-stale-credential',
    provider: 'claude',
    config_dir: '~/.claude-demo-stale-credential',
  },
  { id: 'claude:demo-waiting', provider: 'claude', config_dir: '~/.claude-demo-waiting' },
  { id: 'claude:demo-behind', provider: 'claude', config_dir: '~/.claude-demo-behind' },
  { id: 'claude:demo-nolimits', provider: 'claude', config_dir: '~/.claude-demo-nolimits' },
  { id: 'claude:demo-longnames', provider: 'claude', config_dir: '~/.claude-demo-longnames' },
  { id: 'claude:demo-severity', provider: 'claude', config_dir: '~/.claude-demo-severity' },
  { id: 'claude:demo-statusline', provider: 'claude', config_dir: '~/.claude-demo-statusline' },
]

export async function listAccounts(): Promise<AccountDescriptor[]> {
  return delay(MOCK_ACCOUNTS)
}

export async function fetchSnapshot(account: AccountDescriptor): Promise<RawSnapshot> {
  const n = (callCounts.get(account.id) ?? 0) + 1
  callCounts.set(account.id, n)

  const fail = (error: FetchError): Promise<never> => {
    const rejection = delay(error).then((e) => {
      throw e
    })
    return rejection as Promise<never>
  }

  switch (account.id) {
    case 'claude:claude':
      return delay({
        ...snapshotEnvelope(account),
        usage: buildUsagePayload(2, 3, 3),
        profile: buildProfilePayload('Personal', 'claude_max'),
      })
    case 'claude:claude-team':
      return delay({
        ...snapshotEnvelope(account),
        usage: buildUsagePayload(78, 82, 45),
        profile: buildProfilePayload('Scompler', 'claude_team'),
      })
    case 'claude:demo-critical':
      return delay({
        ...snapshotEnvelope(account),
        usage: buildUsagePayload(94, 91, 20),
        profile: buildProfilePayload('Critical demo', 'claude_pro'),
      })
    case 'claude:demo-idle':
      return fail({
        kind: 'not_connected',
        message: 'No Claude Code credentials in the Keychain for this account.',
      })
    case 'claude:demo-broken':
      if (recoveredAccounts.has(account.id)) {
        return delay({
          ...snapshotEnvelope(account),
          usage: buildUsagePayload(5, 8, 2),
          profile: buildProfilePayload('Recovered demo', 'claude_pro'),
        })
      }
      if (n === 1) {
        return fail({
          kind: 'unauthorized',
          message: 'still unauthorized after refreshing the credential',
        })
      }
      return fail({ kind: 'rate_limited', retry_after_secs: 214 })
    case 'claude:demo-stale-credential':
      if (n === 1) {
        return fail({
          kind: 'credential_stale',
          message:
            "This account's access token has expired and Quotos couldn't renew it here. Use Claude Code for this account once and Quotos will pick it up.",
        })
      }
      return delay({
        ...snapshotEnvelope(account),
        usage: buildUsagePayload(11, 19, 6),
        profile: buildProfilePayload('Renewed demo', 'claude_max'),
      })
    case 'claude:demo-waiting':
      return fail({ kind: 'rate_limited', retry_after_secs: 214 })
    case 'claude:demo-behind':
      if (n === 1) {
        return delay({
          ...snapshotEnvelope(account),
          usage: buildUsagePayload(12, 24, 8),
          profile: buildProfilePayload('Flaky demo', 'claude_pro'),
        })
      }
      return fail({ kind: 'network', message: 'the connection timed out' })
    case 'claude:demo-nolimits':
      return delay({
        ...snapshotEnvelope(account),
        usage: { limits: [] },
        profile: buildProfilePayload('No limits demo', 'claude_pro'),
      })
    case 'claude:demo-severity':
      return delay({
        ...snapshotEnvelope(account),
        usage: buildUsagePayload(85, 20, 8),
        profile: buildProfilePayload('Severity demo', 'claude_pro'),
      })
    case 'claude:demo-statusline':
      return delay({
        ...snapshotEnvelope(account, new Date(Date.now() - 30_000).toISOString()),
        usage: buildUsagePayload(12, 24, 8),
        profile: buildProfilePayload('Statusline demo', 'claude_pro'),
        statusline: {
          written_at: new Date().toISOString(),
          rate_limits: {
            five_hour: { used_percentage: 45, resets_at: Math.floor(Date.now() / 1000) + 3 * 3600 },
            seven_day: null,
          },
        },
      })
    case 'claude:demo-longnames':
      return delay({
        ...snapshotEnvelope(account),
        usage: {
          limits: [
            {
              kind: 'session',
              group: 'session',
              percent: 34,
              severity: 'normal',
              resets_at: new Date(Date.now() + 3 * 3600_000).toISOString(),
              scope: null,
              is_active: true,
            },
            {
              kind: 'extended_thinking_weekly_combined_all_models_quota',
              group: 'weekly',
              percent: 61,
              severity: 'normal',
              resets_at: new Date(Date.now() + 5 * 86400_000).toISOString(),
              scope: {
                model: {
                  id: null,
                  display_name: 'Claude Opus 4.5 (extended thinking, research preview)',
                },
                surface: null,
              },
              is_active: false,
            },
          ],
        },
        profile: buildProfilePayload('Long names demo', 'claude_pro'),
      })
    default:
      return fail({ kind: 'other', message: 'unknown demo account' })
  }
}

export async function hidePanel(): Promise<void> {}

export function onPanelVisibility(callback: (visible: boolean) => void): Promise<() => void> {
  callback(true)
  return Promise.resolve(() => {})
}

export async function setDetached(_detached: boolean): Promise<void> {}

export async function startSignIn(accountId: string, _configDir: string): Promise<void> {
  signInSessions.add(accountId)
}

export async function submitSignInCode(accountId: string, _code: string): Promise<void> {
  if (!signInSessions.has(accountId)) return
  signInSessions.delete(accountId)
  recoveredAccounts.add(accountId)
  await delay(undefined)
  signInListeners.forEach((listener) => {
    listener({ account_id: accountId, success: true })
  })
}

export async function cancelSignIn(accountId: string): Promise<void> {
  signInSessions.delete(accountId)
}

export async function forgetSignIn(_accountId: string): Promise<void> {}

export function onSignInFinished(
  callback: (event: SignInFinishedEvent) => void,
): Promise<() => void> {
  signInListeners.push(callback)
  return Promise.resolve(() => {
    signInListeners = signInListeners.filter((l) => l !== callback)
  })
}

const statuslineState = new Map<string, StatuslineIntegrationStatus>()

export async function statuslineStatus(configDir: string): Promise<StatuslineIntegrationStatus> {
  return delay(statuslineState.get(configDir) ?? { kind: 'not_installed' })
}

export async function statuslineEnable(configDir: string): Promise<void> {
  statuslineState.set(configDir, { kind: 'installed' })
  return delay(undefined)
}

export async function statuslineDisable(configDir: string): Promise<void> {
  statuslineState.set(configDir, { kind: 'not_installed' })
  return delay(undefined)
}
