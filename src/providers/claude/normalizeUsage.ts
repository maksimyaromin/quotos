import type { LimitWindowEntity, Severity } from "../../types/entities";

/** Friendly labels for the `limits[].kind` values seen in the wild.
 * Anything not listed here still renders, humanized from the kind string
 * itself, because the API ships no display name and window kinds churn,
 * including internal codenames such as `nimbus_quill` alongside stable
 * ones. */
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

/** A window's id must be stable across re-reads, since pinning keys off
 * it, see `LimitWindowEntity.id`, and unique within one subscription's own
 * window list. `kind` alone collides when more than one window shares it,
 * for example two `weekly_scoped` entries, one per model, so scope
 * disambiguates. */
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
  // A null window means the account has no such window. It is hidden, not zero.
  if (entry === null || typeof entry !== "object") return null;
  const record = entry as { utilization?: unknown; resets_at?: unknown };
  return {
    id: windowId(key, null),
    name: FIXED_WINDOW_LABELS[key] ?? humanizeKind(key),
    scope: null,
    used: clampPercent(record.utilization),
    resetsAt: asString(record.resets_at),
    // The fixed top-level shape carries no is_active flag. Treat every
    // present window as a headline candidate.
    isActive: true,
  };
}

interface Headline {
  used: number | null;
  resetsAt: string | null;
  id: string | null;
}

/** Fallback headline when there is no account-wide weekly window in the
 * response at all: the most-consumed active window. */
function pickMostConsumed(windows: LimitWindowEntity[]): Headline {
  const candidates = windows.filter((w) => w.isActive && w.used !== null);
  const pool = candidates.length > 0 ? candidates : windows.filter((w) => w.used !== null);
  if (pool.length === 0) return { used: null, resetsAt: null, id: null };
  const binding = pool.reduce((max, w) => ((w.used ?? 0) > (max.used ?? 0) ? w : max));
  return { used: binding.used, resetsAt: binding.resetsAt, id: binding.id };
}

/** The headline is the account-wide weekly window, `weekly_all` from
 * `limits[]` or `seven_day` from the fixed top-level shape, never simply
 * the most-consumed window. Selecting by consumption alone would let a
 * per-model weekly outrank the account-wide total, for example a 17%
 * per-model window outranking a 15% account weekly. This reads the raw
 * response rather than the already-built `windows` list, because
 * `windowFromLimit` and `windowFromFixed` do not retain the raw `kind` or
 * key needed to identify the account-wide window specifically. Returns
 * `null` when no such window exists at all, so the caller can fall back to
 * `pickMostConsumed`. */
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

/** `critical` when any window, active or not, is at least 90% used. A
 * session at 85% still matters even while the weekly headline reads 20%.
 * `warn` when any window is at least 75% used, otherwise `healthy`. These
 * thresholds mirror the design system's `--cap-critical` and `--cap-warn`
 * tokens exactly, and this is the only place they are encoded for Claude. */
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

/** Turns a raw `/api/oauth/usage` response into the generic window list
 * plus headline, everywhere in percent-consumed terms, never remaining.
 * Defensive throughout: tolerates unknown keys, absent fields, a
 * non-array `limits`, and a completely empty or null response. Never
 * throws. */
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
