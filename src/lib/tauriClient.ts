import type { ScheduledRefreshEvent, StatusItemSegment } from "../types/entities";
import * as live from "./liveClient";
import * as mock from "./mockClient";

/** Tauri injects `__TAURI_INTERNALS__` at runtime; a plain browser never has it. */
const isTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

const client = isTauri ? live : mock;

export const listAccounts = client.listAccounts;
export const fetchSnapshot = client.fetchSnapshot;
export const hidePanel = client.hidePanel;
export const onPanelVisibility = client.onPanelVisibility;
export const setDetached = client.setDetached;

// The browser harness has no native window or status item, so these six
// override mockClient.ts's missing exports with an inline no-op instead of
// adding unused stubs there.
export const dragWindowStep: () => Promise<void> = isTauri ? live.dragWindowStep : async () => {};
export const endWindowDrag: () => Promise<void> = isTauri ? live.endWindowDrag : async () => {};
export const renderStatusItem: (
  segments: StatusItemSegment[],
  worstUsedPercent: number,
  tooltip: string,
) => Promise<void> = isTauri ? live.renderStatusItem : async () => {};
export const onQuotaRefresh: (
  callback: (event: ScheduledRefreshEvent) => void,
) => Promise<() => void> = isTauri ? live.onQuotaRefresh : async () => () => {};
export const kickScheduler: () => Promise<void> = isTauri ? live.kickScheduler : async () => {};
export const onPanelBeakOffset: (callback: (offsetPx: number) => void) => Promise<() => void> =
  isTauri ? live.onPanelBeakOffset : async () => () => {};

export const debugRateLimitSnapshot = client.debugRateLimitSnapshot;
export const startSignIn = client.startSignIn;
export const submitSignInCode = client.submitSignInCode;
export const cancelSignIn = client.cancelSignIn;
export const forgetSignIn = client.forgetSignIn;
export const onSignInFinished = client.onSignInFinished;
export const statuslineStatus = client.statuslineStatus;
export const statuslineInstall = client.statuslineInstall;
export const statuslineRemove = client.statuslineRemove;
