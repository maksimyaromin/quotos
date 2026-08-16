import { invoke } from "@tauri-apps/api/core";
import type { AccountDescriptor } from "@/types/entities";

/** The user-owned list of tracked subscriptions. Nothing is added by
 * default, and discovery only ever feeds this list, never replaces it.
 * Owned natively as a plain JSON file in the app's config directory,
 * written by Rust, see `src-tauri/src/persistence.rs`. The browser-only
 * mock harness has no Rust side to talk to, so it keeps using
 * `localStorage` directly, branching on that the same way `tauriClient.ts`
 * does. */
export interface TrackedAccount extends AccountDescriptor {
  /** User's own name for it, or null to use the provider-derived label. */
  label: string | null;
  /** The persisted set of pinned window ids, see
   * `Subscription.pinnedWindowIds`. An older record on disk may instead
   * carry a `pinned: boolean` field, which `useSubscriptions.ts`'s mount
   * effect migrates into "that subscription's headline window is pinned"
   * once the first read after load reveals the headline window's id. This
   * type describes only the current, post-migration shape. */
  pinnedWindowIds: string[];
}

interface PersistedShape {
  version: 1;
  tracked: TrackedAccount[];
}

const LEGACY_KEY = "quotos.tracked.v1";
// Read fresh on every call, not cached at module scope, so tests can toggle
// it. In the real app, Tauri injects this global before any app JS runs, so
// this is equivalent to a constant in practice.
function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

function loadLegacyLocalStorage(): TrackedAccount[] {
  try {
    const raw = window.localStorage.getItem(LEGACY_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw) as Partial<PersistedShape>;
    if (!Array.isArray(parsed.tracked)) return [];
    return parsed.tracked;
  } catch {
    return [];
  }
}

function saveLocalStorage(tracked: TrackedAccount[]): void {
  try {
    const payload: PersistedShape = { version: 1, tracked };
    window.localStorage.setItem(LEGACY_KEY, JSON.stringify(payload));
  } catch {
    // A full localStorage or a private-browsing-style quota failure must
    // not crash the panel. The in-memory list still works for the rest of
    // this session.
  }
}

/** Migrates a pre-upgrade `localStorage` list into the native store,
 * exactly once, only when the native store is genuinely empty. Once it
 * has any data, even from the user removing everything, this never fires
 * again. The old `localStorage` key is deliberately left untouched after
 * migrating, so a build predating the native store can still be
 * recovered by rolling back to it. */
async function migrateFromLocalStorageIfEmpty(native: TrackedAccount[]): Promise<TrackedAccount[]> {
  if (native.length > 0) return native;
  const legacy = loadLegacyLocalStorage();
  if (legacy.length === 0) return native;
  try {
    await saveTracked(legacy);
  } catch {
    // The legacy list is still the right thing to show this session. The
    // native store stayed empty and the localStorage source is never
    // touched, so the migration simply re-fires on the next launch.
  }
  return legacy;
}

export async function loadTracked(): Promise<TrackedAccount[]> {
  if (!isTauri()) return loadLegacyLocalStorage();
  let native: TrackedAccount[] = [];
  try {
    native = await invoke<TrackedAccount[]>("load_tracked");
  } catch {
    native = [];
  }
  return migrateFromLocalStorageIfEmpty(native);
}

/** A failed native write rejects rather than being swallowed here. The
 * caller holds the record of what was last saved, and a save it believes
 * succeeded is one that never gets retried, so the tracked list would
 * silently die with the process. The browser path keeps its best-effort
 * shape, since `localStorage` is a mock-harness convenience, not the
 * durable store. */
export async function saveTracked(tracked: TrackedAccount[]): Promise<void> {
  if (!isTauri()) {
    saveLocalStorage(tracked);
    return;
  }
  await invoke("save_tracked", { tracked });
}
