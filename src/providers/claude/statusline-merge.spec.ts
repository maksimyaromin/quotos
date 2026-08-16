import { describe, expect, test } from "vitest";
import type { StatuslineFeedWire } from "@/types/entities";
import { reconcileWithStatusline } from "./statusline-merge";

const API_FETCHED_AT = "2026-08-15T12:00:00.000Z";
const FRESHER = "2026-08-15T12:00:30.000Z"; // 30s after the API read
const OLDER = "2026-08-15T11:59:00.000Z"; // before the API read

function feedWith(
  rateLimits: StatuslineFeedWire["rate_limits"],
  writtenAt = FRESHER,
): StatuslineFeedWire {
  return { written_at: writtenAt, rate_limits: rateLimits };
}

describe("reconcileWithStatusline", () => {
  test("leaves usage untouched when there is no feed at all", () => {
    const usage = { five_hour: { utilization: 10 } };
    expect(reconcileWithStatusline(usage, API_FETCHED_AT, null)).toBe(usage);
    expect(reconcileWithStatusline(usage, API_FETCHED_AT, undefined)).toBe(usage);
  });

  test("leaves usage untouched when the feed has no rate_limits", () => {
    const usage = { five_hour: { utilization: 10 } };
    const feed = { written_at: FRESHER, rate_limits: undefined } as unknown as StatuslineFeedWire;
    expect(reconcileWithStatusline(usage, API_FETCHED_AT, feed)).toBe(usage);
  });

  test("leaves usage untouched when the feed is not newer than the API read", () => {
    const usage = { five_hour: { utilization: 10 } };
    const sameAge = feedWith(
      { five_hour: { used_percentage: 99, resets_at: null } },
      API_FETCHED_AT,
    );
    const older = feedWith({ five_hour: { used_percentage: 99, resets_at: null } }, OLDER);
    expect(reconcileWithStatusline(usage, API_FETCHED_AT, sameAge)).toEqual(usage);
    expect(reconcileWithStatusline(usage, API_FETCHED_AT, older)).toEqual(usage);
  });

  test("leaves a non-object usageRaw untouched", () => {
    const feed = feedWith({ five_hour: { used_percentage: 99, resets_at: null } });
    expect(reconcileWithStatusline(null, API_FETCHED_AT, feed)).toBeNull();
    expect(reconcileWithStatusline(undefined, API_FETCHED_AT, feed)).toBeUndefined();
    expect(reconcileWithStatusline("not an object", API_FETCHED_AT, feed)).toBe("not an object");
  });

  describe("limits[] shape", () => {
    const usage = {
      limits: [
        {
          kind: "session",
          percent: 12,
          resets_at: "2026-08-15T15:00:00Z",
          scope: null,
          is_active: true,
        },
        {
          kind: "weekly_all",
          percent: 24,
          resets_at: "2026-08-20T00:00:00Z",
          scope: null,
          is_active: false,
        },
        {
          kind: "weekly_scoped",
          percent: 8,
          resets_at: "2026-08-20T00:00:00Z",
          scope: { model: { display_name: "Fable" } },
          is_active: false,
        },
      ],
    };

    test("patches both session and weekly_all when both windows are fresher", () => {
      const feed = feedWith({
        five_hour: { used_percentage: 45, resets_at: 1786899600 },
        seven_day: { used_percentage: 60, resets_at: 1787270400 },
      });
      const result = reconcileWithStatusline(usage, API_FETCHED_AT, feed) as typeof usage;
      const session = result.limits.find((l) => l.kind === "session")!;
      const weekly = result.limits.find((l) => l.kind === "weekly_all")!;
      expect(session.percent).toBe(45);
      expect(session.resets_at).toBe(new Date(1786899600 * 1000).toISOString());
      expect(weekly.percent).toBe(60);
      expect(weekly.resets_at).toBe(new Date(1787270400 * 1000).toISOString());
    });

    test("never touches the per-model weekly_scoped window", () => {
      const feed = feedWith({
        five_hour: { used_percentage: 45, resets_at: null },
        seven_day: { used_percentage: 60, resets_at: null },
      });
      const result = reconcileWithStatusline(usage, API_FETCHED_AT, feed) as typeof usage;
      const scoped = result.limits.find((l) => l.kind === "weekly_scoped")!;
      expect(scoped.percent).toBe(8);
    });

    test("only patches the window the feed actually reports, never inventing one", () => {
      const feed = feedWith({
        five_hour: { used_percentage: 45, resets_at: null },
        seven_day: null,
      });
      const result = reconcileWithStatusline(usage, API_FETCHED_AT, feed) as typeof usage;
      const session = result.limits.find((l) => l.kind === "session")!;
      const weekly = result.limits.find((l) => l.kind === "weekly_all")!;
      expect(session.percent).toBe(45);
      expect(weekly.percent).toBe(24); // untouched
    });

    test("does not mutate the original usage object", () => {
      const feed = feedWith({
        five_hour: { used_percentage: 45, resets_at: null },
        seven_day: null,
      });
      reconcileWithStatusline(usage, API_FETCHED_AT, feed);
      expect(usage.limits.find((l) => l.kind === "session")!.percent).toBe(12);
    });
  });

  describe("fixed top-level shape", () => {
    test("patches five_hour and seven_day when both are present objects", () => {
      const usage = {
        five_hour: { utilization: 2, resets_at: "2026-08-15T15:00:00Z" },
        seven_day: { utilization: 3, resets_at: "2026-08-20T00:00:00Z" },
      };
      const feed = feedWith({
        five_hour: { used_percentage: 45, resets_at: 1786899600 },
        seven_day: { used_percentage: 60, resets_at: 1787270400 },
      });
      const result = reconcileWithStatusline(usage, API_FETCHED_AT, feed) as typeof usage;
      expect(result.five_hour.utilization).toBe(45);
      expect(result.seven_day.utilization).toBe(60);
    });

    test("never synthesizes a window the API reported as null, so nothing is double-counted", () => {
      const usage = { five_hour: null, seven_day: { utilization: 3, resets_at: null } };
      const feed = feedWith({
        five_hour: { used_percentage: 45, resets_at: null },
        seven_day: { used_percentage: 60, resets_at: null },
      });
      const result = reconcileWithStatusline(usage, API_FETCHED_AT, feed) as typeof usage;
      expect(result.five_hour).toBeNull();
      expect(result.seven_day.utilization).toBe(60);
    });

    test("leaves a window whose feed side is absent untouched", () => {
      const usage = {
        five_hour: { utilization: 2, resets_at: null },
        seven_day: { utilization: 3, resets_at: null },
      };
      const feed = feedWith({
        five_hour: { used_percentage: 45, resets_at: null },
        seven_day: null,
      });
      const result = reconcileWithStatusline(usage, API_FETCHED_AT, feed) as typeof usage;
      expect(result.five_hour.utilization).toBe(45);
      expect(result.seven_day.utilization).toBe(3);
    });
  });

  describe("a feed with no resets_at preserves the API's reset time", () => {
    test("keeps the API's resets_at in the limits[] shape while the percent updates", () => {
      const usage = {
        limits: [
          {
            kind: "session",
            percent: 12,
            resets_at: "2026-08-15T15:00:00Z",
            scope: null,
            is_active: true,
          },
          {
            kind: "weekly_all",
            percent: 24,
            resets_at: "2026-08-20T00:00:00Z",
            scope: null,
            is_active: false,
          },
        ],
      };
      const feed = feedWith({
        five_hour: { used_percentage: 45, resets_at: null },
        seven_day: { used_percentage: 60, resets_at: null },
      });
      const result = reconcileWithStatusline(usage, API_FETCHED_AT, feed) as typeof usage;
      const session = result.limits.find((l) => l.kind === "session")!;
      const weekly = result.limits.find((l) => l.kind === "weekly_all")!;
      expect(session.percent).toBe(45);
      expect(session.resets_at).toBe("2026-08-15T15:00:00Z");
      expect(weekly.percent).toBe(60);
      expect(weekly.resets_at).toBe("2026-08-20T00:00:00Z");
    });

    test("keeps the API's resets_at in the fixed top-level shape while the percent updates", () => {
      const usage = {
        five_hour: { utilization: 2, resets_at: "2026-08-15T15:00:00Z" },
        seven_day: { utilization: 3, resets_at: "2026-08-20T00:00:00Z" },
      };
      const feed = feedWith({
        five_hour: { used_percentage: 45, resets_at: null },
        seven_day: { used_percentage: 60, resets_at: null },
      });
      const result = reconcileWithStatusline(usage, API_FETCHED_AT, feed) as typeof usage;
      expect(result.five_hour.utilization).toBe(45);
      expect(result.five_hour.resets_at).toBe("2026-08-15T15:00:00Z");
      expect(result.seven_day.utilization).toBe(60);
      expect(result.seven_day.resets_at).toBe("2026-08-20T00:00:00Z");
    });

    test("still lets a feed that supplies a reset time win over the API's", () => {
      const usage = {
        limits: [
          {
            kind: "session",
            percent: 12,
            resets_at: "2026-08-15T15:00:00Z",
            scope: null,
            is_active: true,
          },
        ],
      };
      const feed = feedWith({ five_hour: { used_percentage: 45, resets_at: 1786899600 } });
      const result = reconcileWithStatusline(usage, API_FETCHED_AT, feed) as typeof usage;
      expect(result.limits[0].resets_at).toBe(new Date(1786899600 * 1000).toISOString());
    });
  });
});
