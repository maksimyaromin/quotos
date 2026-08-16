/** Turns `/api/oauth/profile` into label fields. Label comes from
 * `organization.name` and `organization_type`, never from `account.email`
 * or `account.full_name`, since those are identical across a person's
 * accounts and would make two subscriptions indistinguishable. */

function humanizeOrgType(type: string): string {
  return type
    .replace(/^claude_/, "")
    .split("_")
    .filter(Boolean)
    .map((w) => w.charAt(0).toUpperCase() + w.slice(1))
    .join(" ");
}

export interface NormalizedProfile {
  label: string | null;
  account: string | null;
}

export function normalizeProfile(raw: unknown): NormalizedProfile {
  if (raw === null || raw === undefined || typeof raw !== "object") {
    return { label: null, account: null };
  }
  const org = (raw as Record<string, unknown>).organization;
  if (org === null || org === undefined || typeof org !== "object") {
    return { label: null, account: null };
  }
  const record = org as Record<string, unknown>;
  const name = typeof record.name === "string" && record.name.length > 0 ? record.name : null;
  const orgType =
    typeof record.organization_type === "string" ? humanizeOrgType(record.organization_type) : null;
  return { label: name, account: orgType };
}
