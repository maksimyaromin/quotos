import { invoke } from '@tauri-apps/api/core'
import type { AccountDescriptor, PinGroup } from '@/types/entities'
import { isGroupColor, nextGroupColor } from './pin-groups'

export interface TrackedAccount extends AccountDescriptor {
  label: string | null
  pinnedWindowIds: string[]
}

interface PersistedShape {
  version: 1
  tracked: TrackedAccount[]
  groups?: PinGroup[]
  // Which single tracked window drives the menu bar mark's gauge, as a
  // `lib/pin-groups.ts` member key. Absent is the arithmetic mean of
  // every subscription's own headline figure.
  iconFillSource?: string | null
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

function loadLocalStorageIconFillSource(): string | null {
  const parsed = readLocalStorage()
  return typeof parsed.iconFillSource === 'string' ? parsed.iconFillSource : null
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
      iconFillSource: existing.iconFillSource ?? null,
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

// A group saved before colours shipped, or one whose colour a hand
// edit spelled wrong, gets one from the palette here rather than
// downstream: every other reader can then count on a group having a
// colour to draw.
function withGroupColors(groups: PinGroup[]): PinGroup[] {
  const settled: PinGroup[] = []
  for (const group of groups) {
    settled.push(isGroupColor(group.color) ? group : { ...group, color: nextGroupColor(settled) })
  }
  return settled
}

export async function loadPinGroups(): Promise<PinGroup[]> {
  if (!isTauri()) return withGroupColors(loadLocalStorageGroups())
  try {
    return withGroupColors(await invoke<PinGroup[]>('load_pin_groups'))
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

export async function loadIconFillSource(): Promise<string | null> {
  if (!isTauri()) return loadLocalStorageIconFillSource()
  try {
    return (await invoke<string | null>('load_icon_fill_source')) ?? null
  } catch {
    return null
  }
}

export async function saveIconFillSource(key: string | null): Promise<void> {
  if (!isTauri()) {
    saveLocalStorage({ iconFillSource: key })
    return
  }
  await invoke('save_icon_fill_source', { key })
}
