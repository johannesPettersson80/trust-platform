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

const PROJECT_ENDPOINT: &str = "unix:///home/user/tank/.trust-runtime/control.sock";

/// `snapshot` with the project's own runtime at [`PROJECT_ENDPOINT`], answering or not.
fn with_project(mut snapshot: Value, reachable: bool) -> Value {
    snapshot["projectEndpoint"] = json!(PROJECT_ENDPOINT);
    snapshot["projectReachable"] = json!(reachable);
    snapshot
}

/// The project's own runtime is listed as `This project's runtime` after the managed runtimes and
/// before the configured ones while it answers, as a runtime to connect to whose selection names no
/// endpoint; while it does not answer and is not selected, it is not listed (specification 25
/// §5.1). Catches a runtime started from the command line that the picker never offers, a stale
/// endpoint file listed as a runtime, and an entry that points the settings at today's endpoint.
#[test]
fn the_project_runtime_is_listed_while_it_answers() {
    let answer = select(json!({
        "action": "selection",
        "snapshot": with_project(snapshot("simulate", "stopped", ""), true),
        "fleetEndpoints": ["tcp://10.0.0.2:9901"],
        "managed": [{ "name": "cell1", "controlEndpoint": "tcp://127.0.0.1:9910", "state": "stopped" }],
    }));
    assert_eq!(
        ids(&answer),
        ["simulator", "cell1", "project", "tcp://10.0.0.2:9901"]
    );
    let project = &answer["options"][2];
    assert_eq!(project["label"], "This project's runtime");
    assert_eq!(project["kind"], "remote");
    assert_eq!(project["project"], true);
    assert_eq!(project["runEndpoint"], Value::Null);
    assert_eq!(answer["options"][3]["project"], false);

    let stopped = select(json!({
        "action": "selection",
        "snapshot": with_project(snapshot("simulate", "stopped", ""), false),
    }));
    assert_eq!(ids(&stopped), ["simulator"]);
}

/// The project's own runtime Connects and Disconnects and is never started or stopped; while it
/// does not answer it stays listed when selected, `Not running` with Connect disabled and that
/// reason (specification 25 §5.1). Catches Start or Stop offered for a runtime started outside VS
/// Code, a selected entry that disappears when its runtime stops, a connected session shown as
/// not running when one probe fails, and an answering runtime shown as not reachable because the
/// configured endpoint, its own, is switched off and so not probed.
#[test]
fn the_project_runtime_action() {
    let mut connecting = with_project(snapshot("online", "stopped", PROJECT_ENDPOINT), true);
    connecting["starting"] = json!(true);
    let mut endpoint_off = with_project(snapshot("online", "stopped", PROJECT_ENDPOINT), true);
    endpoint_off["endpointReachable"] = json!(false);
    let cases = [
        (
            endpoint_off,
            "disconnected",
            "Not connected",
            "connect",
            true,
        ),
        (
            with_project(snapshot("simulate", "stopped", ""), true),
            "disconnected",
            "Not connected",
            "connect",
            true,
        ),
        (
            with_project(snapshot("online", "connected", PROJECT_ENDPOINT), true),
            "connected",
            "Connected",
            "disconnect",
            true,
        ),
        (
            with_project(snapshot("online", "connected", PROJECT_ENDPOINT), false),
            "connected",
            "Connected",
            "disconnect",
            true,
        ),
        (connecting, "starting", "Connecting…", "none", false),
        (
            with_project(snapshot("simulate", "stopped", ""), false),
            "unreachable",
            "Not running",
            "connect",
            false,
        ),
        (
            snapshot("simulate", "stopped", ""),
            "unreachable",
            "Not running",
            "connect",
            false,
        ),
    ];
    for (snapshot, status, status_label, action, enabled) in cases {
        let answer = select(json!({
            "action": "selection",
            "snapshot": snapshot,
            "selectedId": "project",
        }));
        let selected = &answer["selected"];
        assert_eq!(answer["selectionValid"], true, "{snapshot}");
        assert_eq!(selected["id"], "project", "{snapshot}");
        assert_eq!(selected["label"], "This project's runtime", "{snapshot}");
        assert_eq!(selected["status"], status, "{snapshot}");
        assert_eq!(selected["statusLabel"], status_label, "{snapshot}");
        assert_eq!(selected["primary"]["action"], action, "{snapshot}");
        assert_eq!(selected["primary"]["enabled"], enabled, "{snapshot}");
        if status_label == "Not running" {
            assert_eq!(
                selected["primary"]["hint"],
                "This project's runtime is not running."
            );
        }
    }
}

