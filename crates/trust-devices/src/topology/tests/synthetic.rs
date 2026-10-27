//! Nodes drawn without a report of their own, the managed runtimes and the session's marks
//! (specification 25 §5.6), ported from `network-canvas.test.ts`.

use std::collections::BTreeSet;

use serde_json::json;
use trust_runtime_client::{AuthFailure, ControlError, TargetProbe};

use super::fixtures::{answered, fleet_topology, runtime, unanswered};
use crate::topology::synthetic::unanswered_peer;
use crate::topology::{
    self, ConfiguredDevice, HostOrigin, ManagedRuntime, ManagedState, Runtime, RuntimeOrigin,
    RuntimeReport, Session, Simulator, SimulatorState, TopologyInputs, TopologyIssue,
};

const CELL1: &str = "tcp://127.0.0.1:9902";

fn managed(name: &str, endpoint: &str, state: ManagedState) -> ManagedRuntime {
    ManagedRuntime {
        name: name.to_owned(),
        control_endpoint: Some(endpoint.to_owned()),
        state,
    }
}

/// A report in which the runtime `cell1` answers at `CELL1` with the given health and detail.
fn cell1_report(health: &str, detail: &str) -> serde_json::Value {
    json!({
        "schema_version": 3, "links": [], "shared": [], "external": [],
        "hosts": [{
            "host_id": "fleet:peer", "hostname": "peer", "arch": "", "os": "", "ips": [],
            "containers": [],
            "runtimes": [{
                "runtime_id": "fleet:peer:rt", "name": "cell1", "control_endpoint": CELL1,
                "mode": "online", "cycle_ms": 0, "health": health, "detail": detail,
                "endpoints": [],
            }],
        }],
    })
}

/// The simulator that the round draws.
fn drawn_simulator(inputs: &TopologyInputs) -> Runtime {
    topology::merge(inputs)
        .runtimes()
        .find(|runtime| runtime.origin == RuntimeOrigin::SimulatorFallback)
        .cloned()
        .expect("the drawn simulator")
}

#[test]
fn unreachable_peer_is_unknown_never_green_or_stopped() {
    let peer = unanswered("10.0.0.9:5510", "cell1", TargetProbe::Unreachable);
    let merged = topology::merge(&TopologyInputs {
        peers: vec![peer.clone()],
        ..TopologyInputs::default()
    });
    let host = merged
        .hosts
        .iter()
        .find(|host| host.id == "fleet:10.0.0.9:5510")
        .expect("an unreachable configured peer should still appear (synthesized)");
    assert_eq!(host.hostname, "cell1");
    assert_eq!(host.origin, HostOrigin::Unanswered);
    let cell = &host.runtimes[0];
    // Configured and not reachable: whether it is stopped or only unreachable is unknown.
    assert_eq!(
        cell.health, "unknown",
        "unknown/grey, never connected/green"
    );
    assert_eq!(cell.mode, "unknown");
    assert_eq!(cell.endpoints, Vec::<crate::topology::Endpoint>::new());
    assert_eq!(cell.origin, RuntimeOrigin::Unreachable);
    assert_eq!(cell.control_endpoint.as_deref(), Some("10.0.0.9:5510"));
    assert_eq!(
        cell.detail,
        "Configured endpoint not reachable — open it in Devices & Connections to connect or \
         diagnose."
    );

    // The added peer appears beside the stopped local simulator, which stays.
    let local = merged
        .hosts
        .iter()
        .find(|host| host.origin == HostOrigin::ThisComputer)
        .expect("the local simulator node is preserved");
    assert_eq!(local.runtimes[0].health, "stopped");

    let reachable = RuntimeReport {
        probe: TargetProbe::Reachable(json!({})),
        ..peer
    };
    assert_eq!(
        unanswered_peer(&reachable),
        None,
        "a reachable peer uses its real fleet.topology, not a synthetic node"
    );
    let without_endpoint = unanswered(" ", "cell1", TargetProbe::Unreachable);
    assert_eq!(
        unanswered_peer(&without_endpoint),
        None,
        "no endpoint, nothing to show"
    );
    let disabled = unanswered("10.0.0.9:5510", "cell1", TargetProbe::MissingEndpoint);
    assert_eq!(unanswered_peer(&disabled), None);
}

