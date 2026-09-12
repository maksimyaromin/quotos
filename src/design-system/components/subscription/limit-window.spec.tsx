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

describe("the pin button is a plain toggle: where a pin sits is the customize screen's business", () => {
  test('pinning reports the window id and nothing else', () => {
    const onTogglePin = vi.fn()
    render(<LimitWindow id="w1" name="Session" used={40} onTogglePin={onTogglePin} />)

    const button = screen.getByLabelText('Show in menu bar')
    expect(button.getAttribute('aria-haspopup')).toBeNull()
    fireEvent.click(button)
    expect(onTogglePin).toHaveBeenCalledWith('w1')
  })

  test('unpinning is the same one click, from the pressed state', () => {
    const onTogglePin = vi.fn()
    render(<LimitWindow id="w1" name="Session" used={40} pinned onTogglePin={onTogglePin} />)

    const button = screen.getByLabelText('Remove from menu bar')
    expect(button.getAttribute('aria-pressed')).toBe('true')
    fireEvent.click(button)
    expect(onTogglePin).toHaveBeenCalledWith('w1')
  })
})

describe("the menu bar mark's fill source is its own toggle beside the pin", () => {
  test('choosing this window reports its id and nothing else', () => {
    const onToggleIconSource = vi.fn()
    render(
      <LimitWindow id="weekly" name="Weekly" used={45} onToggleIconSource={onToggleIconSource} />,
    )
    fireEvent.click(screen.getByLabelText('Fill the menu bar icon from this'))
    expect(onToggleIconSource).toHaveBeenCalledWith('weekly')
  })

  test('the chosen window says so, and offers to stop rather than to start', () => {
    render(<LimitWindow id="weekly" name="Weekly" used={45} iconSource />)
    const button = screen.getByLabelText('Stop filling the menu bar icon from this')
    expect(button.getAttribute('aria-pressed')).toBe('true')
  })

  test('it is a separate affordance from the pin, not the same button twice', () => {
    const onTogglePin = vi.fn()
    const onToggleIconSource = vi.fn()
    render(
      <LimitWindow
        id="weekly"
        name="Weekly"
        used={45}
        onTogglePin={onTogglePin}
        onToggleIconSource={onToggleIconSource}
      />,
    )
    fireEvent.click(screen.getByLabelText('Show in menu bar'))
    expect(onTogglePin).toHaveBeenCalledWith('weekly')
    expect(onToggleIconSource).not.toHaveBeenCalled()
  })

  test('a window with no id cannot be chosen, the same as it cannot be pinned', () => {
    const onToggleIconSource = vi.fn()
    render(<LimitWindow name="Weekly" used={45} onToggleIconSource={onToggleIconSource} />)
    fireEvent.click(screen.getByLabelText('Fill the menu bar icon from this'))
    expect(onToggleIconSource).not.toHaveBeenCalled()
  })
})
