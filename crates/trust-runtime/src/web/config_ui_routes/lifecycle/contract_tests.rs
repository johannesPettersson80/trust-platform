#[cfg(unix)]
use std::sync::atomic::{AtomicU64, Ordering};

use super::*;

#[cfg(unix)]
static NEXT_SOCKET: AtomicU64 = AtomicU64::new(1);

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
    let path = std::env::temp_dir().join(format!(
        "trust-config-ui-lifecycle-{}-{}.sock",
        std::process::id(),
        NEXT_SOCKET.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_file(&path);
    let listener = std::os::unix::net::UnixListener::bind(&path).expect("bind Unix listener");
    let endpoint = format!("unix://{}", path.display());
    assert!(control_endpoint_online(&endpoint));
    drop(listener);
    let _ = std::fs::remove_file(path);
}
