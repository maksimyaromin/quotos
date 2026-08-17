import type { LimitWindowEntity, Severity } from "@/types/entities";

const KNOWN_KIND_LABELS: Record<string, string> = {
  session: "Session",
  weekly_all: "Weekly",
  weekly_scoped: "Weekly",
  weekly_opus: "Weekly",
  weekly_sonnet: "Weekly",
  weekly_cowork: "Weekly",
};

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

function buildWindowId(kind: string, scope: string | null): string {
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
    id: buildWindowId(kind, scope),
    name,
    scope,
    used: clampPercent(limit.percent),
    resetsAt: asString(limit.resets_at),
    isActive: limit.is_active === true,
  };
}

function windowFromFixed(key: string, entry: unknown): LimitWindowEntity | null {
  if (entry === null || typeof entry !== "object") return null;
  const record = entry as { utilization?: unknown; resets_at?: unknown };
  return {
    id: buildWindowId(key, null),
    name: FIXED_WINDOW_LABELS[key] ?? humanizeKind(key),
    scope: null,
    used: clampPercent(record.utilization),
    resetsAt: asString(record.resets_at),
    isActive: true,
  };
}

interface Headline {
  used: number | null;
  resetsAt: string | null;
  id: string | null;
}

function pickMostConsumed(windows: LimitWindowEntity[]): Headline {
  const candidates = windows.filter((w) => w.isActive && w.used !== null);
  const pool = candidates.length > 0 ? candidates : windows.filter((w) => w.used !== null);
  if (pool.length === 0) return { used: null, resetsAt: null, id: null };
  const binding = pool.reduce((max, w) => ((w.used ?? 0) > (max.used ?? 0) ? w : max));
  return { used: binding.used, resetsAt: binding.resetsAt, id: binding.id };
}

function pickAccountWideWeekly(
  usage: Record<string, unknown>,
): { used: number; resetsAt: string | null; id: string } | null {
  const rawLimits = usage.limits;
  if (Array.isArray(rawLimits) && rawLimits.length > 0) {
    for (const l of rawLimits) {
      if (l === null || typeof l !== "object") continue;
      const limit = l as RawLimit;
      if (limit.kind !== "weekly_all") continue;
      const used = clampPercent(limit.percent);
      if (used === null) continue;
      return { used, resetsAt: asString(limit.resets_at), id: buildWindowId("weekly_all", null) };
    }
    return null;
  }
  const fixed = usage.seven_day;
  if (fixed && typeof fixed === "object") {
    const record = fixed as { utilization?: unknown; resets_at?: unknown };
    const used = clampPercent(record.utilization);
    if (used !== null)
      return { used, resetsAt: asString(record.resets_at), id: buildWindowId("seven_day", null) };
  }
  return null;
}

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
  headlineWindowId: string | null;
}

const EMPTY: NormalizedUsage = {
  windows: [],
  used: null,
  resetsAt: null,
  severity: "healthy",
  headlineWindowId: null,
};

function buildWindows(usage: Record<string, unknown>): LimitWindowEntity[] {
  const rawLimits = usage.limits;
  if (Array.isArray(rawLimits) && rawLimits.length > 0) {
    return rawLimits
      .filter((l): l is RawLimit => l !== null && typeof l === "object")
      .map(windowFromLimit);
  }

  const windows = Object.keys(FIXED_WINDOW_LABELS)
    .map((key) => windowFromFixed(key, usage[key]))
    .filter((w): w is LimitWindowEntity => w !== null);

  const extra = usage.extra_usage;
  if (extra && typeof extra === "object") {
    const e = extra as { is_enabled?: unknown; utilization?: unknown };
    if (e.is_enabled === true) {
      const used = clampPercent(e.utilization);
      if (used !== null) {
        windows.push({
          id: buildWindowId("extra_usage", null),
          name: "Extra usage",
          scope: null,
          used,
          resetsAt: null,
          isActive: true,
        });
      }
    }
  }
  return windows;
}

export function normalizeUsage(raw: unknown): NormalizedUsage {
  if (raw === null || raw === undefined || typeof raw !== "object") {
    return EMPTY;
  }
  const usage = raw as Record<string, unknown>;
  const windows = buildWindows(usage);
  const headline = pickAccountWideWeekly(usage) ?? pickMostConsumed(windows);
  return {
    windows,
    used: headline.used,
    resetsAt: headline.resetsAt,
    severity: computeSeverity(windows),
    headlineWindowId: headline.id,
  };
}
