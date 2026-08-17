use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

pub const AUTO_REFRESH_INTERVAL: Duration = Duration::from_secs(60);

fn next_wait(retry_after: Option<Duration>) -> Duration {
    retry_after
        .unwrap_or(AUTO_REFRESH_INTERVAL)
        .max(AUTO_REFRESH_INTERVAL)
}

pub struct Scheduler {
    next_due: Mutex<HashMap<String, Instant>>,
    pass_running: AtomicBool,
}

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

    pub fn begin_pass(&self) -> Option<PassGuard<'_>> {
        self.pass_running
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
            .then_some(PassGuard { scheduler: self })
    }

    pub fn is_due(&self, account_id: &str) -> bool {
        let next_due = self.next_due.lock().expect("scheduler mutex poisoned");
        match next_due.get(account_id) {
            Some(&at) => Instant::now() >= at,
            None => true,
        }
    }

    pub fn mark_attempted(&self, account_id: &str, retry_after: Option<Duration>) {
        let wait = next_wait(retry_after);
        let mut next_due = self.next_due.lock().expect("scheduler mutex poisoned");
        next_due.insert(account_id.to_string(), Instant::now() + wait);
    }

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

        assert!(scheduler.is_due("claude:claude"));
        assert!(!scheduler.is_due("claude:team"));
    }

    #[test]
    fn different_accounts_are_scheduled_independently() {
        let scheduler = Scheduler::new();
        scheduler.mark_attempted("claude:claude", None);
        assert!(!scheduler.is_due("claude:claude"));
        assert!(scheduler.is_due("claude:team"));
    }

    #[test]
    fn a_second_pass_is_refused_while_one_is_running() {
        let scheduler = Scheduler::new();
        let running = scheduler.begin_pass();
        assert!(running.is_some());
        assert!(scheduler.begin_pass().is_none());
    }

    #[test]
    fn the_pass_gate_frees_when_the_guard_drops() {
        let scheduler = Scheduler::new();
        drop(scheduler.begin_pass());
        assert!(scheduler.begin_pass().is_some());
    }
}
