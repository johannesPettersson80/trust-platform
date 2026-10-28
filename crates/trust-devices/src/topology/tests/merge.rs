//! The snapshot merge, the project's configuration over its live runtime, and the connector
//! reports (specification 25 §5.6; specification 11, "Connector status and discovery truth
//! contract"), ported from `network-canvas.test.ts` and `connector-status-contract.test.ts`.

use serde_json::{json, Value};
use trust_runtime::connectors::{
    ConnectorHealth, ConnectorPointCounts, ConnectorState, DiscoveryConfidence,
};
use trust_runtime_client::{ControlError, TargetProbe};

use super::fixtures::{
    answered, distinct, endpoint, fleet_topology, runtime, snapshot, unanswered, Mesh,
};
use crate::topology::merge::merge_snapshots;
use crate::topology::{
    self, wire, ConnectorStatus, RuntimeOrigin, RuntimeReport, Simulator, SimulatorState,
    TopologyInputs, TopologyIssue,
};

/// The control endpoint of the local simulator.
const SIMULATOR: &str = "unix:///tmp/trust-local-sim.sock";
/// The control endpoint of the runtime of `fleet_topology()`.
const LINE: &str = "tcp://192.168.77.10:5680";
const RESTART_TO_APPLY: &str =
    "Configured in project files; restart the runtime to apply this change.";

/// A report of one runtime on this computer.
fn one_runtime(schema: u64, runtime: Value) -> Value {
    let mut report = json!({
        "schema_version": schema, "links": [], "shared": [], "external": [],
        "hosts": [{
            "host_id": "host:this-computer", "hostname": "pi", "arch": "aarch64", "os": "linux",
            "ips": ["127.0.0.1"], "containers": [],
        }],
    });
    report["hosts"][0]["runtimes"] = Value::Array(vec![runtime]);
    report
}

/// An endpoint of `runtime`; `source` is set from schema version 4 on.
fn endpoint_of(
    runtime: &str,
    protocol: &str,
    health: &str,
    detail: &str,
    source: Option<&str>,
) -> Value {
    let mut endpoint = json!({
        "id": format!("endpoint:{runtime}:{protocol}"), "kind": "field", "protocol": protocol,
        "name": protocol, "role": "owned_driver", "health": health, "detail": detail,
        "owned": true, "supports_test": true,
    });
    if let Some(source) = source {
        endpoint["source"] = json!(source);
    }
    endpoint
}

/// The configuration of the project's runtime `name`, as `offline_fleet_topology_json` reports
/// it: stopped, with configured endpoints.
fn configuration(name: &str, endpoints: Value) -> Value {
    let mut runtime = json!({
        "runtime_id": name, "name": name, "control_endpoint": SIMULATOR, "mode": "stopped",
        "cycle_ms": 20, "health": "configured_policy",
        "detail": "Configured in project files; runtime is not running.", "source": "config",
    });
    runtime["endpoints"] = endpoints;
    one_runtime(4, runtime)
}

/// The simulator, running the project and answering for it, with the project's configuration.
fn simulator_round(live: Value, configuration: Value) -> TopologyInputs {
    TopologyInputs {
        project: Some(configuration),
        primary: Some(answered(SIMULATOR, live)),
        simulator: Simulator {
            endpoint: Some(SIMULATOR.to_owned()),
            state: SimulatorState::Running,
            io_values: true,
            ..Simulator::default()
        },
        ..TopologyInputs::default()
    }
}

fn with_connectors(mut report: RuntimeReport, connectors: Value) -> RuntimeReport {
    let mut answer = json!({"schema_version": 1});
    answer["connectors"] = connectors;
    report.connectors = Some(Ok(answer));
    report
}

fn connector(protocol: &str, state: &str, health: &str, confidence: &str) -> Value {
    json!({
        "schema_version": 1, "connector_id": format!("io:{protocol}"), "protocol": protocol,
        "kind": "process_image", "state": state, "health": health, "confidence": confidence,
        "point_counts": {"total": 1, "good": 1, "degraded": 0, "unavailable": 0},
    })
}

