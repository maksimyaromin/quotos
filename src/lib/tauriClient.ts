import type { ScheduledRefreshEvent, TraySegment } from "../types/entities";
import * as live from "./liveClient";
import * as mock from "./mockClient";

/** Real Tauri build vs. `npm run dev` opened directly in a browser for
 * visual QA. See mockClient.ts for why this seam exists. */
const isTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

const client = isTauri ? live : mock;

export const listAccounts = client.listAccounts;
export const fetchSnapshot = client.fetchSnapshot;
export const hidePanel = client.hidePanel;
export const onPanelVisibility = client.onPanelVisibility;
export const setDetached = client.setDetached;

/** The manual, frame-based detached-window drag lives in `liveClient.ts`'s
 * `dragWindowStep` and `endWindowDrag`. The mock harness has no native
 * window to move, so it no-ops the same way `setTrayStatus` does below.
 * App.tsx's own `!isTauri` branch handles dragging locally through React
 * state instead. */
export const dragWindowStep: () => Promise<void> = isTauri ? live.dragWindowStep : async () => {};
export const endWindowDrag: () => Promise<void> = isTauri ? live.endWindowDrag : async () => {};
export const debugRateLimitSnapshot = client.debugRateLimitSnapshot;

/** Both sides implement the same sign-in lifecycle. `mockClient.ts`
 * simulates `claude setup-token` well enough for the paste-code UI to be
 * reviewed in a browser, since the real process and the browser it opens
 * cannot be driven from here. See signin.rs and mockClient.ts. */
export const startSignIn = client.startSignIn;
export const submitSignInCode = client.submitSignInCode;
export const cancelSignIn = client.cancelSignIn;
export const forgetSignIn = client.forgetSignIn;
export const onSignInFinished = client.onSignInFinished;

/** Colored tray digits only exist on the native side, see `liveClient.ts`'s
 * doc comment. The browser mock harness has no real tray to update, so
 * this branches directly here instead of adding an unused stub export to
 * mockClient.ts. */
export const setTrayStatus: (
  segments: TraySegment[],
  worstUsedPercent: number,
  tooltip: string,
) => Promise<void> = isTauri ? live.setTrayStatus : async () => {};

/** The mock harness has no Rust scheduler to push events from. The browser
 * path never calls back, matching `setTrayStatus`'s pattern above. */
export const onQuotaRefresh: (
  callback: (event: ScheduledRefreshEvent) => void,
) => Promise<() => void> = isTauri ? live.onQuotaRefresh : async () => () => {};

/** No-op in the browser harness. `useSubscriptions.ts` only calls this on
 * the native path, where a real scheduler exists to kick. */
export const kickScheduler: () => Promise<void> = isTauri ? live.kickScheduler : async () => {};

/** The statusline opt-in's install, status and remove lifecycle. Both sides
 * implement the same shape. The mock client simulates it well enough for
 * the offer, conflict, replace and remove UI to be reviewed in a plain
 * browser, see mockClient.ts. */
export const statuslineStatus = client.statuslineStatus;
export const statuslineInstall = client.statuslineInstall;
export const statuslineRemove = client.statuslineRemove;

/** No real tray glyph exists in the browser mock harness to compute an
 * offset from. App.tsx keeps its own static fallback constant for that
 * case, matching `setTrayStatus`'s pattern above. */
export const onPanelBeakOffset: (callback: (offsetPx: number) => void) => Promise<() => void> =
  isTauri ? live.onPanelBeakOffset : async () => () => {};
