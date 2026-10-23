//! Owner-only control socket contract (spec 11 sections 6.8.3 and 6.9.5).

use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use super::bind_owner_only_socket;

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
fn binding_leaves_directories_created_by_other_threads_usable() {
    // The process umask is shared by every thread. Binding never narrows it, so a directory that
    // another thread creates meanwhile keeps its owner's search permission. Narrowing it around
    // `bind` (the earlier design) created such directories without it.
    let dir = ScratchDir::new();
    let created = dir.0.join("created");
    std::fs::create_dir(&created).expect("create parent for concurrent directories");
    let worker = {
        let created = created.clone();
        std::thread::spawn(move || {
            for index in 0..400 {
                std::fs::create_dir(created.join(index.to_string())).expect("create directory");
            }
        })
    };
    for _ in 0..400 {
        drop(bind_owner_only_socket(&dir.socket()).expect("bind owner-only socket"));
    }
    worker.join().expect("directory worker");
    for entry in std::fs::read_dir(&created).expect("list directories") {
        let mode = entry
            .expect("directory entry")
            .metadata()
            .expect("directory metadata")
            .permissions()
            .mode();
        assert_eq!(mode & 0o700, 0o700, "directory lost its owner permissions");
    }
}

#[test]
fn the_owner_can_connect_once_the_socket_listens() {
    let dir = ScratchDir::new();
    let listener = bind_owner_only_socket(&dir.socket()).expect("bind owner-only socket");

    let _client = std::os::unix::net::UnixStream::connect(dir.socket()).expect("owner connects");
    listener.accept().expect("accept the owner's connection");
}

#[test]
fn a_bind_into_a_missing_directory_fails_and_leaves_no_file() {
    let dir = ScratchDir::new();
    let missing_parent = dir.0.join("missing").join("ctl.sock");

    assert!(bind_owner_only_socket(&missing_parent).is_err());
    assert!(!missing_parent.exists());
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
