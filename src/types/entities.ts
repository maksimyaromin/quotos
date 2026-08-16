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
 * headline one, so a 20% headline with a session window nearly exhausted
 * still reads as amber. `critical` when any window is at least 90% used,
 * `warn` when any window is at least 75% used, otherwise `healthy`. These
 * thresholds mirror the design system's `--cap-critical` and `--cap-warn`
 * tokens and must not diverge from them. */
export type Severity = "healthy" | "warn" | "critical";

export interface Subscription {
  id: string;
  provider: string;
  providerName: string;
  label: string;
  /** Set once the user has renamed the subscription inline. Overrides the
   * provider-derived label whenever present. */
  labelOverride: string | null;
  account: string | null;
  state: SubscriptionState;
  /** Provider-computed from every window. Drives the color of the headline
   * number, its bar, and the tray digits, never the headline percentage's
   * own magnitude. A failed or rate-limited read leaves it untouched. Only
   * a successful read updates it, the same as `used` and `windows` below. */
  severity: Severity;
  /** Percent of the headline window consumed, 0 to 100. The headline is the
   * account-wide weekly window, not simply the most-consumed one. Selection
   * is provider-owned, see `providers/claude/normalizeUsage.ts`. */
  used: number | null;
  resetsAt: string | null;
  lastReadAt: string | null;
  windows: LimitWindowEntity[];
  reason: string | null;
  /** Whether signing in is genuinely what this subscription needs,
   * provider-classified, see `providers/claude/index.ts`'s `mapOutcome`.
   * Deliberately separate from `state`, since `broken` alone driving the
   * "Needs sign-in" badge would make an offline launch or an HTTP 403 look
   * like a signed-out account. This flag also silences the rate-budget
   * wait, because a retry timer is meaningless on a row whose answer is
   * sign in. */
  needsSignIn: boolean;
  /** Pinning is per limit window, not per subscription. This is the
   * persisted set of pinned window ids, see `LimitWindowEntity.id`, any
   * number from any number of subscriptions. Order does not matter here.
   * The tray's own figure order comes from `windows`' own order, filtered
   * to whichever ids are in this list, see `lib/traySegments.ts`. */
  pinnedWindowIds: string[];
  /** Which of this subscription's current `windows` is the headline, the
   * same window `used` and `resetsAt` are drawn from. Recomputed by the
   * provider on every read, see `NormalizedRead.headlineWindowId`. Null
   * before any read, or when a read had no windows to point at. This is
   * what the "…" menu's "Show/Hide in menu bar" toggles: it pins or unpins
   * this one window id rather than the whole subscription. */
  headlineWindowId: string | null;
  configDir: string;
  /** Set while the app's own rate budget, not the subscription's health, is
   * the only thing blocking a read. Null once it is free to poll again.
   * This must never replace `state`. It is rendered as a quiet fact
   * alongside whatever health state already holds. */
  rateLimitedUntil: string | null;
  /** A `claude setup-token` session is running for this account.
   * Shell-owned UI state, not provider data, see `signin.rs`. Drives
   * whether the panel shows the code-paste field for this row. */
  signInInProgress: boolean;
  /** Set while "Stop tracking" has been pressed and the undo window is
   * still open. The account is already untracked as far as every consumer,
   * including persistence, the tray and the Subscriptions screen, is
   * concerned. This flag exists only so the panel can keep the row's slot
   * in the list and draw the Undo row there. Never persisted, see
   * `useSubscriptions`'s save effect, which filters on it. */
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
   * dir, attached by the Rust side on every read attempt, manual or
   * scheduled. Null or absent whenever nothing was ever installed, no
   * session has fed it yet, or the feed file is stale or unreadable. See
   * `providers/claude/statuslineMerge.ts` for how this reconciles with
   * `usage`: the freshest reading wins, and it never invents a window the
   * API did not already report. */
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
 * way it applies a manual refresh's direct result. It just arrives as an
 * event instead of an `invoke` return value. */
export type ScheduledRefreshEvent =
  | { kind: "ok"; snapshot: RawSnapshot }
  | { kind: "err"; account_id: string; error: FetchError };

/** Pushed once when a `start_sign_in` session ends, meaning the `claude
 * setup-token` process exited, so the panel can re-read the account and
 * drop out of the waiting-for-a-pasted-code UI. `success` only reflects the
 * process's own exit status. Quotos never inspects the credential itself,
 * so the row's next read is still the real proof either way. */
export interface SignInFinishedEvent {
  account_id: string;
  success: boolean;
}

/** One colored piece of the tray title. `tray-icon` v0.24.2's macOS
 * `set_title` takes a plain string with no color channel, verified by
 * reading the crate's `platform_impl/macos/mod.rs`. The real implementation
 * composites a bitmap on the Rust side in `src-tauri/src/tray_render.rs` so
 * each pinned window's digits can carry their own color. The mock client
 * has no real tray to update, so it is a no-op there instead of a stub
 * export. One figure exists per pinned window, not per pinned subscription.
 * `lib/traySegments.ts` builds this list in panel order, rows top to
 * bottom, then in each subscription's own window order, left to right,
 * matching the bar's own order in the panel. */
export interface TraySegment {
  text: string;
  color: "neutral" | "amber" | "red";
  /** True for the first segment of a new subscription's group. The Rust
   * side in `tray_render.rs` draws its hairline immediately before any
   * segment with this set, never before the very first segment overall. */
  groupStart: boolean;
}
