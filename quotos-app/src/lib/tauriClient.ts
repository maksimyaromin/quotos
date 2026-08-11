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
export const setTrayTitle = client.setTrayTitle;
