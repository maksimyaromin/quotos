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

/** R2-2: computed by the provider adapter from *every* window it saw, not
 * just the headline one — "weekly is 20% but the session is nearly out"
 * must still read as amber. `critical` when any window is >=90% used,
 * `warn` when any window is >=75%, otherwise `healthy`. Thresholds mirror
 * the design system's `--cap-critical` / `--cap-warn` tokens; do not
 * introduce separate numbers here. */
export type Severity = "healthy" | "warn" | "critical";

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
  /** R2-2: provider-computed from every window, drives the headline
   * number's, its bar's, and the tray digits' color — never the headline
   * percentage's own magnitude. Untouched by a failed/rate-limited read;
   * only a successful read updates it, same as `used`/`windows` below. */
  severity: Severity;
  /** Percent of the headline window *consumed*, 0-100. R2-2: the headline
   * is the account-wide weekly window, not simply the most-consumed one —
   * selection is provider-owned, see `providers/claude/normalizeUsage.ts`. */
  used: number | null;
  resetsAt: string | null;
  lastReadAt: string | null;
  windows: LimitWindowEntity[];
  reason: string | null;
  /** R3-4: whether *signing in* is genuinely what this subscription needs.
   * Provider-classified (see `providers/claude/index.ts`'s `mapOutcome`) and
   * deliberately separate from `state`: `broken` alone used to drive the
   * "Needs sign-in" badge, so an offline launch or an HTTP 403 accused the
   * captain's working account of being signed out. It is also what silences
   * the rate-budget wait — a retry timer is meaningless on a row whose
   * answer is "sign in", and showing both at once is the contradiction the
   * captain photographed. */
  needsSignIn: boolean;
  pinned: boolean;
  configDir: string;
  /** Set while our own rate budget (not the subscription's health) is the
   * only thing blocking a read; null once it's free to poll again. B5/B6:
   * this must never replace `state` — it is rendered as a quiet fact
   * alongside whatever health state already holds. */
  rateLimitedUntil: string | null;
  /** R2-6: a `claude setup-token` session is running for this account —
   * shell-owned UI state, not provider data (see `signin.rs`). Drives
   * whether the panel shows the code-paste field for this row. */
  signInInProgress: boolean;
  /** R4-3: "Stop tracking" has been pressed and the undo window is still
   * open. The account is *already* untracked as far as every consumer is
   * concerned — persistence, the tray, the Subscriptions screen — and this
   * flag exists only so the panel can keep its slot in the list and draw the
   * Undo row there. It is what makes the panel and the Subscriptions screen
   * observe one state change instead of two: the row used to be dropped from
   * this list five seconds later, by a timer the Subscriptions screen knew
   * nothing about, which is why that screen went on offering [Remove] for
   * something the captain had already stopped tracking. Never persisted (see
   * `useSubscriptions`'s save effect, which filters on it). */
  pendingRemoval: boolean;
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
  /** S2: the Claude Code statusline feed's most recent reading for this
   * config dir, attached by the Rust side on every read attempt (manual or
   * scheduled) — `null`/absent whenever nothing was ever installed, no
   * session has fed it yet, or the feed file is stale/unreadable. See
   * `providers/claude/statuslineMerge.ts` for how this reconciles with
   * `usage` (freshest wins, never inventing a window the API didn't already
   * report). */
  statusline?: StatuslineFeedWire | null;
}

/** One window's reading from Claude Code's own statusline hook, exactly as
 * its docs define `rate_limits.five_hour`/`rate_limits.seven_day`:
 * `used_percentage` 0-100, `resets_at` Unix epoch seconds. */
export interface StatuslineWindowWire {
  used_percentage: number;
  resets_at: number | null;
}

export interface StatuslineFeedWire {
  /** When the helper actually saw this reading (ISO 8601) — not when
   * Quotos later happens to read the file. This is the timestamp
   * reconciliation compares against the API read's own `fetched_at`. */
  written_at: string;
  rate_limits: {
    five_hour?: StatuslineWindowWire | null;
    seven_day?: StatuslineWindowWire | null;
  };
}

/** S2: what's currently configured for an account's Claude Code
 * `statusLine` — the in-app opt-in offer's own status check. */
export type StatuslineIntegrationStatus =
  | { kind: "not_installed" }
  | { kind: "installed" }
  | { kind: "conflict"; existing_command: string };

/** Wire shape of the Rust `StatuslineError` enum (tagged by `kind`) — the
 * install-flow errors the write-mechanism contract requires: refuse on
 * unparseable JSON, and surface (never silently overwrite) a differing
 * existing `statusLine`. */
export type StatuslineError =
  | { kind: "parse_failed"; message: string }
  | { kind: "read_failed"; message: string }
  | { kind: "write_failed"; message: string }
  | { kind: "helper_install_failed"; message: string }
  | { kind: "conflict"; existing_command: string };

export function isStatuslineError(value: unknown): value is StatuslineError {
  return (
    typeof value === "object" &&
    value !== null &&
    "kind" in value &&
    typeof (value as { kind: unknown }).kind === "string"
  );
}

/** Wire shape of the Rust `FetchError` enum (tagged by `kind`). */
export type FetchError =
  | { kind: "not_connected"; message: string }
  | { kind: "unauthorized"; message: string }
  /** R3-4: the credential is present and renewable, its access token has
   * just aged out and Quotos couldn't renew it here. A local problem — see
   * the Rust `FetchError::CredentialStale`. Never a sign-in warning. */
  | { kind: "credential_stale"; message: string }
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
  severity: Severity;
}

/** R2-4: pushed by the Rust-side scheduler (`src-tauri/src/scheduler.rs`)
 * for each automatic, once-a-minute read — the frontend applies these the
 * same way it applies a manual refresh's direct result, just arriving as an
 * event instead of an `invoke` return value. */
export type ScheduledRefreshEvent =
  | { kind: "ok"; snapshot: RawSnapshot }
  | { kind: "err"; account_id: string; error: FetchError };

/** R2-6: pushed once when a `start_sign_in` session ends (the `claude
 * setup-token` process exited), so the panel can re-read the account and
 * drop out of the "waiting for a pasted code" UI. `success` only reflects
 * the process's own exit status — Quotos never inspects the credential
 * itself, so the row's next read is still the real proof either way. */
export interface SignInFinishedEvent {
  account_id: string;
  success: boolean;
}

/** One colored piece of the tray title (R2-2). `tray-icon` v0.24.2's macOS
 * `set_title` takes a plain string with no color channel — verified by
 * reading `platform_impl/macos/mod.rs`, the same way the `set_title(None)`
 * no-op was found (see CLAUDE.md). The real implementation composites a
 * bitmap on the Rust side (`src-tauri/src/tray_render.rs`) so each pinned
 * subscription's digits can carry their own color; the mock client has no
 * real tray to update, so it's a no-op there instead of a stub export. */
export interface TraySegment {
  text: string;
  color: "neutral" | "amber" | "red";
}
