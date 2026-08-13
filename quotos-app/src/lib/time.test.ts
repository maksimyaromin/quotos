import { afterEach, describe, expect, it, vi } from "vitest";
import { formatExactReset, formatRelativePast } from "./time";

// R2-7: "мне не нравится что я вижу Пн по русски" — the root cause was
// every Intl.DateTimeFormat in this file being built with locale
// `undefined` (the system locale, Russian on the captain's Mac). A test
// that merely eyeballs formatted output can't catch a regression here on a
// machine whose own locale already happens to be English — it has to
// inspect what locale argument the formatters were actually constructed
// with, which is why this spies on the Intl.DateTimeFormat constructor
// itself rather than just asserting on strings.
describe("R2-7: locale is pinned to en-US, never the system locale", () => {
  afterEach(() => {
    vi.restoreAllMocks();
    vi.resetModules();
  });

  it("every Intl.DateTimeFormat this module builds is constructed with an explicit locale, never undefined", async () => {
    const seenLocales: unknown[] = [];
    const RealDateTimeFormat = Intl.DateTimeFormat;
    vi.spyOn(Intl, "DateTimeFormat").mockImplementation(function (
      this: unknown,
      locale?: unknown,
      options?: Intl.DateTimeFormatOptions,
    ) {
      seenLocales.push(locale);
      return new RealDateTimeFormat(locale as string | string[] | undefined, options);
    } as unknown as typeof Intl.DateTimeFormat);

    vi.resetModules();
    await import("./time"); // re-import so the module-level formatters are (re)constructed under the spy

    expect(seenLocales.length).toBeGreaterThan(0);
    for (const locale of seenLocales) {
      expect(locale).toBe("en-US");
    }
  });
});

describe("formatExactReset", () => {
  // Wednesday 2026-08-12, 12:00 local.
  const now = new Date(2026, 7, 12, 12, 0, 0);

  it("names today when the reset is later the same day", () => {
    const iso = new Date(2026, 7, 12, 16, 5, 0).toISOString();
    expect(formatExactReset(iso, now)).toMatch(/^Resets today at/);
  });

  it("names tomorrow when the reset is the next calendar day", () => {
    const iso = new Date(2026, 7, 13, 10, 0, 0).toISOString();
    expect(formatExactReset(iso, now)).toMatch(/^Resets tomorrow at/);
  });

  it("names the weekday for a reset within the week but not today/tomorrow", () => {
    const iso = new Date(2026, 7, 16, 9, 0, 0).toISOString(); // Sunday, 4 days out
    const label = formatExactReset(iso, now);
    // F10: the handoff's exact wording is "Resets Sun at 10:00 AM" — the
    // weekday and time must not run together without "at" between them.
    expect(label).toMatch(/^Resets (Sun|Sunday) at/);
    expect(label).not.toMatch(/today|tomorrow/);
  });

  it("names the calendar date for a reset more than a week out", () => {
    const iso = new Date(2026, 7, 24, 9, 0, 0).toISOString(); // 12 days out
    expect(formatExactReset(iso, now)).toMatch(/^Resets Aug 24 at/);
  });

  it("never emits a relative offset like 'in 5d'", () => {
    const iso = new Date(2026, 7, 17, 10, 0, 0).toISOString();
    expect(formatExactReset(iso, now)).not.toMatch(/in \d+[dhm]/);
  });

  it("returns null for a missing timestamp", () => {
    expect(formatExactReset(null, now)).toBeNull();
  });

  it("returns null for an unparseable timestamp", () => {
    expect(formatExactReset("not a date", now)).toBeNull();
  });
});

describe("formatRelativePast", () => {
  const now = new Date(2026, 7, 12, 12, 0, 0);

  it("says 'just now' for a read seconds ago", () => {
    const iso = new Date(now.getTime() - 5_000).toISOString();
    expect(formatRelativePast(iso, now)).toBe("just now");
  });

  it("returns null for a missing timestamp", () => {
    expect(formatRelativePast(null, now)).toBeNull();
  });
});
