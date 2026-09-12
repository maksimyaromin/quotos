import { cleanup, render } from '@testing-library/react'
import { afterEach, describe, expect, test } from 'vitest'
import { Wordmark } from './wordmark'

afterEach(cleanup)

describe('Wordmark', () => {
  test('sets the whole lockup as one run of text, with no mark beside it', () => {
    const { container } = render(<Wordmark />)
    expect(container.textContent).toBe('supolka(quotos)│')
    expect(container.querySelector('svg')).toBeNull()
    expect(container.querySelector('img')).toBeNull()
  })

  test('colours the three parts apart: the name in peach, the bar in rosewater', () => {
    const { container, getByText } = render(<Wordmark />)
    const root = container.firstElementChild as HTMLElement
    expect(getComputedStyle(getByText('quotos')).color).toBe('var(--peach)')
    expect(getComputedStyle(getByText('│')).color).toBe('var(--rosewater)')
    expect(getComputedStyle(root).color).toBe('var(--text-primary)')
  })
})
