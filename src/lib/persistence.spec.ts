import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest'

const invoke = vi.fn()

vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => invoke(...args),
}))

import type { PinGroup } from '@/types/entities'
import {
  loadPinGroups,
  loadTracked,
  savePinGroups,
  saveTracked,
  type TrackedAccount,
} from './persistence'

const LEGACY_KEY = 'quotos.tracked.v1'

const SAMPLE: TrackedAccount = {
  id: 'claude:claude',
  provider: 'claude',
  config_dir: '~/.claude',
  label: 'Renamed Personal',
  pinnedWindowIds: ['weekly_all'],
}

const SAMPLE_GROUP: PinGroup = {
  id: 'g1',
  name: 'Money',
  collapsed: true,
  order: 0,
  memberKeys: ['claude%3Aclaude::weekly_all'],
}

function setNative(): void {
  ;(window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = {}
}

function clearNative(): void {
  delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__
}

describe('persistence in the browser mock harness, without a native store', () => {
  beforeEach(() => {
    clearNative()
    window.localStorage.clear()
    invoke.mockReset()
  })

  test('reads and writes localStorage directly, never invoking the Rust side', async () => {
    await saveTracked([SAMPLE])
    expect(invoke).not.toHaveBeenCalled()
    const loaded = await loadTracked()
    expect(loaded).toEqual([SAMPLE])
  })
})

describe('persistence on the native path, migrating from localStorage', () => {
  beforeEach(() => {
    setNative()
    window.localStorage.clear()
    invoke.mockReset()
  })

  afterEach(() => {
    clearNative()
  })

  test('migrates a legacy localStorage list into the native store on first run', async () => {
    window.localStorage.setItem(LEGACY_KEY, JSON.stringify({ version: 1, tracked: [SAMPLE] }))
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'load_tracked') return Promise.resolve([])
      if (cmd === 'save_tracked') return Promise.resolve()
      throw new Error(`unexpected command ${cmd}`)
    })

    const loaded = await loadTracked()

    expect(loaded).toEqual([SAMPLE])
    expect(invoke).toHaveBeenCalledWith('save_tracked', { tracked: [SAMPLE] })
  })

  test('never clears or overwrites the legacy localStorage key after migrating', async () => {
    const raw = JSON.stringify({ version: 1, tracked: [SAMPLE] })
    window.localStorage.setItem(LEGACY_KEY, raw)
    invoke.mockImplementation((cmd: string) =>
      cmd === 'load_tracked' ? Promise.resolve([]) : Promise.resolve(),
    )

    await loadTracked()

    expect(window.localStorage.getItem(LEGACY_KEY)).toBe(raw)
  })

  test('migrates once, never twice: a non-empty native store is never overwritten from stale localStorage', async () => {
    const nativeAlready: TrackedAccount = { ...SAMPLE, label: 'Already Native' }
    window.localStorage.setItem(LEGACY_KEY, JSON.stringify({ version: 1, tracked: [SAMPLE] }))
    invoke.mockImplementation((cmd: string) =>
      cmd === 'load_tracked' ? Promise.resolve([nativeAlready]) : Promise.resolve(),
    )

    const loaded = await loadTracked()

    expect(loaded).toEqual([nativeAlready])
    expect(invoke).not.toHaveBeenCalledWith('save_tracked', expect.anything())
  })

  test('stays empty when the legacy key is absent, so a fresh install migrates nothing', async () => {
    invoke.mockImplementation((cmd: string) =>
      cmd === 'load_tracked' ? Promise.resolve([]) : Promise.resolve(),
    )

    const loaded = await loadTracked()

    expect(loaded).toEqual([])
    expect(invoke).not.toHaveBeenCalledWith('save_tracked', expect.anything())
  })

  test('stays empty when the legacy key is corrupt JSON, without throwing', async () => {
    window.localStorage.setItem(LEGACY_KEY, '{ not valid json')
    invoke.mockImplementation((cmd: string) =>
      cmd === 'load_tracked' ? Promise.resolve([]) : Promise.resolve(),
    )

    const loaded = await loadTracked()

    expect(loaded).toEqual([])
    expect(invoke).not.toHaveBeenCalledWith('save_tracked', expect.anything())
  })

  test('loadTracked falls back to empty if the native invoke itself fails', async () => {
    invoke.mockImplementation((cmd: string) =>
      cmd === 'load_tracked' ? Promise.reject(new Error('no such command')) : Promise.resolve(),
    )

    const loaded = await loadTracked()

    expect(loaded).toEqual([])
  })

  test('saveTracked calls the native command with the tracked list', async () => {
    invoke.mockResolvedValue(undefined)
    await saveTracked([SAMPLE])
    expect(invoke).toHaveBeenCalledWith('save_tracked', { tracked: [SAMPLE] })
  })

  test('saveTracked rejects when the native write fails, so the caller can retry', async () => {
    invoke.mockRejectedValue(new Error('disk full'))
    await expect(saveTracked([SAMPLE])).rejects.toThrow('disk full')
  })

  test('a failed migration save still returns the legacy list and leaves the migration to re-fire next launch', async () => {
    const raw = JSON.stringify({ version: 1, tracked: [SAMPLE] })
    window.localStorage.setItem(LEGACY_KEY, raw)
    invoke.mockImplementation((cmd: string) =>
      cmd === 'load_tracked' ? Promise.resolve([]) : Promise.reject(new Error('disk full')),
    )

    const loaded = await loadTracked()

    expect(loaded).toEqual([SAMPLE])
    expect(window.localStorage.getItem(LEGACY_KEY)).toBe(raw)
  })
})

describe('pin groups alongside the tracked list', () => {
  beforeEach(() => {
    clearNative()
    window.localStorage.clear()
    invoke.mockReset()
  })

  afterEach(() => {
    clearNative()
  })

  test('round-trips through the browser harness without disturbing the tracked list', async () => {
    await saveTracked([SAMPLE])
    await savePinGroups([SAMPLE_GROUP])

    expect(await loadPinGroups()).toEqual([SAMPLE_GROUP])
    expect(await loadTracked()).toEqual([SAMPLE])
  })

  test('saving the tracked list afterwards leaves the groups intact', async () => {
    await savePinGroups([SAMPLE_GROUP])
    await saveTracked([SAMPLE])

    expect(await loadPinGroups()).toEqual([SAMPLE_GROUP])
  })

  test('is empty for a payload written before pin groups shipped', async () => {
    window.localStorage.setItem(LEGACY_KEY, JSON.stringify({ version: 1, tracked: [SAMPLE] }))
    expect(await loadPinGroups()).toEqual([])
  })

  test('round-trips through the native commands', async () => {
    setNative()
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'load_pin_groups') return Promise.resolve([SAMPLE_GROUP])
      return Promise.resolve()
    })

    await savePinGroups([SAMPLE_GROUP])
    expect(invoke).toHaveBeenCalledWith('save_pin_groups', { groups: [SAMPLE_GROUP] })
    expect(await loadPinGroups()).toEqual([SAMPLE_GROUP])
  })

  test('falls back to none if the native load itself fails', async () => {
    setNative()
    invoke.mockRejectedValue(new Error('no such command'))
    expect(await loadPinGroups()).toEqual([])
  })

  test('rejects when the native write fails, so the caller can retry', async () => {
    setNative()
    invoke.mockRejectedValue(new Error('disk full'))
    await expect(savePinGroups([SAMPLE_GROUP])).rejects.toThrow('disk full')
  })
})
