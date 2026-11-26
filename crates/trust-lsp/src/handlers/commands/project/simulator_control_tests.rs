//! The simulator's control endpoint (specification 25 §6.8). Each socket name's hash is the first
//! 8 hexadecimal digits that `printf '%s' <path> | sha256sum` prints for the workspace path, and a
//! Windows port is 20000 plus their value modulo 20000.

use super::*;
use std::net::TcpListener;

fn create_answer(workspace: Option<&str>) -> Value {
    answer(json!({ "action": "create", "workspace": workspace })).expect("answer")
}

fn problem(endpoint: &str) -> Value {
    answer(json!({ "action": "check", "endpoint": endpoint })).expect("answer")["problem"].clone()
}

/// A loopback address that was free a moment ago: its listener is closed at once.
// port-retry-ok: the free-port check is asked again about another port when this one was taken.
fn released_address() -> std::net::SocketAddr {
    TcpListener::bind("127.0.0.1:0")
        .expect("a listener")
        .local_addr()
        .expect("its address")
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
    let socket = |name: &str| {
        // temp-root-ok: the simulator's own socket path in the temporary folder.
        format!("unix://{}", std::env::temp_dir().join(name).display())
    };
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

#[test]
fn each_simulator_gets_a_new_token_of_32_random_bytes() {
    // 32 bytes as unpadded base64url are 43 characters (specification 11 §6.8.0a), on every
    // platform. Catches: a fixed or predictable token, which would let another local program write
    // to the simulator, and a simulator on Windows without one, whose TCP control server then
    // refuses every request, the debug adapter's own included.
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
fn on_windows_each_workspace_has_its_own_loopback_port() {
    // `C:\work\plant` hashes to e6c2c079… (3,871,522,937, port 22937), `C:\work\other` to
    // 4e86a825… (1,317,447,717, port 27717), `default` to 37a8eec1… (933,818,049, port 38049).
    // Catches: no endpoint, which leaves the adapter on its default port without a token, a Unix
    // socket named to an adapter that cannot bind one, and two workspaces sharing one port.
    let endpoint = |workspace| {
        create_answer(workspace)["endpoint"]
            .as_str()
            .expect("an endpoint")
            .to_owned()
    };
    assert_eq!(endpoint(Some(r"C:\work\plant")), "tcp://127.0.0.1:22937");
    assert_eq!(endpoint(Some(r"C:\work\other")), "tcp://127.0.0.1:27717");
    assert_eq!(endpoint(None), "tcp://127.0.0.1:38049");
}

#[cfg(not(unix))]
#[test]
fn on_windows_two_workspaces_whose_hashes_share_a_port_both_get_one() {
    // `C:\work\project2` hashes to c9ad11cb… (3,383,562,699) and `C:\work\project76` to
    // 5ce886ab… (1,558,742,699): both leave 2699, port 22699. With the first's simulator holding
    // it, the second workspace gets the next port. Catches two workspaces named one port, so that
    // the second launch cannot bind while the first simulator runs.
    let endpoint = |workspace| {
        create_answer(Some(workspace))["endpoint"]
            .as_str()
            .expect("an endpoint")
            .to_owned()
    };
    assert_eq!(endpoint(r"C:\work\project2"), "tcp://127.0.0.1:22699");
    let held = TcpListener::bind("127.0.0.1:22699").expect("the first workspace's simulator");
    assert_eq!(endpoint(r"C:\work\project76"), "tcp://127.0.0.1:22700");
    drop(held);
}

#[test]
fn a_windows_port_that_does_not_bind_is_passed_over_for_the_next() {
    // Catches: a port that another simulator or program holds named again, a search that leaves
    // the range at its end instead of wrapping to 20000, and no port at all when every try fails
    // (the launch then names the port it cannot bind).
    assert_eq!(free_port_from(20_660, |port| port != 20_660), 20_661);
    assert_eq!(free_port_from(20_660, |port| port > 20_662), 20_663);
    assert_eq!(free_port_from(39_999, |port| port != 39_999), 20_000);
    assert_eq!(free_port_from(20_660, |_| false), 20_660);
}

#[test]
fn a_windows_simulator_port_is_20000_plus_the_workspace_hash_modulo_20000() {
    // `/work/plant` hashes to 4a5761d4… = 1,247,240,660, which leaves 660; the ports run from 20000
    // to 39999, below the dynamic range from 49152 that Windows hands ports out from. Catches: a
    // port in the dynamic range, where an outgoing connection's port can take it, or one outside
    // the range of a u16.
    assert_eq!(workspace_port(0x4a57_61d4), 20_660);
    assert_eq!(
        workspace_port(workspace_hash(Some("/work/plant".into()))),
        20_660
    );
    assert_eq!(workspace_port(0), 20_000);
    assert_eq!(workspace_port(19_999), 39_999);
    assert_eq!(workspace_port(20_000), 20_000);
    assert_eq!(workspace_port(u32::MAX), 27_295);
}

#[test]
fn a_launch_on_a_taken_tcp_port_is_refused_and_on_a_free_one_allowed() {
    // Catches: a launch that starts the adapter on a port another program holds, or a refusal of
    // a free port.
    // port-retry-ok: the free half asks again about another port when this one was taken.
    let listener = TcpListener::bind("127.0.0.1:0").expect("a listener");
    let endpoint = format!("tcp://{}", listener.local_addr().expect("its address"));
    assert_eq!(
        problem(&endpoint),
        json!("The runtime port is already in use.")
    );
    drop(listener);
    // Another socket can take the released port before the check (specification 22); the check is
    // then asked about another port that was free a moment ago, and only then, five times at most.
    let mut candidate = endpoint;
    let mut attempts = 1;
    loop {
        let answer = problem(&candidate);
        if answer == Value::Null {
            break;
        }
        assert!(
            answer == json!("The runtime port is already in use.") && attempts < 5,
            "the free port {candidate} answered {answer} at attempt {attempts}"
        );
        attempts += 1;
        candidate = format!("tcp://{}", released_address());
    }
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
