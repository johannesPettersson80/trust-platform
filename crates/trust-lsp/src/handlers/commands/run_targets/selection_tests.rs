//! The sidebar's run targets and the selected target's one action (specification 25 §5.1, §6.1
//! and §6.5).

use serde_json::{json, Value};

use super::answer;

fn select(argument: Value) -> Value {
    answer(argument).expect("an answer")
}

fn snapshot(mode: &str, state: &str, endpoint: &str) -> Value {
    json!({
        "runtimeMode": mode,
        "runtimeState": state,
        "endpoint": endpoint,
        "endpointConfigured": !endpoint.is_empty(),
        "endpointReachable": true,
        "starting": false,
    })
}

fn ids(answer: &Value) -> Vec<String> {
    answer["options"]
        .as_array()
        .expect("options")
        .iter()
        .map(|option| option["id"].as_str().expect("an id").to_owned())
        .collect()
}

/// The list is the Simulator, the managed runtimes, the configured runtimes (each once, blank ones
/// left out), then the runtime the extension is online with when nothing names it and no managed
/// runtime has its endpoint. Catches a duplicated remote, a blank entry, and a second entry for a
/// managed runtime the extension connected to.
#[test]
fn the_targets_in_order() {
    let answer = select(json!({
        "action": "selection",
        "snapshot": snapshot("online", "connected", "tcp://10.0.0.9:9901"),
        "fleetEndpoints": ["tcp://10.0.0.2:9901", " tcp://10.0.0.2:9901 ", "", null, "tcp://10.0.0.3:9901"],
        "managed": [{ "name": "cell1", "controlEndpoint": "tcp://127.0.0.1:9910", "state": "running" }],
        "selectedId": "simulator",
    }));
    assert_eq!(
        ids(&answer),
        [
            "simulator",
            "cell1",
            "tcp://10.0.0.2:9901",
            "tcp://10.0.0.3:9901",
            "tcp://10.0.0.9:9901"
        ]
    );
    assert_eq!(answer["options"][1]["label"], "cell1 (this computer)");
    assert_eq!(answer["options"][2]["label"], "10.0.0.2:9901");
    assert_eq!(answer["remotes"].as_array().map(Vec::len), Some(2));
    let owned = select(json!({
        "action": "selection",
        "snapshot": snapshot("online", "connected", "tcp://127.0.0.1:9910"),
        "managed": [{ "name": "cell1", "controlEndpoint": "tcp://127.0.0.1:9910", "state": "running" }],
    }));
    assert_eq!(ids(&owned), ["simulator", "cell1"]);
}

/// A selection that names no target selects the Simulator and says so. Catches a stale selection
/// that keeps a removed runtime as the target.
#[test]
fn an_unknown_selection_falls_back_to_the_simulator() {
    let answer = select(json!({
        "action": "selection",
        "snapshot": snapshot("simulate", "stopped", ""),
        "selectedId": "tcp://gone:1",
    }));
    assert_eq!(answer["selectionValid"], false);
    assert_eq!(answer["selected"]["id"], "simulator");
    assert_eq!(answer["selected"]["status"], "stopped");
    assert_eq!(answer["selected"]["primary"]["action"], "start");
    assert_eq!(answer["selected"]["primary"]["gateable"], true);
}

/// The Simulator Starts and Stops by its session: `Starting…` while it starts, Stop while it runs.
/// Catches a running online session shown as the Simulator running.
#[test]
fn the_simulator_action() {
    let mut starting = snapshot("simulate", "stopped", "");
    starting["starting"] = json!(true);
    let cases = [
        (starting, "starting", "Starting…", "none", false),
        (
            snapshot("simulate", "running", ""),
            "running",
            "Running",
            "stop",
            true,
        ),
        (
            snapshot("online", "running", ""),
            "stopped",
            "Stopped",
            "start",
            true,
        ),
    ];
    for (snapshot, status, status_label, action, enabled) in cases {
        let selected =
            select(json!({ "action": "selection", "snapshot": snapshot }))["selected"].clone();
        assert_eq!(selected["status"], status);
        assert_eq!(selected["statusLabel"], status_label);
        assert_eq!(selected["primary"]["action"], action);
        assert_eq!(selected["primary"]["enabled"], enabled);
    }
}

