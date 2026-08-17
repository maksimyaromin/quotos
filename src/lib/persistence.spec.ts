import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest'

const invoke = vi.fn()

vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => invoke(...args),
}))

import { loadTracked, saveTracked, type TrackedAccount } from './persistence'

const LEGACY_KEY = 'quotos.tracked.v1'

const SAMPLE: TrackedAccount = {
  id: 'claude:claude',
  provider: 'claude',
  config_dir: '~/.claude',
  label: 'Renamed Personal',
  pinnedWindowIds: ['weekly_all'],
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
