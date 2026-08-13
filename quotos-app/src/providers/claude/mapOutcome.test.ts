import { describe, expect, it } from "vitest";
import { mapOutcome } from "./index";
import type { NormalizedRead } from "../../types/entities";

const OK_READ: NormalizedRead = {
  label: "Personal",
  account: "Max",
  windows: [{ name: "Session", used: 2, resetsAt: null, scope: null, isActive: true }],
  used: 2,
  resetsAt: null,
  severity: "healthy",
};

// R2-3: the outcome->state+reason table, now provider-owned. The state
// vocabulary itself (working/reading/behind/broken/connecting/idle) is
// fixed — see types/entities.ts — only which state a given outcome maps to
// is provider-specific.
describe("claude mapOutcome", () => {
  it("a successful read is always 'working'", () => {
    expect(mapOutcome({ kind: "ok", normalized: OK_READ }, false)).toEqual({ state: "working", reason: null });
    expect(mapOutcome({ kind: "ok", normalized: OK_READ }, true)).toEqual({ state: "working", reason: null });
  });

  it("a successful read with no windows at all reports the no-limits reason, still 'working'", () => {
    const empty: NormalizedRead = { ...OK_READ, windows: [], used: null };
    const result = mapOutcome({ kind: "ok", normalized: empty }, false);
    expect(result.state).toBe("working");
    expect(result.reason).toBe("No limits reported yet.");
  });

  it("not_connected is 'idle' on a first read, 'behind' once real data existed", () => {
    const err = { kind: "not_connected" as const, message: "no credentials" };
    expect(mapOutcome({ kind: "error", error: err }, false).state).toBe("idle");
    expect(mapOutcome({ kind: "error", error: err }, true).state).toBe("behind");
  });

  it("unauthorized is 'broken' on a first read, 'behind' once real data existed, with a fixed reason", () => {
    const err = { kind: "unauthorized" as const, message: "still unauthorized after refreshing the credential" };
    const first = mapOutcome({ kind: "error", error: err }, false);
    expect(first.state).toBe("broken");
    expect(first.reason).toMatch(/sign-in expired/i);
    expect(mapOutcome({ kind: "error", error: err }, true).state).toBe("behind");
  });

  it("network failures are 'broken' first, 'behind' thereafter, carrying the raw message", () => {
    const err = { kind: "network" as const, message: "the connection timed out" };
    const first = mapOutcome({ kind: "error", error: err }, false);
    expect(first.state).toBe("broken");
    expect(first.reason).toBe("the connection timed out");
    expect(mapOutcome({ kind: "error", error: err }, true).state).toBe("behind");
  });

  it("a non-FetchError (unexpected throw) still degrades gracefully", () => {
    const result = mapOutcome({ kind: "error", error: null }, false);
    expect(result.state).toBe("broken");
    expect(result.reason).toBeTruthy();
  });
});
