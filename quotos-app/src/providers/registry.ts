import type { NormalizedRead } from "../types/entities";
import { normalize as normalizeClaude } from "./claude";

export type Normalizer = (usage: unknown, profile: unknown, fallbackLabel: string) => NormalizedRead;

/** One entry per provider adapter. Adding a second provider means one new
 * module plus one new line here — nothing else in the app changes. */
export const PROVIDER_NORMALIZERS: Record<string, Normalizer> = {
  claude: normalizeClaude,
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
    return { label: fallbackLabel, account: null, windows: [], used: null, resetsAt: null };
  }
  return normalizer(usage, profile, fallbackLabel);
}
