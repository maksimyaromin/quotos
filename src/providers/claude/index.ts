import type { FetchError, NormalizedRead } from "../../types/entities";
import type { NormalizeContext, OutcomeResult, ReadOutcome } from "../registry";
import { normalizeProfile } from "./normalizeProfile";
import { normalizeUsage } from "./normalizeUsage";
import { reconcileWithStatusline } from "./statuslineMerge";

/** S2: reconciles the zero-cost statusline feed (`context.statuslineFeed`)
 * with the API payload before normalizing — see `statuslineMerge.ts` for
 * why that happens on the raw shape rather than the normalized window
 * list. A context with no feed (or an older one) leaves `usageRaw`
 * untouched, so this is exactly today's behavior whenever nothing is
 * feeding the statusline. */
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

/** R2-3: the outcome→state+reason mapping, moved here from the generic
 * `useSubscriptions.ts` so a second provider can supply its own — per the
 * captain's own words, "статусы думаю тоже на уровне провайдера надо
 * кодировать." The state vocabulary itself is fixed (see
 * `types/entities.ts`'s `SubscriptionState` doc); only which state+reason a
 * given outcome maps to is provider-owned.
 *
 * `rate_limited` is deliberately not one of the cases handled here — B5/B6
 * (CLAUDE.md, `useSubscriptions.test.ts`'s "B6" block): a self-imposed
 * budget wait is never a health state, so the shell intercepts it before
 * calling into any provider's mapper at all and restores whatever state
 * this same function returned for the *previous* attempt. */
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
  // F16 (handoff's exact wording): once a prior good read exists, every
  // error kind reads as "behind" with this one fixed sentence — never the
  // specific (often quite technical) underlying error text, which would
  // contradict "these numbers are from the last successful read" by
  // describing a brand-new failure instead.
  if (hadGoodRead) {
    return {
      state: "behind",
      reason: "The provider didn't answer. These numbers are from the last successful read.",
      needsSignIn: false,
    };
  }

  switch (err.kind) {
    case "not_connected":
      // No credential at all for this config dir — Claude Code is not
      // signed in here. `idle` (the hollow ring) is the honest health
      // state; the sign-in flag is what makes it actionable.
      return { state: "idle", reason: err.message, needsSignIn: true };
    case "unauthorized":
      // F17 (handoff's exact wording). R3-4: the Rust side now only sends
      // this when the sign-in itself is what failed — a token that merely
      // aged out arrives as `credential_stale` below, because telling the
      // captain to sign in to an account he is signed into is the exact
      // bug this round fixes.
      return {
        state: "broken",
        reason: "The sign-in expired. Log in again in Claude Code and Quotos will pick it up.",
        needsSignIn: true,
      };
    case "credential_stale":
      // Signed in, renewable, just not renewable *from here*. Carries the
      // Rust side's own sentence, which says what is actually wrong.
      return { state: "broken", reason: err.message, needsSignIn: false };
    case "network":
      return { state: "broken", reason: err.message, needsSignIn: false };
    case "rate_limited":
      // Unreachable in practice — see the doc comment above. Handled here
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