fn decoded_status(report: &Value) -> Result<ConnectorStatus, String> {
    wire::connector_reports(&json!({"connectors": [report]}))
        .remove(0)
        .status
}

#[test]
fn merge_unions_hosts_and_keeps_the_first_host_facts() {
    let service = |id: &str| {
        json!({
            "runtime_id": id, "name": id, "mode": "online", "cycle_ms": 10, "health": "connected",
            "detail": "",
            "endpoints": [{
                "id": format!("endpoint:{id}:service"), "kind": "service",
                "protocol": "opcua_client", "name": format!("{id} service"),
                "health": "connected", "detail": "Ready.", "owned": true, "supports_test": false,
            }],
        })
    };
    let host = |id: &str, hostname: &str, arch: &str, runtime: Value| {
        json!({"host_id": id, "hostname": hostname, "arch": arch, "os": "linux", "ips": [],
               "containers": [], "runtimes": [runtime]})
    };
    let link = |id: &str, runtime: &str, external: &str| {
        json!({"id": id, "from": format!("endpoint:{runtime}:service"), "to": external,
               "protocol": "opcua_client", "role": "client", "direction": "out",
               "same_host": false, "status": "ok", "secure": true})
    };
    let external = |id: &str| {
        json!({"id": id, "name": id.to_lowercase(), "kind": "peer", "via_protocol": ["mesh"],
               "direction": "out"})
    };
    let a = json!({
        "schema_version": 3,
        "hosts": [host("H", "first", "x", service("A"))],
        "links": [link("L1", "A", "E1")],
        "shared": [{"id": "S1", "kind": "broker", "name": "b", "address": "x", "used_by": ["A"]}],
        "external": [external("E1")],
    });
    let b = json!({
        "schema_version": 2,
        "hosts": [host("H", "second", "y", service("B")), host("H2", "H2", "x", service("C"))],
        "links": [link("L2", "B", "E2")],
        "shared": [{"id": "S1", "kind": "broker", "name": "b", "address": "x", "used_by": ["B"]}],
        "external": [external("E1"), external("E2")],
    });
    let merged = merge_snapshots(&[snapshot(&a), snapshot(&b)]);
    assert_eq!(merged.schema_version, 3, "schema_version is the max");
    assert_eq!(merged.hosts.len(), 2, "host H appears once + H2");
    let h = &merged.hosts[0];
    assert_eq!(h.id, "H");
    assert_eq!(h.runtimes.len(), 2, "H unions runtimes A + B");
    assert_eq!(
        (h.hostname.as_str(), h.arch.as_str()),
        ("first", "x"),
        "the first host facts stay"
    );
    assert_eq!(merged.links.len(), 2, "both uniquely owned links are kept");
    assert_eq!(merged.external.len(), 2, "E1 deduped, E1 + E2 kept");
    let runtime_ids: Vec<String> = h
        .runtimes
        .iter()
        .map(|runtime| runtime.id.clone())
        .collect();
    assert_eq!(
        merged.shared[0].used_by, runtime_ids,
        "shared.used_by is rewritten to and unioned by normalized runtime identity"
    );

    // Repeated snapshots from one complete owner deduplicate.
    let repeated = Mesh::new(
        4,
        "host:repeat",
        "RESOURCE",
        "tcp/192.168.77.41:7447",
        "connected",
        "peer ready",
    );
    let repeated = snapshot(&repeated.report());
    let deduplicated = merge_snapshots(&[repeated.clone(), repeated]);
    assert_eq!(deduplicated.runtimes().count(), 1);
    assert_eq!(deduplicated.external.len(), 1);
    assert_eq!(deduplicated.links.len(), 1);
}

