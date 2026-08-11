/** Generic, provider-agnostic entities. Nothing above this line, and nothing
 * that renders UI, is allowed to know the word "claude" — see brief section 2. */

export type SubscriptionState =
  | "idle" // not connected — needs one more step
  | "connecting" // verifying a new connection
  | "working" // data is current
  | "reading" // refresh in flight, previous numbers still shown
  | "behind" // refresh failed, older data held
  | "repairing" // recoverable problem being fixed automatically
  | "broken" // cannot be read at all
  | "waiting"; // provider refused a too-frequent read

export interface LimitWindowEntity {
  name: string;
  remaining: number | null;
  resetsAt: string | null;
  scope: string | null;
  isActive: boolean;
}

export interface Subscription {
  id: string;
  provider: string;
  providerName: string;
  label: string;
  account: string | null;
  state: SubscriptionState;
  remaining: number | null;
  resetsAt: string | null;
  lastReadAt: string | null;
  windows: LimitWindowEntity[];
  reason: string | null;
  pinned: boolean;
  configDir: string;
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
  remaining: number | null;
  resetsAt: string | null;
}
