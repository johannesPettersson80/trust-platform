//! The inputs of the view tests: gathers built from the answers that runtimes send, without any
//! socket, and the lookups into a view.

use std::path::PathBuf;

use serde_json::{json, Value};
use trust_runtime::config::fleet_manifest::{FleetManifestRuntime, FleetRuntimeLifecycleResponse};
use trust_runtime_client::TargetProbe;

use crate::gather::{
    Gathered, LiveData, ManagedState, ManagedStatus, ProjectInputs, Reach, TargetReport,
};
use crate::request::{
    DebugFailure, DebugSessionFacts, DebugSessionKind, HostFacts, HostSettings, RunMode,
    SimulatorFacts, ViewOptions, ViewRequest,
};
use crate::targets::{ManagedTarget, Target, TargetOrigin};
use crate::view::types::{Action, Computer, DevicesView, Runtime};

/// The time of every view in the tests.
pub(crate) const NOW_MS: u64 = 1_760_692_130_112;

/// The simulator's control endpoint.
pub(crate) const SIMULATOR: &str = "unix:///tmp/trust-debug-ab12cd34.sock";

/// The project folder of the tests.
pub(super) const PROJECT: &str = "/home/u/plant/line1";

/// A view request with the extension's facts `host`.
pub(crate) fn request(host: HostFacts) -> ViewRequest {
    ViewRequest {
        session: "dc-test".to_owned(),
        seq: 1,
        gather: crate::request::GatherMode::Auto,
        project: None,
        workspace_folders: Vec::new(),
        view: ViewOptions::default(),
        host,
    }
}

/// The extension's facts in `mode`, with the project's control endpoint `endpoint`.
pub(crate) fn host(mode: RunMode, endpoint: &str) -> HostFacts {
    HostFacts {
        settings: HostSettings {
            mode,
            control_endpoint: endpoint.to_owned(),
            ..HostSettings::default()
        },
        ..HostFacts::default()
    }
}

/// `host` with the simulator's debug session launched and `starting` or running.
pub(crate) fn with_simulator(mut host: HostFacts, starting: bool) -> HostFacts {
    host.debug_session = Some(DebugSessionFacts {
        kind: DebugSessionKind::Launch,
        endpoint: Some(SIMULATOR.to_owned()),
        label: Some("Simulator".to_owned()),
        starting,
        failure: None,
    });
    host.simulator = Some(SimulatorFacts {
        endpoint: SIMULATOR.to_owned(),
        token: None,
    });
    host
}

/// `host` whose last start of the simulator failed with `kind` and `message`.
pub(super) fn with_failed_start(mut host: HostFacts, kind: &str, message: &str) -> HostFacts {
    host.debug_session = Some(DebugSessionFacts {
        kind: DebugSessionKind::Launch,
        endpoint: None,
        label: None,
        starting: false,
        failure: Some(DebugFailure {
            kind: kind.to_owned(),
            message: message.to_owned(),
        }),
    });
    host
}

/// `host` attached to the runtime at `endpoint`.
pub(super) fn attached(mut host: HostFacts, endpoint: &str) -> HostFacts {
    host.debug_session = Some(DebugSessionFacts {
        kind: DebugSessionKind::Attach,
        endpoint: Some(endpoint.to_owned()),
        label: None,
        starting: false,
        failure: None,
    });
    host
}

/// A workspace without a truST project.
pub(crate) fn no_project() -> ProjectInputs {
    ProjectInputs {
        projects: Vec::new(),
        chosen: None,
        topology: None,
        managed: Ok(Vec::new()),
        catalog: Ok(json!({ "schema_version": 4, "protocols": [] })),
    }
}

/// The project `PROJECT`, with its configured topology.
pub(super) fn project(topology: Value) -> ProjectInputs {
    ProjectInputs {
        projects: vec![PathBuf::from(PROJECT)],
        chosen: Some(PathBuf::from(PROJECT)),
        topology: Some(Ok(topology)),
        ..no_project()
    }
}

/// A gather of `project` and `targets`.
pub(crate) fn gathered(project: ProjectInputs, targets: Vec<TargetReport>) -> Gathered {
    Gathered { project, targets }
}

fn target(origin: TargetOrigin, endpoint: &str, label: Option<&str>) -> Target {
    Target {
        origin,
        endpoint: endpoint.to_owned(),
        enabled: true,
        token: None,
        label: label.map(str::to_owned),
        managed: None,
        events: false,
    }
}

