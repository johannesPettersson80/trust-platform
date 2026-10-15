//! Owner-only control socket contract (spec 11 sections 6.8.3 and 6.9.5).

use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::PoisonError;

use super::{bind_owner_only_socket, UMASK_LOCK};

static DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

/// A scratch directory under `/tmp`, not the configured temporary directory: a socket path must
/// fit the platform limit (107 usable bytes on Linux, 103 on macOS), and a test runner's TMPDIR can
/// be far longer than that.
struct ScratchDir(PathBuf);

impl ScratchDir {
    fn new() -> Self {
        let sequence = DIR_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path =
            PathBuf::from("/tmp").join(format!("trust-sock-{}-{sequence}", std::process::id()));
        std::fs::create_dir_all(&path).expect("create socket test directory");
        Self(path)
    }

    fn socket(&self) -> PathBuf {
        self.0.join("ctl.sock")
    }
}

impl Drop for ScratchDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Reads the process umask. The mask is set back in the same call, under the lock that the
/// binds use, so no bind can run while it is read.
fn current_umask() -> rustix::fs::Mode {
    let _guard = UMASK_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
    let current = rustix::process::umask(rustix::fs::Mode::empty());
    rustix::process::umask(current);
    current
}

#[test]
fn bound_socket_is_owner_only_from_creation() {
    let dir = ScratchDir::new();

    let _listener = bind_owner_only_socket(&dir.socket()).expect("bind owner-only socket");

    let mode = std::fs::metadata(dir.socket())
        .expect("socket metadata")
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o600);
}

#[test]
fn bind_restores_the_process_umask_on_success_and_failure() {
    let dir = ScratchDir::new();
    let before = current_umask();

    let _listener = bind_owner_only_socket(&dir.socket()).expect("bind owner-only socket");
    assert_eq!(
        current_umask(),
        before,
        "a successful bind restores the umask"
    );

    let missing_parent = dir.0.join("missing").join("ctl.sock");
    assert!(bind_owner_only_socket(&missing_parent).is_err());
    assert_eq!(current_umask(), before, "a failed bind restores the umask");
}

#[test]
fn regular_file_at_the_socket_path_is_refused_and_kept() {
    let dir = ScratchDir::new();
    std::fs::write(dir.socket(), "operator data").expect("write regular file");

    let err =
        bind_owner_only_socket(&dir.socket()).expect_err("a regular file must not be replaced");

    assert!(
        err.to_string().contains("is not a socket"),
        "unclear error: {err}"
    );
    assert_eq!(
        std::fs::read_to_string(dir.socket()).expect("regular file is kept"),
        "operator data"
    );
}

#[test]
fn stale_socket_from_an_earlier_run_is_replaced() {
    let dir = ScratchDir::new();
    // Dropping the listener leaves the socket file behind, as a crashed run would.
    drop(bind_owner_only_socket(&dir.socket()).expect("first bind"));

    let _listener = bind_owner_only_socket(&dir.socket()).expect("stale socket is replaced");
}