#[test]
fn configured_endpoints_stay_on_the_live_runtime() {
    let report = |endpoint: Value| {
        let mut report = one_runtime(
            4,
            json!({
                "runtime_id": "RESOURCE", "name": "Simulator", "mode": "online", "cycle_ms": 50,
                "health": "connected", "detail": "Running.", "endpoints": [endpoint],
            }),
        );
        report["hosts"][0]["host_id"] = json!("host:local");
        report
    };
    let simulated = endpoint_of("RESOURCE", "simulated", "connected", "Running.", None);
    let mut ads = endpoint_of(
        "RESOURCE",
        "ads",
        "configured_policy",
        "Configured in ads.toml. Restart the runtime to load it.",
        None,
    );
    ads["kind"] = json!("service");
    let merged = merge_snapshots(&[snapshot(&report(simulated)), snapshot(&report(ads))]);
    let endpoints = &merged.hosts[0].runtimes[0].endpoints;
    let mut protocols: Vec<&str> = endpoints
        .iter()
        .map(|endpoint| endpoint.protocol.as_str())
        .collect();
    protocols.sort_unstable();
    assert_eq!(
        protocols,
        ["ads", "simulated"],
        "same-runtime live topology keeps configured endpoints waiting for restart"
    );
    let identities: Vec<String> = endpoints
        .iter()
        .map(|endpoint| endpoint.id.clone())
        .collect();
    assert_eq!(
        distinct(&identities),
        2,
        "the retained live and configured endpoints keep distinct identities"
    );
    project_runtime_keeps_its_configured_endpoints();
}

/// The project's runtime answering itself under the resource name of its configuration: the
/// endpoint it already runs appears once, and the one it does not run yet stays on it.
fn project_runtime_keeps_its_configured_endpoints() {
    let live = one_runtime(
        4,
        json!({
            "runtime_id": "Line", "name": "Line", "mode": "online", "cycle_ms": 20,
            "health": "connected", "detail": "Running.", "source": "self",
            "endpoints": [
                endpoint_of("Line", "simulated", "connected", "Driver is healthy.", Some("self")),
            ],
        }),
    );
    let configured = configuration(
        "Line",
        json!([
            endpoint_of(
                "Line",
                "simulated",
                "configured_policy",
                "Configured in io.toml; runtime is not running.",
                Some("config")
            ),
            endpoint_of(
                "Line",
                "modbus_tcp",
                "configured_policy",
                "Configured in io.toml; runtime is not running.",
                Some("config")
            ),
        ]),
    );
    let merged = topology::merge(&TopologyInputs {
        project: Some(configured),
        primary: Some(answered("tcp://127.0.0.1:9901", live)),
        ..TopologyInputs::default()
    });
    assert_eq!(merged.runtimes().count(), 1);
    let project_runtime = runtime(&merged, "Line");
    assert_eq!(project_runtime.origin, RuntimeOrigin::Answered);
    assert_eq!(
        project_runtime.health, "connected",
        "the runtime keeps its live state"
    );
    assert_eq!(project_runtime.endpoints.len(), 2);
    assert_eq!(endpoint(project_runtime, "simulated").health, "connected");
    assert_eq!(
        endpoint(project_runtime, "modbus_tcp").detail,
        RESTART_TO_APPLY
    );
}

