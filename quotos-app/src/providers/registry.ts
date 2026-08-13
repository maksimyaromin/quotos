import type { FetchError, NormalizedRead, SubscriptionState } from "../types/entities";
import { normalize as normalizeClaude, mapOutcome as mapOutcomeClaude } from "./claude";

export type Normalizer = (usage: unknown, profile: unknown, fallbackLabel: string) => NormalizedRead;

/** What a refresh attempt produced, for the provider's outcome→state
 * mapper. Deliberately excludes `rate_limited` — B5/B6: a self-imposed
 * budget wait is never a health state, so the shell (`useSubscriptions.ts`)
 * intercepts it before it ever reaches a provider's mapper. */
export type ReadOutcome =
  | { kind: "ok"; normalized: NormalizedRead }
  | { kind: "error"; error: FetchError | null };

export interface OutcomeResult {
  state: SubscriptionState;
  reason: string | null;
}

/** R2-3: maps a (non-rate-limited) read outcome to a health state + reason.
 * Provider-owned so a second provider can encode its own read outcomes
 * without touching the shell — see `providers/claude/index.ts`'s
 * `mapOutcome` for the reference implementation and its B5/B6 note. */
export type OutcomeMapper = (outcome: ReadOutcome, hadGoodRead: boolean) => OutcomeResult;

/** One entry per provider adapter. Adding a second provider means one new
 * module plus one new line here — nothing else in the app changes. */
export const PROVIDER_NORMALIZERS: Record<string, Normalizer> = {
  claude: normalizeClaude,
};

export const PROVIDER_OUTCOME_MAPPERS: Record<string, OutcomeMapper> = {
  claude: mapOutcomeClaude,
};

/** Display name of who issued the subscription — the one place "Claude" as
 * a provider concept is named; the panel itself never branches on it. */
export const PROVIDER_DISPLAY_NAMES: Record<string, string> = {
  claude: "Anthropic",
};

export function providerDisplayName(provider: string): string {
  return PROVIDER_DISPLAY_NAMES[provider] ?? provider;
}

export function normalizeFor(provider: string, usage: unknown, profile: unknown, fallbackLabel: string): NormalizedRead {
  const normalizer = PROVIDER_NORMALIZERS[provider];
  if (!normalizer) {
    return { label: fallbackLabel, account: null, windows: [], used: null, resetsAt: null, severity: "healthy" };
  }
  return normalizer(usage, profile, fallbackLabel);
}

export function mapOutcomeFor(provider: string, outcome: ReadOutcome, hadGoodRead: boolean): OutcomeResult {
  const mapper = PROVIDER_OUTCOME_MAPPERS[provider];
  if (!mapper) {
    return { state: hadGoodRead ? "behind" : "broken", reason: `No adapter for provider '${provider}'.` };
  }
  return mapper(outcome, hadGoodRead);
}
