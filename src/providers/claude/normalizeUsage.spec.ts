import { describe, expect, test } from "vitest";
import { normalizeUsage } from "./normalizeUsage";

describe("normalizeUsage", () => {
  test("prefers limits[] over the fixed top-level windows", () => {
    const result = normalizeUsage({
      five_hour: { utilization: 99, resets_at: "2026-08-11T23:20:00Z" },
      limits: [
        {
          kind: "session",
          group: "session",
          percent: 2,
          resets_at: "2026-08-11T23:20:00Z",
          scope: null,
          is_active: false,
        },
      ],
    });
    expect(result.windows).toHaveLength(1);
    expect(result.windows[0].name).toBe("Session");
    expect(result.windows[0].used).toBe(2);
  });

  test("handles an arbitrary-length window list of 1, 3 or 8 windows", () => {
    for (const n of [1, 3, 8]) {
      const limits = Array.from({ length: n }, (_, i) => ({
        kind: `window_${i}`,
        percent: i * 5,
        resets_at: null,
        scope: null,
        is_active: i === 0,
      }));
      const result = normalizeUsage({ limits });
      expect(result.windows).toHaveLength(n);
    }
  });

  test("renders a window kind it has never seen before, using humanized provider wording", () => {
    const result = normalizeUsage({
      limits: [
        {
          kind: "nimbus_quill_experimental",
          percent: 12,
          resets_at: null,
          scope: null,
          is_active: true,
        },
      ],
    });
    expect(result.windows[0].name).toBe("Nimbus Quill Experimental");
    expect(result.windows[0].used).toBe(12);
  });

  test("keeps a window with a name but no percentage, leaving used null rather than zero", () => {
    const result = normalizeUsage({
      limits: [{ kind: "session", percent: null, resets_at: null, scope: null, is_active: true }],
    });
    expect(result.windows).toHaveLength(1);
    expect(result.windows[0].used).toBeNull();
    expect(result.used).toBeNull();
  });

  test("carries a model scope separately from the window name", () => {
    const result = normalizeUsage({
      limits: [
        {
          kind: "weekly_scoped",
          percent: 9,
          resets_at: "2026-08-17T09:59:59Z",
          scope: { model: { id: null, display_name: "Fable" }, surface: null },
          is_active: false,
        },
      ],
    });
    expect(result.windows[0].scope).toBe("Fable");
    expect(result.windows[0].name).toBe("Weekly");
  });

  test("falls back to fixed top-level windows when limits[] is absent", () => {
    const result = normalizeUsage({
      five_hour: { utilization: 2, resets_at: "2026-08-11T23:20:00Z" },
      seven_day: { utilization: 3, resets_at: "2026-08-17T10:00:00Z" },
      seven_day_opus: null,
    });
    expect(result.windows.map((w) => w.name)).toEqual(["Session", "Weekly"]);
  });

  test("falls back to fixed top-level windows when limits[] is an empty array", () => {
    const result = normalizeUsage({
      five_hour: { utilization: 10, resets_at: null },
      limits: [],
    });
    expect(result.windows).toHaveLength(1);
  });

  test("treats a null fixed window as absent, never as zero", () => {
    const result = normalizeUsage({
      five_hour: null,
      seven_day: { utilization: 0, resets_at: null },
    });
    expect(result.windows).toHaveLength(1);
    expect(result.windows[0].name).toBe("Weekly");
  });

  test("includes extra_usage only when enabled and numeric", () => {
    const enabled = normalizeUsage({
      extra_usage: { is_enabled: true, utilization: 40, currency: "USD" },
    });
    expect(enabled.windows.map((w) => w.name)).toContain("Extra usage");

    const disabled = normalizeUsage({
      extra_usage: { is_enabled: false, utilization: 40 },
    });
    expect(disabled.windows).toHaveLength(0);
  });

  test("returns an empty, non-throwing result for a completely empty response", () => {
    expect(normalizeUsage({})).toEqual({
      windows: [],
      used: null,
      resetsAt: null,
      severity: "healthy",
      headlineWindowId: null,
    });
  });

  test("returns an empty, non-throwing result for null or undefined", () => {
    expect(normalizeUsage(null)).toEqual({
      windows: [],
      used: null,
      resetsAt: null,
      severity: "healthy",
      headlineWindowId: null,
    });
    expect(normalizeUsage(undefined)).toEqual({
      windows: [],
      used: null,
      resetsAt: null,
      severity: "healthy",
      headlineWindowId: null,
    });
  });

  test("handles a subscription with no windows at all, every fixed field null and no limits", () => {
    const result = normalizeUsage({
      five_hour: null,
      seven_day: null,
      seven_day_opus: null,
      seven_day_sonnet: null,
      seven_day_cowork: null,
      extra_usage: { is_enabled: false },
    });
    expect(result.windows).toHaveLength(0);
    expect(result.used).toBeNull();
  });

  describe("headline is the account-wide weekly", () => {
    test("picks weekly_all even when a per-model weekly is more consumed and 'active'", () => {
      const result = normalizeUsage({
        limits: [
          { kind: "session", percent: 2, resets_at: null, scope: null, is_active: true },
          {
            kind: "weekly_all",
            percent: 15,
            resets_at: "2026-08-17T10:00:00Z",
            scope: null,
            is_active: true,
          },
          {
            kind: "weekly_scoped",
            percent: 17,
            resets_at: "2026-08-17T09:59:59Z",
            scope: { model: { id: null, display_name: "Fable" }, surface: null },
            is_active: false,
          },
        ],
      });
      expect(result.used).toBe(15);
      expect(result.resetsAt).toBe("2026-08-17T10:00:00Z");
    });

    test("picks weekly_all's own resets_at even when it's not marked active", () => {
      const result = normalizeUsage({
        limits: [
          { kind: "session", percent: 70, resets_at: null, scope: null, is_active: false },
          {
            kind: "weekly_all",
            percent: 20,
            resets_at: "2026-08-17T10:00:00Z",
            scope: null,
            is_active: false,
          },
        ],
      });
      expect(result.used).toBe(20);
    });

    test("falls back to the fixed-shape seven_day when limits[] has no weekly_all", () => {
      const result = normalizeUsage({
        limits: [],
        five_hour: { utilization: 40, resets_at: null },
        seven_day: { utilization: 8, resets_at: "2026-08-17T10:00:00Z" },
      });
      // An empty limits[] falls through to the fixed shape the same way an
      // absent one does, and seven_day is that shape's account-wide weekly.
      expect(result.used).toBe(8);
    });

    test("falls back to most-consumed-active when no account-wide weekly window exists at all", () => {
      const result = normalizeUsage({
        limits: [
          { kind: "session", percent: 70, resets_at: null, scope: null, is_active: true },
          { kind: "weekly_scoped", percent: 95, resets_at: null, scope: null, is_active: false },
        ],
      });
      expect(result.used).toBe(70);
    });
  });

  describe("severity from every window, not just the headline", () => {
    test("is healthy when every window is below 75%", () => {
      const result = normalizeUsage({
        limits: [
          { kind: "session", percent: 40, is_active: true },
          { kind: "weekly_all", percent: 20, is_active: true },
        ],
      });
      expect(result.severity).toBe("healthy");
    });

    test("is warn when any window is at least 75%, even if it's not the headline", () => {
      // A weekly headline at 20% with a session nearly out at 85% must
      // still read amber for the tray and the headline alike.
      const result = normalizeUsage({
        limits: [
          { kind: "session", percent: 85, is_active: true },
          { kind: "weekly_all", percent: 20, is_active: true },
        ],
      });
      expect(result.used).toBe(20);
      expect(result.severity).toBe("warn");
    });

    test("is critical when any window is at least 90%, outranking warn", () => {
      const result = normalizeUsage({
        limits: [
          { kind: "session", percent: 92, is_active: true },
          { kind: "weekly_all", percent: 80, is_active: true },
        ],
      });
      expect(result.severity).toBe("critical");
    });

    test("counts an inactive window toward severity too", () => {
      const result = normalizeUsage({
        limits: [
          { kind: "session", percent: 10, is_active: true },
          { kind: "weekly_scoped", percent: 91, is_active: false },
        ],
      });
      expect(result.severity).toBe("critical");
    });
  });

  test("tolerates malformed entries inside limits[] without throwing", () => {
    const result = normalizeUsage({
      limits: [null, "not an object", 42, { kind: "session", percent: 5, is_active: true }],
    });
    expect(result.windows).toHaveLength(1);
    expect(result.windows[0].used).toBe(5);
  });

  test("clamps out-of-range percentages defensively", () => {
    const result = normalizeUsage({
      limits: [{ kind: "session", percent: 150, is_active: true }],
    });
    expect(result.windows[0].used).toBe(100);
  });

  describe("window ids and headlineWindowId", () => {
    test("gives two same-kind windows distinct ids via their scope", () => {
      const result = normalizeUsage({
        limits: [
          {
            kind: "weekly_scoped",
            percent: 38,
            scope: { model: { display_name: "Opus" }, surface: null },
            is_active: true,
          },
          {
            kind: "weekly_scoped",
            percent: 17,
            scope: { model: { display_name: "Fable" }, surface: null },
            is_active: true,
          },
        ],
      });
      const ids = result.windows.map((w) => w.id);
      expect(new Set(ids).size).toBe(2);
    });

    test("headlineWindowId points at the weekly_all window's own id", () => {
      const result = normalizeUsage({
        limits: [
          { kind: "session", percent: 2, is_active: true },
          { kind: "weekly_all", percent: 15, is_active: true },
        ],
      });
      const weeklyAll = result.windows.find((w) => w.name === "Weekly" && w.scope === null);
      expect(result.headlineWindowId).toBe(weeklyAll?.id);
    });

    test("headlineWindowId points at the fixed-shape seven_day window's own id", () => {
      const result = normalizeUsage({
        five_hour: { utilization: 2, resets_at: null },
        seven_day: { utilization: 8, resets_at: null },
      });
      const weekly = result.windows.find((w) => w.name === "Weekly");
      expect(result.headlineWindowId).toBe(weekly?.id);
    });

    test("headlineWindowId falls back to the most-consumed window's own id", () => {
      const result = normalizeUsage({
        limits: [
          { kind: "session", percent: 70, is_active: true },
          { kind: "weekly_scoped", percent: 95, is_active: false },
        ],
      });
      const session = result.windows.find((w) => w.name === "Session");
      expect(result.headlineWindowId).toBe(session?.id);
    });

    test("headlineWindowId is null when there are no windows at all", () => {
      const result = normalizeUsage({});
      expect(result.headlineWindowId).toBeNull();
    });
  });
});
