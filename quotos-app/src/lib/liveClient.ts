import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { AccountDescriptor, RawSnapshot } from "../types/entities";

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

export async function setTrayTitle(title: string): Promise<void> {
  return invoke("set_tray_title", { title });
}

export async function setDetached(detached: boolean): Promise<void> {
  return invoke("set_detached", { detached });
}

export async function debugRateLimitSnapshot(): Promise<Record<string, unknown>> {
  return invoke("debug_rate_limit_snapshot");
}
