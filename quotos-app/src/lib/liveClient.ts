import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { AccountDescriptor, RawSnapshot, ScheduledRefreshEvent, TraySegment } from "../types/entities";

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

/** R2-4: the Rust-side scheduler's once-a-minute automatic reads arrive
 * here, one event per attempt — see `src-tauri/src/scheduler.rs`. */
export function onQuotaRefresh(callback: (event: ScheduledRefreshEvent) => void): Promise<() => void> {
  return listen<ScheduledRefreshEvent>("quota-refresh", (event) => callback(event.payload));
}

/** R2-4: nudges the Rust scheduler to run a due-check pass immediately —
 * called once, right after subscribing to `onQuotaRefresh`, so the launch
 * read is still near-instant instead of waiting out the periodic loop's
 * first (deliberately delayed) tick. See `kick_scheduler`'s doc comment in
 * `src-tauri/src/lib.rs` for why the delay exists at all. */
export async function kickScheduler(): Promise<void> {
  return invoke("kick_scheduler");
}

/** R2-2: replaces the old plain-string `set_tray_title` — `tray-icon`
 * v0.24.2's macOS `set_title` has no color channel, so the Rust side
 * composites a bitmap from these segments instead (see
 * `src-tauri/src/tray_render.rs`). */
export async function setTrayStatus(segments: TraySegment[]): Promise<void> {
  return invoke("set_tray_status", { segments });
}

export async function setDetached(detached: boolean): Promise<void> {
  return invoke("set_detached", { detached });
}

export async function debugRateLimitSnapshot(): Promise<Record<string, unknown>> {
  return invoke("debug_rate_limit_snapshot");
}
