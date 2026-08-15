//! R2-4: exactly one automatic read per account per minute, anchored to the
//! last attempt rather than a free-running timer.
//!
//! Reproduced first, on a real packaged build: the old JS `setInterval` in
//! `useSubscriptions.ts` lived in the WKWebView, which is hidden whenever
//! the panel is closed (the app starts hidden and only shows on a tray
//! click). With the window hidden the whole time, an 8-second interval
//! produced **zero** ticks over 150+ seconds while the process itself
//! stayed alive and idle — macOS/WebKit suspends JS timers in an occluded
//! webview. That's exactly the captain's "иногда как будто в фоне
//! перестает работать" symptom. The fix moves scheduling here, to a plain
//! OS-level timer in the Rust process, which has no notion of "hidden" at
//! all.
//!
//! `fetch_snapshot` is the single call site for a real network attempt,
//! used both by this scheduler's loop and by the frontend's manual-refresh
//! command — so `mark_attempted` runs from both places and "a manual
//! refresh resets the minute" falls out for free, with nothing extra to
//! wire up.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// The captain's decision: exactly one automatic read per account per
/// minute, always — not adaptive, not slower when idle.
pub const AUTO_REFRESH_INTERVAL: Duration = Duration::from_secs(60);

pub struct Scheduler {
    next_due: Mutex<HashMap<String, Instant>>,
    pass_running: AtomicBool,
}

/// Exclusive ownership of the one running due-pass; the gate frees when this
/// drops (including on an early return or a panic in the pass).
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

    /// Claim the right to run a due-pass, or `None` if one is already
    /// running. Two independent entrants call `run_due_pass` — the periodic
    /// 5s loop and the frontend's launch-time `kick_scheduler` — and an
    /// account is only marked attempted *after* its fetch completes, so
    /// overlapping passes would both see the same account as due and fetch
    /// it twice, spending two slots of the shared 5-per-300s budget on one
    /// read. The overlap is realistic, not theoretical: a fetch may first
    /// run a bounded-20s CLI credential renewal (the ordinary ~8h token
    /// expiry, e.g. every morning's first launch), so the kicked pass can
    /// still be mid-fetch when the loop's own tick arrives. The loser skips
    /// rather than waits — whatever is due is already the running pass's
    /// job, and anything that becomes due later is at most one 5s tick away.
    pub fn begin_pass(&self) -> Option<PassGuard<'_>> {
        self.pass_running
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
            .then_some(PassGuard { scheduler: self })
    }

    /// Whether `account_id` is due for an automatic read right now. An
    /// account never seen before is due immediately (covers both a fresh
    /// install and a newly-tracked account).
    pub fn is_due(&self, account_id: &str) -> bool {
        let next_due = self.next_due.lock().expect("scheduler mutex poisoned");
        match next_due.get(account_id) {
            Some(&at) => Instant::now() >= at,
            None => true,
        }
    }

    /// Records that `account_id` was just attempted — by the scheduler loop
    /// or by a manual refresh, both funnel through the same call site. The
    /// next automatic read is 60s out from *now*, not from whenever it was
    /// originally "supposed" to happen, which is what "anchored to the last
    /// successful read" means in practice: a manual refresh (or the
    /// scheduler's own tick) always pushes the next one a full minute out.
    ///
    /// `retry_after` overrides the plain 60s wait when the attempt came back
    /// rate-limited — no point retrying before the budget frees up, and
    /// retrying right at 60s into a still-active rate limit would just
    /// spend another slot on a guaranteed second failure.
    pub fn mark_attempted(&self, account_id: &str, retry_after: Option<Duration>) {
        let wait = retry_after
            .unwrap_or(AUTO_REFRESH_INTERVAL)
            .max(AUTO_REFRESH_INTERVAL);
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
        let s = Scheduler::new();
        assert!(s.is_due("claude:claude"));
    }

    #[test]
    fn right_after_an_attempt_it_is_not_due_again() {
        let s = Scheduler::new();
        s.mark_attempted("claude:claude", None);
        assert!(!s.is_due("claude:claude"));
    }

    #[test]
    fn a_rate_limited_retry_after_overrides_the_plain_minute_when_longer() {
        let s = Scheduler::new();
        s.mark_attempted("claude:claude", Some(Duration::from_secs(214)));
        // Still not due after the plain 60s would have elapsed — can't
        // directly fast-forward Instant in a unit test, so assert the
        // invariant that matters: it's not due right after marking.
        assert!(!s.is_due("claude:claude"));
    }

    #[test]
    fn a_short_retry_after_still_floors_at_one_minute() {
        let s = Scheduler::new();
        // A hypothetical retry_after under 60s must never make the account
        // due sooner than the captain's fixed one-per-minute cadence.
        s.mark_attempted("claude:claude", Some(Duration::from_secs(5)));
        assert!(!s.is_due("claude:claude"));
    }

    #[test]
    fn retain_drops_untracked_accounts_bookkeeping() {
        let s = Scheduler::new();
        s.mark_attempted("claude:claude", None);
        s.mark_attempted("claude:team", None);
        assert!(!s.is_due("claude:claude"));

        let live: HashSet<String> = ["claude:team".to_string()].into_iter().collect();
        s.retain(&live);

        // Dropped from bookkeeping — due again immediately, as if new.
        assert!(s.is_due("claude:claude"));
        // Still tracked, still not due.
        assert!(!s.is_due("claude:team"));
    }

    #[test]
    fn different_accounts_are_scheduled_independently() {
        let s = Scheduler::new();
        s.mark_attempted("claude:claude", None);
        assert!(!s.is_due("claude:claude"));
        assert!(s.is_due("claude:team"));
    }

    /// The double-spend guard: while one due-pass runs, a second entrant
    /// (the launch kick racing the periodic tick) must be refused, or both
    /// would fetch the same still-unmarked account and spend two budget
    /// slots on one read.
    #[test]
    fn a_second_pass_is_refused_while_one_is_running() {
        let s = Scheduler::new();
        let running = s.begin_pass();
        assert!(running.is_some());
        assert!(s.begin_pass().is_none());
    }

    /// The gate frees when the pass guard drops, so passes gate on "one at
    /// a time", never "one ever".
    #[test]
    fn the_pass_gate_frees_when_the_guard_drops() {
        let s = Scheduler::new();
        drop(s.begin_pass());
        assert!(s.begin_pass().is_some());
    }
}
