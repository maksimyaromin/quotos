import { normalizeUsage } from "./normalizeUsage";
import { normalizeProfile } from "./normalizeProfile";
import type { FetchError, NormalizedRead } from "../../types/entities";
import type { ReadOutcome, OutcomeResult } from "../registry";

export function normalize(usageRaw: unknown, profileRaw: unknown, fallbackLabel: string): NormalizedRead {
  const usage = normalizeUsage(usageRaw);
  const profile = normalizeProfile(profileRaw);
  return {
    label: profile.label ?? fallbackLabel,
    account: profile.account,
    windows: usage.windows,
    used: usage.used,
    resetsAt: usage.resetsAt,
    severity: usage.severity,
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
    return { state: "working", reason: noLimits ? "No limits reported yet." : null };
  }

  const err = outcome.error;
  if (!err) {
    return { state: hadGoodRead ? "behind" : "broken", reason: "Something went wrong reading this subscription." };
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
    return { state: "behind", reason: "The provider didn't answer. These numbers are from the last successful read." };
  }

  switch (err.kind) {
    case "not_connected":
      return { state: "idle", reason: err.message };
    case "unauthorized":
      // F17 (handoff's exact wording).
      return { state: "broken", reason: "The sign-in expired. Log in again in Claude Code and Quotos will pick it up." };
    case "network":
      return { state: "broken", reason: err.message };
    case "rate_limited":
      // Unreachable in practice — see the doc comment above. Handled here
      // only so this switch stays exhaustive over `FetchError["kind"]`.
      return { state: "broken", reason: "Unexpected rate-limit outcome reached the provider mapper." };
    default:
      return { state: "broken", reason: err.message };
  }
}
