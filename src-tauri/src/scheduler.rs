//! Exactly one automatic read per account per minute, anchored to the last
//! attempt rather than a free-running timer.
//!
//! Scheduling lives here, in a plain OS-level timer in the Rust process,
//! rather than in a JS `setInterval` in `useSubscriptions.ts`. A JS timer
//! lives in the WKWebView, which is hidden whenever the panel is closed,
//! since the app starts hidden and only shows on a status item click. macOS and
//! WebKit suspend JS timers in an occluded webview: measured, an 8-second
//! interval produced zero ticks over 150 seconds while the window stayed
//! hidden and the process itself stayed alive and idle. An OS-level timer
//! has no notion of "hidden" at all.
//!
//! `fetch_snapshot` is the single call site for a real network attempt,
//! used both by this scheduler's loop and by the frontend's manual-refresh
//! command. `mark_attempted` runs from both places, so a manual refresh
//! resets the minute automatically, with nothing extra to wire up.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Exactly one automatic read per account per minute, always. Not
/// adaptive, never slower when idle.
pub const AUTO_REFRESH_INTERVAL: Duration = Duration::from_secs(60);

/// How long until the next automatic read, given what the last attempt
/// returned. A rate-limited `retry_after` under one minute is floored at
/// `AUTO_REFRESH_INTERVAL`: retrying earlier would spend another slot on
/// a guaranteed second failure.
fn next_wait(retry_after: Option<Duration>) -> Duration {
    retry_after
        .unwrap_or(AUTO_REFRESH_INTERVAL)
        .max(AUTO_REFRESH_INTERVAL)
}

pub struct Scheduler {
    next_due: Mutex<HashMap<String, Instant>>,
    pass_running: AtomicBool,
}

/// Exclusive ownership of the one running due-pass. The gate frees when
/// this drops, including on an early return or a panic in the pass.
pub struct PassGuard<'a> {
    scheduler: &'a Scheduler,
}

impl Drop for PassGuard<'_> {
    fn drop(&mut self) {
        self.scheduler.pass_running.store(false, Ordering::Release);
    }
}

impl Scheduler {
    pub fn new() -> Self {
        Self {
            next_due: Mutex::new(HashMap::new()),
            pass_running: AtomicBool::new(false),
        }
    }

    /// Claims the right to run a due-pass, or returns `None` if one is
    /// already running. Two independent entrants call `run_due_pass`: the
    /// periodic 5-second loop and the frontend's launch-time
    /// `kick_scheduler`. An account is only marked attempted after its
    /// fetch completes, so overlapping passes would both see the same
    /// account as due and fetch it twice, spending two slots of the shared
    /// 5-per-300s budget on one read. The overlap is realistic. A fetch may
    /// first run a bounded-20s CLI credential renewal, the ordinary
    /// roughly 8-hour token expiry, so the kicked pass can still be
    /// mid-fetch when the loop's own tick arrives. The loser skips rather
    /// than waits, since whatever is due is already the running pass's
    /// job, and anything that becomes due later is at most one 5-second
    /// tick away.
    pub fn begin_pass(&self) -> Option<PassGuard<'_>> {
        self.pass_running
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
            .then_some(PassGuard { scheduler: self })
    }

    /// Whether `account_id` is due for an automatic read right now. An
    /// account never seen before is due immediately, which covers both a
    /// fresh install and a newly-tracked account.
    pub fn is_due(&self, account_id: &str) -> bool {
        let next_due = self.next_due.lock().expect("scheduler mutex poisoned");
        match next_due.get(account_id) {
            Some(&at) => Instant::now() >= at,
            None => true,
        }
    }

    /// Records that `account_id` was just attempted. The scheduler loop and
    /// a manual refresh both funnel through the same call site. The next
    /// automatic read is 60 seconds out from now, not from whenever it was
    /// originally supposed to happen. This is what anchored to the last
    /// attempt means in practice: a manual refresh or the scheduler's own
    /// tick always pushes the next read a full minute out.
    ///
    /// `retry_after` overrides the plain 60-second wait when the attempt
    /// came back rate-limited. There is no point retrying before the
    /// budget frees up, and retrying right at 60 seconds into a
    /// still-active rate limit would spend another slot on a guaranteed
    /// second failure.
    pub fn mark_attempted(&self, account_id: &str, retry_after: Option<Duration>) {
        let wait = next_wait(retry_after);
        let mut next_due = self.next_due.lock().expect("scheduler mutex poisoned");
        next_due.insert(account_id.to_string(), Instant::now() + wait);
    }

    /// Drops bookkeeping for ids no longer tracked, so stopping and later
    /// re-adding the same account starts its schedule fresh instead of
    /// inheriting a stale wait from before it was removed.
    pub fn retain(&self, live_ids: &HashSet<String>) {
        let mut next_due = self.next_due.lock().expect("scheduler mutex poisoned");
        next_due.retain(|id, _| live_ids.contains(id));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unseen_account_is_due_immediately() {
        let scheduler = Scheduler::new();
        assert!(scheduler.is_due("claude:claude"));
    }

    #[test]
    fn right_after_an_attempt_it_is_not_due_again() {
        let scheduler = Scheduler::new();
        scheduler.mark_attempted("claude:claude", None);
        assert!(!scheduler.is_due("claude:claude"));
    }

    #[test]
    fn next_wait_uses_retry_after_when_it_exceeds_the_plain_minute() {
        assert_eq!(
            next_wait(Some(Duration::from_secs(214))),
            Duration::from_secs(214)
        );
    }

    #[test]
    fn next_wait_floors_a_short_retry_after_at_one_minute() {
        assert_eq!(
            next_wait(Some(Duration::from_secs(5))),
            AUTO_REFRESH_INTERVAL
        );
    }

    #[test]
    fn next_wait_with_no_retry_after_is_the_plain_minute() {
        assert_eq!(next_wait(None), AUTO_REFRESH_INTERVAL);
    }

    #[test]
    fn retain_drops_untracked_accounts_bookkeeping() {
        let scheduler = Scheduler::new();
        scheduler.mark_attempted("claude:claude", None);
        scheduler.mark_attempted("claude:team", None);
        assert!(!scheduler.is_due("claude:claude"));

        let live: HashSet<String> = ["claude:team".to_string()].into_iter().collect();
        scheduler.retain(&live);

        // Dropped from bookkeeping. Due again immediately, as if new.
        assert!(scheduler.is_due("claude:claude"));
        // Still tracked, still not due.
        assert!(!scheduler.is_due("claude:team"));
    }

    #[test]
    fn different_accounts_are_scheduled_independently() {
        let scheduler = Scheduler::new();
        scheduler.mark_attempted("claude:claude", None);
        assert!(!scheduler.is_due("claude:claude"));
        assert!(scheduler.is_due("claude:team"));
    }

    /// The double-spend guard. While one due-pass runs, a second entrant,
    /// such as the launch kick racing the periodic tick, must be refused,
    /// or both would fetch the same still-unmarked account and spend two
    /// budget slots on one read.
    #[test]
    fn a_second_pass_is_refused_while_one_is_running() {
        let scheduler = Scheduler::new();
        let running = scheduler.begin_pass();
        assert!(running.is_some());
        assert!(scheduler.begin_pass().is_none());
    }

    /// The gate frees when the pass guard drops, so passes gate on "one at
    /// a time", never "one ever".
    #[test]
    fn the_pass_gate_frees_when_the_guard_drops() {
        let scheduler = Scheduler::new();
        drop(scheduler.begin_pass());
        assert!(scheduler.begin_pass().is_some());
    }
}
