/** Generic, provider-agnostic entities. Nothing above this line, and nothing
 * that renders UI, is allowed to know the word "claude" — see brief section 2. */

/** A subscription's own health — never touched by our rate-limit budget.
 * B6: a diagnosable failure (e.g. an expired login) must never be
 * overwritten by a self-inflicted wait; health and "are we currently
 * allowed to poll" are separate concerns (see `rateLimitedUntil` below),
 * and health always wins the display. */
export type SubscriptionState =
  | "idle" // not connected — needs one more step
  | "connecting" // verifying a new connection
  | "working" // data is current
  | "reading" // refresh in flight, previous numbers still shown
  | "behind" // refresh failed, older data held
  | "repairing" // recoverable problem being fixed automatically
  | "broken"; // cannot be read at all

export interface LimitWindowEntity {
  name: string;
  /** Percent of this window *consumed*, 0-100 (I2: everywhere is "used", not "left"). */
  used: number | null;
  resetsAt: string | null;
  scope: string | null;
  isActive: boolean;
}

export interface Subscription {
  id: string;
  provider: string;
  providerName: string;
  label: string;
  /** Set only once the user has renamed it inline (I6); overrides the
   * provider-derived label whenever present. */
  labelOverride: string | null;
  account: string | null;
  state: SubscriptionState;
  /** Percent of the headline (most-consumed active) window *consumed*, 0-100. */
  used: number | null;
  resetsAt: string | null;
  lastReadAt: string | null;
  windows: LimitWindowEntity[];
  reason: string | null;
  pinned: boolean;
  configDir: string;
  /** Set while our own rate budget (not the subscription's health) is the
   * only thing blocking a read; null once it's free to poll again. B5/B6:
   * this must never replace `state` — it is rendered as a quiet fact
   * alongside whatever health state already holds. */
  rateLimitedUntil: string | null;
}

/** Wire shape returned by the Rust `list_accounts` command. */
export interface AccountDescriptor {
  id: string;
  provider: string;
  config_dir: string;
}

/** Wire shape returned by the Rust `fetch_snapshot` command on success. */
export interface RawSnapshot {
  account_id: string;
  provider: string;
  config_dir: string;
  fetched_at: string;
  usage: unknown;
  profile: unknown | null;
}

/** Wire shape of the Rust `FetchError` enum (tagged by `kind`). */
export type FetchError =
  | { kind: "not_connected"; message: string }
  | { kind: "unauthorized"; message: string }
  | { kind: "rate_limited"; retry_after_secs: number }
  | { kind: "network"; message: string }
  | { kind: "other"; message: string };

export function isFetchError(value: unknown): value is FetchError {
  return (
    typeof value === "object" &&
    value !== null &&
    "kind" in value &&
    typeof (value as { kind: unknown }).kind === "string"
  );
}

/** What a provider adapter's normaliser produces from a successful raw read. */
export interface NormalizedRead {
  label: string;
  account: string | null;
  windows: LimitWindowEntity[];
  /** Percent of the headline window *consumed*, 0-100 (I2). */
  used: number | null;
  resetsAt: string | null;
}
