use std::fs::{self, File, TryLockError};
use std::io;
use std::path::Path;

const LOCK_FILE_NAME: &str = "instance.lock";

pub(crate) enum Claim {
    Held(File),
    TakenByOther,
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