#[test]
fn configured_overlay_on_running_simulator_says_restart_required() {
    let live = one_runtime(
        4,
        json!({
            "runtime_id": "RESOURCE", "name": "RESOURCE", "mode": "simulate", "cycle_ms": 50,
            "health": "simulate",
            "detail": "Runtime answered fleet.topology from its control channel.",
            "source": "self", "endpoints": [],
        }),
    );
    let mut ads = endpoint_of(
        "ADS live TwinCAT",
        "ads",
        "configured_policy",
        "Configured in ADS project config; runtime is not running.",
        Some("config"),
    );
    ads["kind"] = json!("service");
    ads["role"] = json!("client");
    let merged = topology::merge(&simulator_round(
        live,
        configuration("ADS live TwinCAT", json!([ads])),
    ));
    assert_eq!(merged.runtimes().count(), 1);
    let simulator = merged.runtimes().next().expect("the simulator");
    assert_eq!(
        simulator.health, "simulate",
        "a running simulator must not become Stopped because a new ADS endpoint needs restart"
    );
    assert_eq!(simulator.mode, "simulate");
    let ads = endpoint(simulator, "ads");
    assert!(
        ads.detail
            .to_lowercase()
            .contains("restart the runtime to apply"),
        "the configured endpoint should tell the user to restart/apply, not claim the runtime is \
         not running: {}",
        ads.detail
    );
    assert_eq!(ads.detail, RESTART_TO_APPLY);
    assert_eq!(ads.health, "configured_policy");
}

#[test]
fn live_simulator_replaces_the_stopped_project_overlay() {
    let mut live_endpoint = endpoint_of("live", "simulated", "connected", "Live", None);
    live_endpoint["name"] = json!("Simulated I/O");
    let live = one_runtime(
        3,
        json!({
            "runtime_id": "RESOURCE", "name": "RESOURCE", "control_endpoint": SIMULATOR,
            "mode": "simulate", "cycle_ms": 20, "health": "simulate", "detail": "Online",
            "source": "self", "endpoints": [live_endpoint],
        }),
    );
    let configured = json!([
        endpoint_of(
            "ADS live TwinCAT",
            "simulated",
            "configured_policy",
            "Configured in io.toml; runtime is not running.",
            Some("config")
        ),
        endpoint_of(
            "ADS live TwinCAT",
            "ads",
            "configured_policy",
            "Configured in ADS project config; runtime is not running.",
            Some("config")
        ),
    ]);
    let merged = topology::merge(&simulator_round(
        live.clone(),
        configuration("ADS live TwinCAT", configured),
    ));
    assert_eq!(merged.hosts.len(), 1, "local simulator stays on one host");
    assert_eq!(
        merged.runtimes().count(),
        1,
        "one local simulator runtime, not RESOURCE + configured project twin"
    );
    let simulator = runtime(&merged, "RESOURCE");
    assert_eq!(
        simulator.origin,
        RuntimeOrigin::Simulator,
        "the view names it Simulator; ST resource names must not replace the run target"
    );
    assert_eq!(simulator.health, "simulate");
    assert_eq!(simulator.endpoints.len(), 2);
    let simulated = simulator
        .endpoints
        .iter()
        .filter(|endpoint| endpoint.protocol == "simulated")
        .count();
    assert_eq!(
        simulated, 1,
        "the project-file overlay must not duplicate the live Simulated I/O endpoint"
    );
    assert_eq!(endpoint(simulator, "simulated").detail, "Live");
    assert_eq!(
        endpoint(simulator, "ads").detail,
        RESTART_TO_APPLY,
        "configured ADS endpoint stays visible and explains the required restart"
    );
    assert!(
        simulator.run_target,
        "the simulator is the run target while nothing else is selected"
    );
    assert!(
        simulator.attached,
        "the running debug session is the simulator's session"
    );

    // The simulator stays the simulator when its report has no mode.
    let mut modeless = live;
    modeless["hosts"][0]["runtimes"][0]["mode"] = json!("");
    let merged = topology::merge(&TopologyInputs {
        primary: Some(answered(SIMULATOR, modeless)),
        simulator: Simulator {
            endpoint: Some(SIMULATOR.to_owned()),
            ..Simulator::default()
        },
        ..TopologyInputs::default()
    });
    assert_eq!(
        runtime(&merged, "RESOURCE").origin,
        RuntimeOrigin::Simulator
    );
    configured_connections_fold_by_their_place();
}

