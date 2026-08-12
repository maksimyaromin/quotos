import { describe, expect, it } from "vitest";
import { formatExactReset, formatRelativePast } from "./time";

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
    expect(label).toMatch(/^Resets (Sun|Sunday)/);
    expect(label).not.toMatch(/today|tomorrow/);
  });

  it("names the calendar date for a reset more than a week out", () => {
    const iso = new Date(2026, 7, 24, 9, 0, 0).toISOString(); // 12 days out
    expect(formatExactReset(iso, now)).toMatch(/^Resets Aug 24/);
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
