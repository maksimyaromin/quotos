import * as live from "./liveClient";
import * as mock from "./mockClient";
import type { ScheduledRefreshEvent, TraySegment } from "../types/entities";

/** Real Tauri build vs. `npm run dev` opened directly in a browser for
 * visual QA. See mockClient.ts for why this seam exists. */
const isTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

const client = isTauri ? live : mock;

export const listAccounts = client.listAccounts;
export const fetchSnapshot = client.fetchSnapshot;
export const hidePanel = client.hidePanel;
export const onPanelVisibility = client.onPanelVisibility;
export const setDetached = client.setDetached;
export const debugRateLimitSnapshot = client.debugRateLimitSnapshot;

/** R2-6: both sides implement the same sign-in lifecycle — mockClient.ts
 * simulates `claude setup-token` well enough for the paste-code UI to be
 * reviewed in a browser, since the real process (and the browser it opens)
 * can't be driven from here. See signin.rs and mockClient.ts. */
export const startSignIn = client.startSignIn;
export const submitSignInCode = client.submitSignInCode;
export const cancelSignIn = client.cancelSignIn;
export const forgetSignIn = client.forgetSignIn;
export const onSignInFinished = client.onSignInFinished;

/** R2-2: colored tray digits only exist on the native side (see
 * liveClient.ts's doc comment) — there's no real tray to update in the
 * browser mock harness, so this branches directly here rather than adding
 * an unused stub export to mockClient.ts (owned by the surface half of this
 * round; see CLAUDE.md's file-ownership split). */
export const setTrayStatus: (segments: TraySegment[]) => Promise<void> = isTauri
  ? live.setTrayStatus
  : async () => {};

/** R2-4: the mock harness has no Rust scheduler to push events from — the
 * browser path never calls back, matching setTrayStatus's pattern above. */
export const onQuotaRefresh: (callback: (event: ScheduledRefreshEvent) => void) => Promise<() => void> = isTauri
  ? live.onQuotaRefresh
  : async () => () => {};

/** R2-4: no-op in the browser harness — useSubscriptions.ts only calls this
 * on the native path, where a real scheduler exists to kick. */
export const kickScheduler: () => Promise<void> = isTauri ? live.kickScheduler : async () => {};

/** S2: the statusline opt-in's install/status/remove lifecycle. Both sides
 * implement the same shape — the mock client simulates it well enough for
 * the offer/conflict/replace/remove UI to be reviewed in a plain browser;
 * see mockClient.ts. */
export const statuslineStatus = client.statuslineStatus;
export const statuslineInstall = client.statuslineInstall;
export const statuslineRemove = client.statuslineRemove;

/** B3/B5: no real tray glyph exists in the browser mock harness to compute
 * an offset from — App.tsx keeps its own static fallback constant for that
 * case, matching setTrayStatus's pattern above. */
export const onPanelBeakOffset: (callback: (offsetPx: number) => void) => Promise<() => void> = isTauri
  ? live.onPanelBeakOffset
  : async () => () => {};
