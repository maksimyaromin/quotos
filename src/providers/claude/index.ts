import type { FetchError, NormalizedRead } from "@/types/entities";
import type { NormalizeContext, OutcomeResult, ReadOutcome } from "../registry";
import { normalizeProfile } from "./normalize-profile";
import { normalizeUsage } from "./normalize-usage";
import { reconcileWithStatusline } from "./statusline-merge";

/** Reconciles the zero-cost statusline feed, `context.statuslineFeed`,
 * with the API payload before normalizing. See `statusline-merge.ts` for
 * why that happens on the raw shape rather than the normalized window
 * list. A context with no feed, or an older one, leaves `usageRaw`
 * untouched. */
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

/** The outcome-to-state-and-reason mapping is provider-owned, so a second
 * provider can supply its own without touching the generic
 * `use-subscriptions.ts`; the state vocabulary itself, see `types/entities.ts`'s
 * `SubscriptionState` doc, is fixed. See `ReadOutcome`'s doc for why
 * `rate_limited` never reaches this function; the shell restores whatever
 * state this function returned for the previous attempt instead. */
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
  // Once a prior good read exists, every error kind reads as "behind" with
  // this one fixed sentence, never the specific and often quite technical
  // underlying error text. The raw error would contradict "these numbers
  // are from the last successful read" by describing a brand-new failure
  // instead. Tests pin this sentence exactly, so reword it only alongside
  // them.
  if (hadGoodRead) {
    return {
      state: "behind",
      reason: "The provider didn't answer. These numbers are from the last successful read.",
      needsSignIn: false,
    };
  }

  switch (err.kind) {
    case "not_connected":
      // No credential at all for this config dir. Claude Code is not
      // signed in here. `idle`, the hollow ring, is the honest health
      // state. The sign-in flag is what makes it actionable.
      return { state: "idle", reason: err.message, needsSignIn: true };
    case "unauthorized":
      // The Rust side only sends this when the sign-in itself is what
      // failed. A token that merely aged out arrives as `credential_stale`
      // below instead, so a signed-in account is never told to sign in
      // again.
      return {
        state: "broken",
        reason: "The sign-in expired. Log in again in Claude Code and Quotos will pick it up.",
        needsSignIn: true,
      };
    case "credential_stale":
      // Signed in and renewable, just not renewable from here. Carries the
      // Rust side's own sentence, which says what is actually wrong.
      return { state: "broken", reason: err.message, needsSignIn: false };
    case "network":
      return { state: "broken", reason: err.message, needsSignIn: false };
    case "rate_limited":
      // Unreachable in practice, see the doc comment above. Handled here
      // only so this switch stays exhaustive over `FetchError["kind"]`.
      return {
        state: "broken",
        reason: "Unexpected rate-limit outcome reached the provider mapper.",
        needsSignIn: false,
      };
    default:
      return { state: "broken", reason: err.message, needsSignIn: false };
  }
}
