import { act, cleanup, render, screen } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest'
import type { StatuslineIntegrationStatus } from '@/types/entities'

afterEach(cleanup)

const statuslineStatus = vi.fn<() => Promise<StatuslineIntegrationStatus>>()
const statuslineEnable = vi.fn<() => Promise<void>>()
const statuslineDisable = vi.fn<() => Promise<void>>()

vi.mock('../lib/tauri-client', () => ({
  statuslineStatus: () => statuslineStatus(),
  statuslineEnable: () => statuslineEnable(),
  statuslineDisable: () => statuslineDisable(),
}))

import { StatuslineControl } from './statusline-control'

const CONFIG_DIR = '/Users/x/.claude'

describe('StatuslineControl', () => {
  beforeEach(() => {
    statuslineStatus.mockReset()
    statuslineEnable.mockReset().mockResolvedValue(undefined)
    statuslineDisable.mockReset().mockResolvedValue(undefined)
  })

  test('the off state shows a button and names the settings file it would write', async () => {
    statuslineStatus.mockResolvedValue({ kind: 'not_installed' })
    render(<StatuslineControl configDir={CONFIG_DIR} />)

    const button = await screen.findByRole('button', { name: 'Enable live updates' })
    expect(button).toBeTruthy()
    expect(screen.getByText(new RegExp(`${CONFIG_DIR}/settings.json`))).toBeTruthy()
  })

  test('the on state shows a turn-off button and no longer offers to enable', async () => {
    statuslineStatus.mockResolvedValue({ kind: 'installed' })
    render(<StatuslineControl configDir={CONFIG_DIR} />)

    expect(await screen.findByRole('button', { name: 'Turn off live updates' })).toBeTruthy()
    expect(screen.queryByRole('button', { name: 'Enable live updates' })).toBeNull()
  })

  test('clicking enable calls the enable command and flips to the on state', async () => {
    statuslineStatus.mockResolvedValue({ kind: 'not_installed' })
    render(<StatuslineControl configDir={CONFIG_DIR} />)

    const button = await screen.findByRole('button', { name: 'Enable live updates' })
    await act(async () => button.click())

    expect(statuslineEnable).toHaveBeenCalledTimes(1)
    expect(await screen.findByRole('button', { name: 'Turn off live updates' })).toBeTruthy()
  })

  test('clicking turn off calls the disable command and flips to the off state', async () => {
    statuslineStatus.mockResolvedValue({ kind: 'installed' })
    render(<StatuslineControl configDir={CONFIG_DIR} />)

    const button = await screen.findByRole('button', { name: 'Turn off live updates' })
    await act(async () => button.click())

    expect(statuslineDisable).toHaveBeenCalledTimes(1)
    expect(await screen.findByRole('button', { name: 'Enable live updates' })).toBeTruthy()
  })
})