/// The simulator reports `RESOURCE` while the configuration names `Line`. Both number the
/// connections of the same `io.toml` in configuration order, so each configured connection folds
/// into the live one with the same place, a disabled one too; only a configured connection that
/// the simulator does not report stays, and only one waiting for its policy says to restart.
fn configured_connections_fold_by_their_place() {
    let at = |runtime: &str, suffix: &str, health: &str, detail: &str| {
        let mut endpoint = endpoint_of(runtime, "modbus_tcp", health, detail, Some("self"));
        endpoint["id"] = json!(format!("endpoint:{runtime}:{suffix}"));
        endpoint["protocol"] = json!(suffix.split(':').next());
        endpoint
    };
    let healthy = "Driver is healthy.";
    let disabled = "Disabled in io.toml; it will not run until enabled again.";
    let waiting = "Configured in io.toml; runtime is not running.";
    let mut live = one_runtime(
        4,
        json!({
            "runtime_id": "RESOURCE", "name": "RESOURCE", "mode": "simulate", "cycle_ms": 20,
            "health": "simulate", "detail": "Online", "source": "self",
        }),
    );
    live["hosts"][0]["runtimes"][0]["endpoints"] = json!([
        at("RESOURCE", "modbus_tcp", "connected", healthy),
        at("RESOURCE", "modbus_tcp:1", "connected", healthy),
        at("RESOURCE", "gpio:2", "disabled", disabled),
    ]);
    let configured = configuration(
        "Line",
        json!([
            at("Line", "modbus_tcp", "configured_policy", waiting),
            at("Line", "modbus_tcp:1", "configured_policy", waiting),
            at("Line", "gpio:2", "disabled", disabled),
            at("Line", "mqtt:3", "configured_policy", waiting),
            at("Line", "ethercat:4", "disabled", disabled),
        ]),
    );
    let merged = topology::merge(&simulator_round(live, configured));
    assert_eq!(merged.runtimes().count(), 1);
    let simulator = runtime(&merged, "RESOURCE");
    let words: Vec<(&str, &str, &str)> = simulator
        .endpoints
        .iter()
        .map(|endpoint| {
            let detail = endpoint.detail.as_str();
            (endpoint.protocol.as_str(), endpoint.health.as_str(), detail)
        })
        .collect();
    assert_eq!(
        words,
        [
            ("modbus_tcp", "connected", healthy),
            ("modbus_tcp", "connected", healthy),
            ("gpio", "disabled", disabled),
            ("mqtt", "configured_policy", RESTART_TO_APPLY),
            ("ethercat", "disabled", disabled),
        ]
    );
}

#[test]
fn invalid_connector_report_keeps_the_peer_and_raises_an_issue() {
    // Every word of the closed vocabularies reads back unchanged.
    for state in [
        "disabled",
        "configured",
        "starting",
        "ready",
        "degraded",
        "reconnecting",
        "stale",
        "not_ready",
        "faulted",
    ] {
        let status = decoded_status(&connector("mqtt", state, "ok", "confirmed")).expect(state);
        assert_eq!(json!(status.state), json!(state));
    }
    for health in ["ok", "degraded", "faulted", "unknown"] {
        let status =
            decoded_status(&connector("mqtt", "ready", health, "confirmed")).expect(health);
        assert_eq!(json!(status.health), json!(health));
    }
    for confidence in ["confirmed", "likely", "port_reachable", "unavailable"] {
        let status =
            decoded_status(&connector("mqtt", "ready", "ok", confidence)).expect(confidence);
        assert_eq!(json!(status.confidence), json!(confidence));
    }

    // Unknown and missing words are rejected instead of rendered healthy.
    let rejected = [
        (
            connector("mqtt", "invented_healthy", "ok", "confirmed"),
            "unknown connector state: invented_healthy",
        ),
        (
            connector("mqtt", "ready", "excellent", "confirmed"),
            "unknown connector health: excellent",
        ),
        (
            connector("mqtt", "ready", "ok", "certainly_healthy"),
            "unknown connector confidence: certainly_healthy",
        ),
        (
            json!({"protocol": "mqtt", "health": "ok", "confidence": "likely"}),
            "missing connector state",
        ),
    ];
    for (report, reason) in rejected {
        assert_eq!(decoded_status(&report), Err(reason.to_owned()));
    }

    peer_keeps_its_topology();
    project_runtime_keeps_its_topology();
}

