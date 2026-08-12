import { normalizeUsage } from "./normalizeUsage";
import { normalizeProfile } from "./normalizeProfile";
import type { NormalizedRead } from "../../types/entities";

export function normalize(usageRaw: unknown, profileRaw: unknown, fallbackLabel: string): NormalizedRead {
  const usage = normalizeUsage(usageRaw);
  const profile = normalizeProfile(profileRaw);
  return {
    label: profile.label ?? fallbackLabel,
    account: profile.account,
    windows: usage.windows,
    used: usage.used,
    resetsAt: usage.resetsAt,
  };
}
