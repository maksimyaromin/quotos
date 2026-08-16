import type {
  FetchError,
  NormalizedRead,
  StatuslineFeedWire,
  SubscriptionState,
} from "@/types/entities";
import { mapOutcome as mapOutcomeClaude, normalize as normalizeClaude } from "./claude";

/** Everything a provider's normalizer needs about this particular read
 * beyond the raw usage and profile payloads: when it was fetched, and
 * whatever the statusline feed most recently reported, so a provider can
 * reconcile the two itself. See `providers/claude/index.ts`'s `normalize`
 * and `statusline-merge.ts`. A provider with no such feed just ignores it. */
export interface NormalizeContext {
  fetchedAt: string;
  statuslineFeed?: StatuslineFeedWire | null;
}

export type Normalizer = (
  usage: unknown,
  profile: unknown,
  fallbackLabel: string,
  context: NormalizeContext,
) => NormalizedRead;

/** What a refresh attempt produced, for the provider's outcome-to-state
 * mapper. Deliberately excludes `rate_limited`, since a self-imposed
 * budget wait is never a health state. The shell in `use-subscriptions.ts`
 * intercepts it before it ever reaches a provider's mapper. */
export type ReadOutcome =
  | { kind: "ok"; normalized: NormalizedRead }
  | { kind: "error"; error: FetchError | null };

export interface OutcomeResult {
  state: SubscriptionState;
  reason: string | null;
  /** Whether signing in is what this outcome actually calls for. See
   * `Subscription.needsSignIn` for why this is classified here rather than
   * inferred from `state === "broken"` at render time. */
  needsSignIn: boolean;
}

/** Maps a non-rate-limited read outcome to a health state and reason.
 * Provider-owned so a second provider can encode its own read outcomes
 * without touching the shell. See `providers/claude/index.ts`'s
 * `mapOutcome` for the reference implementation. */
export type OutcomeMapper = (outcome: ReadOutcome, hadGoodRead: boolean) => OutcomeResult;

/** One entry per provider adapter. Adding a second provider means one new
 * module plus one new line here. Nothing else in the app changes. */
export const PROVIDER_NORMALIZERS: Record<string, Normalizer> = {
  claude: normalizeClaude,
};

export const PROVIDER_OUTCOME_MAPPERS: Record<string, OutcomeMapper> = {
  claude: mapOutcomeClaude,
};

/** Display name of who issued the subscription. The one place "Claude" as
 * a provider concept is named. The panel itself never branches on it. */
export const PROVIDER_DISPLAY_NAMES: Record<string, string> = {
  claude: "Anthropic",
};

export function resolveProviderDisplayName(provider: string): string {
  return PROVIDER_DISPLAY_NAMES[provider] ?? provider;
}

export function normalizeFor(
  provider: string,
  usage: unknown,
  profile: unknown,
  fallbackLabel: string,
  context: NormalizeContext,
): NormalizedRead {
  const normalizer = PROVIDER_NORMALIZERS[provider];
  if (!normalizer) {
    return {
      label: fallbackLabel,
      account: null,
      windows: [],
      used: null,
      resetsAt: null,
      severity: "healthy",
      headlineWindowId: null,
    };
  }
  return normalizer(usage, profile, fallbackLabel, context);
}

export function mapOutcomeFor(
  provider: string,
  outcome: ReadOutcome,
  hadGoodRead: boolean,
): OutcomeResult {
  const mapper = PROVIDER_OUTCOME_MAPPERS[provider];
  if (!mapper) {
    return {
      state: hadGoodRead ? "behind" : "broken",
      reason: `No adapter for provider '${provider}'.`,
      needsSignIn: false,
    };
  }
  return mapper(outcome, hadGoodRead);
}
