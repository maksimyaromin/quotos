import { describe, expect, it } from "vitest";
import { buildTraySegments, worstActiveLimitPercent } from "./traySegments";
import type { LimitWindowEntity, Subscription } from "../types/entities";

function window(overrides: Partial<LimitWindowEntity> & { id: string }): LimitWindowEntity {
  return { name: "Window", used: null, resetsAt: null, scope: null, isActive: true, ...overrides };
}

function subscription(overrides: Partial<Subscription> & { id: string }): Subscription {
  return {
    provider: "claude",
    providerName: "Anthropic",
    label: overrides.id,
    labelOverride: null,
    account: null,
    state: "working",
    severity: "healthy",
    used: null,
    resetsAt: null,
    lastReadAt: null,
    windows: [],
    reason: null,
    needsSignIn: false,
    pinnedWindowIds: [],
    headlineWindowId: null,
    configDir: "~/.claude",
    rateLimitedUntil: null,
    signInInProgress: false,
    pendingRemoval: false,
    ...overrides,
  };
}

describe("buildTraySegments", () => {
  it("emits nothing when nothing is pinned", () => {
    const subs = [subscription({ id: "a", windows: [window({ id: "session", used: 10 })] })];
    expect(buildTraySegments(subs)).toEqual([]);
  });

  it("skips a pinned window with no numeric value yet — never '!' or '…'", () => {
    const subs = [
      subscription({
        id: "a",
        pinnedWindowIds: ["session"],
        windows: [window({ id: "session", used: null })],
      }),
    ];
    expect(buildTraySegments(subs)).toEqual([]);
  });

  it("skips a pinned id whose window no longer exists in the latest read", () => {
    const subs = [subscription({ id: "a", pinnedWindowIds: ["gone"], windows: [window({ id: "session", used: 10 })] })];
    expect(buildTraySegments(subs)).toEqual([]);
  });

  it("orders figures in panel order then provider (window list) order, marking group starts", () => {
    const subs = [
      subscription({
        id: "a",
        pinnedWindowIds: ["session", "weekly_all"],
        windows: [window({ id: "session", used: 61 }), window({ id: "weekly_all", used: 74 })],
      }),
      subscription({
        id: "b",
        pinnedWindowIds: ["weekly_all"],
        windows: [window({ id: "weekly_all", used: 52 })],
      }),
    ];
    expect(buildTraySegments(subs)).toEqual([
      { text: "61%", color: "neutral", groupStart: false },
      { text: "74%", color: "neutral", groupStart: false },
      { text: "52%", color: "neutral", groupStart: true },
    ]);
  });

  it("colors a figure from its own window's used%, not the subscription's aggregate severity", () => {
    // The subscription is "warn" overall (some other window at 85%), but the
    // pinned figure itself is a healthy 20% — it must read neutral, not amber.
    const subs = [
      subscription({
        id: "a",
        severity: "warn",
        pinnedWindowIds: ["weekly_all"],
        windows: [window({ id: "session", used: 85 }), window({ id: "weekly_all", used: 20 })],
      }),
    ];
    expect(buildTraySegments(subs)).toEqual([{ text: "20%", color: "neutral", groupStart: false }]);
  });

  it("colors past 75 amber and past 90 red", () => {
    const subs = [
      subscription({ id: "a", pinnedWindowIds: ["w"], windows: [window({ id: "w", used: 80 })] }),
      subscription({ id: "b", pinnedWindowIds: ["w"], windows: [window({ id: "w", used: 95 })] }),
    ];
    expect(buildTraySegments(subs).map((s) => s.color)).toEqual(["amber", "red"]);
  });

  it("turns every contributing figure amber when any contributing subscription is stale", () => {
    const subs = [
      subscription({ id: "a", pinnedWindowIds: ["w"], windows: [window({ id: "w", used: 10 })] }),
      subscription({ id: "b", state: "behind", pinnedWindowIds: ["w"], windows: [window({ id: "w", used: 20 })] }),
    ];
    expect(buildTraySegments(subs).map((s) => s.color)).toEqual(["amber", "amber"]);
  });

  it("a stale subscription with nothing pinned doesn't taint the bar", () => {
    const subs = [
      subscription({ id: "a", pinnedWindowIds: ["w"], windows: [window({ id: "w", used: 10 })] }),
      subscription({ id: "b", state: "behind", pinnedWindowIds: [], windows: [window({ id: "w", used: 99 })] }),
    ];
    expect(buildTraySegments(subs)).toEqual([{ text: "10%", color: "neutral", groupStart: false }]);
  });
});

describe("worstActiveLimitPercent", () => {
  it("is 0 when nothing tracked has any active numeric window", () => {
    expect(worstActiveLimitPercent([])).toBe(0);
    expect(worstActiveLimitPercent([subscription({ id: "a" })])).toBe(0);
  });

  it("is the max used% across every active window, pinned or not", () => {
    const subs = [
      subscription({ id: "a", windows: [window({ id: "w1", used: 61, isActive: true })] }),
      subscription({ id: "b", windows: [window({ id: "w2", used: 74, isActive: true }), window({ id: "w3", used: 99, isActive: false })] }),
    ];
    expect(worstActiveLimitPercent(subs)).toBe(74);
  });
});
