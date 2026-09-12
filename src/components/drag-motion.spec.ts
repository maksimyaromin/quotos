import { describe, expect, test } from 'vitest'
import {
  AT_REST,
  dropSettle,
  GRAB_SPRING,
  grabTransition,
  PICKED_UP,
  REFLOW_SPRING,
  reflowTransition,
} from './drag-motion'

describe('the screen has one motion language', () => {
  test('nothing eases: every transition on the screen is a spring', () => {
    expect(grabTransition(false).type).toBe('spring')
    expect(reflowTransition(false).type).toBe('spring')
  })

  test('a row moving aside is heavier and slower than the one being carried', () => {
    expect(REFLOW_SPRING.stiffness).toBeLessThan(GRAB_SPRING.stiffness as number)
    expect(REFLOW_SPRING.mass).toBeGreaterThan(GRAB_SPRING.mass as number)
  })

  test('both springs overshoot, which is what reads as weight', () => {
    for (const spring of [GRAB_SPRING, REFLOW_SPRING]) {
      const critical = 2 * Math.sqrt((spring.stiffness as number) * (spring.mass as number))
      expect(spring.damping).toBeLessThan(critical)
    }
  })

  test('the drop settles on the same overshooting curve, not the library default', () => {
    expect(dropSettle(false).easing).toContain('cubic-bezier')
    expect(dropSettle(false).duration).toBeGreaterThan(0)
  })

  test('a picked-up row lifts, but stays the same row', () => {
    expect(PICKED_UP.scale).toBeGreaterThan(1)
    expect(PICKED_UP.scale).toBeLessThan(1.1)
    expect(PICKED_UP.rotate).not.toBe(0)
  })
})

describe('Reduce Motion', () => {
  test('arrives at the same place with no motion at all', () => {
    expect(grabTransition(true)).toEqual({ duration: 0 })
    expect(reflowTransition(true)).toEqual({ duration: 0 })
    expect(dropSettle(true).duration).toBe(0)
  })

  test('and lifts nothing, since a lift that cannot animate is only a jump', () => {
    expect(AT_REST).toEqual({ scale: 1, rotate: 0 })
  })
})
