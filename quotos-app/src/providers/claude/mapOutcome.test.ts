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

  it("unauthorized is 'broken' on a first read, 'behind' once real data existed, with F17's exact reason", () => {
    const err = { kind: "unauthorized" as const, message: "still unauthorized after refreshing the credential" };
    const first = mapOutcome({ kind: "error", error: err }, false);
    expect(first.state).toBe("broken");
    // F17, handoff's exact wording — character for character, not just "mentions sign-in".
    expect(first.reason).toBe("The sign-in expired. Log in again in Claude Code and Quotos will pick it up.");
    expect(mapOutcome({ kind: "error", error: err }, true).state).toBe("behind");
  });

  it("network failures are 'broken' first, carrying the raw message", () => {
    const err = { kind: "network" as const, message: "the connection timed out" };
    const first = mapOutcome({ kind: "error", error: err }, false);
    expect(first.state).toBe("broken");
    expect(first.reason).toBe("the connection timed out");
  });

  it("F16: any error once real data existed reads 'behind' with the fixed stale reason, not the raw error text", () => {
    // Handoff's exact wording — this must win over the underlying error's
    // own (often technical) message, e.g. "the connection timed out" below
    // must not leak into the UI once there's a prior good read to fall
    // back on.
    const fixed = "The provider didn't answer. These numbers are from the last successful read.";
    const cases = [
      { kind: "not_connected" as const, message: "no credentials" },
      { kind: "unauthorized" as const, message: "still unauthorized after refreshing the credential" },
      { kind: "network" as const, message: "the connection timed out" },
    ];
    for (const err of cases) {
      const result = mapOutcome({ kind: "error", error: err }, true);
      expect(result.state).toBe("behind");
      expect(result.reason).toBe(fixed);
    }
  });

  it("a non-FetchError (unexpected throw) still degrades gracefully", () => {
    const result = mapOutcome({ kind: "error", error: null }, false);
    expect(result.state).toBe("broken");
    expect(result.reason).toBeTruthy();
  });
});
