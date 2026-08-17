import { afterEach, describe, expect, test, vi } from 'vitest'
import { formatExactReset, formatRelativePast } from './time'

afterEach(() => {
  vi.restoreAllMocks()
  vi.resetModules()
})

test('time module constructs every Intl.DateTimeFormat with an explicit locale, never the system default', async () => {
  const seenLocales: unknown[] = []
  const RealDateTimeFormat = Intl.DateTimeFormat
  vi.spyOn(Intl, 'DateTimeFormat').mockImplementation(function (
    this: unknown,
    locale?: unknown,
    options?: Intl.DateTimeFormatOptions,
  ) {
    seenLocales.push(locale)
    return new RealDateTimeFormat(locale as string | string[] | undefined, options)
  } as unknown as typeof Intl.DateTimeFormat)

  vi.resetModules()
  await import('./time')

  expect(seenLocales.length).toBeGreaterThan(0)
  for (const locale of seenLocales) {
    expect(locale).toBe('en-US')
  }
})

describe('formatExactReset', () => {
  const now = new Date(2026, 7, 12, 12, 0, 0)

  test('names today when the reset is later the same day', () => {
    const iso = new Date(2026, 7, 12, 16, 5, 0).toISOString()
    expect(formatExactReset(iso, now)).toMatch(/^Resets today at/)
  })

  test('names tomorrow when the reset is the next calendar day', () => {
    const iso = new Date(2026, 7, 13, 10, 0, 0).toISOString()
    expect(formatExactReset(iso, now)).toMatch(/^Resets tomorrow at/)
  })

  test("names the weekday and joins it to the time with 'at' for a reset within the week", () => {
    const iso = new Date(2026, 7, 16, 9, 0, 0).toISOString()
    const label = formatExactReset(iso, now)
    expect(label).toMatch(/^Resets (Sun|Sunday) at/)
    expect(label).not.toMatch(/today|tomorrow/)
  })

  test('names the calendar date for a reset more than a week out', () => {
    const iso = new Date(2026, 7, 24, 9, 0, 0).toISOString()
    expect(formatExactReset(iso, now)).toMatch(/^Resets Aug 24 at/)
  })

  test("never emits a relative offset such as 'in 5d'", () => {
    const iso = new Date(2026, 7, 17, 10, 0, 0).toISOString()
    expect(formatExactReset(iso, now)).not.toMatch(/in \d+[dhm]/)
  })

  test('returns null for a missing or unparseable timestamp', () => {
    expect(formatExactReset(null, now)).toBeNull()
    expect(formatExactReset('not a date', now)).toBeNull()
  })
})

describe('formatRelativePast', () => {
  const now = new Date(2026, 7, 12, 12, 0, 0)

  test("says 'just now' for a read seconds ago", () => {
    const iso = new Date(now.getTime() - 5_000).toISOString()
    expect(formatRelativePast(iso, now)).toBe('just now')
  })

  test("switches from 'just now' to a second count at the ten-second boundary", () => {
    expect(formatRelativePast(new Date(now.getTime() - 9_000).toISOString(), now)).toBe('just now')
    expect(formatRelativePast(new Date(now.getTime() - 10_000).toISOString(), now)).toBe('10s ago')
  })

  test('returns null for a missing timestamp', () => {
    expect(formatRelativePast(null, now)).toBeNull()
  })
})