/// The peer stays with its raw topology, and the operator sees the validation failure; the other
/// configured peers stay too (`connector-status-contract.test.ts`).
fn peer_keeps_its_topology() {
    let peer_report = json!({
        "schema_version": 3, "links": [], "shared": [], "external": [],
        "hosts": [{
            "host_id": "peer-host", "hostname": "peer-host", "arch": "x86_64", "os": "linux",
            "ips": ["10.0.0.2"], "containers": [],
            "runtimes": [{
                "runtime_id": "peer-runtime", "name": "peer-runtime", "mode": "production",
                "cycle_ms": 10, "health": "connected", "detail": "reachable",
                "endpoints": [{
                    "id": "mqtt", "kind": "service", "protocol": "mqtt", "name": "mqtt",
                    "address": "mqtt://10.0.0.2", "health": "connected", "detail": "reachable",
                    "owned": true, "supports_test": true,
                }],
            }],
        }],
    });
    let invalid = json!([{
        "connector_id": "peer-mqtt", "protocol": "mqtt", "state": "ready", "health": "ok",
        "confidence": "certainly_healthy",
    }]);
    let peer = RuntimeReport {
        label: "peer-a".to_owned(),
        ..with_connectors(answered("tcp://10.0.0.2:5680", peer_report), invalid)
    };
    let other = unanswered("tcp://10.0.0.3:5680", "peer-b", TargetProbe::Unreachable);
    let merged = topology::merge(&TopologyInputs {
        peers: vec![peer, other],
        ..TopologyInputs::default()
    });
    let mqtt = endpoint(runtime(&merged, "peer-runtime"), "mqtt");
    assert_eq!(mqtt.connector, None, "an invalid report is never rendered");
    assert_eq!(
        mqtt.health, "connected",
        "the peer's raw topology remains visible"
    );
    assert_eq!(
        merged.issues,
        [TopologyIssue {
            item: Some(mqtt.id.clone()),
            project: false,
            text: "peer-a connector status: unknown connector confidence: certainly_healthy"
                .to_owned(),
        }]
    );
    assert!(merged
        .hosts
        .iter()
        .any(|host| host.id == "fleet:tcp://10.0.0.3:5680"));
}

/// Inventory 3 E22: one unknown connector value of the project's own runtime used to drop its
/// whole live topology. The topology and the valid reports stay; the bad report is an issue, and
/// so is a failed `connectors.status` request.
fn project_runtime_keeps_its_topology() {
    let primary = with_connectors(
        answered(LINE, fleet_topology()),
        json!([
            connector("mqtt", "exploded", "ok", "confirmed"),
            connector("modbus_tcp", "ready", "ok", "confirmed"),
        ]),
    );
    let merged = topology::merge(&TopologyInputs {
        primary: Some(primary),
        ..TopologyInputs::default()
    });
    let line = runtime(&merged, "Line runtime");
    assert_eq!(
        line.origin,
        RuntimeOrigin::Answered,
        "the live topology stays"
    );
    let modbus = endpoint(line, "modbus_tcp").connector.as_ref();
    assert_eq!(
        modbus.map(|status| status.state),
        Some(ConnectorState::Ready)
    );
    assert_eq!(endpoint(line, "mqtt").connector, None);
    assert_eq!(
        merged.issues,
        [TopologyIssue {
            item: Some(endpoint(line, "mqtt").id.clone()),
            project: true,
            text: format!("{LINE} connector status: unknown connector state: exploded"),
        }]
    );

    let mut primary = answered(LINE, fleet_topology());
    primary.connectors = Some(Err(ControlError::Transport(
        "connection reset by peer".to_owned(),
    )));
    let merged = topology::merge(&TopologyInputs {
        primary: Some(primary),
        ..TopologyInputs::default()
    });
    assert_eq!(
        merged.issues,
        [TopologyIssue {
            item: Some(runtime(&merged, "Line runtime").id.clone()),
            project: true,
            text: format!("{LINE} connector status: connection reset by peer"),
        }]
    );
}

