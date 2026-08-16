import { describe, expect, it } from "vitest";
import type { NormalizedRead } from "../../types/entities";
import { mapOutcome } from "./index";

const OK_READ: NormalizedRead = {
  label: "Personal",
  account: "Max",
  windows: [
    { id: "session", name: "Session", used: 2, resetsAt: null, scope: null, isActive: true },
  ],
  used: 2,
  resetsAt: null,
  severity: "healthy",
  headlineWindowId: "session",
};

// R2-3: the outcome->state+reason table, now provider-owned. The state
// vocabulary itself (working/reading/behind/broken/connecting/idle) is
// fixed — see types/entities.ts — only which state a given outcome maps to
// is provider-specific.
describe("claude mapOutcome", () => {
  it("a successful read is always 'working'", () => {
    const expected = { state: "working", reason: null, needsSignIn: false };
    expect(mapOutcome({ kind: "ok", normalized: OK_READ }, false)).toEqual(expected);
    expect(mapOutcome({ kind: "ok", normalized: OK_READ }, true)).toEqual(expected);
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
    const err = {
      kind: "unauthorized" as const,
      message: "still unauthorized after refreshing the credential",
    };
    const first = mapOutcome({ kind: "error", error: err }, false);
    expect(first.state).toBe("broken");
    // F17, handoff's exact wording — character for character, not just "mentions sign-in".
    expect(first.reason).toBe(
      "The sign-in expired. Log in again in Claude Code and Quotos will pick it up.",
    );
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
      {
        kind: "unauthorized" as const,
        message: "still unauthorized after refreshing the credential",
      },
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
    expect(result.needsSignIn).toBe(false);
  });
});

// R3-4: which outcomes are *actually* about signing in. The captain's bug
// was Quotos claiming a signed-in account needed signing in; the mirror-image
// failure would be losing the warning where it is deserved, so both
// directions are pinned here.
describe("claude mapOutcome — what really needs a sign-in", () => {
  it("credential_stale is never a sign-in warning: the account is signed in, its token just aged out", () => {
    const err = {
      kind: "credential_stale" as const,
      message:
        "This account's access token has expired and Quotos couldn't renew it here. Use Claude Code for this account once and Quotos will pick it up.",
    };
    const result = mapOutcome({ kind: "error", error: err }, false);
    expect(result.needsSignIn).toBe(false);
    expect(result.reason).toBe(err.message);
    // The sentence that made the false claim must not appear for this kind.
    expect(result.reason).not.toMatch(/sign-in expired/i);
  });

  it("unauthorized still asks for a sign-in — the genuinely signed-out account keeps its warning", () => {
    const err = {
      kind: "unauthorized" as const,
      message: "the stored sign-in is no longer accepted",
    };
    expect(mapOutcome({ kind: "error", error: err }, false).needsSignIn).toBe(true);
  });

  it("not_connected — no credential at all — asks for a sign-in", () => {
    const err = {
      kind: "not_connected" as const,
      message: "Claude Code isn't signed in for this account.",
    };
    const result = mapOutcome({ kind: "error", error: err }, false);
    expect(result.state).toBe("idle");
    expect(result.needsSignIn).toBe(true);
  });

  it("a transport failure or a 403 never asks for a sign-in", () => {
    const network = { kind: "network" as const, message: "The provider answered with HTTP 503." };
    const refused = {
      kind: "other" as const,
      message: "The provider refused this request (HTTP 403).",
    };
    expect(mapOutcome({ kind: "error", error: network }, false).needsSignIn).toBe(false);
    expect(mapOutcome({ kind: "error", error: refused }, false).needsSignIn).toBe(false);
  });

  it("once real data exists, no error kind asks for a sign-in — the row is merely behind", () => {
    const cases = [
      { kind: "unauthorized" as const, message: "x" },
      { kind: "not_connected" as const, message: "x" },
      { kind: "credential_stale" as const, message: "x" },
    ];
    for (const err of cases) {
      const result = mapOutcome({ kind: "error", error: err }, true);
      expect(result.state).toBe("behind");
      expect(result.needsSignIn).toBe(false);
    }
  });
});
