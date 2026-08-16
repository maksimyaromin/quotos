import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type {
  AccountDescriptor,
  RawSnapshot,
  ScheduledRefreshEvent,
  SignInFinishedEvent,
  StatusItemSegment,
  StatuslineIntegrationStatus,
} from "../types/entities";

export async function listAccounts(): Promise<AccountDescriptor[]> {
  return invoke<AccountDescriptor[]>("list_accounts");
}

export async function fetchSnapshot(account: AccountDescriptor): Promise<RawSnapshot> {
  return invoke<RawSnapshot>("fetch_snapshot", {
    accountId: account.id,
    provider: account.provider,
    configDir: account.config_dir,
  });
}

export async function hidePanel(): Promise<void> {
  return invoke("hide_panel");
}

export function onPanelVisibility(callback: (visible: boolean) => void): Promise<() => void> {
  return listen<boolean>("panel-visibility", (event) => callback(event.payload));
}

/** Recomputed natively on every dock and re-dock, since it depends on the
 * status item's real position, see `compute_docked_layout` in
 * `src-tauri/src/shell.rs`. */
export function onPanelBeakOffset(callback: (offsetPx: number) => void): Promise<() => void> {
  return listen<number>("panel-beak-offset", (event) => callback(event.payload));
}

export function onQuotaRefresh(
  callback: (event: ScheduledRefreshEvent) => void,
): Promise<() => void> {
  return listen<ScheduledRefreshEvent>("quota-refresh", (event) => callback(event.payload));
}

/** Called once, right after subscribing to `onQuotaRefresh`, so the launch
 * read does not wait out the scheduler's own first tick, see
 * `src-tauri/src/accounts.rs`. */
export async function kickScheduler(): Promise<void> {
  return invoke("kick_scheduler");
}

/** macOS gives a status item's title no color channel, so the native side
 * composites a bitmap from these segments instead. `worstUsedPercent` fills
 * the glyph's own arc regardless of what is pinned. */
export async function renderStatusItem(
  segments: StatusItemSegment[],
  worstUsedPercent: number,
  tooltip: string,
): Promise<void> {
  return invoke("set_status_item_state", { segments, worstUsedPercent, tooltip });
}

export async function setDetached(detached: boolean): Promise<void> {
  return invoke("set_detached", { detached });
}

/** Moving the window's frame at all while a mouse-down gesture is live over
 * it reactivates the app, see `drag_window_step` in `src-tauri/src/shell.rs`. */
export async function dragWindowStep(): Promise<void> {
  return invoke("drag_window_step");
}

export async function endWindowDrag(): Promise<void> {
  return invoke("end_window_drag");
}

export async function debugRateLimitSnapshot(): Promise<Record<string, unknown>> {
  return invoke("debug_rate_limit_snapshot");
}

/** Quotos never touches the Keychain or a credential directly. This only
 * starts Claude Code's own sign-in process and later relays a pasted code
 * back into it, see `src-tauri/src/signin.rs`. */
export async function startSignIn(accountId: string, configDir: string): Promise<void> {
  return invoke("start_sign_in", { accountId, configDir });
}

export async function submitSignInCode(accountId: string, code: string): Promise<void> {
  return invoke("submit_sign_in_code", { accountId, code });
}

export async function cancelSignIn(accountId: string): Promise<void> {
  return invoke("cancel_sign_in", { accountId });
}

export async function forgetSignIn(accountId: string): Promise<void> {
  return invoke("forget_sign_in", { accountId });
}

export function onSignInFinished(
  callback: (event: SignInFinishedEvent) => void,
): Promise<() => void> {
  return listen<SignInFinishedEvent>("sign-in-finished", (event) => callback(event.payload));
}

/** Never claims "not installed" for an account someone already pointed
 * `statusLine` at some other way. */
export async function statuslineStatus(configDir: string): Promise<StatuslineIntegrationStatus> {
  return invoke("statusline_status", { configDir });
}

/** Rejects with a typed `StatuslineError`, in particular `conflict` when a
 * different `statusLine` is already configured and `force` was not set. */
export async function statuslineInstall(
  configDir: string,
  force: boolean,
): Promise<{ replaced_existing: boolean }> {
  return invoke("statusline_install", { configDir, force });
}

/** Restores exactly the previous `statusLine` state, or clears the key if
 * there was none. */
export async function statuslineRemove(configDir: string): Promise<void> {
  return invoke("statusline_remove", { configDir });
}
