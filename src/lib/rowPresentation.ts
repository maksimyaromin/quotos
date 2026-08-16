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
 * One place decides what a row says, because the two rules that keep it
 * truthful are rules between the pieces rather than facts any single piece
 * can enforce alone.
 *
 * 1. **The wait and a sign-in are mutually exclusive.** A retry timer is
 *    meaningless on a row whose answer is "sign in", since no amount of
 *    waiting fixes it, so a row that needs signing in never shows the
 *    budget wait.
 * 2. **The time is stated once.** A pending wait states its reset time only
 *    in the footer note. While a wait is pending there is nothing to
 *    press, so the action is dropped.
 *
 * The badge follows the provider's `needsSignIn` classification rather than
 * `state === "broken"`, because that shortcut would also accuse an offline
 * launch or an HTTP 403 of being signed out.
 */
export function rowPresentation(sub: Subscription, now: number): RowPresentation {
  const waitingUntil = pendingWaitUntil(sub, now);

  const badge = sub.state === "behind" ? "Not current" : sub.needsSignIn ? "Needs sign-in" : null;

  if (sub.signInInProgress) {
    // The row is showing the paste-code field. It owns the body and needs
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
 * user and a fresh read. See rule 1 above. */
function pendingWaitUntil(sub: Subscription, now: number): string | null {
  if (sub.needsSignIn) return null;
  if (!sub.rateLimitedUntil) return null;
  return new Date(sub.rateLimitedUntil).getTime() > now ? sub.rateLimitedUntil : null;
}
