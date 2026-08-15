//! One running Quotos per machine, enforced with an OS file lock.
//!
//! Why it matters: the `/api/oauth/usage` allowance is 5 requests per 300s
//! *per account*, shared with Claude Code itself (see `ratelimit.rs`), and
//! every instance runs its own limiter with no knowledge of its siblings —
//! two live instances each believe the whole allowance is theirs and spend
//! it double-speed until the provider answers 429. The realistic way to end
//! up with two is not double-clicking the bundle (Launch Services activates
//! the running copy instead) but a dev run (`npm run tauri dev`) beside an
//! installed build, or a duplicated `.app` — all sharing one bundle
//! identifier and therefore one config dir, which is exactly where this
//! lock lives.
//!
//! Why `File::try_lock` (`flock`) and not a pid file: the kernel releases
//! the lock when the owning process dies, however it dies — there is no
//! stale-lockfile state to detect or repair, and no pid reuse to misread.
//! And why not `tauri-plugin-single-instance`: this needs no second-launch
//! argv forwarding (a tray app has no window to focus), and std has owned
//! file locking since 1.89 — the plugin would be a dependency for one
//! syscall.

use std::fs::{self, File, TryLockError};
use std::io;
use std::path::Path;

/// The lock file's name inside the app's own config dir (next to
/// `tracked.json`). Its presence on disk means nothing — only the live OS
/// lock on it does — so it is never cleaned up.
const LOCK_FILE_NAME: &str = "instance.lock";

pub(crate) enum Claim {
    /// This process holds the instance lock now. The lock lives exactly as
    /// long as the contained handle stays open: keep it for the process's
    /// lifetime, or the guarantee silently ends.
    Held(File),
    /// Another live process holds it — a Quotos is already running.
    TakenByOther,
    /// The lock could be neither taken *nor* refused (unwritable dir, a
    /// filesystem without `flock`). The caller should run anyway: this
    /// guard protects the shared rate budget, and refusing to launch over
    /// an optional protection would cost more than a double-spent budget.
    Unavailable(io::Error),
}

pub(crate) fn claim(dir: &Path) -> Claim {
    let open = fs::create_dir_all(dir).and_then(|()| {
        fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .open(dir.join(LOCK_FILE_NAME))
    });
    let file = match open {
        Ok(file) => file,
        Err(err) => return Claim::Unavailable(err),
    };
    match file.try_lock() {
        Ok(()) => Claim::Held(file),
        Err(TryLockError::WouldBlock) => Claim::TakenByOther,
        Err(TryLockError::Error(err)) => Claim::Unavailable(err),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    struct TempDir {
        path: PathBuf,
    }

    impl TempDir {
        fn new() -> Self {
            let n = COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "quotos-single-instance-test-{}-{n}",
                std::process::id()
            ));
            fs::create_dir_all(&path).expect("create temp dir");
            Self { path }
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn the_first_claim_is_held() {
        let dir = TempDir::new();
        assert!(matches!(claim(&dir.path), Claim::Held(_)));
    }

    /// `flock` locks belong to the open file description, so a second
    /// open+lock conflicts even from the same process — which is what lets
    /// one test model two instances.
    #[test]
    fn a_second_claim_is_refused_while_the_first_lives_and_frees_with_it() {
        let dir = TempDir::new();
        let first = claim(&dir.path);
        assert!(matches!(first, Claim::Held(_)));
        assert!(matches!(claim(&dir.path), Claim::TakenByOther));
        drop(first);
        assert!(
            matches!(claim(&dir.path), Claim::Held(_)),
            "the lock must die with its holder"
        );
    }

    #[test]
    fn a_missing_config_dir_is_created_rather_than_failing() {
        let dir = TempDir::new();
        let nested = dir.path.join("nested").join("deeper");
        assert!(matches!(claim(&nested), Claim::Held(_)));
    }

    #[test]
    fn an_uncreatable_dir_reports_unavailable_never_taken() {
        let dir = TempDir::new();
        // A path with a regular file as a parent component can never be a dir.
        let obstruction = dir.path.join("file");
        fs::write(&obstruction, "x").unwrap();
        assert!(matches!(
            claim(&obstruction.join("sub")),
            Claim::Unavailable(_)
        ));
    }
}