/// A target whose probe found `probe`, with nothing read after it.
pub(super) fn probed(
    origin: TargetOrigin,
    endpoint: &str,
    label: Option<&str>,
    probe: TargetProbe,
) -> TargetReport {
    TargetReport {
        target: target(origin, endpoint, label),
        reach: Reach::Probe(probe),
        live: LiveData::default(),
    }
}

/// A target that answered `status` with `status` and `fleet.topology` with `topology`.
pub(crate) fn answered(
    origin: TargetOrigin,
    endpoint: &str,
    label: Option<&str>,
    status: Value,
    topology: Value,
) -> TargetReport {
    TargetReport {
        target: target(origin, endpoint, label),
        reach: Reach::Probe(TargetProbe::Reachable(status)),
        live: LiveData {
            topology: Some(Ok(topology)),
            ..LiveData::default()
        },
    }
}

/// A target that did not answer before the gather's deadline.
pub(super) fn unanswered(origin: TargetOrigin, endpoint: &str) -> TargetReport {
    TargetReport {
        target: target(origin, endpoint, None),
        reach: Reach::Unanswered,
        live: LiveData::default(),
    }
}

/// A runtime of the project's `fleet.toml` in `state`; a running one answered `status` with
/// `status` and `fleet.topology` with `topology`.
pub(crate) fn managed(
    name: &str,
    endpoint: &str,
    state: ManagedState,
    answers: Option<(Value, Value)>,
) -> TargetReport {
    let mut report = TargetReport {
        target: Target {
            managed: Some(ManagedTarget {
                fleet_root: PathBuf::from(PROJECT),
                runtime: FleetManifestRuntime {
                    name: name.to_owned(),
                    path: format!("runtimes/{name}"),
                    control_endpoint: endpoint.to_owned(),
                    web_port: 8_081,
                },
            }),
            ..target(TargetOrigin::Managed, endpoint, None)
        },
        reach: Reach::Managed(Box::new(ManagedStatus {
            state,
            report: None,
            error: None,
        })),
        live: LiveData::default(),
    };
    if let Some((status, topology)) = answers {
        report.reach = Reach::Managed(Box::new(ManagedStatus {
            state,
            report: Some(FleetRuntimeLifecycleResponse {
                schema_version: 1,
                name: name.to_owned(),
                path: format!("runtimes/{name}"),
                control_endpoint: endpoint.to_owned(),
                status: "running".to_owned(),
                pid: None,
                log_path: format!("runtimes/{name}/runtime.log"),
                message: String::new(),
                control_status: Some(status),
            }),
            error: None,
        }));
        report.live.topology = Some(Ok(topology));
    }
    report
}

/// A `status` answer: the program's `state` and the connection's `role`.
pub(crate) fn status(state: &str, role: &str) -> Value {
    let engineer = matches!(role, "engineer" | "admin");
    let reason = if engineer {
        Value::Null
    } else {
        json!("Viewer role — connect with an engineer token to write or force.")
    };
    json!({
        "state": state,
        "fault": null,
        "resource": "RESOURCE",
        "uptime_ms": 22_440_000,
        "debug_enabled": true,
        "metrics": { "cycle_ms": { "last": 4.2, "max": 4.9 }, "faults": 0 },
        "access": {
            "role": role,
            "io": { "write": engineer, "force": engineer, "release": engineer },
            "hmi": { "write": engineer },
            "reason": reason,
        },
    })
}

/// `status` as a runtime answers it that still answers after `shutdown`, so that `restart` starts
/// its program again, as a chip does (decision O4).
pub(super) fn keeps_control(mut status: Value) -> Value {
    status["program_control"] = json!("keeps_control");
    status
}

/// A `fleet.topology` answer of one host with its runtimes.
pub(crate) fn report(host_id: &str, hostname: &str, ips: &[&str], runtimes: Vec<Value>) -> Value {
    json!({
        "schema_version": 4,
        "hosts": [{
            "host_id": host_id, "hostname": hostname, "arch": "aarch64", "os": "linux",
            "ips": ips, "containers": [], "runtimes": Value::Array(runtimes), "source": "self",
            "last_seen_ms": NOW_MS - 2_000,
        }],
        "links": [], "shared": [], "external": [],
    })
}

/// A runtime of a `fleet.topology` answer with `endpoints`.
pub(crate) fn runtime(id: &str, name: &str, health: &str, endpoints: Vec<Value>) -> Value {
    json!({
        "runtime_id": id, "name": name, "mode": "online", "cycle_ms": 10,
        "health": health, "detail": "Runtime answered fleet.topology from its control channel.",
        "endpoints": Value::Array(endpoints), "source": "self",
    })
}