#[test]
fn connector_matched_by_exact_name_before_address() {
    // Reports without a name or a target attach to an endpoint of their protocol, and the
    // connector's state, health, confidence and point counts survive.
    let report = with_connectors(
        answered(LINE, fleet_topology()),
        json!([
            {"connector_id": "io:modbus-tcp", "protocol": "modbus_tcp", "kind": "process_image",
             "state": "ready", "health": "ok", "confidence": "confirmed",
             "point_counts": {"total": 4, "good": 4, "degraded": 0, "unavailable": 0}},
            {"connector_id": "io:mqtt", "protocol": "mqtt", "kind": "process_image",
             "state": "stale", "health": "degraded", "confidence": "port_reachable",
             "point_counts": {"total": 3, "good": 1, "degraded": 1, "unavailable": 1}},
        ]),
    );
    let merged = topology::merge(&TopologyInputs {
        primary: Some(report),
        ..TopologyInputs::default()
    });
    let line = runtime(&merged, "Line runtime");
    assert_eq!(
        endpoint(line, "modbus_tcp").connector,
        Some(ConnectorStatus {
            connector_id: "io:modbus-tcp".to_owned(),
            state: ConnectorState::Ready,
            health: ConnectorHealth::Ok,
            confidence: DiscoveryConfidence::Confirmed,
            point_counts: ConnectorPointCounts {
                total: 4,
                good: 4,
                degraded: 0,
                unavailable: 0
            },
        })
    );
    assert_eq!(
        endpoint(line, "mqtt").connector,
        Some(ConnectorStatus {
            connector_id: "io:mqtt".to_owned(),
            state: ConnectorState::Stale,
            health: ConnectorHealth::Degraded,
            confidence: DiscoveryConfidence::PortReachable,
            point_counts: ConnectorPointCounts {
                total: 3,
                good: 1,
                degraded: 1,
                unavailable: 1
            },
        })
    );
    assert_eq!(endpoint(line, "mesh").connector, None);

    stronger_matches_come_first();
}

/// A disabled endpoint runs no connection, so it takes no connector report in any matching step:
/// the enabled endpoint listed after it gets its driver's report, and a report aimed at the
/// disabled endpoint's address does not attach to it.
#[test]
fn disabled_endpoint_never_takes_a_connector_report() {
    let field = |id: &str, health: &str, address: &str| {
        json!({"id": id, "kind": "field", "protocol": "modbus_tcp", "name": address,
               "address": address, "health": health, "detail": "", "owned": true,
               "supports_test": false})
    };
    let report = one_runtime(
        4,
        json!({
            "runtime_id": "cell", "name": "cell", "mode": "online", "cycle_ms": 10,
            "health": "connected", "detail": "",
            "endpoints": [
                field("endpoint:cell:modbus_tcp", "disabled", "10.0.0.5:502"),
                field("endpoint:cell:modbus_tcp:1", "connected", "10.0.0.6:502"),
            ],
        }),
    );
    // An I/O driver's report names neither its connection nor its target.
    let driver = json!({
        "connector_id": "io:modbus-tcp", "protocol": "modbus_tcp", "kind": "process_image",
        "state": "ready", "health": "ok", "confidence": "confirmed",
    });
    let aimed = json!({
        "connector_id": "io:old", "protocol": "modbus_tcp", "kind": "process_image",
        "endpoint": "10.0.0.5:502", "state": "ready", "health": "ok", "confidence": "confirmed",
    });
    for reports in [json!([driver]), json!([driver, aimed])] {
        let merged = topology::merge(&TopologyInputs {
            primary: Some(with_connectors(
                answered("tcp://10.0.0.1:5680", report.clone()),
                reports,
            )),
            ..TopologyInputs::default()
        });
        let claimed: Vec<(&str, Option<&str>)> = runtime(&merged, "cell")
            .endpoints
            .iter()
            .map(|endpoint| {
                let id = endpoint
                    .connector
                    .as_ref()
                    .map(|status| status.connector_id.as_str());
                (endpoint.health.as_str(), id)
            })
            .collect();
        assert_eq!(
            claimed,
            [("disabled", None), ("connected", Some("io:modbus-tcp"))]
        );
        assert_eq!(merged.issues, Vec::<crate::topology::TopologyIssue>::new());
    }
}

