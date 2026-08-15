import { describe, expect, it } from "vitest";
import { buildTraySegments, buildTrayTooltip, worstActiveLimitPercent } from "./traySegments";
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

describe("buildTrayTooltip", () => {
  it("is just the product name when nothing contributes a figure", () => {
    expect(buildTrayTooltip([])).toBe("Quotos");
    // A pinned window with no number contributes no segment (never "!") and
    // therefore no tooltip line either — the two surfaces must agree.
    const subs = [
      subscription({
        id: "a",
        pinnedWindowIds: ["session"],
        windows: [window({ id: "session", used: null })],
      }),
    ];
    expect(buildTrayTooltip(subs)).toBe("Quotos");
  });

  it("names each contributing figure, one line per subscription, in bar order", () => {
    const subs = [
      subscription({
        id: "a",
        label: "Claude Max",
        pinnedWindowIds: ["weekly_all", "session"],
        windows: [
          window({ id: "session", name: "Session", used: 70 }),
          window({ id: "weekly_all", name: "Weekly", used: 40 }),
          window({ id: "weekly_scoped:Fable", name: "Weekly", scope: "Fable", used: 12 }),
        ],
      }),
      subscription({
        id: "b",
        label: "Claude Pro",
        pinnedWindowIds: ["weekly_scoped:Opus"],
        windows: [window({ id: "weekly_scoped:Opus", name: "Weekly", scope: "Opus", used: 55 })],
      }),
    ];
    expect(buildTrayTooltip(subs)).toBe(
      "Quotos\nClaude Max: Session 70% · Weekly 40%\nClaude Pro: Weekly (Opus) 55%",
    );
  });

  it("prefers the user's rename over the provider label", () => {
    const subs = [
      subscription({
        id: "a",
        label: "Claude Max",
        labelOverride: "Work",
        pinnedWindowIds: ["w"],
        windows: [window({ id: "w", name: "Weekly", used: 40 })],
      }),
    ];
    expect(buildTrayTooltip(subs)).toBe("Quotos\nWork: Weekly 40%");
  });

  it("marks only the stale subscription's own line, in the row badge's words", () => {
    const subs = [
      subscription({
        id: "a",
        label: "Claude Max",
        state: "behind",
        pinnedWindowIds: ["w"],
        windows: [window({ id: "w", name: "Weekly", used: 40 })],
      }),
      subscription({
        id: "b",
        label: "Claude Pro",
        pinnedWindowIds: ["w"],
        windows: [window({ id: "w", name: "Weekly", used: 55 })],
      }),
    ];
    expect(buildTrayTooltip(subs)).toBe(
      "Quotos\nClaude Max: Weekly 40% — not current\nClaude Pro: Weekly 55%",
    );
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
