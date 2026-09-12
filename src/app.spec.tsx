import { act, cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest'

afterEach(cleanup)

class ResizeObserverStub {
  observe() {}
  unobserve() {}
  disconnect() {}
}
globalThis.ResizeObserver ??= ResizeObserverStub as unknown as typeof ResizeObserver

const hidePanel = vi.fn()
const fetchSnapshotSpy = vi.fn()
const renderStatusItem = vi.fn()
let trayGroupClick: ((groupId: string) => void) | null = null

const ONE_LIMIT = [{ kind: 'session', percent: 10, is_active: true, resets_at: null, scope: null }]
let usageLimits: unknown[] = ONE_LIMIT
let visibilityCallback: ((visible: boolean) => void) | null = null

vi.mock('./lib/tauri-client', () => ({
  listAccounts: () => Promise.resolve([]),
  fetchSnapshot: () => {
    fetchSnapshotSpy()
    return Promise.resolve({
      account_id: 'claude:claude',
      provider: 'claude',
      config_dir: '~/.claude',
      fetched_at: new Date().toISOString(),
      usage: { limits: usageLimits },
      profile: null,
    })
  },
  hidePanel: () => hidePanel(),
  onPanelVisibility: (callback: (visible: boolean) => void) => {
    visibilityCallback = callback
    return Promise.resolve(() => {})
  },
  setDetached: () => Promise.resolve(),
  dragWindowStep: () => Promise.resolve(),
  endWindowDrag: () => Promise.resolve(),
  startSignIn: () => Promise.resolve(),
  submitSignInCode: () => Promise.resolve(),
  cancelSignIn: () => Promise.resolve(),
  forgetSignIn: () => Promise.resolve(),
  onSignInFinished: () => Promise.resolve(() => {}),
  renderStatusItem: (...args: unknown[]) => {
    renderStatusItem(...args)
    return Promise.resolve()
  },
  onQuotaRefresh: () => Promise.resolve(() => {}),
  kickScheduler: () => Promise.resolve(),
  statuslineStatus: () => Promise.resolve({ kind: 'not_installed' }),
  statuslineEnable: () => Promise.resolve(),
  statuslineDisable: () => Promise.resolve(),
  onPanelBeakOffset: () => Promise.resolve(() => {}),
  onStatusItemGroupClicked: (callback: (groupId: string) => void) => {
    trayGroupClick = callback
    return Promise.resolve(() => {
      trayGroupClick = null
    })
  },
}))

vi.mock('./lib/persistence', () => ({
  loadTracked: () =>
    Promise.resolve([
      {
        id: 'claude:claude',
        provider: 'claude',
        config_dir: '~/.claude',
        label: null,
        pinnedWindowIds: [],
      },
    ]),
  saveTracked: () => Promise.resolve(),
  loadPinGroups: () => Promise.resolve([]),
  savePinGroups: () => Promise.resolve(),
}))

import App from './app'

async function renderAppWithRow() {
  render(<App />)
  return await screen.findByLabelText('More')
}

describe('Escape dismissal layering', () => {
  beforeEach(() => {
    hidePanel.mockReset()
    visibilityCallback = null
  })

  test('a bare Escape hides the panel', async () => {
    await renderAppWithRow()
    fireEvent.keyDown(window, { key: 'Escape' })
    expect(hidePanel).toHaveBeenCalledTimes(1)
  })

  test('Escape closes an open row menu and leaves the panel up; the next Escape hides', async () => {
    const trigger = await renderAppWithRow()
    fireEvent.click(trigger)
    expect(screen.getByText('Stop tracking')).toBeTruthy()

    fireEvent.keyDown(window, { key: 'Escape' })
    expect(screen.queryByText('Stop tracking')).toBeNull()
    expect(hidePanel).not.toHaveBeenCalled()

    fireEvent.keyDown(window, { key: 'Escape' })
    expect(hidePanel).toHaveBeenCalledTimes(1)
  })

  test('Escape in the rename field cancels the rename without hiding the panel', async () => {
    const trigger = await renderAppWithRow()
    fireEvent.click(trigger)
    fireEvent.click(screen.getByText('Rename'))
    const input = screen.getByRole('textbox')

    fireEvent.keyDown(input, { key: 'Escape' })
    expect(screen.queryByRole('textbox')).toBeNull()
    expect(hidePanel).not.toHaveBeenCalled()
  })

  test('the row menu does not survive a panel hide', async () => {
    const trigger = await renderAppWithRow()
    fireEvent.click(trigger)
    expect(screen.getByText('Stop tracking')).toBeTruthy()

    act(() => visibilityCallback!(false))
    expect(screen.queryByText('Stop tracking')).toBeNull()
  })
})

describe('pointer dismissal consumes the dismissing click', () => {
  function dismissByClicking(target: Element) {
    fireEvent.mouseDown(target)
    fireEvent.click(target)
  }

  test('a click on the row body only dismisses the menu, the row does not expand', async () => {
    const trigger = await renderAppWithRow()
    fireEvent.click(trigger)
    expect(screen.getByText('Stop tracking')).toBeTruthy()
    const expandToggle = screen.getByRole('button', { name: /1 limit/ })
    expect(expandToggle.getAttribute('aria-expanded')).toBe('false')

    dismissByClicking(screen.getByText('Claude'))
    expect(screen.queryByText('Stop tracking')).toBeNull()
    expect(expandToggle.getAttribute('aria-expanded')).toBe('false')
  })

  test('only the dismissing click is consumed, the next click acts normally', async () => {
    const trigger = await renderAppWithRow()
    fireEvent.click(trigger)

    dismissByClicking(screen.getByText('Claude'))
    dismissByClicking(screen.getByText('Claude'))
    expect(screen.getByRole('button', { name: /1 limit/ }).getAttribute('aria-expanded')).toBe(
      'true',
    )
  })

  test('a dismissing click on a button does not press it', async () => {
    const trigger = await renderAppWithRow()
    const callsBefore = fetchSnapshotSpy.mock.calls.length
    fireEvent.click(trigger)
    expect(screen.getByText('Stop tracking')).toBeTruthy()

    const refresh = screen.getByRole('button', { name: 'Read all now' })
    await act(async () => dismissByClicking(refresh))
    expect(screen.queryByText('Stop tracking')).toBeNull()
    expect(fetchSnapshotSpy.mock.calls.length).toBe(callsBefore)

    await act(async () => dismissByClicking(refresh))
    expect(fetchSnapshotSpy.mock.calls.length).toBe(callsBefore + 1)
  })
})

describe('panel reopen refreshes the presentation clock', () => {
  beforeEach(() => {
    visibilityCallback = null
  })

  test('re-reads `now` on visible=true so relative times are not hours stale', async () => {
    await renderAppWithRow()
    expect(screen.getByText('Last read just now')).toBeTruthy()

    const twoHoursLater = Date.now() + 2 * 60 * 60 * 1000
    const nowSpy = vi.spyOn(Date, 'now').mockReturnValue(twoHoursLater)
    act(() => visibilityCallback!(true))
    nowSpy.mockRestore()

    expect(screen.getByText('Last read 2h ago')).toBeTruthy()
  })

  test('opening the panel sends a real read for every tracked account, not a local wait', async () => {
    await renderAppWithRow()
    const callsBefore = fetchSnapshotSpy.mock.calls.length

    await act(async () => {
      visibilityCallback!(true)
    })

    expect(fetchSnapshotSpy.mock.calls.length).toBe(callsBefore + 1)
  })
})

describe('pinning a limit window into a group, end to end', () => {
  beforeEach(() => {
    renderStatusItem.mockReset()
    trayGroupClick = null
    usageLimits = [
      ...ONE_LIMIT,
      { kind: 'weekly_all', percent: 20, is_active: true, resets_at: null, scope: null },
    ]
  })

  afterEach(() => {
    usageLimits = ONE_LIMIT
  })

  function lastSegments() {
    return renderStatusItem.mock.calls[renderStatusItem.mock.calls.length - 1]?.[0]
  }

  async function pinSessionIntoNewGroup(name: string) {
    render(<App />)
    fireEvent.click(await screen.findByRole('button', { name: /2 limits/ }))
    fireEvent.click(screen.getAllByLabelText('Show in menu bar')[0])
    fireEvent.click(screen.getByText('New group…'))
    const field = screen.getByPlaceholderText('Group name')
    fireEvent.change(field, { target: { value: name } })
    await act(async () => {
      fireEvent.keyDown(field, { key: 'Enter' })
    })
  }

  test('the tray shows the new group as one rolled-up figure, tagged with the group', async () => {
    await pinSessionIntoNewGroup('Money')

    const segments = lastSegments()
    expect(segments).toHaveLength(1)
    expect(segments[0]).toMatchObject({ text: '10%', color: 'neutral' })
    expect(segments[0].groupId).toBeTruthy()
  })

  test('the tooltip names the member under its group', async () => {
    await pinSessionIntoNewGroup('Money')

    const tooltip = renderStatusItem.mock.calls[renderStatusItem.mock.calls.length - 1]?.[2]
    expect(tooltip).toContain('Money: ')
    expect(tooltip).toContain('Session 10%')
  })

  test('the panel itself grows no pinned section, just the row it always had', async () => {
    await pinSessionIntoNewGroup('Money')

    expect(screen.queryByText('Money')).toBeNull()
    expect(screen.queryByText('PINNED')).toBeNull()
    expect(screen.queryByRole('button', { name: /^(Expand|Collapse) all$/ })).toBeNull()
  })

  test("a click on the group's figure in the menu bar opens it out to its members", async () => {
    await pinSessionIntoNewGroup('Money')
    fireEvent.click(screen.getAllByLabelText('Show in menu bar')[0])
    fireEvent.click(screen.getByText('Add to Money'))
    await act(async () => {})
    expect(lastSegments()).toHaveLength(1)

    const groupId = lastSegments()[0].groupId
    await act(async () => {
      trayGroupClick?.(groupId)
    })

    const segments = lastSegments()
    expect(segments.map((s: { text: string }) => s.text)).toEqual(['10%', '20%'])
    expect(segments.every((s: { groupId: string }) => s.groupId === groupId)).toBe(true)
  })

  test('clicking it again rolls the group back up to one figure', async () => {
    await pinSessionIntoNewGroup('Money')
    const groupId = lastSegments()[0].groupId

    await act(async () => {
      trayGroupClick?.(groupId)
    })
    await act(async () => {
      trayGroupClick?.(groupId)
    })

    expect(lastSegments()).toHaveLength(1)
  })

  test('unpinning the window empties the tray again', async () => {
    await pinSessionIntoNewGroup('Money')

    await act(async () => {
      fireEvent.click(screen.getAllByLabelText('Remove from menu bar')[0])
    })

    expect(lastSegments()).toEqual([])
  })
})
