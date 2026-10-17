#[cfg(unix)]
use std::sync::atomic::{AtomicU64, Ordering};

#[cfg(unix)]
use std::path::{Path, PathBuf};

use super::*;

#[cfg(unix)]
static NEXT_SOCKET: AtomicU64 = AtomicU64::new(1);

#[cfg(unix)]
fn unix_control_socket_path(temp_root: &Path, nonce: u64) -> PathBuf {
    const PORTABLE_SOCKET_PATH_BUDGET: usize = 100;

    let file_name = format!(
        "trust-config-ui-lifecycle-{}-{nonce}.sock",
        std::process::id()
    );
    let preferred = temp_root.join(&file_name);
    if preferred.as_os_str().as_encoded_bytes().len() <= PORTABLE_SOCKET_PATH_BUDGET {
        preferred
    } else {
        Path::new("/tmp").join(file_name)
    }
}

#[test]
fn empty_control_endpoint_is_offline() {
    assert!(!control_endpoint_online(""));
    assert!(!control_endpoint_online(" \t\n "));
}

#[test]
fn unsupported_control_endpoint_scheme_is_offline() {
    for endpoint in [
        "http://127.0.0.1:8080",
        "udp://127.0.0.1:8080",
        "127.0.0.1:8080",
        "TCP://127.0.0.1:8080",
    ] {
        assert!(!control_endpoint_online(endpoint), "{endpoint}");
    }
}

#[test]
fn malformed_tcp_control_endpoint_is_offline() {
    for endpoint in [
        "tcp://",
        "tcp://127.0.0.1",
        "tcp://127.0.0.1:not-a-port",
        "tcp://[::1",
    ] {
        assert!(!control_endpoint_online(endpoint), "{endpoint}");
    }
}

#[test]
fn listening_tcp_control_endpoint_is_online() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind listener");
    let endpoint = format!("tcp://{}", listener.local_addr().expect("address"));
    assert!(control_endpoint_online(&endpoint));
}

#[test]
fn endpoint_probe_trims_outer_whitespace() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind listener");
    let endpoint = format!(" \t tcp://{} \n", listener.local_addr().expect("address"));
    assert!(control_endpoint_online(&endpoint));
}

#[cfg(unix)]
#[test]
fn empty_or_missing_unix_control_endpoint_is_offline() {
    assert!(!control_endpoint_online("unix://"));
    assert!(!control_endpoint_online("unix://   "));
    assert!(!control_endpoint_online(
        "unix:///tmp/trust-config-ui-definitely-missing.sock"
    ));
}

#[cfg(unix)]
#[test]
fn listening_unix_control_endpoint_is_online() {
    let path = unix_control_socket_path(
        &std::env::temp_dir(),
        NEXT_SOCKET.fetch_add(1, Ordering::Relaxed),
    );
    let _ = std::fs::remove_file(&path);
    let listener = std::os::unix::net::UnixListener::bind(&path).expect("bind Unix listener");
    let endpoint = format!("unix://{}", path.display());
    assert!(control_endpoint_online(&endpoint));
    drop(listener);
    let _ = std::fs::remove_file(path);
}

#[cfg(unix)]
#[test]
fn unix_control_socket_fixture_path_is_bounded_under_long_temp_root() {
    let long_temp_root = Path::new("/tmp").join("x".repeat(180));
    let path = unix_control_socket_path(&long_temp_root, 1);

    assert!(
        path.as_os_str().as_encoded_bytes().len() <= 100,
        "Unix control socket fixture path exceeded the portable 100-byte budget: {}",
        path.display()
    );
}

/// A killed runtime cannot remove its endpoint file, so the config UI does (spec 11, section
/// 6.8.0a).
#[cfg(unix)]
#[test]
fn stopping_a_managed_runtime_removes_the_endpoint_file_that_names_it() {
    let root = std::env::temp_dir().join(format!(
        "trust-config-ui-stop-{}-{}",
        std::process::id(),
        NEXT_SOCKET.fetch_add(1, Ordering::Relaxed)
    ));
    let endpoint_dir = root.join(crate::control::endpoint::ENDPOINT_DIR);
    std::fs::create_dir_all(&endpoint_dir).expect("create endpoint folder");
    let child = Command::new("sleep")
        .arg("30")
        .spawn()
        .expect("start a stand-in runtime process");
    let endpoint_file = endpoint_dir.join(crate::control::endpoint::ENDPOINT_FILE);
    std::fs::write(
        &endpoint_file,
        format!(
            r#"{{"schema_version":1,"endpoint":"tcp://127.0.0.1:9","pid":{}}}"#,
            child.id()
        ),
    )
    .expect("write the endpoint file of the stand-in runtime");
    let mut process = ConfigUiManagedRuntimeProcess {
        listen: "127.0.0.1:0".to_string(),
        child,
        started_at_ns: 0,
        root: root.clone(),
    };

    stop_managed_runtime_process(&mut process).expect("stop the managed runtime");

    assert!(!endpoint_file.exists());
    let _ = std::fs::remove_dir_all(root);
}
