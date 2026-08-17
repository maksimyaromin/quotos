import type { Subscription } from '@/types/entities'
import { formatClockTime } from './time'

export interface RowPresentation {
  badge: 'Not current' | 'Needs sign-in' | null
  actionLabel: 'Try again' | 'Open Claude Code' | null
  footerNote: string | null
}

export function presentRow(sub: Subscription, now: number): RowPresentation {
  const waitingUntil = pendingWaitUntil(sub, now)

  const badge = sub.state === 'behind' ? 'Not current' : sub.needsSignIn ? 'Needs sign-in' : null

  if (sub.signInInProgress) {
    return { badge, actionLabel: null, footerNote: null }
  }

  if (waitingUntil) {
    return {
      badge,
      actionLabel: null,
      footerNote: `Waiting for the rate budget — retry at ${formatClockTime(waitingUntil)}`,
    }
  }

  const actionLabel = sub.needsSignIn
    ? 'Open Claude Code'
    : sub.state === 'broken' || sub.state === 'behind'
      ? 'Try again'
      : null

  return { badge, actionLabel, footerNote: null }
}

function pendingWaitUntil(sub: Subscription, now: number): string | null {
  if (sub.needsSignIn) return null
  if (!sub.rateLimitedUntil) return null
  return new Date(sub.rateLimitedUntil).getTime() > now ? sub.rateLimitedUntil : null
}
