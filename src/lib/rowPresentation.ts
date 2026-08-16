import type { Subscription } from "../types/entities";
import { formatClockTime } from "./time";

/** What a subscription row shows besides its numbers: the badge, the one
 * action offered, and the footer note. */
export interface RowPresentation {
  badge: "Not current" | "Needs sign-in" | null;
  actionLabel: "Try again" | "Open Claude Code" | null;
  /** Replaces the "Read 2 min ago" line when there is something more
   * important to say. `null` leaves the row's normal footer alone. */
  footerNote: string | null;
}

/**
 * R3-4: one place decides what a row says, because the two rules that make
 * it truthful are rules *between* the pieces, and splitting them across the
 * component is what produced the contradiction the captain photographed —
 * a single row reading "Needs sign-in" / "The sign-in expired." / "Waiting
 * for the rate budget — available 4:59 PM" / "Retry at 4:59 PM" all at once.
 *
 * 1. **The wait and a sign-in are mutually exclusive.** A retry timer is
 *    meaningless on a row whose answer is "sign in" — no amount of waiting
 *    fixes it — so a row that needs signing in never shows the budget wait.
 * 2. **The time is stated once.** It used to appear in the footer note
 *    *and* in the action button's label, in the same row, in two different
 *    sentences. While a wait is pending there is nothing to press, so the
 *    action is dropped and the footer note carries the time alone.
 *
 * The badge follows the provider's `needsSignIn` classification rather than
 * `state === "broken"` — that shortcut is why an offline launch or an HTTP
 * 403 also accused the account of being signed out.
 */
export function rowPresentation(sub: Subscription, now: number): RowPresentation {
  const waitingUntil = pendingWaitUntil(sub, now);

  const badge = sub.state === "behind" ? "Not current" : sub.needsSignIn ? "Needs sign-in" : null;

  if (sub.signInInProgress) {
    // The row is showing the paste-code field; it owns the body and needs
    // no competing action of its own.
    return { badge, actionLabel: null, footerNote: null };
  }

  if (waitingUntil) {
    return {
      badge,
      actionLabel: null,
      footerNote: `Waiting for the rate budget — retry at ${formatClockTime(waitingUntil)}`,
    };
  }

  const actionLabel = sub.needsSignIn
    ? "Open Claude Code"
    : sub.state === "broken" || sub.state === "behind"
      ? "Try again"
      : null;

  return { badge, actionLabel, footerNote: null };
}

/** The wait, but only when it is genuinely the thing standing between the
 * user and a fresh read (rule 1 above). */
function pendingWaitUntil(sub: Subscription, now: number): string | null {
  if (sub.needsSignIn) return null;
  if (!sub.rateLimitedUntil) return null;
  return new Date(sub.rateLimitedUntil).getTime() > now ? sub.rateLimitedUntil : null;
}
