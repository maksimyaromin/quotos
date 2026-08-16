import type { LimitWindowEntity, Severity } from "../../types/entities";

/** Friendly labels for the `limits[].kind` values seen in the wild (see
 * `data/quotos-source-s1/report.md`). Anything not listed here still
 * renders — just humanized from the kind string itself — because the API
 * ships no display name and window kinds churn (the report found internal
 * codenames like `nimbus_quill` mixed in with stable ones). */
const KNOWN_KIND_LABELS: Record<string, string> = {
  session: "Session",
  weekly_all: "Weekly",
  weekly_scoped: "Weekly",
  weekly_opus: "Weekly",
  weekly_sonnet: "Weekly",
  weekly_cowork: "Weekly",
};

/** Friendly labels for the fixed top-level windows, used only when
 * `limits[]` is absent or empty. */
const FIXED_WINDOW_LABELS: Record<string, string> = {
  five_hour: "Session",
  seven_day: "Weekly",
  seven_day_opus: "Weekly",
  seven_day_sonnet: "Weekly",
  seven_day_cowork: "Weekly",
};

function humanizeKind(kind: string): string {
  return kind
    .split("_")
    .filter(Boolean)
    .map((word) => word.charAt(0).toUpperCase() + word.slice(1))
    .join(" ");
}

function clampPercent(value: unknown): number | null {
  if (typeof value !== "number" || Number.isNaN(value)) return null;
  return Math.max(0, Math.min(100, Math.round(value)));
}

function asString(value: unknown): string | null {
  return typeof value === "string" && value.length > 0 ? value : null;
}

/** v4: a window's id needs to be stable across re-reads (pinning keys off
 * it — see `LimitWindowEntity.id`) and unique within one subscription's own
 * window list. `kind` alone collides when more than one window shares it
 * (e.g. two `weekly_scoped` entries, one per model) — scope disambiguates. */
function windowId(kind: string, scope: string | null): string {
  return scope ? `${kind}:${scope}` : kind;
}

interface RawLimit {
  kind?: unknown;
  percent?: unknown;
  resets_at?: unknown;
  scope?: { model?: { display_name?: unknown }; surface?: unknown } | null;
  is_active?: unknown;
}

function windowFromLimit(limit: RawLimit): LimitWindowEntity {
  const kind = typeof limit.kind === "string" ? limit.kind : "unknown";
  const name = KNOWN_KIND_LABELS[kind] ?? humanizeKind(kind);
  const scopeModel = asString(limit.scope?.model?.display_name);
  const scopeSurface = asString(limit.scope?.surface);
  const scope = scopeModel ?? scopeSurface;
  return {
    id: windowId(kind, scope),
    name,
    scope,
    used: clampPercent(limit.percent),
    resetsAt: asString(limit.resets_at),
    isActive: limit.is_active === true,
  };
}

function windowFromFixed(key: string, entry: unknown): LimitWindowEntity | null {
  // A null window means "the account has no such window" — hidden, not zero.
  if (entry === null || typeof entry !== "object") return null;
  const record = entry as { utilization?: unknown; resets_at?: unknown };
  return {
    id: windowId(key, null),
    name: FIXED_WINDOW_LABELS[key] ?? humanizeKind(key),
    scope: null,
    used: clampPercent(record.utilization),
    resetsAt: asString(record.resets_at),
    // The fixed top-level shape carries no is_active flag; treat every
    // present window as a headline candidate.
    isActive: true,
  };
}

interface Headline {
  used: number | null;
  resetsAt: string | null;
  id: string | null;
}

/** Fallback headline when there's no account-wide weekly window in the
 * response at all: the most-consumed *active* window, as before R2-2. */
function pickMostConsumed(windows: LimitWindowEntity[]): Headline {
  const candidates = windows.filter((w) => w.isActive && w.used !== null);
  const pool = candidates.length > 0 ? candidates : windows.filter((w) => w.used !== null);
  if (pool.length === 0) return { used: null, resetsAt: null, id: null };
  const binding = pool.reduce((max, w) => ((w.used ?? 0) > (max.used ?? 0) ? w : max));
  return { used: binding.used, resetsAt: binding.resetsAt, id: binding.id };
}

