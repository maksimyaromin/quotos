import type { LimitWindowEntity } from "../../types/entities";

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
  const used = clampPercent(limit.percent);
  return {
    name,
    scope: scopeModel ?? scopeSurface,
    remaining: used === null ? null : 100 - used,
    resetsAt: asString(limit.resets_at),
    isActive: limit.is_active === true,
  };
}

function windowFromFixed(key: string, entry: unknown): LimitWindowEntity | null {
  // A null window means "the account has no such window" — hidden, not zero.
  if (entry === null || typeof entry !== "object") return null;
  const record = entry as { utilization?: unknown; resets_at?: unknown };
  const used = clampPercent(record.utilization);
  return {
    name: FIXED_WINDOW_LABELS[key] ?? humanizeKind(key),
    scope: null,
    remaining: used === null ? null : 100 - used,
    resetsAt: asString(record.resets_at),
    // The fixed top-level shape carries no is_active flag; treat every
    // present window as a headline candidate.
    isActive: true,
  };
}

/** Pick the headline: the most-consumed *active* window, per the brief. */
function pickHeadline(windows: LimitWindowEntity[]): { remaining: number | null; resetsAt: string | null } {
  const candidates = windows.filter((w) => w.isActive && w.remaining !== null);
  const pool = candidates.length > 0 ? candidates : windows.filter((w) => w.remaining !== null);
  if (pool.length === 0) return { remaining: null, resetsAt: null };
  const binding = pool.reduce((min, w) => ((w.remaining ?? 100) < (min.remaining ?? 100) ? w : min));
  return { remaining: binding.remaining, resetsAt: binding.resetsAt };
}

export interface NormalizedUsage {
  windows: LimitWindowEntity[];
  remaining: number | null;
  resetsAt: string | null;
}

const EMPTY: NormalizedUsage = { windows: [], remaining: null, resetsAt: null };

/** Turn a raw `/api/oauth/usage` response into the generic window list plus
 * headline. Defensive throughout: tolerates unknown keys, absent fields,
 * non-array `limits`, and a completely empty/null response — never throws. */
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
            name: "Extra usage",
            scope: null,
            remaining: 100 - used,
            resetsAt: null,
            isActive: true,
          });
        }
      }
    }
  }

  const headline = pickHeadline(windows);
  return { windows, remaining: headline.remaining, resetsAt: headline.resetsAt };
}
