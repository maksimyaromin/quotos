import type { DndContextProps, DragEndEvent, DragStartEvent } from '@dnd-kit/core'
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import type { ReactNode } from 'react'
import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest'
import { type DragItem, pinDragId, UNGROUPED_DROP_ID } from '@/lib/customize-drag'
import { pinMemberKey } from '@/lib/pin-groups'

afterEach(cleanup)

// jsdom has no layout, so the drag library has no geometry to report
// from; its context is stood in for and handed the answers a real drag
// would reach. See customize-display-screen.spec.tsx.
const dnd = vi.hoisted(() => ({ props: null as unknown }))

vi.mock('@dnd-kit/core', async (importOriginal) => {
  const actual = await importOriginal<typeof import('@dnd-kit/core')>()
  return {
    ...actual,
    DndContext: (props: DndContextProps) => {
      dnd.props = props
      return props.children
    },
    DragOverlay: ({ children }: { children?: ReactNode }) => children ?? null,
  }
})

class ResizeObserverStub {
  observe() {}
  unobserve() {}
  disconnect() {}
}
globalThis.ResizeObserver ??= ResizeObserverStub as unknown as typeof ResizeObserver

const hidePanel = vi.fn()
const fetchSnapshotSpy = vi.fn()
const renderStatusItem = vi.fn()
const renderStatusItemPreview = vi.fn()
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
  renderStatusItemPreview: (...args: unknown[]) => {
    renderStatusItemPreview(...args)
    return Promise.resolve({ width: 120, height: 36, rgbaBase64: '' })
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
  loadIconFillSource: () => Promise.resolve(null),
  saveIconFillSource: () => Promise.resolve(),
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

describe('arranging pins into groups on the customize screen, end to end', () => {
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

  const ACCOUNT = 'claude:claude'
  const SESSION = pinMemberKey(ACCOUNT, 'session')
  const WEEKLY = pinMemberKey(ACCOUNT, 'weekly_all')

  interface Target {
    id: string
    data: { current?: DragItem }
  }

  function pin(key: string, groupId: string | null = null): Target {
    return { id: pinDragId(key), data: { current: { kind: 'pin', key, groupId } } }
  }

  const LEAVE_ZONE: Target = { id: UNGROUPED_DROP_ID, data: {} }

  async function dragOnto(active: Target, over: Target | null) {
    const context = dnd.props as DndContextProps
    act(() => context.onDragStart?.({ active } as unknown as DragStartEvent))
    await act(async () => {
      context.onDragEnd?.({ active, over } as unknown as DragEndEvent)
    })
  }

  async function pinBothLimits() {
    fireEvent.click(await screen.findByRole('button', { name: /2 limits/ }))
    for (const button of screen.getAllByLabelText('Show in menu bar')) {
      await act(async () => {
        fireEvent.click(button)
      })
    }
  }

  async function openCustomizeScreen() {
    render(<App />)
    await pinBothLimits()
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'Customize display' }))
    })
  }

  async function groupTheTwoPins() {
    await openCustomizeScreen()
    await dragOnto(pin(WEEKLY), pin(SESSION))
  }

  test('pinning is a plain toggle again, with no destination menu to answer', async () => {
    render(<App />)
    fireEvent.click(await screen.findByRole('button', { name: /2 limits/ }))
    await act(async () => {
      fireEvent.click(screen.getAllByLabelText('Show in menu bar')[0])
    })

    expect(screen.queryByText('Pin standalone')).toBeNull()
    expect(screen.queryByText('New group…')).toBeNull()
    expect(lastSegments()).toEqual([{ kind: 'figure', text: '10%', color: 'neutral' }])
  })

  test('the way in is offered only once something is pinned', async () => {
    render(<App />)
    await screen.findByRole('button', { name: /2 limits/ })
    expect(screen.queryByRole('button', { name: 'Customize display' })).toBeNull()

    await pinBothLimits()
    expect(screen.getByRole('button', { name: 'Customize display' })).toBeTruthy()
  })

  // Not "the preview looks like the tray": the preview is drawn by the
  // tray's own compositor, from the very list the tray was last set to.
  test('the preview is drawn from the very segment list the menu bar was set to', async () => {
    await openCustomizeScreen()

    expect(lastSegments().map((s: { text: string }) => s.text)).toEqual(['10%', '20%'])
    await waitFor(() =>
      expect(renderStatusItemPreview).toHaveBeenLastCalledWith(lastSegments(), expect.any(Number)),
    )
  })

  test('dragging one pin onto the other rolls them up into one chip and no figure', async () => {
    await groupTheTwoPins()

    expect(lastSegments()).toEqual([
      { kind: 'chip', slug: 'GRO', color: expect.any(String), groupId: expect.any(String) },
    ])
  })

  test('the preview follows the arrangement without leaving the screen', async () => {
    await groupTheTwoPins()

    await waitFor(() =>
      expect(renderStatusItemPreview).toHaveBeenLastCalledWith(lastSegments(), expect.any(Number)),
    )
  })

  test("a click on the group's chip in the menu bar opens it out to its members", async () => {
    await groupTheTwoPins()

    const groupId = lastSegments()[0].groupId
    await act(async () => {
      trayGroupClick?.(groupId)
    })

    expect(lastSegments()).toEqual([
      { kind: 'chip', slug: 'GRO', color: expect.any(String), groupId },
      { kind: 'figure', text: '10%', color: 'neutral' },
      { kind: 'figure', text: '20%', color: 'neutral' },
    ])
  })

  test('taking a member back out leaves it pinned, standing on its own', async () => {
    await groupTheTwoPins()
    const groupId: string = lastSegments()[0].groupId

    await dragOnto(pin(WEEKLY, groupId), LEAVE_ZONE)

    expect(lastSegments()).toEqual([
      { kind: 'chip', slug: 'GRO', color: expect.any(String), groupId },
      { kind: 'figure', text: '20%', color: 'neutral' },
    ])
  })

  test('deleting the group leaves both windows pinned, each on its own figure', async () => {
    await groupTheTwoPins()

    await act(async () => {
      fireEvent.click(screen.getByLabelText('Delete Group 1'))
    })

    expect(lastSegments().map((s: { text: string }) => s.text)).toEqual(['10%', '20%'])
  })

  test('unpinning from the list takes the window out of its group as well', async () => {
    await groupTheTwoPins()

    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'Back' }))
    })
    await act(async () => {
      fireEvent.click(screen.getAllByLabelText('Remove from menu bar')[1])
    })

    // The group keeps the member that is still pinned, so it still
    // draws its chip, and the unpinned one is gone from behind it.
    expect(lastSegments()).toEqual([
      { kind: 'chip', slug: 'GRO', color: expect.any(String), groupId: expect.any(String) },
    ])

    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'Customize display' }))
    })
    expect(screen.queryByText('Claude — Weekly')).toBeNull()
    expect(screen.getByText('Claude — Session')).toBeTruthy()
  })
})
