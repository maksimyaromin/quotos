import { invoke } from '@tauri-apps/api/core'
import type { AccountDescriptor } from '@/types/entities'

export interface TrackedAccount extends AccountDescriptor {
  label: string | null
  pinnedWindowIds: string[]
}

interface PersistedShape {
  version: 1
  tracked: TrackedAccount[]
}

const LEGACY_KEY = 'quotos.tracked.v1'
function isTauri(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window
}

function loadLegacyLocalStorage(): TrackedAccount[] {
  try {
    const raw = window.localStorage.getItem(LEGACY_KEY)
    if (!raw) return []
    const parsed = JSON.parse(raw) as Partial<PersistedShape>
    if (!Array.isArray(parsed.tracked)) return []
    return parsed.tracked
  } catch {
    return []
  }
}

function saveLocalStorage(tracked: TrackedAccount[]): void {
  try {
    const payload: PersistedShape = { version: 1, tracked }
    window.localStorage.setItem(LEGACY_KEY, JSON.stringify(payload))
  } catch {}
}

async function migrateFromLocalStorageIfEmpty(native: TrackedAccount[]): Promise<TrackedAccount[]> {
  if (native.length > 0) return native
  const legacy = loadLegacyLocalStorage()
  if (legacy.length === 0) return native
  try {
    await saveTracked(legacy)
  } catch {}
  // The legacy key is left in place, not cleared, so rolling back to a
  // pre-native-store build can still recover this list.
  return legacy
}

export async function loadTracked(): Promise<TrackedAccount[]> {
  if (!isTauri()) return loadLegacyLocalStorage()
  let native: TrackedAccount[] = []
  try {
    native = await invoke<TrackedAccount[]>('load_tracked')
  } catch {
    native = []
  }
  return migrateFromLocalStorageIfEmpty(native)
}

export async function saveTracked(tracked: TrackedAccount[]): Promise<void> {
  if (!isTauri()) {
    saveLocalStorage(tracked)
    return
  }
  await invoke('save_tracked', { tracked })
}
