import { cleanup, render } from '@testing-library/react'
import { afterEach, describe, expect, test } from 'vitest'
import { CapacityBar } from './components/indicators/capacity-bar'
import elevation from './tokens/elevation.css?raw'

afterEach(cleanup)

const reducedBlock = elevation.split('@media (prefers-reduced-motion: reduce)')[1]

describe('prefers-reduced-motion contract', () => {
  test('elevation.css has the reduced-motion block', () => {
    expect(reducedBlock).toBeTruthy()
  })

  test('zeroes every --dur-* token the base block defines', () => {
    const tokens = new Set(elevation.match(/--dur-[a-z]+(?=\s*:)/g) ?? [])
    expect(tokens.size).toBeGreaterThanOrEqual(4)
    for (const token of tokens) {
      expect(reducedBlock).toContain(`${token}: 0ms`)
    }
  })

  test('stops keyframe animations and hides the shimmer overlay', () => {
    expect(reducedBlock).toMatch(/animation:\s*none\s*!important/)
    expect(reducedBlock).toContain('[data-quotos-shimmer]')
  })

  test("CapacityBar's reading shimmer carries the data-quotos-shimmer hook", () => {
    const reading = render(<CapacityBar used={40} reading />)
    expect(reading.container.querySelector('[data-quotos-shimmer]')).not.toBeNull()
    cleanup()
    const settled = render(<CapacityBar used={40} />)
    expect(settled.container.querySelector('[data-quotos-shimmer]')).toBeNull()
  })
})
