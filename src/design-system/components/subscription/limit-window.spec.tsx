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