#[test]
fn auth_failed_peer_is_an_error_with_missing_or_rejected_text() {
    let endpoint = "tcp://127.0.0.1:33101";
    for (failure, text) in [
        (
            AuthFailure::Missing,
            "No auth token provided — this runtime requires one.",
        ),
        (
            AuthFailure::Rejected,
            "Auth token rejected — check it and try again.",
        ),
    ] {
        let peer = unanswered(endpoint, "discoveredcell", TargetProbe::AuthFailed(failure));
        let merged = topology::merge(&TopologyInputs {
            peers: vec![peer],
            ..TopologyInputs::default()
        });
        let cell = runtime(&merged, "discoveredcell");
        assert_eq!(cell.health, "error", "auth failure is a real error");
        assert_eq!(cell.mode, "error");
        assert_eq!(cell.detail, text);
        assert_eq!(cell.origin, RuntimeOrigin::AuthFailed(failure));
        assert_eq!(
            cell.control_endpoint.as_deref(),
            Some(endpoint),
            "runtime inspector actions need the endpoint for Set auth token and Set as run target"
        );
        let host = merged
            .hosts
            .iter()
            .find(|host| host.origin == HostOrigin::Unanswered)
            .expect("the peer's host");
        assert_eq!(
            host.hostname, "discoveredcell",
            "unreachable configured peers keep their configured label, not 'This computer'"
        );
    }
}

#[test]
fn managed_runtime_not_duplicated_and_stays_owned() {
    managed_runtime_on_the_canvas_is_annotated();
    managed_status_overrides_reported_health();
    managed_runtimes_join_this_computer();
    managed_runtimes_join_the_simulators_computer();
    live_managed_runtime_keeps_the_stopped_project_runtime();
    runtime_never_also_appears_as_an_external_system();
}

/// Without a configuration that names this computer, the host of the simulator's own report is
/// this computer, and the managed runtimes share it.
fn managed_runtimes_join_the_simulators_computer() {
    let simulator = "unix:///tmp/trust-local-sim.sock";
    let report = json!({
        "schema_version": 4, "links": [], "shared": [], "external": [],
        "hosts": [{
            "host_id": "host:pi", "hostname": "pi", "arch": "aarch64", "os": "linux",
            "ips": ["192.168.10.31"], "containers": [],
            "runtimes": [{
                "runtime_id": "RESOURCE", "name": "RESOURCE", "mode": "simulate",
                "cycle_ms": 20, "health": "simulate", "detail": "Online", "endpoints": [],
            }],
        }],
    });
    let merged = topology::merge(&TopologyInputs {
        primary: Some(answered(simulator, report)),
        simulator: Simulator {
            endpoint: Some(simulator.to_owned()),
            state: SimulatorState::Running,
            ..Simulator::default()
        },
        managed: vec![managed(
            "cell3",
            "tcp://127.0.0.1:9904",
            ManagedState::Stopped,
        )],
        ..TopologyInputs::default()
    });
    assert_eq!(merged.hosts.len(), 1, "one host for this computer");
    let names: Vec<&str> = merged.hosts[0]
        .all_runtimes()
        .map(|runtime| runtime.name.as_str())
        .collect();
    assert_eq!(names, ["RESOURCE", "cell3"]);
}

/// A managed runtime already shown by a report is not doubled, and the node that shows it keeps
/// managed ownership.
fn managed_runtime_on_the_canvas_is_annotated() {
    let merged = topology::merge(&TopologyInputs {
        peers: vec![answered(CELL1, cell1_report("connected", ""))],
        managed: vec![managed("cell1", CELL1, ManagedState::Running)],
        ..TopologyInputs::default()
    });
    assert!(
        !merged
            .hosts
            .iter()
            .any(|host| host.id == "host:managed-local"),
        "no duplicate managed node for an endpoint already on the canvas"
    );
    assert_eq!(
        merged
            .runtimes()
            .filter(|runtime| runtime.name == "cell1")
            .count(),
        1
    );
    let cell = runtime(&merged, "cell1");
    assert_eq!(cell.control_endpoint.as_deref(), Some(CELL1));
    assert_eq!(cell.origin, RuntimeOrigin::Answered);
    assert_eq!(
        cell.managed,
        Some(managed("cell1", CELL1, ManagedState::Running)),
        "the surviving node is marked managed (owned lifecycle)"
    );
}