/** R2-2: the headline is the account-wide weekly window — `weekly_all` from
 * `limits[]`, or `seven_day` from the fixed top-level shape — never simply
 * the most-consumed window (that let a per-model weekly like Fable's 17%
 * outrank the account weekly's 15% and become the headline, which the
 * captain called out directly). Reads the raw response rather than the
 * already-built `windows` list because `windowFromLimit`/`windowFromFixed`
 * don't retain the raw `kind`/key needed to identify "the account-wide one"
 * specifically. Returns `null` when no such window exists at all, so the
 * caller can fall back to the old most-consumed behaviour. */
function pickAccountWideWeekly(
  usage: Record<string, unknown>,
): { used: number; resetsAt: string | null; id: string } | null {
  const rawLimits = usage.limits;
  // Mirrors the windows-building rule below: an empty limits[] is treated
  // the same as an absent one, falling through to the fixed top-level shape.
  if (Array.isArray(rawLimits) && rawLimits.length > 0) {
    for (const l of rawLimits) {
      if (l === null || typeof l !== "object") continue;
      const limit = l as RawLimit;
      if (limit.kind !== "weekly_all") continue;
      const used = clampPercent(limit.percent);
      if (used === null) continue;
      return { used, resetsAt: asString(limit.resets_at), id: windowId("weekly_all", null) };
    }
    return null;
  }
  const fixed = usage.seven_day;
  if (fixed && typeof fixed === "object") {
    const record = fixed as { utilization?: unknown; resets_at?: unknown };
    const used = clampPercent(record.utilization);
    if (used !== null)
      return { used, resetsAt: asString(record.resets_at), id: windowId("seven_day", null) };
  }
  return null;
}

/** R2-2: `critical` when *any* window (active or not — a session at 85%
 * still matters even while the weekly headline reads 20%) is >=90% used,
 * `warn` when any is >=75%, otherwise `healthy`. Thresholds mirror the
 * design system's `--cap-critical`/`--cap-warn` tokens exactly; this is the
 * only place they're encoded for Claude. */
function computeSeverity(windows: LimitWindowEntity[]): Severity {
  let severity: Severity = "healthy";
  for (const w of windows) {
    if (w.used === null) continue;
    if (w.used >= 90) return "critical";
    if (w.used >= 75) severity = "warn";
  }
  return severity;
}

export interface NormalizedUsage {
  windows: LimitWindowEntity[];
  used: number | null;
  resetsAt: string | null;
  severity: Severity;
  /** See `Subscription.headlineWindowId`. */
  headlineWindowId: string | null;
}

const EMPTY: NormalizedUsage = {
  windows: [],
  used: null,
  resetsAt: null,
  severity: "healthy",
  headlineWindowId: null,
};

/** Turn a raw `/api/oauth/usage` response into the generic window list plus
 * headline — everywhere in "percent consumed" terms (I2), never "remaining".
 * Defensive throughout: tolerates unknown keys, absent fields, non-array
 * `limits`, and a completely empty/null response — never throws. */
export function normalizeUsage(raw: unknown): NormalizedUsage {
  if (raw === null || raw === undefined || typeof raw !== "object") {
    return EMPTY;
  }
  const usage = raw as Record<string, unknown>;

  const rawLimits = usage.limits;
  let windows: LimitWindowEntity[];

  if (Array.isArray(rawLimits) && rawLimits.length > 0) {
    windows = rawLimits
      .filter((l): l is RawLimit => l !== null && typeof l === "object")
      .map(windowFromLimit);
  } else {
    windows = Object.keys(FIXED_WINDOW_LABELS)
      .map((key) => windowFromFixed(key, usage[key]))
      .filter((w): w is LimitWindowEntity => w !== null);

    const extra = usage.extra_usage;
    if (extra && typeof extra === "object") {
      const e = extra as { is_enabled?: unknown; utilization?: unknown };
      if (e.is_enabled === true) {
        const used = clampPercent(e.utilization);
        if (used !== null) {
          windows.push({
            id: windowId("extra_usage", null),
            name: "Extra usage",
            scope: null,
            used,
            resetsAt: null,
            isActive: true,
          });
        }
      }
    }
  }

  const headline = pickAccountWideWeekly(usage) ?? pickMostConsumed(windows);
  return {
    windows,
    used: headline.used,
    resetsAt: headline.resetsAt,
    severity: computeSeverity(windows),
    headlineWindowId: headline.id,
  };
}