/// Online with the project's endpoint, the project's entry is that runtime's only one, the
/// endpoint a Connect stored as the selection selects it, and the status bar names it; while it
/// does not answer, the stored endpoint still selects it, not running (specification 25 §5.1).
/// Catches the project's runtime listed twice, and a Connect to it that shows the Simulator as the
/// target.
#[test]
fn the_project_runtime_is_its_only_entry() {
    let answer = select(json!({
        "action": "selection",
        "snapshot": with_project(snapshot("online", "connected", PROJECT_ENDPOINT), true),
        "selectedId": PROJECT_ENDPOINT,
    }));
    assert_eq!(ids(&answer), ["simulator", "project"]);
    assert_eq!(answer["selectionValid"], true);
    assert_eq!(answer["selected"]["id"], "project");
    assert_eq!(answer["selected"]["status"], "connected");
    assert_eq!(answer["connectedLabel"], "This project's runtime");

    let mut gone = with_project(snapshot("online", "stopped", PROJECT_ENDPOINT), false);
    gone["endpointReachable"] = json!(false);
    let answer = select(json!({
        "action": "selection",
        "snapshot": gone,
        "selectedId": PROJECT_ENDPOINT,
    }));
    assert_eq!(ids(&answer), ["simulator", "project"]);
    assert_eq!(answer["selected"]["id"], "project");
    assert_eq!(answer["selected"]["statusLabel"], "Not running");
}

fn choose(snapshot: Value, id: &str, endpoint: Option<&str>) -> Value {
    super::select(
        json!({ "action": "select", "snapshot": snapshot, "id": id, "endpoint": endpoint }),
    )
    .expect("an answer")
}

/// Selecting a target names it in the runtime settings: an endpoint in online mode, the project's
/// own runtime online with no endpoint so that each connection asks for it again, and a target
/// without an endpoint out of online mode; another target than the settings' ends the session first
/// (specification 25 §5.1). Catches the project's endpoint of today written into the settings, a
/// session to the project's runtime ended by selecting its own entry, and a Simulator selection
/// that leaves online mode on.
#[test]
fn selecting_a_target() {
    let remote = "tcp://10.0.0.2:9901";
    let cases = [
        (
            snapshot("simulate", "running", ""),
            "simulator",
            None,
            false,
            "keep",
            "",
        ),
        (
            snapshot("online", "connected", remote),
            "simulator",
            None,
            true,
            "simulate",
            "",
        ),
        (
            snapshot("simulate", "running", ""),
            remote,
            Some(remote),
            true,
            "online",
            remote,
        ),
        (
            snapshot("online", "connected", remote),
            remote,
            Some(remote),
            false,
            "online",
            remote,
        ),
        (
            snapshot("online", "connected", remote),
            "tcp://10.0.0.3:9901",
            Some(" tcp://10.0.0.3:9901 "),
            true,
            "online",
            "tcp://10.0.0.3:9901",
        ),
        (
            snapshot("simulate", "running", ""),
            "cell1",
            None,
            true,
            "keep",
            "",
        ),
        (
            with_project(snapshot("simulate", "running", ""), true),
            "project",
            None,
            true,
            "online",
            "",
        ),
        (
            with_project(snapshot("online", "connected", PROJECT_ENDPOINT), true),
            "project",
            None,
            false,
            "online",
            "",
        ),
        (
            with_project(snapshot("online", "connected", remote), true),
            "project",
            Some(PROJECT_ENDPOINT),
            true,
            "online",
            "",
        ),
    ];
    for (snapshot, id, endpoint, changes, settings, named) in cases {
        let answer = choose(snapshot.clone(), id, endpoint);
        assert_eq!(answer["changesTarget"], changes, "{id} from {snapshot}");
        assert_eq!(answer["settings"], settings, "{id} from {snapshot}");
        assert_eq!(answer["endpoint"], named, "{id} from {snapshot}");
    }
    assert!(super::select(json!({ "action": "select", "snapshot": {}, "id": " " })).is_err());
}

/// Stop or Disconnect while the extension holds no session leaves a runtime it is connected to by
/// turning the control endpoint off, and changes nothing when no runtime is connected (§5.1).
/// Catches the endpoint turned off for a runtime that is not connected, and a connected runtime
/// left connected.
#[test]
fn stop_without_a_session() {
    let connected = super::stop(json!({
        "snapshot": snapshot("online", "connected", "tcp://10.0.0.2:9901"),
    }))
    .expect("an answer");
    assert_eq!(
        connected,
        json!({ "ok": true, "settings": "disableEndpoint", "message": "Runtime endpoint disabled." })
    );
    for state in ["stopped", "running", "unavailable", ""] {
        let answer =
            super::stop(json!({ "snapshot": snapshot("online", state, "tcp://10.0.0.2:9901") }))
                .expect("an answer");
        assert_eq!(
            answer,
            json!({ "ok": true, "settings": "keep", "message": "Runtime already stopped." }),
            "{state}"
        );
    }
}