fn managed_status_overrides_reported_health() {
    for (state, health, detail) in [
        (
            ManagedState::Running,
            "connected",
            "Running (managed local runtime).",
        ),
        (
            ManagedState::Stopped,
            "stopped",
            "Stopped — Start it from this node.",
        ),
        (
            ManagedState::Starting,
            "pending",
            "Starting managed local runtime…",
        ),
        (
            ManagedState::Stopping,
            "pending",
            "Stopping managed local runtime…",
        ),
        (
            ManagedState::Unavailable,
            "error",
            "Status unavailable — refresh before starting.",
        ),
    ] {
        let merged = topology::merge(&TopologyInputs {
            peers: vec![answered(
                CELL1,
                cell1_report("connected", "Stale topology health."),
            )],
            managed: vec![managed("cell1", CELL1, state)],
            ..TopologyInputs::default()
        });
        let cell = runtime(&merged, "cell1");
        assert_eq!(
            cell.health, health,
            "{state:?} health must come from managed status"
        );
        assert_eq!(
            cell.detail, detail,
            "{state:?} detail must come from managed status"
        );
        assert_eq!(
            cell.managed.as_ref().map(|local| local.state),
            Some(state),
            "{state:?} lifecycle must remain authoritative"
        );
    }
}

/// Managed runtimes that no report shows join this computer's host beside the simulator, and the
/// selection is projected as one run-target flag.
fn managed_runtimes_join_this_computer() {
    let merged = topology::merge(&TopologyInputs {
        managed: vec![
            managed("cell1", CELL1, ManagedState::Stopped),
            managed("cell2", "tcp://127.0.0.1:9903", ManagedState::Running),
        ],
        session: Session {
            run_target: Some("cell1".to_owned()),
            ..Session::default()
        },
        ..TopologyInputs::default()
    });
    assert_eq!(
        merged.hosts.len(),
        1,
        "managed injection must not draw a duplicate host"
    );
    assert_eq!(merged.hosts[0].origin, HostOrigin::ThisComputer);
    assert_eq!(merged.runtimes().count(), 3);
    let (cell1, cell2) = (runtime(&merged, "cell1"), runtime(&merged, "cell2"));
    assert!(
        cell1.managed.is_some() && cell2.managed.is_some(),
        "nodes are marked managed"
    );
    assert_eq!(cell1.id, "managed:cell1");
    assert_eq!(cell1.origin, RuntimeOrigin::Managed);
    assert_eq!(
        cell1.health, "stopped",
        "stopped managed runtime is honest grey"
    );
    assert_eq!(
        cell2.health, "connected",
        "running managed runtime is green"
    );
    assert!(
        cell1.run_target,
        "the selected managed runtime gets a run-target flag"
    );
    assert!(
        !cell2.run_target,
        "non-selected managed runtimes are not flagged"
    );
    assert!(
        !runtime(&merged, "Simulator").run_target,
        "the simulator is not flagged when a managed runtime is selected"
    );
}

