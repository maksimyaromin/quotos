import type { FetchError, NormalizedRead } from "@/types/entities";
import type { NormalizeContext, OutcomeResult, ReadOutcome } from "../registry";
import { normalizeProfile } from "./normalize-profile";
import { normalizeUsage } from "./normalize-usage";
import { reconcileWithStatusline } from "./statusline-merge";

export function normalize(
  usageRaw: unknown,
  profileRaw: unknown,
  fallbackLabel: string,
  context: NormalizeContext,
): NormalizedRead {
  const reconciled = reconcileWithStatusline(usageRaw, context.fetchedAt, context.statuslineFeed);
  const usage = normalizeUsage(reconciled);
  const profile = normalizeProfile(profileRaw);
  return {
    label: profile.label ?? fallbackLabel,
    account: profile.account,
    windows: usage.windows,
    used: usage.used,
    resetsAt: usage.resetsAt,
    severity: usage.severity,
    headlineWindowId: usage.headlineWindowId,
  };
}

export function mapOutcome(outcome: ReadOutcome, hadGoodRead: boolean): OutcomeResult {
  if (outcome.kind === "ok") {
    const noLimits = outcome.normalized.used === null && outcome.normalized.windows.length === 0;
    return {
      state: "working",
      reason: noLimits ? "No limits reported yet." : null,
      needsSignIn: false,
    };
  }

  const err = outcome.error;
  if (!err) {
    return {
      state: hadGoodRead ? "behind" : "broken",
      reason: "Something went wrong reading this subscription.",
      needsSignIn: false,
    };
  }
  return mapFetchError(err, hadGoodRead);
}

function mapFetchError(err: FetchError, hadGoodRead: boolean): OutcomeResult {
  if (hadGoodRead) {
    return {
      state: "behind",
      reason: "The provider didn't answer. These numbers are from the last successful read.",
      needsSignIn: false,
    };
  }

  switch (err.kind) {
    case "not_connected":
      return { state: "idle", reason: err.message, needsSignIn: true };
    case "unauthorized":
      return {
        state: "broken",
        reason: "The sign-in expired. Log in again in Claude Code and Quotos will pick it up.",
        needsSignIn: true,
      };
    case "credential_stale":
      return { state: "broken", reason: err.message, needsSignIn: false };
    case "network":
      return { state: "broken", reason: err.message, needsSignIn: false };
    case "rate_limited":
      // Intercepted in use-subscriptions.ts before it reaches a provider
      // mapper; this branch exists only as a defensive fallback.
      return {
        state: "broken",
        reason: "Unexpected rate-limit outcome reached the provider mapper.",
        needsSignIn: false,
      };
    default:
      return { state: "broken", reason: err.message, needsSignIn: false };
  }
}
