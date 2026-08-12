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
            if now.duration_since(front) > self.window {
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
                if now.duration_since(front) > self.window {
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
}
