//! Support that the workspace's tests share; no product uses it.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

/// A path under the system's temporary directory that is this test's alone: `name`, after the
/// process id and a counter of this process (specification 22, "Native test and docs-capture
/// lifecycle portability"). Tests run at once in one process and in several, so a name made unique
/// by a clock stamp alone gave two of them one path. The unique part comes first, so a file name
/// keeps its extension. The path is not created.
#[must_use]
pub fn unique_temp_path(name: &str) -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    std::env::temp_dir().join(format!(
        "{}-{}-{name}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ))
}

#[cfg(test)]
mod tests {
    use super::unique_temp_path;

    /// Two paths of one name in one process are two, and a file name keeps its extension. Catches a
    /// path that two tests of one name share, which the first to finish removed under the other.
    #[test]
    fn two_paths_of_one_name_are_two() {
        let first = unique_temp_path("trust-test-support-log.jsonl");
        let second = unique_temp_path("trust-test-support-log.jsonl");
        assert_ne!(first, second);
        assert_eq!(
            first.extension().and_then(|ext| ext.to_str()),
            Some("jsonl")
        );
        assert_eq!(first.parent(), Some(std::env::temp_dir().as_path()));
    }
}
