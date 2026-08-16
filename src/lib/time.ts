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

// Intl.DateTimeFormat falls back to the system locale when none is given.
// Every formatter in this module is built with an explicit locale instead,
// so the app's output stays in English regardless of the machine it runs on.
const LOCALE = "en-US";

const TIME_FMT = new Intl.DateTimeFormat(LOCALE, { hour: "numeric", minute: "2-digit" });
const WEEKDAY_FMT = new Intl.DateTimeFormat(LOCALE, { weekday: "short" });
const DATE_FMT = new Intl.DateTimeFormat(LOCALE, { month: "short", day: "numeric" });

function isSameDay(a: Date, b: Date): boolean {
  return (
    a.getFullYear() === b.getFullYear() &&
    a.getMonth() === b.getMonth() &&
    a.getDate() === b.getDate()
  );
}

/** Bare clock time, for example "4:05 PM". Used for compact labels such
 * as a disabled refresh control's "available at …" tooltip. */
export function formatClockTime(iso: string | null): string | null {
  if (!iso) return null;
  const then = new Date(iso);
  if (Number.isNaN(then.getTime())) return null;
  return TIME_FMT.format(then);
}

/** The exact moment a window resets, not a relative offset the caller has to
 * do arithmetic on. Formats as "Today at 4:05 PM", "Tomorrow at 10:00 AM",
 * "Wed at 10:00 AM" or "Aug 17 at 10:00 AM" depending on how far out the
 * reset is, close enough to need the day named and far enough to need the
 * date. */
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
  if (daysAway < 7) return `Resets ${WEEKDAY_FMT.format(then)} at ${time}`;
  return `Resets ${DATE_FMT.format(then)} at ${time}`;
}
