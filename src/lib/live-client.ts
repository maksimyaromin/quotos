import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import type {
  AccountDescriptor,
  RawSnapshot,
  ScheduledRefreshEvent,
  SignInFinishedEvent,
  StatusItemSegment,
  StatuslineIntegrationStatus,
} from '@/types/entities'

export async function listAccounts(): Promise<AccountDescriptor[]> {
  return invoke<AccountDescriptor[]>('list_accounts')
}

export async function fetchSnapshot(account: AccountDescriptor): Promise<RawSnapshot> {
  return invoke<RawSnapshot>('fetch_snapshot', {
    accountId: account.id,
    provider: account.provider,
    configDir: account.config_dir,
  })
}

export async function hidePanel(): Promise<void> {
  return invoke('hide_panel')
}

export function onPanelVisibility(callback: (visible: boolean) => void): Promise<() => void> {
  return listen<boolean>('panel-visibility', (event) => callback(event.payload))
}

export function onPanelBeakOffset(callback: (offsetPx: number) => void): Promise<() => void> {
  return listen<number>('panel-beak-offset', (event) => callback(event.payload))
}

export function onQuotaRefresh(
  callback: (event: ScheduledRefreshEvent) => void,
): Promise<() => void> {
  return listen<ScheduledRefreshEvent>('quota-refresh', (event) => callback(event.payload))
}

export async function kickScheduler(): Promise<void> {
  return invoke('kick_scheduler')
}

export async function renderStatusItem(
  segments: StatusItemSegment[],
  worstUsedPercent: number,
  tooltip: string,
): Promise<void> {
  return invoke('set_status_item_state', { segments, worstUsedPercent, tooltip })
}

export async function setDetached(detached: boolean): Promise<void> {
  return invoke('set_detached', { detached })
}

export async function dragWindowStep(): Promise<void> {
  return invoke('drag_window_step')
}

export async function endWindowDrag(): Promise<void> {
  return invoke('end_window_drag')
}

export async function debugRateLimitSnapshot(): Promise<Record<string, unknown>> {
  return invoke('debug_rate_limit_snapshot')
}

export async function startSignIn(accountId: string, configDir: string): Promise<void> {
  return invoke('start_sign_in', { accountId, configDir })
}

export async function submitSignInCode(accountId: string, code: string): Promise<void> {
  return invoke('submit_sign_in_code', { accountId, code })
}

export async function cancelSignIn(accountId: string): Promise<void> {
  return invoke('cancel_sign_in', { accountId })
}

export async function forgetSignIn(accountId: string): Promise<void> {
  return invoke('forget_sign_in', { accountId })
}

export function onSignInFinished(
  callback: (event: SignInFinishedEvent) => void,
): Promise<() => void> {
  return listen<SignInFinishedEvent>('sign-in-finished', (event) => callback(event.payload))
}

export async function statuslineStatus(configDir: string): Promise<StatuslineIntegrationStatus> {
  return invoke('statusline_status', { configDir })
}

export async function statuslineInstall(
  configDir: string,
  force: boolean,
): Promise<{ replaced_existing: boolean }> {
  return invoke('statusline_install', { configDir, force })
}

export async function statuslineRemove(configDir: string): Promise<void> {
  return invoke('statusline_remove', { configDir })
}
