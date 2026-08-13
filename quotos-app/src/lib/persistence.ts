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
  pinned: boolean;
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
  await saveTracked(legacy);
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

export async function saveTracked(tracked: TrackedAccount[]): Promise<void> {
  if (!isTauri()) {
    saveLocalStorage(tracked);
    return;
  }
  try {
    await invoke("save_tracked", { tracked });
  } catch {
    // Best-effort, matching the old localStorage path's failure mode: a
    // write failure shouldn't crash the panel — the in-memory list still
    // works for the rest of this session.
  }
}