/// The report that names an endpoint's connection wins over one whose target matches its
/// address, which wins over the first one left of its protocol, whatever the order of the
/// reports and endpoints; and ADS and OPC UA reports describe the endpoint of their role.
fn stronger_matches_come_first() {
    let field = |id: &str, protocol: &str, name: &str, address: Option<&str>| {
        json!({"id": id, "kind": "field", "protocol": protocol, "name": name, "address": address,
               "health": "connected", "detail": "", "owned": true, "supports_test": false})
    };
    let report = one_runtime(
        4,
        json!({
            "runtime_id": "cell", "name": "cell", "mode": "online", "cycle_ms": 10,
            "health": "connected", "detail": "",
            "endpoints": [
                field("meter", "modbus_tcp", "Modbus meter", Some("10.0.0.5:502")),
                field("pump", "modbus_tcp", "Modbus pump", Some("10.0.0.6:502")),
                field("spare", "modbus_tcp", "Modbus spare", None),
                field("client", "ads", "ADS client", None),
                field("server", "ads_server", "ADS server", None),
                field("ua", "opcua_client", "OPC UA client", None),
            ],
        }),
    );
    let status =
        |id: &str, protocol: &str, kind: &str, name: Option<&str>, target: Option<&str>| {
            json!({"connector_id": id, "protocol": protocol, "kind": kind, "display_name": name,
               "endpoint": target, "state": "ready", "health": "ok", "confidence": "confirmed"})
        };
    let reports = json!([
        status("io:spare", "modbus_tcp", "process_image", None, None),
        status(
            "io:pump",
            "modbus_tcp",
            "process_image",
            Some("Modbus pump"),
            Some("10.0.0.5:502")
        ),
        status(
            "io:meter",
            "modbus_tcp",
            "process_image",
            None,
            Some("tcp://10.0.0.5:502")
        ),
        status("ads:server", "ads", "supervisory_server", None, None),
        status("ads:client", "ads", "supervisory_client", None, None),
        status("ua:client", "opcua", "supervisory_client", None, None),
    ]);
    let merged = topology::merge(&TopologyInputs {
        primary: Some(with_connectors(
            answered("tcp://10.0.0.1:5680", report),
            reports,
        )),
        ..TopologyInputs::default()
    });
    let cell = runtime(&merged, "cell");
    let claimed: Vec<(&str, Option<&str>)> = cell
        .endpoints
        .iter()
        .map(|endpoint| {
            let id = endpoint
                .connector
                .as_ref()
                .map(|status| status.connector_id.as_str());
            (endpoint.name.as_str(), id)
        })
        .collect();
    assert_eq!(
        claimed,
        [
            ("Modbus meter", Some("io:meter")),
            ("Modbus pump", Some("io:pump")),
            ("Modbus spare", Some("io:spare")),
            ("ADS client", Some("ads:client")),
            ("ADS server", Some("ads:server")),
            ("OPC UA client", Some("ua:client")),
        ]
    );
}
