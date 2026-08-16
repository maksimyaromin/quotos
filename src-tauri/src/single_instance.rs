//! One running Quotos per machine, enforced with an OS file lock, since
//! the shared per-account request budget has no cross-instance
//! coordination. See docs/architecture.md.

use std::fs::{self, File, TryLockError};
use std::io;
use std::path::Path;

/// The file's presence on disk means nothing; only the live OS lock on it
/// does, so this file is never cleaned up.
const LOCK_FILE_NAME: &str = "instance.lock";

pub(crate) enum Claim {
    /// The lock lives exactly as long as this handle stays open, so the
    /// caller must keep it for the whole process lifetime.
    Held(File),
    TakenByOther,
    /// For example an unwritable directory or a filesystem without flock.
    /// Refusing to launch over an optional protection would cost more than
    /// a double-spent budget, so the caller should run anyway.
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
            let count = COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "quotos-single-instance-test-{}-{count}",
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

    /// `flock` locks belong to the open file description, so a second open
    /// and lock conflicts even from the same process, which is what lets
    /// this test model two instances.
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
        let obstruction = dir.path.join("file");
        fs::write(&obstruction, "x").unwrap();
        assert!(matches!(
            claim(&obstruction.join("sub")),
            Claim::Unavailable(_)
        ));
    }
}
