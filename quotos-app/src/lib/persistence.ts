import type { AccountDescriptor } from "../types/entities";

/** I6: the user-owned list of tracked subscriptions — nothing is added by
 * default, and discovery only ever feeds this list, never replaces it.
 * Stored in the webview's own localStorage, which macOS scopes per bundle
 * identifier (WKWebView's data store), so it naturally diverges between
 * side-by-side builds with distinct identifiers — no extra plumbing needed
 * for v1/v2 to keep separate state. */
export interface TrackedAccount extends AccountDescriptor {
  /** User's own name for it, or null to use the provider-derived label. */
  label: string | null;
  pinned: boolean;
}

interface PersistedShape {
  version: 1;
  tracked: TrackedAccount[];
}

const KEY = "quotos.tracked.v1";

export function loadTracked(): TrackedAccount[] {
  try {
    const raw = window.localStorage.getItem(KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw) as Partial<PersistedShape>;
    if (!Array.isArray(parsed.tracked)) return [];
    return parsed.tracked;
  } catch {
    return [];
  }
}

export function saveTracked(tracked: TrackedAccount[]): void {
  try {
    const payload: PersistedShape = { version: 1, tracked };
    window.localStorage.setItem(KEY, JSON.stringify(payload));
  } catch {
    // Best-effort: a full localStorage or a private-browsing-style quota
    // failure shouldn't crash the panel — the in-memory list still works
    // for the rest of this session.
  }
}
