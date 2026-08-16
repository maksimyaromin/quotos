import { describe, expect, test } from "vitest";
import type { Subscription } from "../types/entities";
import { presentRow } from "./rowPresentation";

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
    pinnedWindowIds: [],
    headlineWindowId: null,
    configDir: "/Users/someone/.claude",
    rateLimitedUntil: null,
    signInInProgress: false,
    pendingRemoval: false,
    ...overrides,
  };
}

describe("presentRow, failure states are mutually exclusive", () => {
  test("a row that needs signing in never also shows a rate-budget wait", () => {
    const result = presentRow(
      sub({
        state: "broken",
        needsSignIn: true,
        used: null,
        lastReadAt: null,
        rateLimitedUntil: IN_AN_HOUR,
      }),
      NOW,
    );
    expect(result.badge).toBe("Needs sign-in");
    expect(result.actionLabel).toBe("Open Claude Code");
    expect(result.footerNote).toBeNull();
  });

  test("a rate-budget wait states the time once, and offers nothing to press", () => {
    const result = presentRow(sub({ state: "behind", rateLimitedUntil: IN_AN_HOUR }), NOW);
    expect(result.actionLabel).toBeNull();
    expect(result.footerNote).toMatch(/^Waiting for the rate budget — retry at /);
    const times = result.footerNote!.match(/\d{1,2}:\d{2}/g) ?? [];
    expect(times).toHaveLength(1);
  });

  test("an elapsed wait is not a wait at all", () => {
    const result = presentRow(sub({ state: "behind", rateLimitedUntil: A_MINUTE_AGO }), NOW);
    expect(result.footerNote).toBeNull();
    expect(result.actionLabel).toBe("Try again");
  });
});

describe("presentRow, the badge tells the truth", () => {
  test("only a row that really needs signing in gets the sign-in badge", () => {
    expect(presentRow(sub({ state: "broken", needsSignIn: true }), NOW).badge).toBe(
      "Needs sign-in",
    );
  });

  test("a broken row that does not need signing in gets no sign-in badge", () => {
    // Offline at launch, an HTTP 403, and a credential Quotos could not
    // renew are all `broken`, but none of them is a sign-in problem.
    const offline = presentRow(
      sub({ state: "broken", needsSignIn: false, used: null, lastReadAt: null }),
      NOW,
    );
    expect(offline.badge).toBeNull();
    expect(offline.actionLabel).toBe("Try again");
  });

  test("held-over numbers still read 'Not current'", () => {
    expect(presentRow(sub({ state: "behind" }), NOW).badge).toBe("Not current");
  });

  test("a healthy row has no badge and no action", () => {
    const result = presentRow(sub(), NOW);
    expect(result.badge).toBeNull();
    expect(result.actionLabel).toBeNull();
    expect(result.footerNote).toBeNull();
  });

  test("a sign-in in progress owns the row body, with no competing action", () => {
    const result = presentRow(
      sub({ state: "broken", needsSignIn: true, signInInProgress: true }),
      NOW,
    );
    expect(result.actionLabel).toBeNull();
    expect(result.footerNote).toBeNull();
  });
});
