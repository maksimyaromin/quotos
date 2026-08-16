import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type {
  AccountDescriptor,
  RawSnapshot,
  ScheduledRefreshEvent,
  SignInFinishedEvent,
  StatuslineIntegrationStatus,
  TraySegment,
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

/** The beak's horizontal offset in logical CSS pixels from the panel's own
 * left edge, recomputed natively every time the panel docks or re-docks. It
 * depends on the tray icon's real position and how much the panel's own
 * left edge got clamped, see `show_panel` and `compute_docked_layout` in
 * `src-tauri/src/shell.rs`. */
export function onPanelBeakOffset(callback: (offsetPx: number) => void): Promise<() => void> {
  return listen<number>("panel-beak-offset", (event) => callback(event.payload));
}

/** The Rust-side scheduler's once-a-minute automatic reads arrive here,
 * one event per attempt, see `src-tauri/src/scheduler.rs`. */
export function onQuotaRefresh(
  callback: (event: ScheduledRefreshEvent) => void,
): Promise<() => void> {
  return listen<ScheduledRefreshEvent>("quota-refresh", (event) => callback(event.payload));
}

/** Nudges the Rust scheduler to run a due-check pass immediately. Called
 * once, right after subscribing to `onQuotaRefresh`, so the launch read is
 * still near-instant instead of waiting out the periodic loop's first,
 * deliberately delayed, tick. See `kick_scheduler`'s doc comment in
 * `src-tauri/src/accounts.rs` for why the delay exists at all. */
export async function kickScheduler(): Promise<void> {
  return invoke("kick_scheduler");
}

/** `tray-icon` v0.24.2's macOS `set_title` has no color channel, so the
 * Rust side composites a bitmap from these segments instead, see
 * `src-tauri/src/tray_render.rs`. `worstUsedPercent`, 0 to 100, is the bare
 * glyph's own arc fill, the worst active limit across everything tracked,
 * sent alongside the segments so the Rust side can draw it whether or not
 * anything is pinned. `tooltip` is the tray item's hover and VoiceOver
 * text, composed in full by `lib/traySegments.ts`'s `buildTrayTooltip` and
 * applied verbatim on the Rust side. */
export async function setTrayStatus(
  segments: TraySegment[],
  worstUsedPercent: number,
  tooltip: string,
): Promise<void> {
  return invoke("set_tray_status", { segments, worstUsedPercent, tooltip });
}

export async function setDetached(detached: boolean): Promise<void> {
  return invoke("set_detached", { detached });
}

/** Detached-window drag, called on every `mousemove` of a header-drag
 * gesture. See `drag_window_step`'s doc comment in
 * `src-tauri/src/shell.rs` for why moving the window's frame while the
 * mouse is held down over it also reactivates the app. */
export async function dragWindowStep(): Promise<void> {
  return invoke("drag_window_step");
}

/** Ends a header-drag gesture, mouseup, so the next one re-anchors instead
 * of jumping from a stale position. See `end_window_drag`. */
export async function endWindowDrag(): Promise<void> {
  return invoke("end_window_drag");
}

export async function debugRateLimitSnapshot(): Promise<Record<string, unknown>> {
  return invoke("debug_rate_limit_snapshot");
}

/** Starts Claude Code's own sign-in for a broken row's account, see
 * `src-tauri/src/signin.rs`. Quotos never touches the Keychain or a
 * credential. It only starts the process and later relays a pasted code
 * back into it. */
export async function startSignIn(accountId: string, configDir: string): Promise<void> {
  return invoke("start_sign_in", { accountId, configDir });
}

/** Relays a code pasted into the panel to the waiting process. */
export async function submitSignInCode(accountId: string, code: string): Promise<void> {
  return invoke("submit_sign_in_code", { accountId, code });
}

export async function cancelSignIn(accountId: string): Promise<void> {
  return invoke("cancel_sign_in", { accountId });
}

export async function forgetSignIn(accountId: string): Promise<void> {
  return invoke("forget_sign_in", { accountId });
}

/** Fires once when the sign-in process for `accountId` exits. */
export function onSignInFinished(
  callback: (event: SignInFinishedEvent) => void,
): Promise<() => void> {
  return listen<SignInFinishedEvent>("sign-in-finished", (event) => callback(event.payload));
}

/** What is currently configured for this account's `statusLine`. The
 * opt-in offer's own status check, so it never claims "not installed" for
 * an account someone already pointed `statusLine` at some other way. */
export async function statuslineStatus(configDir: string): Promise<StatuslineIntegrationStatus> {
  return invoke("statusline_status", { configDir });
}

/** The explicit in-app opt-in write. Rejects with a typed
 * `StatuslineError`, see types/entities.ts, in particular `conflict` when
 * a different `statusLine` is already configured and `force` was not set. */
export async function statuslineInstall(
  configDir: string,
  force: boolean,
): Promise<{ replaced_existing: boolean }> {
  return invoke("statusline_install", { configDir, force });
}

/** Removes the integration, restoring exactly the previous `statusLine`
 * state, or clearing the key if there was none. */
export async function statuslineRemove(configDir: string): Promise<void> {
  return invoke("statusline_remove", { configDir });
}
