import { invoke } from '@tauri-apps/api/core'
import type { AccountDescriptor, PinGroup } from '@/types/entities'

export interface TrackedAccount extends AccountDescriptor {
  label: string | null
  pinnedWindowIds: string[]
}

interface PersistedShape {
  version: 1
  tracked: TrackedAccount[]
  groups?: PinGroup[]
}

const LEGACY_KEY = 'quotos.tracked.v1'
function isTauri(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window
}

function readLocalStorage(): Partial<PersistedShape> {
  try {
    const raw = window.localStorage.getItem(LEGACY_KEY)
    if (!raw) return {}
    return JSON.parse(raw) as Partial<PersistedShape>
  } catch {
    return {}
  }
}

function loadLegacyLocalStorage(): TrackedAccount[] {
  const parsed = readLocalStorage()
  return Array.isArray(parsed.tracked) ? parsed.tracked : []
}

function loadLocalStorageGroups(): PinGroup[] {
  const parsed = readLocalStorage()
  return Array.isArray(parsed.groups) ? parsed.groups : []
}

// Read-modify-write, so saving one half of the payload never drops the
// other; the native store keeps both under one lock for the same reason.
function saveLocalStorage(patch: Partial<PersistedShape>): void {
  try {
    const existing = readLocalStorage()
    const payload: PersistedShape = {
      version: 1,
      tracked: Array.isArray(existing.tracked) ? existing.tracked : [],
      groups: Array.isArray(existing.groups) ? existing.groups : [],
      ...patch,
    }
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
    saveLocalStorage({ tracked })
    return
  }
  await invoke('save_tracked', { tracked })
}

export async function loadPinGroups(): Promise<PinGroup[]> {
  if (!isTauri()) return loadLocalStorageGroups()
  try {
    return await invoke<PinGroup[]>('load_pin_groups')
  } catch {
    return []
  }
}

export async function savePinGroups(groups: PinGroup[]): Promise<void> {
  if (!isTauri()) {
    saveLocalStorage({ groups })
    return
  }
  await invoke('save_pin_groups', { groups })
}
