/** "2 min ago" for an ISO timestamp in the past. */
export function formatRelativePast(iso: string | null, now: Date = new Date()): string | null {
  if (!iso) return null;
  const then = new Date(iso).getTime();
  if (Number.isNaN(then)) return null;
  const seconds = Math.max(0, Math.round((now.getTime() - then) / 1000));
  if (seconds < 10) return "just now";
  if (seconds < 60) return `${seconds}s ago`;
  const minutes = Math.round(seconds / 60);
  if (minutes < 60) return `${minutes} min ago`;
  const hours = Math.round(minutes / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.round(hours / 24);
  return `${days}d ago`;
}

// R2-7: "мне не нравится что я вижу Пн по русски" — every formatter here
// used to build with locale `undefined`, i.e. whatever the system locale
// is, which is Russian on the captain's Mac. The handoff's strings are
// fixed to English, 12-hour clock ("Resets today at 4:05 PM"), so the
// locale is pinned to `en-US` explicitly — the app's own language must not
// depend on the machine it runs on.
const LOCALE = "en-US";

const TIME_FMT = new Intl.DateTimeFormat(LOCALE, { hour: "numeric", minute: "2-digit" });
const WEEKDAY_TIME_FMT = new Intl.DateTimeFormat(LOCALE, {
  weekday: "short",
  hour: "numeric",
  minute: "2-digit",
});
const DATE_TIME_FMT = new Intl.DateTimeFormat(LOCALE, {
  month: "short",
  day: "numeric",
  hour: "numeric",
  minute: "2-digit",
});

function isSameDay(a: Date, b: Date): boolean {
  return a.getFullYear() === b.getFullYear() && a.getMonth() === b.getMonth() && a.getDate() === b.getDate();
}

/** Bare clock time, e.g. "4:05 PM" — used for compact labels like a
 * disabled refresh control's "available at …" tooltip. */
export function formatClockTime(iso: string | null): string | null {
  if (!iso) return null;
  const then = new Date(iso);
  if (Number.isNaN(then.getTime())) return null;
  return TIME_FMT.format(then);
}

/** I3: the exact moment a window resets, not a relative offset the person has
 * to do arithmetic on. "Today at 4:05 PM" / "Tomorrow at 10:00 AM" / "Wed at
 * 10:00 AM" / "Aug 17 at 10:00 AM" depending on how far out it is — close
 * enough to need the day named, far enough to need the date. */
export function formatExactReset(iso: string | null, now: Date = new Date()): string | null {
  if (!iso) return null;
  const then = new Date(iso);
  if (Number.isNaN(then.getTime())) return null;

  const time = TIME_FMT.format(then);
  if (then.getTime() <= now.getTime()) return `Resets shortly (${time})`;

  const tomorrow = new Date(now.getFullYear(), now.getMonth(), now.getDate() + 1);
  if (isSameDay(then, now)) return `Resets today at ${time}`;
  if (isSameDay(then, tomorrow)) return `Resets tomorrow at ${time}`;

  const daysAway = Math.round((then.getTime() - now.getTime()) / 86_400_000);
  if (daysAway < 7) return `Resets ${WEEKDAY_TIME_FMT.format(then)}`;
  return `Resets ${DATE_TIME_FMT.format(then)}`;
}
