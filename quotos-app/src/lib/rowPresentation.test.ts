import { describe, expect, it } from "vitest";
import { rowPresentation } from "./rowPresentation";
import type { Subscription } from "../types/entities";

const NOW = new Date("2026-08-15T14:00:00Z").getTime();
const IN_AN_HOUR = new Date(NOW + 59 * 60 * 1000).toISOString();
const A_MINUTE_AGO = new Date(NOW - 60 * 1000).toISOString();

function sub(overrides: Partial<Subscription> = {}): Subscription {
  return {
    id: "claude:claude",
    provider: "claude",
    providerName: "Anthropic",
    label: "Claude",
    labelOverride: null,
    account: null,
    state: "working",
    severity: "healthy",
    used: 20,
    resetsAt: null,
    lastReadAt: A_MINUTE_AGO,
    windows: [],
    reason: null,
    needsSignIn: false,
    pinned: false,
    configDir: "/Users/someone/.claude",
    rateLimitedUntil: null,
    signInInProgress: false,
    pendingRemoval: false,
    ...overrides,
  };
}

// R3-4: the exact row the captain photographed — badge "Needs sign-in", body
// "The sign-in expired.", footer "Waiting for the rate budget — available
// 4:59 PM" and an action reading "Retry at 4:59 PM", all in one row. Two
// separate defects: contradictory states shown together, and the same time
// stated twice in two different sentences.
describe("rowPresentation — failure states are mutually exclusive", () => {
  it("a row that needs signing in never also shows a rate-budget wait", () => {
    const result = rowPresentation(
      sub({ state: "broken", needsSignIn: true, used: null, lastReadAt: null, rateLimitedUntil: IN_AN_HOUR }),
      NOW,
    );
    expect(result.badge).toBe("Needs sign-in");
    expect(result.actionLabel).toBe("Open Claude Code");
    expect(result.footerNote).toBeNull();
  });

  it("a rate-budget wait states the time once, and offers nothing to press", () => {
    const result = rowPresentation(sub({ state: "behind", rateLimitedUntil: IN_AN_HOUR }), NOW);
    expect(result.actionLabel).toBeNull();
    expect(result.footerNote).toMatch(/^Waiting for the rate budget — retry at /);
    // The old row said the time in the footer note *and* again in the
    // action label ("Retry at 4:59 PM").
    const times = result.footerNote!.match(/\d{1,2}:\d{2}/g) ?? [];
    expect(times).toHaveLength(1);
  });

  it("an elapsed wait is not a wait at all", () => {
    const result = rowPresentation(sub({ state: "behind", rateLimitedUntil: A_MINUTE_AGO }), NOW);
    expect(result.footerNote).toBeNull();
    expect(result.actionLabel).toBe("Try again");
  });
});

describe("rowPresentation — the badge tells the truth", () => {
  it("only a row that really needs signing in gets the sign-in badge", () => {
    expect(rowPresentation(sub({ state: "broken", needsSignIn: true }), NOW).badge).toBe("Needs sign-in");
  });

  it("a broken row that does not need signing in gets no sign-in badge", () => {
    // Offline at launch, an HTTP 403, or a credential Quotos couldn't renew
    // — all `broken`, none of them a sign-in problem. The old row inferred
    // the badge from `state === "broken"` and accused all three.
    const offline = rowPresentation(sub({ state: "broken", needsSignIn: false, used: null, lastReadAt: null }), NOW);
    expect(offline.badge).toBeNull();
    expect(offline.actionLabel).toBe("Try again");
  });

  it("held-over numbers still read 'Not current'", () => {
    expect(rowPresentation(sub({ state: "behind" }), NOW).badge).toBe("Not current");
  });

  it("a healthy row has no badge and no action", () => {
    const result = rowPresentation(sub(), NOW);
    expect(result.badge).toBeNull();
    expect(result.actionLabel).toBeNull();
    expect(result.footerNote).toBeNull();
  });

  it("a sign-in in progress owns the row body — no competing action", () => {
    const result = rowPresentation(sub({ state: "broken", needsSignIn: true, signInInProgress: true }), NOW);
    expect(result.actionLabel).toBeNull();
    expect(result.footerNote).toBeNull();
  });
});
