//! Support that the workspace's tests share; no product uses it.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

/// A path under the system's temporary directory that is this test's alone: `name`, after this
/// process's tag and a counter of this process (specification 22, "Native test and docs-capture
/// lifecycle portability"). Tests run at once in one process and in several, so a name made unique
/// by a clock stamp alone gave two of them one path. The unique part comes first, so a file name
/// keeps its extension. The path is not created.
#[must_use]
pub fn unique_temp_path(name: &str) -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    std::env::temp_dir().join(format!(
        "{}-{}-{name}",
        process_tag(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ))
}

/// A path under the system's temporary directory for one name in this process: `name`, after this
/// process's tag. The same name gives the same path within the process, for a test that reopens what
/// it wrote, as across a restart, or that names one path twice; processes keep apart by their tags,
/// and the name must be the test's own. The path is not created.
#[must_use]
pub fn process_temp_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("{}-{name}", process_tag()))
}

/// This process among the processes of every run: its id, and the microsecond of the second in which
/// it first asked for a path, so that a later process given the same id does not take a path that an
/// earlier run left behind.
fn process_tag() -> &'static str {
    static TAG: OnceLock<String> = OnceLock::new();
    TAG.get_or_init(|| {
        let micros = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |since| since.subsec_micros());
        format!("{}-{micros}", std::process::id())
    })
}

#[cfg(test)]
mod tests {
    use super::{process_temp_path, unique_temp_path};

    /// Two paths of one name in one process are two, both start with the process's id, and a file
    /// name keeps its extension. Catches a path that two tests of one name share, which the first to
    /// finish removed under the other, and a path that another process could take.
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
        let file_name = first
            .file_name()
            .and_then(|name| name.to_str())
            .expect("a name");
        assert!(
            file_name.starts_with(&format!("{}-", std::process::id())),
            "{file_name}"
        );
    }

    /// One name gives one path within the process, and two names give two. Catches a test that names
    /// one path twice and finds two, and two tests of two names on one path.
    #[test]
    fn a_name_keeps_its_path_within_the_process() {
        assert_eq!(
            process_temp_path("trust-store.bin"),
            process_temp_path("trust-store.bin")
        );
        assert_ne!(
            process_temp_path("trust-store.bin"),
            process_temp_path("trust-other.bin")
        );
    }
}
