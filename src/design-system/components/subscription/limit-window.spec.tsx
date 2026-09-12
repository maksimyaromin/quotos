import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, test, vi } from 'vitest'
import { LimitWindow } from './limit-window'

afterEach(cleanup)

describe('LimitWindow scope badge', () => {
  test("keeps the base Badge's inline-flex display so its text stays vertically centered", () => {
    const { getByText } = render(
      <LimitWindow name="Weekly" used={45} scope="Fable" resetLabel="Resets Tue at 9:05 PM" />,
    )
    const badge = getByText('Fable').parentElement!
    expect(getComputedStyle(badge).display).toBe('inline-flex')
  })

  test('truncates a long scope tag via an inner block span, where text-overflow actually applies', () => {
    const longName = 'Claude Opus 4.5 (extended thinking, research preview)'
    const { getByText } = render(
      <LimitWindow name="Extended thinking" used={61} scope={longName} />,
    )
    const inner = getByText(longName)
    const style = getComputedStyle(inner)
    expect(style.display).toBe('block')
    expect(style.overflow).toBe('hidden')
    expect(style.textOverflow).toBe('ellipsis')
    expect(style.whiteSpace).toBe('nowrap')
    const badgeStyle = getComputedStyle(inner.parentElement!)
    expect(badgeStyle.display).toBe('inline-flex')
    expect(badgeStyle.textOverflow).not.toBe('ellipsis')
  })

  test('renders with no scope tag at all, the common case, without a badge element', () => {
    const { queryByText } = render(<LimitWindow name="Session" used={78} />)
    expect(queryByText('Fable')).toBeNull()
  })
})

describe('pinning a window that can be filed into a group', () => {
  const groups = [{ id: 'g1', name: 'Money' }]

  test("opens the destination menu instead of pinning outright, once there's somewhere to file it", () => {
    const onTogglePinMenu = vi.fn()
    const onTogglePin = vi.fn()
    render(
      <LimitWindow
        id="w1"
        name="Session"
        used={40}
        pinGroups={groups}
        onTogglePinMenu={onTogglePinMenu}
        onTogglePin={onTogglePin}
      />,
    )
    fireEvent.click(screen.getByLabelText('Show in menu bar'))
    expect(onTogglePinMenu).toHaveBeenCalledWith('w1')
    expect(onTogglePin).not.toHaveBeenCalled()
  })

  test('unpinning stays one click, with no menu in the way', () => {
    const onTogglePinMenu = vi.fn()
    const onTogglePin = vi.fn()
    render(
      <LimitWindow
        id="w1"
        name="Session"
        used={40}
        pinned
        pinGroups={groups}
        onTogglePinMenu={onTogglePinMenu}
        onTogglePin={onTogglePin}
      />,
    )
    fireEvent.click(screen.getByLabelText('Remove from menu bar'))
    expect(onTogglePin).toHaveBeenCalledWith('w1')
    expect(onTogglePinMenu).not.toHaveBeenCalled()
  })

  test('pins standalone in one click where no destination menu is wired up at all', () => {
    const onTogglePin = vi.fn()
    render(<LimitWindow id="w1" name="Session" used={40} onTogglePin={onTogglePin} />)
    fireEvent.click(screen.getByLabelText('Show in menu bar'))
    expect(onTogglePin).toHaveBeenCalledWith('w1')
  })

  test('the open menu names the groups and the window it is about to pin into one', () => {
    render(
      <LimitWindow
        id="w1"
        name="Session"
        used={40}
        pinGroups={groups}
        pinMenuOpen
        onTogglePinMenu={() => {}}
      />,
    )
    const menu = screen.getByRole('menu')
    expect(menu.getAttribute('aria-label')).toBe('Where to pin this limit')
    expect(menu.getAttribute('data-quotos-menu-scope')).toBe('true')
    expect(screen.getByText('Add to Money')).toBeTruthy()
  })

  test('choosing a group reports the window and the group, then closes the menu', () => {
    const onPin = vi.fn()
    const onTogglePinMenu = vi.fn()
    render(
      <LimitWindow
        id="w1"
        name="Session"
        used={40}
        pinGroups={groups}
        pinMenuOpen
        onTogglePinMenu={onTogglePinMenu}
        onPin={onPin}
      />,
    )
    fireEvent.click(screen.getByText('Add to Money'))
    expect(onPin).toHaveBeenCalledWith('w1', 'g1')
    expect(onTogglePinMenu).toHaveBeenCalledWith('w1')
  })

  test('naming a new group reports the window and the name', () => {
    const onCreateGroupWithWindow = vi.fn()
    render(
      <LimitWindow
        id="w1"
        name="Session"
        used={40}
        pinMenuOpen
        onTogglePinMenu={() => {}}
        onCreateGroupWithWindow={onCreateGroupWithWindow}
      />,
    )
    fireEvent.click(screen.getByText('New group…'))
    const field = screen.getByPlaceholderText('Group name')
    fireEvent.change(field, { target: { value: 'Current limit' } })
    fireEvent.keyDown(field, { key: 'Enter' })
    expect(onCreateGroupWithWindow).toHaveBeenCalledWith('w1', 'Current limit')
  })
})