/// A managed runtime that answers for the project's endpoint is another runtime of this
/// computer: the project's stopped runtime stays beside it instead of folding into it.
fn live_managed_runtime_keeps_the_stopped_project_runtime() {
    let host = |runtime: serde_json::Value| {
        json!({
            "schema_version": 3, "links": [], "shared": [], "external": [],
            "hosts": [{"host_id": "host:this-computer", "hostname": "This computer",
                       "arch": "aarch64", "os": "linux", "ips": ["127.0.0.1"], "containers": [],
                       "runtimes": [runtime]}],
        })
    };
    let simulated = |id: &str, health: &str, detail: &str| {
        json!({"id": id, "kind": "field", "protocol": "simulated", "name": "Simulated",
               "role": "owned_driver", "health": health, "detail": detail, "owned": true,
               "supports_test": true})
    };
    let live = host(json!({
        "runtime_id": "runtime:cell1", "name": "cell1", "control_endpoint": CELL1, "mode": "online",
        "cycle_ms": 10, "health": "connected", "detail": "Running (managed local runtime).",
        "endpoints": [simulated("endpoint:cell1:simulated", "connected", "Driver is healthy.")],
    }));
    let configured = host(json!({
        "runtime_id": "runtime:project", "name": "truST runtime", "mode": "simulate",
        "cycle_ms": 10, "health": "configured_policy",
        "detail": "Stopped - configured in this project.",
        "endpoints": [
            simulated("endpoint:project:simulated", "configured_policy", "Configured in io.toml."),
        ],
    }));
    let merged = topology::merge(&TopologyInputs {
        project: Some(configured),
        primary: Some(answered(CELL1, live)),
        managed: vec![managed("cell1", CELL1, ManagedState::Running)],
        ..TopologyInputs::default()
    });
    assert_eq!(merged.hosts.len(), 1, "same computer stays one host");
    let mut names: Vec<&str> = merged
        .runtimes()
        .map(|runtime| runtime.name.as_str())
        .collect();
    names.sort_unstable();
    assert_eq!(
        names,
        ["cell1", "truST runtime"],
        "managed Start must not delete the project runtime the user just saw"
    );
    let endpoints: usize = merged
        .runtimes()
        .map(|runtime| runtime.endpoints.len())
        .sum();
    assert_eq!(endpoints, 2);
    let cell1 = runtime(&merged, "cell1");
    assert!(
        cell1.managed.is_some(),
        "managed runtime keeps owned lifecycle controls"
    );
    assert_eq!(
        cell1.health, "connected",
        "started managed runtime is honestly running"
    );
    let project = runtime(&merged, "truST runtime");
    assert_eq!(project.origin, RuntimeOrigin::Configured);
    assert_eq!(
        project.health, "configured_policy",
        "project runtime remains visible with an honest stopped state"
    );
}

/// A runtime never also appears as an external system; unrelated systems stay, and every link
/// still names nodes that appear.
fn runtime_never_also_appears_as_an_external_system() {
    let mut report = fleet_topology();
    report["external"]
        .as_array_mut()
        .expect("external")
        .push(json!({
            "id": "external:self", "kind": "runtime", "name": "Line runtime",
            "via_protocol": ["discovery"], "direction": "outbound",
        }));
    report["links"].as_array_mut().expect("links").push(json!({
        "id": "link:self", "from": "endpoint:runtime-a:mesh", "to": "external:self",
        "protocol": "discovery", "direction": "observed", "same_host": true,
        "status": "connected", "secure": false,
    }));
    let merged = topology::merge(&TopologyInputs {
        primary: Some(answered("tcp://192.168.77.10:5680", report)),
        ..TopologyInputs::default()
    });
    let named = |name: &str| {
        merged
            .runtimes()
            .filter(|runtime| runtime.name == name)
            .count()
    };
    assert_eq!(named("Line runtime"), 1, "the runtime itself stays visible");
    assert!(
        !merged
            .external
            .iter()
            .any(|external| external.name == "Line runtime"),
        "the same runtime must not also appear as an external system"
    );
    assert!(
        merged
            .external
            .iter()
            .any(|external| external.name == "tcp/192.168.77.11:7447"),
        "unrelated external systems stay visible"
    );
    let mut nodes: BTreeSet<&str> = merged
        .runtimes()
        .map(|runtime| runtime.id.as_str())
        .collect();
    nodes.extend(
        merged
            .runtimes()
            .flat_map(|runtime| &runtime.endpoints)
            .map(|e| e.id.as_str()),
    );
    nodes.extend(merged.external.iter().map(|external| external.id.as_str()));
    nodes.extend(merged.shared.iter().map(|shared| shared.id.as_str()));
    assert_eq!(merged.links.len(), 2);
    for link in &merged.links {
        assert!(nodes.contains(link.from.as_str()) && nodes.contains(link.to.as_str()));
    }
}

