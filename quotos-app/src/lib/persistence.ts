import { invoke } from "@tauri-apps/api/core";
import type { AccountDescriptor } from "../types/entities";

/** I6: the user-owned list of tracked subscriptions — nothing is added by
 * default, and discovery only ever feeds this list, never replaces it.
 *
 * R2-5: now owned natively (a plain JSON file in the app's config
 * directory, written by Rust — see `src-tauri/src/persistence.rs` for why,
 * including the real reproduction that ruled out just fixing the
 * `localStorage` path). The browser-only mock harness has no Rust side to
 * talk to, so it keeps using `localStorage` directly — this file branches
 * on that the same way `tauriClient.ts` does, rather than depending on it.
 */
export interface TrackedAccount extends AccountDescriptor {
  /** User's own name for it, or null to use the provider-derived label. */
  label: string | null;
  /** v4: the persisted set of pinned window ids — see
   * `Subscription.pinnedWindowIds`. A pre-v4 record on disk carries the old
   * `pinned: boolean` field instead of this one; `useSubscriptions.ts`'s
   * mount effect is what migrates it (to "that subscription's headline
   * window is pinned", once the first read after load reveals what the
   * headline window's id actually is) — this type only describes the
   * current, post-migration shape that gets written back out. */
  pinnedWindowIds: string[];
}

interface PersistedShape {
  version: 1;
  tracked: TrackedAccount[];
}

const LEGACY_KEY = "quotos.tracked.v1";
// Read fresh on every call (not module scope) so tests can toggle it; in the
// real app Tauri injects this global before any app JS runs, so it's
// equivalent to a constant in practice.
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
    // Best-effort: a full localStorage or a private-browsing-style quota
    // failure shouldn't crash the panel — the in-memory list still works
    // for the rest of this session.
  }
}

/** R2-5: migrates a pre-upgrade `localStorage` list into the native store,
 * exactly once. Only runs when the native store is genuinely empty — once
 * it has any data (even an empty save from the user removing everything),
 * this never fires again, so a deliberately-emptied list can't be
 * resurrected from stale `localStorage` on a later launch.
 *
 * Followup-3: the old `localStorage` key is deliberately left untouched
 * after migrating — never cleared, never overwritten. If the native file
 * turns out wrong, going back to the previous build must still show his
 * real list; a migration that deletes its own source as it runs would make
 * that recovery impossible. Leftover legacy data is otherwise inert: once
 * the native store is non-empty, this function returns before ever reading
 * `localStorage` again. */
async function migrateFromLocalStorageIfEmpty(native: TrackedAccount[]): Promise<TrackedAccount[]> {
  if (native.length > 0) return native;
  const legacy = loadLegacyLocalStorage();
  if (legacy.length === 0) return native;
  try {
    await saveTracked(legacy);
  } catch {
    // The legacy list is still the right thing to show this session; the
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

/** R2: a failed native write rejects rather than being swallowed here — the
 * caller is the one holding the "what was last saved" record, and a save it
 * believes succeeded is a save that never gets retried (the tracked list
 * then silently dies with the process). The browser path keeps its
 * best-effort shape: `localStorage` is a mock-harness convenience, not the
 * durable store. */
export async function saveTracked(tracked: TrackedAccount[]): Promise<void> {
  if (!isTauri()) {
    saveLocalStorage(tracked);
    return;
  }
  await invoke("save_tracked", { tracked });
}
