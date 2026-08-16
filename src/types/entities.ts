/** Generic, provider-agnostic entities. Nothing above this line, and nothing
 * that renders UI, may reference the word "claude". See docs/design/brief.md
 * section 2. */

/** A subscription's own health, never touched by the rate-limit budget. A
 * diagnosable failure such as an expired login must never be overwritten by
 * a self-inflicted wait. Health and whether a read is currently allowed to
 * run are separate concerns, see `rateLimitedUntil` below, and health always
 * wins the display. */
export type SubscriptionState =
  | "idle" // Not connected. Needs one more step.
  | "connecting" // Verifying a new connection.
  | "working" // Data is current.
  | "reading" // A refresh is in flight. Previous numbers are still shown.
  | "behind" // The last refresh failed. Older data is held.
  | "broken"; // Cannot be read at all.

export interface LimitWindowEntity {
  /** Stable within a subscription's own window list across re-reads. Built
   * from the provider's own `kind`, plus `scope` when more than one window
   * can share a `kind`, see `providers/claude/normalizeUsage.ts`'s
   * `windowId`. Pinning keys off this id instead of a subscription-wide
   * boolean, see `Subscription.pinnedWindowIds`. */
  id: string;
  name: string;
  /** Percent of this window consumed, 0 to 100. Every percent field in this
   * module means consumed, never remaining. */
  used: number | null;
  resetsAt: string | null;
  scope: string | null;
  isActive: boolean;
}

/** Computed by the provider adapter from every window it saw, not just the
 * headline one, so a healthy headline percent does not mask a
 * near-exhausted window elsewhere. See `providers/claude/normalizeUsage.ts`'s
 * `computeSeverity` for the exact thresholds. */
export type Severity = "healthy" | "warn" | "critical";

export interface Subscription {
  id: string;
  provider: string;
  providerName: string;
  label: string;
  labelOverride: string | null;
  account: string | null;
  state: SubscriptionState;
  /** Untouched by a failed or rate-limited read; see `Severity`'s own doc
   * for how it's computed. */
  severity: Severity;
  /** Percent of the headline window consumed, 0 to 100. The headline is
   * the account-wide weekly window, not the most-consumed one, see
   * `providers/claude/normalizeUsage.ts`. */
  used: number | null;
  resetsAt: string | null;
  lastReadAt: string | null;
  windows: LimitWindowEntity[];
  reason: string | null;
  /** Provider-classified, see `providers/claude/index.ts`'s `mapOutcome`.
   * Separate from `state` so a stale credential or a network failure never
   * reads as a sign-in problem. */
  needsSignIn: boolean;
  /** Per limit window, not per subscription, see `LimitWindowEntity.id`.
   * Order does not matter: figure order in the status item comes from
   * `windows`' own order, see `lib/statusItemSegments.ts`. */
  pinnedWindowIds: string[];
  /** Which of this subscription's current `windows` is the headline, see
   * `NormalizedRead.headlineWindowId`. Null before any read. */
  headlineWindowId: string | null;
  configDir: string;
  /** Set while the app's own rate budget, not health, blocks a read.
   * Never replaces `state`. */
  rateLimitedUntil: string | null;
  /** A `claude setup-token` session is running for this account, see
   * `signin.rs`. */
  signInInProgress: boolean;
  /** Set while "Stop tracking" is pending its undo window. The account is
   * already untracked everywhere except the panel's own row slot. */
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
  /** The Claude Code statusline feed's most recent reading for this config
   * dir, attached by the Rust side on every read attempt. Null or absent
   * whenever nothing was installed or no session has fed it yet. See
   * `providers/claude/statuslineMerge.ts`: the freshest reading wins, and
   * it never invents a window the API did not already report. */
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
  /** When the helper actually saw this reading, ISO 8601, not when Quotos
   * later happens to read the file. This is the timestamp reconciliation
   * compares against the API read's own `fetched_at`. */
  written_at: string;
  rate_limits: {
    five_hour?: StatuslineWindowWire | null;
    seven_day?: StatuslineWindowWire | null;
  };
}

/** What is currently configured for an account's Claude Code `statusLine`.
 * The in-app opt-in offer's own status check. */
export type StatuslineIntegrationStatus =
  | { kind: "not_installed" }
  | { kind: "installed" }
  | { kind: "conflict"; existing_command: string };

/** Wire shape of the Rust `StatuslineError` enum, tagged by `kind`. The
 * install flow refuses on unparseable JSON, and surfaces rather than
 * silently overwriting a differing existing `statusLine`. */
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
  /** The credential is present and renewable. Its access token has just
   * aged out and Quotos could not renew it here. A local problem, see the
   * Rust `FetchError::CredentialStale`. Never a sign-in warning. */
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
  /** Percent of the headline window consumed, 0 to 100. */
  used: number | null;
  resetsAt: string | null;
  severity: Severity;
  /** See `Subscription.headlineWindowId`. Null when `windows` is empty. */
  headlineWindowId: string | null;
}

/** Pushed by the Rust-side scheduler, `src-tauri/src/scheduler.rs`, for
 * each automatic, once-a-minute read. The frontend applies these the same
 * way it applies a manual refresh's direct result. */
export type ScheduledRefreshEvent =
  | { kind: "ok"; snapshot: RawSnapshot }
  | { kind: "err"; account_id: string; error: FetchError };

/** Pushed once a `start_sign_in` session's `claude setup-token` process
 * exits, so the panel can re-read the account and drop out of the
 * waiting-for-a-pasted-code UI. `success` reflects only the process's own
 * exit status; the row's next read is the real proof either way. */
export interface SignInFinishedEvent {
  account_id: string;
  success: boolean;
}

/** One colored piece of the status item's title. `tray-icon`'s macOS
 * `set_title` takes a plain string with no color channel, so the native
 * side composites a bitmap from these segments instead. One figure per
 * pinned window, in panel order, then each subscription's own window
 * order. */
export interface StatusItemSegment {
  text: string;
  color: "neutral" | "amber" | "red";
  /** True for the first segment of a new subscription's group, so the
   * native side knows where to draw the hairline between groups. */
  groupStart: boolean;
}