#[test]
fn new_project_without_runtime_shows_one_simulator() {
    let merged = topology::merge(&TopologyInputs::default());
    assert_eq!(merged.hosts.len(), 1);
    let host = &merged.hosts[0];
    assert_eq!(host.id, "host:this-computer");
    assert_eq!(host.origin, HostOrigin::ThisComputer);
    assert_eq!(
        host.runtimes.len(),
        1,
        "local simulator runtime node is always present"
    );
    let simulator = &host.runtimes[0];
    assert_eq!(
        (simulator.id.as_str(), simulator.name.as_str()),
        ("runtime:local", "Simulator")
    );
    assert_eq!(simulator.origin, RuntimeOrigin::SimulatorFallback);
    assert_ne!(
        simulator.health, "connected",
        "honest: not green until proven"
    );
    assert_eq!(simulator.health, "stopped");
    assert_eq!(simulator.detail, "Stopped — start the simulator to run it.");
    assert_eq!(
        simulator.control_endpoint, None,
        "no local socket path leaks into the graph"
    );
    assert!(
        simulator.run_target,
        "the simulator is the run target while nothing is selected"
    );
    assert!(!simulator.attached);
    assert!(
        merged.issues.is_empty(),
        "a stopped simulator does not consume the fault channel"
    );

    simulator_follows_its_lifecycle();
    local_connections_need_live_evidence();
    peer_failure_keeps_the_local_view();
}

fn simulator_follows_its_lifecycle() {
    let port = "The runtime port is already in use.";
    for (state, failure, health, detail) in [
        (SimulatorState::Running, None, "connected", "● Running"),
        (
            SimulatorState::Starting,
            None,
            "pending",
            "Starting Simulator…",
        ),
        (SimulatorState::Running, Some(port), "error", port),
    ] {
        let simulator = drawn_simulator(&TopologyInputs {
            simulator: Simulator {
                state,
                failure: failure.map(str::to_owned),
                ..Simulator::default()
            },
            ..TopologyInputs::default()
        });
        assert_eq!(
            (simulator.health.as_str(), simulator.detail.as_str()),
            (health, detail)
        );
        assert_eq!(
            simulator.attached,
            health == "connected",
            "{state:?}, {failure:?}"
        );
    }
}

/// The project's local I/O connections on the drawn simulator: a field device is connected only
/// after the runtime reports real I/O values (specification 25 §5.5).
fn local_connections_need_live_evidence() {
    let device = |id: &str, protocol: &str, health: &str| ConfiguredDevice {
        id: id.to_owned(),
        name: id.to_owned(),
        protocol: protocol.to_owned(),
        health: Some(health.to_owned()),
    };
    let devices = vec![
        device("Simulated I/O", "simulated", "connected"),
        device("10.0.0.9:502", "modbus_tcp", "degraded"),
        device("OPC UA client", "opcua_client", "connected"),
    ];
    for (io_values, health, detail) in [
        (false, "pending", "Pending proof"),
        (true, "connected", "Connected"),
    ] {
        let simulator = drawn_simulator(&TopologyInputs {
            simulator: Simulator {
                state: SimulatorState::Running,
                io_values,
                devices: devices.clone(),
                ..Simulator::default()
            },
            ..TopologyInputs::default()
        });
        let words: Vec<(&str, &str, &str)> = simulator
            .endpoints
            .iter()
            .map(|endpoint| {
                (
                    endpoint.protocol.as_str(),
                    endpoint.health.as_str(),
                    endpoint.detail.as_str(),
                )
            })
            .collect();
        assert_eq!(
            words,
            [
                ("simulated", health, detail),
                ("modbus_tcp", "degraded", "Degraded")
            ],
            "with I/O values: {io_values}"
        );
    }
}

/// A peer whose requests fail is an inline error, one for each failed request, while the local
/// view stays visible.
fn peer_failure_keeps_the_local_view() {
    let reset = || {
        Some(Err(ControlError::Transport(
            "connection reset by peer".to_owned(),
        )))
    };
    let peer = RuntimeReport {
        label: "peer-a".to_owned(),
        topology: reset(),
        connectors: reset(),
        ..answered("tcp://10.0.0.7:5680", fleet_topology())
    };
    let merged = topology::merge(&TopologyInputs {
        peers: vec![peer],
        ..TopologyInputs::default()
    });
    assert!(
        merged
            .runtimes()
            .any(|runtime| runtime.origin == RuntimeOrigin::SimulatorFallback),
        "the local runtime remains visible"
    );
    let issue = |text: &str| TopologyIssue {
        item: None,
        project: false,
        text: text.to_owned(),
    };
    assert_eq!(
        merged.issues,
        [
            issue("peer-a topology: connection reset by peer"),
            issue("peer-a connector status: connection reset by peer"),
        ]
    );
}
