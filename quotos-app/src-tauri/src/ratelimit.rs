use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Serialize;

/// One account's current standing against the budget — the read-only view
/// exposed to the dev state dump (see `debug_state.rs`).
#[derive(Serialize, Clone, Debug)]
pub struct RateLimitStatus {
    pub used: usize,
    pub max: usize,
    /// `None` when under budget; `Some(secs)` when the account is currently
    /// blocked and this is how long until the oldest reservation ages out.
    pub retry_after_secs: Option<u64>,
}

/// Per-account sliding-window limiter matching the measured provider limit:
/// 5 requests per 300 seconds, shared with the official client. One
/// instance guards the frontier so refresh timing lives in exactly one
/// place, not scattered across timers.
pub struct RateLimiter {
    windows: Mutex<HashMap<String, VecDeque<Instant>>>,
    max_requests: usize,
    window: Duration,
}

impl RateLimiter {
    pub fn new(max_requests: usize, window: Duration) -> Self {
        Self {
            windows: Mutex::new(HashMap::new()),
            max_requests,
            window,
        }
    }

    /// Returns `Ok(())` and reserves a slot if under budget, or
    /// `Err(retry_after_secs)` if the account is at capacity.
    pub fn try_acquire(&self, key: &str) -> Result<(), u64> {
        let mut windows = self.windows.lock().expect("ratelimit mutex poisoned");
        let now = Instant::now();
        let entry = windows.entry(key.to_string()).or_default();
        while let Some(&front) = entry.front() {
            // R2-4: >= , not > . At exactly the window boundary (t=300s for
            // five 60-second-spaced entries under the new 1/min schedule),
            // the oldest entry has fully aged out and must be pruned so the
            // 6th request is admitted — with a strict `>` it stays counted
            // for one more instant and the read is wrongly refused, which
            // would make our own limiter fight our own schedule.
            if now.duration_since(front) >= self.window {
                entry.pop_front();
            } else {
                break;
            }
        }
        if entry.len() >= self.max_requests {
            let oldest = *entry.front().expect("len >= max_requests > 0");
            let elapsed = now.duration_since(oldest);
            let retry_after = self.window.saturating_sub(elapsed).as_secs().max(1);
            return Err(retry_after);
        }
        entry.push_back(now);
        Ok(())
    }

    /// Read-only snapshot of every account's current standing, for the dev
    /// state dump. Prunes expired entries first so the count reflects
    /// reality, same as `try_acquire` — but never reserves a slot.
    pub fn snapshot(&self) -> HashMap<String, RateLimitStatus> {
        let mut windows = self.windows.lock().expect("ratelimit mutex poisoned");
        let now = Instant::now();
        let mut out = HashMap::new();
        for (key, entry) in windows.iter_mut() {
            while let Some(&front) = entry.front() {
                // R2-4: mirrors the same >= fix in try_acquire, above.
                if now.duration_since(front) >= self.window {
                    entry.pop_front();
                } else {
                    break;
                }
            }
            let retry_after_secs = if entry.len() >= self.max_requests {
                let oldest = *entry.front().expect("len >= max_requests > 0");
                let elapsed = now.duration_since(oldest);
                Some(self.window.saturating_sub(elapsed).as_secs().max(1))
            } else {
                None
            };
            out.insert(
                key.clone(),
                RateLimitStatus {
                    used: entry.len(),
                    max: self.max_requests,
                    retry_after_secs,
                },
            );
        }
        out
    }

    /// Test-only: back-date every reservation for `key` by `age`, so the
    /// window-boundary behavior can be exercised without a real sleep.
    #[cfg(test)]
    fn age_entries_by(&self, key: &str, age: Duration) {
        let mut windows = self.windows.lock().expect("ratelimit mutex poisoned");
        if let Some(entry) = windows.get_mut(key) {
            for instant in entry.iter_mut() {
                *instant = *instant - age;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// R2-4: five requests spaced exactly 60s apart (the new schedule) must
    /// not deadlock the budget — once the oldest is exactly 300s old, it
    /// must be prunable so a 6th request is admitted. This is the exact
    /// off-by-one the brief called out: with a strict `>` prune condition,
    /// the oldest entry at precisely t=300s is not yet pruned and the read
    /// is wrongly refused.
    #[test]
    fn admits_a_sixth_request_once_the_oldest_is_exactly_one_window_old() {
        let limiter = RateLimiter::new(5, Duration::from_secs(300));
        for _ in 0..5 {
            limiter.try_acquire("acct").expect("first five requests must be admitted");
        }
        assert!(limiter.try_acquire("acct").is_err(), "sixth request with a full, fresh window must be refused");

        limiter.age_entries_by("acct", Duration::from_secs(300));
        assert!(
            limiter.try_acquire("acct").is_ok(),
            "once the oldest reservation is exactly one window old it must be pruned and the request admitted"
        );
    }

    #[test]
    fn a_reservation_one_second_short_of_the_window_still_counts() {
        let limiter = RateLimiter::new(5, Duration::from_secs(300));
        for _ in 0..5 {
            limiter.try_acquire("acct").unwrap();
        }
        limiter.age_entries_by("acct", Duration::from_secs(299));
        assert!(limiter.try_acquire("acct").is_err(), "an entry not yet a full window old must still count against the budget");
    }

    #[test]
    fn accounts_are_isolated_from_each_other() {
        let limiter = RateLimiter::new(5, Duration::from_secs(300));
        for _ in 0..5 {
            limiter.try_acquire("personal").unwrap();
        }
        assert!(limiter.try_acquire("personal").is_err());
        assert!(limiter.try_acquire("team").is_ok(), "a different account's budget must be untouched");
    }

    #[test]
    fn snapshot_reports_the_same_boundary_as_try_acquire() {
        let limiter = RateLimiter::new(5, Duration::from_secs(300));
        for _ in 0..5 {
            limiter.try_acquire("acct").unwrap();
        }
        limiter.age_entries_by("acct", Duration::from_secs(300));
        let snap = limiter.snapshot();
        assert_eq!(snap["acct"].used, 0, "snapshot must prune the same way try_acquire does");
        assert!(snap["acct"].retry_after_secs.is_none());
    }
}
