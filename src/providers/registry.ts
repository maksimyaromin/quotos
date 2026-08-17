import type {
  FetchError,
  NormalizedRead,
  StatuslineFeedWire,
  SubscriptionState,
} from "@/types/entities";
import { mapOutcome as mapOutcomeClaude, normalize as normalizeClaude } from "./claude";

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

export type ReadOutcome =
  | { kind: "ok"; normalized: NormalizedRead }
  | { kind: "error"; error: FetchError | null };

export interface OutcomeResult {
  state: SubscriptionState;
  reason: string | null;
  needsSignIn: boolean;
}

export type OutcomeMapper = (outcome: ReadOutcome, hadGoodRead: boolean) => OutcomeResult;

export const PROVIDER_NORMALIZERS: Record<string, Normalizer> = {
  claude: normalizeClaude,
};

export const PROVIDER_OUTCOME_MAPPERS: Record<string, OutcomeMapper> = {
  claude: mapOutcomeClaude,
};

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
