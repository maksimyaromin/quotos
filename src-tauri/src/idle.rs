use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

pub(crate) const AUTO_READ_PAUSE_THRESHOLD: Duration = Duration::from_secs(600);

pub(crate) fn should_pause_automatic_reads(
    idle: Duration,
    screen_locked: bool,
    threshold: Duration,
) -> bool {
    screen_locked || idle >= threshold
}

#[cfg(target_os = "macos")]
pub(crate) fn system_idle_seconds() -> Duration {
    use objc2_core_graphics::{CGEventSource, CGEventSourceStateID, CGEventType};

    // kCGAnyInputEventType has no named constant in objc2-core-graphics;
    // Apple's own header defines it as `(CGEventType)~0`.
    let secs = CGEventSource::seconds_since_last_event_type(
        CGEventSourceStateID::HIDSystemState,
        CGEventType(u32::MAX),
    );
    Duration::from_secs_f64(secs.max(0.0))
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn system_idle_seconds() -> Duration {
    Duration::ZERO
}

/// Keeps `screen_locked` in sync with macOS's screen-lock notifications.
/// The two observer tokens are deliberately leaked: a block-based
/// observer stops firing once its own returned token is deallocated.
#[cfg(target_os = "macos")]
pub(crate) fn watch_screen_lock_state(screen_locked: Arc<AtomicBool>) {
    use core::ptr::NonNull;
    use objc2_foundation::{NSDistributedNotificationCenter, NSNotification, ns_string};

    let center = NSDistributedNotificationCenter::defaultCenter();

    let locked_flag = screen_locked.clone();
    let on_lock = block2::RcBlock::new(move |_note: NonNull<NSNotification>| {
        locked_flag.store(true, Ordering::Relaxed);
    });
    let on_unlock = block2::RcBlock::new(move |_note: NonNull<NSNotification>| {
        screen_locked.store(false, Ordering::Relaxed);
    });

    unsafe {
        let lock_token = center.addObserverForName_object_queue_usingBlock(
            Some(ns_string!("com.apple.screenIsLocked")),
            None,
            None,
            &on_lock,
        );
        let unlock_token = center.addObserverForName_object_queue_usingBlock(
            Some(ns_string!("com.apple.screenIsUnlocked")),
            None,
            None,
            &on_unlock,
        );
        std::mem::forget(lock_token);
        std::mem::forget(unlock_token);
    }
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn watch_screen_lock_state(_screen_locked: Arc<AtomicBool>) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ten_minutes_idle_pauses_even_when_unlocked() {
        assert!(should_pause_automatic_reads(
            Duration::from_secs(600),
            false,
            AUTO_READ_PAUSE_THRESHOLD
        ));
    }

    #[test]
    fn just_under_the_threshold_does_not_pause() {
        assert!(!should_pause_automatic_reads(
            Duration::from_secs(599),
            false,
            AUTO_READ_PAUSE_THRESHOLD
        ));
    }

    #[test]
    fn a_locked_screen_pauses_regardless_of_idle_time() {
        assert!(should_pause_automatic_reads(
            Duration::ZERO,
            true,
            AUTO_READ_PAUSE_THRESHOLD
        ));
    }

    #[test]
    fn active_and_unlocked_never_pauses() {
        assert!(!should_pause_automatic_reads(
            Duration::ZERO,
            false,
            AUTO_READ_PAUSE_THRESHOLD
        ));
    }
}