/// An endpoint of runtime `runtime` of a `fleet.topology` answer.
pub(crate) fn endpoint(runtime: &str, protocol: &str, name: &str, health: &str) -> Value {
    json!({
        "id": format!("endpoint:{runtime}:{protocol}"), "kind": "field", "protocol": protocol,
        "name": name, "role": "owned_driver", "health": health, "detail": "Driver is healthy.",
        "owned": true, "supports_test": true,
    })
}

/// The runtime named `name`.
pub(super) fn runtime_named<'a>(view: &'a DevicesView, name: &str) -> &'a Runtime {
    view.computers
        .iter()
        .flat_map(|computer| &computer.runtimes)
        .find(|runtime| runtime.name == name)
        .unwrap_or_else(|| panic!("a runtime named {name}"))
}

/// The computer headed `heading`.
pub(super) fn computer_headed<'a>(view: &'a DevicesView, heading: &str) -> &'a Computer {
    view.computers
        .iter()
        .find(|computer| computer.heading == heading)
        .unwrap_or_else(|| {
            let headings: Vec<&str> = view
                .computers
                .iter()
                .map(|computer| computer.heading.as_str())
                .collect();
            panic!("a computer headed {heading}; the headings are {headings:?}")
        })
}

/// The action `id` of `actions`.
pub(super) fn action<'a>(actions: &'a [Action], id: &str) -> &'a Action {
    actions
        .iter()
        .find(|action| action.id == id)
        .unwrap_or_else(|| panic!("an action {id}"))
}

/// The value of the row `label` of a runtime's inspector.
pub(super) fn row<'a>(runtime: &'a Runtime, label: &str) -> Option<&'a str> {
    runtime
        .inspector
        .sections
        .iter()
        .flat_map(|section| &section.rows)
        .find(|row| row.label == label)
        .map(|row| row.value.as_str())
}

/// The project's configured topology: one runtime `RESOURCE` that is not running, with one
/// simulated I/O connection, on this computer's host `host:line-pc`.
pub(super) fn configured_project() -> Value {
    json!({
        "schema_version": 4,
        "hosts": [{
            "host_id": "host:line-pc", "hostname": "line-pc", "arch": "x86_64", "os": "linux",
            "ips": ["127.0.0.1"], "containers": [], "source": "config",
            "runtimes": [{
                "runtime_id": "RESOURCE", "name": "RESOURCE",
                "control_endpoint": "unix:///tmp/trust-runtime-line1.sock", "mode": "stopped",
                "cycle_ms": 100, "health": "configured_policy",
                "detail": "Configured in project files; runtime is not running.",
                "source": "config",
                "endpoints": [{
                    "id": "endpoint:RESOURCE:simulated", "kind": "field", "protocol": "simulated",
                    "name": "simulated", "role": "owned_driver", "health": "configured_policy",
                    "detail": "Configured in io.toml; runtime is not running.", "owned": true,
                    "supports_test": false, "source": "config",
                }],
            }],
        }],
        "links": [], "shared": [], "external": [],
    })
}

/// The control endpoint that `configured_project` names for its runtime `RESOURCE`.
pub(super) const CONFIGURED_CONTROL: &str = "unix:///tmp/trust-runtime-line1.sock";

/// The project whose runtime `RESOURCE` is not running, configured with the protocols that browse:
/// ADS, `EtherCAT` and an OPC UA client, which are remote, and an OPC UA server, which is local.
pub(super) fn stopped_browsable_project() -> ProjectInputs {
    let mut configured = configured_project();
    configured["hosts"][0]["runtimes"][0]["endpoints"] = json!([
        configured_endpoint("ads", "ADS PLC"),
        configured_endpoint("ethercat", "EtherCAT bus"),
        configured_endpoint("opcua_client", "OPC UA client"),
        configured_endpoint("opcua", "OPC UA server"),
    ]);
    project(configured)
}

/// An endpoint that the project configures on `RESOURCE`, which is not running.
fn configured_endpoint(protocol: &str, name: &str) -> Value {
    json!({
        "id": format!("endpoint:RESOURCE:{protocol}"), "kind": "field", "protocol": protocol,
        "name": name, "role": "owned_driver", "health": "configured_policy",
        "detail": "Configured in io.toml; runtime is not running.", "owned": true,
        "supports_test": false, "source": "config",
    })
}
