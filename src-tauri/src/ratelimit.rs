use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Serialize;

/// Whether a reservation this old has aged out of the window. An entry
/// exactly one window old counts as expired, not one instant later:
/// otherwise five reservations spaced evenly across the window never
/// admit a sixth once the window is full.
fn is_expired(age: Duration, window: Duration) -> bool {
    age >= window
}

/// One account's current standing against the shared request budget. The
/// `debug_rate_limit_snapshot` command in `accounts.rs` exposes this to the
/// frontend's developer-only state dump.
#[derive(Serialize, Clone, Debug)]
pub struct RateLimitStatus {
    pub used: usize,
    pub max: usize,
    /// `None` when the account is under budget. `Some(secs)` when the
    /// account is currently blocked, naming how long until the oldest
    /// reservation ages out.
    pub retry_after_secs: Option<u64>,
}

/// Tracks each account's standing in a sliding window against
/// `max_requests` per `window`.
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
            if is_expired(now.duration_since(front), self.window) {
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

    /// Returns a read-only snapshot of every account's current standing.
    /// Prunes expired entries first, the same way `try_acquire` does, but
    /// never reserves a slot.
    pub fn snapshot(&self) -> HashMap<String, RateLimitStatus> {
        let mut windows = self.windows.lock().expect("ratelimit mutex poisoned");
        let now = Instant::now();
        let mut out = HashMap::new();
        for (key, entry) in windows.iter_mut() {
            while let Some(&front) = entry.front() {
                if is_expired(now.duration_since(front), self.window) {
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

    /// Back-dates every reservation for `key` by `age`, so the
    /// window-boundary behavior can be exercised without a real sleep.
    #[cfg(test)]
    fn age_entries_by(&self, key: &str, age: Duration) {
        let mut windows = self.windows.lock().expect("ratelimit mutex poisoned");
        if let Some(entry) = windows.get_mut(key) {
            for instant in entry.iter_mut() {
                *instant -= age;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_entry_exactly_one_window_old_is_expired() {
        let window = Duration::from_secs(300);
        assert!(is_expired(window, window));
        assert!(!is_expired(window - Duration::from_nanos(1), window));
    }

    #[test]
    fn admits_a_sixth_request_once_the_oldest_is_exactly_one_window_old() {
        let limiter = RateLimiter::new(5, Duration::from_secs(300));
        for _ in 0..5 {
            limiter
                .try_acquire("acct")
                .expect("first five requests must be admitted");
        }
        assert!(
            limiter.try_acquire("acct").is_err(),
            "sixth request with a full, fresh window must be refused"
        );

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
        assert!(
            limiter.try_acquire("acct").is_err(),
            "an entry not yet a full window old must still count against the budget"
        );
    }

    #[test]
    fn accounts_are_isolated_from_each_other() {
        let limiter = RateLimiter::new(5, Duration::from_secs(300));
        for _ in 0..5 {
            limiter.try_acquire("personal").unwrap();
        }
        assert!(limiter.try_acquire("personal").is_err());
        assert!(
            limiter.try_acquire("team").is_ok(),
            "a different account's budget must be untouched"
        );
    }

    #[test]
    fn snapshot_reports_the_same_boundary_as_try_acquire() {
        let limiter = RateLimiter::new(5, Duration::from_secs(300));
        for _ in 0..5 {
            limiter.try_acquire("acct").unwrap();
        }
        limiter.age_entries_by("acct", Duration::from_secs(300));
        let snap = limiter.snapshot();
        assert_eq!(
            snap["acct"].used, 0,
            "snapshot must prune the same way try_acquire does"
        );
        assert!(snap["acct"].retry_after_secs.is_none());
    }
}
