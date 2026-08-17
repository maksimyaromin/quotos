export type SubscriptionState = 'idle' | 'connecting' | 'working' | 'reading' | 'behind' | 'broken'

export interface LimitWindowEntity {
  id: string
  name: string
  // Percent consumed, not remaining, across every used field in this file.
  used: number | null
  resetsAt: string | null
  scope: string | null
  isActive: boolean
}

export type Severity = 'healthy' | 'warn' | 'critical'

export interface Subscription {
  id: string
  provider: string
  providerName: string
  label: string
  labelOverride: string | null
  account: string | null
  state: SubscriptionState
  severity: Severity
  used: number | null
  resetsAt: string | null
  lastReadAt: string | null
  windows: LimitWindowEntity[]
  reason: string | null
  needsSignIn: boolean
  pinnedWindowIds: string[]
  headlineWindowId: string | null
  configDir: string
  // Kept apart from state and reason so a self-imposed wait never
  // overwrites a real diagnosis.
  rateLimitedUntil: string | null
  signInInProgress: boolean
  pendingRemoval: boolean
}

export interface AccountDescriptor {
  id: string
  provider: string
  config_dir: string
}

export interface RawSnapshot {
  account_id: string
  provider: string
  config_dir: string
  fetched_at: string
  usage: unknown
  profile: unknown | null
  statusline?: StatuslineFeedWire | null
}

export interface StatuslineWindowWire {
  used_percentage: number
  resets_at: number | null
}

export interface StatuslineFeedWire {
  written_at: string
  rate_limits: {
    five_hour?: StatuslineWindowWire | null
    seven_day?: StatuslineWindowWire | null
  }
}

export type StatuslineIntegrationStatus =
  | { kind: 'not_installed' }
  | { kind: 'installed' }
  | { kind: 'conflict'; existing_command: string }

export type StatuslineError =
  | { kind: 'parse_failed'; message: string }
  | { kind: 'read_failed'; message: string }
  | { kind: 'write_failed'; message: string }
  | { kind: 'helper_install_failed'; message: string }
  | { kind: 'conflict'; existing_command: string }

export function isStatuslineError(value: unknown): value is StatuslineError {
  return (
    typeof value === 'object' &&
    value !== null &&
    'kind' in value &&
    typeof (value as { kind: unknown }).kind === 'string'
  )
}

// Mirrors src-tauri/src/providers/mod.rs's FetchError by hand; nothing
// enforces that a new Rust variant is added here too.
export type FetchError =
  | { kind: 'not_connected'; message: string }
  | { kind: 'unauthorized'; message: string }
  | { kind: 'credential_stale'; message: string }
  | { kind: 'rate_limited'; retry_after_secs: number }
  | { kind: 'network'; message: string }
  | { kind: 'other'; message: string }

export function isFetchError(value: unknown): value is FetchError {
  return (
    typeof value === 'object' &&
    value !== null &&
    'kind' in value &&
    typeof (value as { kind: unknown }).kind === 'string'
  )
}

export interface NormalizedRead {
  label: string
  account: string | null
  windows: LimitWindowEntity[]
  used: number | null
  resetsAt: string | null
  severity: Severity
  headlineWindowId: string | null
}

export type ScheduledRefreshEvent =
  | { kind: 'ok'; snapshot: RawSnapshot }
  | { kind: 'err'; account_id: string; error: FetchError }

export interface SignInFinishedEvent {
  account_id: string
  success: boolean
}

export interface StatusItemSegment {
  text: string
  color: 'neutral' | 'amber' | 'red'
  groupStart: boolean
}