/// A managed runtime Starts and Stops by its reported state; a missing or unknown state is
/// `Status unavailable` with Start disabled and its reason, never Stopped (specification 25 §6.5).
/// Selecting it points the settings at its endpoint only while it runs. Catches an unknown state
/// shown as stopped with an enabled Start.
#[test]
fn a_managed_runtime_action() {
    let cases = [
        (
            Some("running"),
            "running",
            "Running",
            "stop",
            true,
            Some("tcp://127.0.0.1:9910"),
        ),
        (Some("stopped"), "stopped", "Stopped", "start", true, None),
        (
            Some("starting"),
            "starting",
            "Starting…",
            "none",
            false,
            None,
        ),
        (
            Some("stopping"),
            "stopping",
            "Stopping…",
            "none",
            false,
            None,
        ),
        (
            Some("crashed"),
            "unavailable",
            "Status unavailable",
            "start",
            false,
            None,
        ),
        (
            None,
            "unavailable",
            "Status unavailable",
            "start",
            false,
            None,
        ),
    ];
    for (state, status, status_label, action, enabled, run_endpoint) in cases {
        let selected = select(json!({
            "action": "selection",
            "snapshot": snapshot("simulate", "stopped", ""),
            "managed": [{ "name": "cell1", "controlEndpoint": "tcp://127.0.0.1:9910", "state": state }],
            "selectedId": "cell1",
        }))["selected"]
            .clone();
        assert_eq!(selected["status"], status, "{state:?}");
        assert_eq!(selected["statusLabel"], status_label, "{state:?}");
        assert_eq!(selected["primary"]["action"], action, "{state:?}");
        assert_eq!(selected["primary"]["enabled"], enabled, "{state:?}");
        assert_eq!(selected["runEndpoint"], json!(run_endpoint), "{state:?}");
    }
    let unavailable = select(json!({
        "action": "selection",
        "snapshot": snapshot("simulate", "stopped", ""),
        "managed": [{ "name": "cell1", "controlEndpoint": "tcp://127.0.0.1:9910" }],
        "selectedId": "cell1",
    }));
    assert_eq!(
        unavailable["selected"]["primary"]["hint"],
        "Status unavailable — refresh managed runtimes before starting."
    );
    assert_eq!(unavailable["selected"]["primary"]["gateable"], false);
}

/// Any other runtime Connects and Disconnects and is never stopped: Disconnect while connected,
/// `Connecting…` while connecting, Connect disabled with its reason while known unreachable, the
/// reason pointing to Devices & Connections. Catches Stop offered for a runtime the extension does
/// not own, and a Connect button that can only fail.
#[test]
fn a_remote_runtime_action() {
    let endpoint = "tcp://10.0.0.2:9901";
    let mut connecting = snapshot("online", "stopped", endpoint);
    connecting["starting"] = json!(true);
    let mut unreachable = snapshot("online", "stopped", endpoint);
    unreachable["endpointReachable"] = json!(false);
    let cases = [
        (
            snapshot("online", "connected", endpoint),
            "connected",
            "Connected",
            "disconnect",
            true,
        ),
        (connecting, "starting", "Connecting…", "none", false),
        (
            unreachable,
            "unreachable",
            "Not reachable",
            "connect",
            false,
        ),
        (
            snapshot("online", "stopped", endpoint),
            "disconnected",
            "Not connected",
            "connect",
            true,
        ),
        (
            snapshot("online", "connected", "tcp://other:1"),
            "disconnected",
            "Not connected",
            "connect",
            true,
        ),
    ];
    for (snapshot, status, status_label, action, enabled) in cases {
        let selected = select(json!({
            "action": "selection",
            "snapshot": snapshot,
            "fleetEndpoints": [endpoint],
            "selectedId": endpoint,
        }))["selected"]
            .clone();
        assert_eq!(selected["status"], status, "{snapshot}");
        assert_eq!(selected["statusLabel"], status_label, "{snapshot}");
        assert_eq!(selected["primary"]["action"], action, "{snapshot}");
        assert_eq!(selected["primary"]["enabled"], enabled, "{snapshot}");
        assert_eq!(selected["runEndpoint"], endpoint);
        assert_ne!(selected["primary"]["action"], "stop");
        if status == "unreachable" {
            assert_eq!(
                selected["primary"]["hint"],
                "Not reachable — open Devices & Connections to start or diagnose this runtime."
            );
        }
    }
}

/// The status bar names a connected runtime by its endpoint without the scheme, and a local control
/// socket `runtime`. Catches a socket path in the status bar.
#[test]
fn the_connected_label() {
    for (endpoint, label) in [
        ("tcp://10.0.0.2:9901", "10.0.0.2:9901"),
        ("unix:///tmp/trust-runtime.sock", "runtime"),
        ("UNIX:///tmp/x.sock", "runtime"),
        ("", "runtime"),
    ] {
        let answer = select(json!({
            "action": "selection",
            "snapshot": snapshot("online", "connected", endpoint),
        }));
        assert_eq!(answer["connectedLabel"], label, "{endpoint}");
    }
}

/// The extension is connected while its session is attached or while it is online with a runtime
/// that answers, such as one started outside the extension; else the status bar names no
/// connected runtime. Catches a reachable runtime started outside the extension shown as
/// stopped, and a running Simulator or an unreachable online target shown as connected.
#[test]
fn connected_or_not() {
    let mut unreachable = snapshot("online", "stopped", "tcp://10.0.0.2:9901");
    unreachable["endpointReachable"] = json!(false);
    let cases = [
        (
            snapshot("online", "stopped", "unix:///tmp/trust-runtime.sock"),
            json!("runtime"),
        ),
        (
            snapshot("simulate", "connected", "tcp://10.0.0.2:9901"),
            json!("10.0.0.2:9901"),
        ),
        (
            snapshot("simulate", "running", "tcp://127.0.0.1:9901"),
            Value::Null,
        ),
        (unreachable, Value::Null),
    ];
    for (snapshot, label) in cases {
        let answer = select(json!({ "action": "selection", "snapshot": snapshot }));
        assert_eq!(answer["connectedLabel"], label, "{snapshot}");
    }
}
