import { describe, expect, it } from "vitest";
import { normalizeUsage } from "./normalizeUsage";

describe("normalizeUsage", () => {
  it("prefers limits[] over the fixed top-level windows", () => {
    const result = normalizeUsage({
      five_hour: { utilization: 99, resets_at: "2026-08-11T23:20:00Z" },
      limits: [
        { kind: "session", group: "session", percent: 2, resets_at: "2026-08-11T23:20:00Z", scope: null, is_active: false },
      ],
    });
    expect(result.windows).toHaveLength(1);
    expect(result.windows[0].name).toBe("Session");
    expect(result.windows[0].remaining).toBe(98);
  });

  it("handles an arbitrary-length window list (1, 3, 8)", () => {
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

  it("renders a window kind it has never seen before, using humanized provider wording", () => {
    const result = normalizeUsage({
      limits: [{ kind: "nimbus_quill_experimental", percent: 12, resets_at: null, scope: null, is_active: true }],
    });
    expect(result.windows[0].name).toBe("Nimbus Quill Experimental");
    expect(result.windows[0].remaining).toBe(88);
  });

  it("keeps a window with a name but no percentage — remaining is null, not zero", () => {
    const result = normalizeUsage({
      limits: [{ kind: "session", percent: null, resets_at: null, scope: null, is_active: true }],
    });
    expect(result.windows).toHaveLength(1);
    expect(result.windows[0].remaining).toBeNull();
    expect(result.remaining).toBeNull();
  });

  it("carries a model scope separately from the window name", () => {
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

  it("falls back to fixed top-level windows when limits[] is absent", () => {
    const result = normalizeUsage({
      five_hour: { utilization: 2, resets_at: "2026-08-11T23:20:00Z" },
      seven_day: { utilization: 3, resets_at: "2026-08-17T10:00:00Z" },
      seven_day_opus: null,
    });
    expect(result.windows.map((w) => w.name)).toEqual(["Session", "Weekly"]);
  });

  it("falls back to fixed top-level windows when limits[] is an empty array", () => {
    const result = normalizeUsage({
      five_hour: { utilization: 10, resets_at: null },
      limits: [],
    });
    expect(result.windows).toHaveLength(1);
  });

  it("treats a null fixed window as absent, never as zero", () => {
    const result = normalizeUsage({
      five_hour: null,
      seven_day: { utilization: 0, resets_at: null },
    });
    expect(result.windows).toHaveLength(1);
    expect(result.windows[0].name).toBe("Weekly");
  });

  it("includes extra_usage only when enabled and numeric", () => {
    const enabled = normalizeUsage({
      extra_usage: { is_enabled: true, utilization: 40, currency: "USD" },
    });
    expect(enabled.windows.map((w) => w.name)).toContain("Extra usage");

    const disabled = normalizeUsage({
      extra_usage: { is_enabled: false, utilization: 40 },
    });
    expect(disabled.windows).toHaveLength(0);
  });

  it("returns an empty, non-throwing result for a completely empty response", () => {
    expect(normalizeUsage({})).toEqual({ windows: [], remaining: null, resetsAt: null });
  });

  it("returns an empty, non-throwing result for null/undefined", () => {
    expect(normalizeUsage(null)).toEqual({ windows: [], remaining: null, resetsAt: null });
    expect(normalizeUsage(undefined)).toEqual({ windows: [], remaining: null, resetsAt: null });
  });

  it("handles a subscription with no windows at all (all fixed fields null, no limits)", () => {
    const result = normalizeUsage({
      five_hour: null,
      seven_day: null,
      seven_day_opus: null,
      seven_day_sonnet: null,
      seven_day_cowork: null,
      extra_usage: { is_enabled: false },
    });
    expect(result.windows).toHaveLength(0);
    expect(result.remaining).toBeNull();
  });

  it("computes the headline as the most-consumed currently-active window", () => {
    const result = normalizeUsage({
      limits: [
        { kind: "session", percent: 2, resets_at: "2026-08-11T23:20:00Z", scope: null, is_active: true },
        { kind: "weekly_all", percent: 60, resets_at: "2026-08-17T10:00:00Z", scope: null, is_active: true },
        { kind: "weekly_scoped", percent: 95, resets_at: null, scope: null, is_active: false },
      ],
    });
    // weekly_scoped is inactive, so it's excluded even though it's the most consumed.
    expect(result.remaining).toBe(40);
    expect(result.resetsAt).toBe("2026-08-17T10:00:00Z");
  });

  it("falls back to any window with a percentage when none are marked active", () => {
    const result = normalizeUsage({
      limits: [
        { kind: "session", percent: 70, resets_at: null, scope: null, is_active: false },
        { kind: "weekly_all", percent: 20, resets_at: null, scope: null, is_active: false },
      ],
    });
    expect(result.remaining).toBe(30);
  });

  it("tolerates malformed entries inside limits[] without throwing", () => {
    const result = normalizeUsage({
      limits: [null, "not an object", 42, { kind: "session", percent: 5, is_active: true }],
    });
    expect(result.windows).toHaveLength(1);
    expect(result.windows[0].remaining).toBe(95);
  });

  it("clamps out-of-range percentages defensively", () => {
    const result = normalizeUsage({
      limits: [{ kind: "session", percent: 150, is_active: true }],
    });
    expect(result.windows[0].remaining).toBe(0);
  });
});
