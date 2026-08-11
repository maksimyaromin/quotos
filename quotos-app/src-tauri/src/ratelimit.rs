use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;
use std::time::{Duration, Instant};

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
}
