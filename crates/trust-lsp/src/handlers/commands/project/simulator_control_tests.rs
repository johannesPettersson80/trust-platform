//! The simulator's control endpoint (specification 25 §6.8). Each socket name's hash is the first
//! 8 hexadecimal digits that `printf '%s' <path> | sha256sum` prints for the workspace path.

use super::*;
use std::net::TcpListener;

fn create_answer(workspace: Option<&str>) -> Value {
    answer(json!({ "action": "create", "workspace": workspace })).expect("answer")
}

fn problem(endpoint: &str) -> Value {
    answer(json!({ "action": "check", "endpoint": endpoint })).expect("answer")["problem"].clone()
}

#[cfg(unix)]
#[test]
fn each_workspace_has_its_own_socket_in_the_temporary_folder() {
    // `/work/plant` hashes to 4a5761d4…, `/work/other` to b243c00c…, `default` to 37a8eec1….
    // Catches: two workspaces sharing one socket (the adapter's fixed default), or a name that
    // differs between the launch and Devices & Connections.
    let endpoint = |workspace| {
        create_answer(workspace)["endpoint"]
            .as_str()
            .expect("an endpoint")
            .to_owned()
    };
    let socket = |name: &str| format!("unix://{}", std::env::temp_dir().join(name).display());
    assert_eq!(
        endpoint(Some("/work/plant")),
        socket("trust-debug-4a5761d4.sock")
    );
    assert_eq!(
        endpoint(Some("/work/plant")),
        socket("trust-debug-4a5761d4.sock"),
        "the same workspace, the same socket"
    );
    assert_eq!(
        endpoint(Some("/work/other")),
        socket("trust-debug-b243c00c.sock")
    );
    assert_eq!(endpoint(None), socket("trust-debug-37a8eec1.sock"));
}

#[cfg(unix)]
#[test]
fn each_simulator_gets_a_new_token_of_32_random_bytes() {
    // 32 bytes as unpadded base64url are 43 characters (specification 11 §6.8.0a). Catches: a fixed
    // or predictable token, which would let another local program write to the simulator.
    let token = || {
        create_answer(Some("/work/plant"))["authToken"]
            .as_str()
            .expect("a token")
            .to_owned()
    };
    let first = token();
    assert_eq!(first.len(), 43, "{first}");
    assert!(
        first
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "-_".contains(character)),
        "{first}"
    );
    assert_ne!(first, token());
}

#[cfg(not(unix))]
#[test]
fn on_windows_the_adapter_keeps_its_default_endpoint() {
    // Catches: a Unix socket named to an adapter that cannot bind one.
    let answer = create_answer(Some(r"C:\work\plant"));
    assert_eq!(answer["endpoint"], Value::Null);
    assert_eq!(answer["authToken"], Value::Null);
}

#[test]
fn a_launch_on_a_taken_tcp_port_is_refused_and_on_a_free_one_allowed() {
    // Catches: a launch that starts the adapter on a port another program holds, or a refusal of
    // a free port.
    let listener = TcpListener::bind("127.0.0.1:0").expect("a listener");
    let endpoint = format!("tcp://{}", listener.local_addr().expect("its address"));
    assert_eq!(
        problem(&endpoint),
        json!("The runtime port is already in use.")
    );
    drop(listener);
    assert_eq!(problem(&endpoint), Value::Null);
}

#[test]
fn other_endpoints_are_left_to_the_adapter() {
    // A socket, an address that is not loopback and text the adapter cannot read are not probed:
    // the adapter binds them and names its own error. Catches: a refusal of an endpoint nobody
    // holds.
    assert_eq!(problem("unix:///tmp/trust-lsp-no-such.sock"), Value::Null);
    assert_eq!(problem("tcp://192.0.2.1:9901"), Value::Null);
    assert_eq!(problem("not an endpoint"), Value::Null);
}
