//! Nodes that the panel draws without a report of their own (specification 25 §5.6): a
//! configured runtime that did not answer, the local simulator while nothing reports the
//! project's runtime, and the managed runtimes; and the session's marks on every runtime.

use std::collections::BTreeSet;

use trust_runtime_client::{AuthFailure, TargetProbe};

use super::{
    ConfiguredDevice, Endpoint, Host, HostOrigin, ManagedRuntime, ManagedState, Runtime,
    RuntimeOrigin, RuntimeReport, Session, Simulator, SimulatorState, Topology,
};

/// The host drawn for this computer while no report gives it.
const THIS_COMPUTER_HOST: &str = "host:this-computer";
/// The host drawn for the managed runtimes while no host stands for this computer.
const MANAGED_HOST: &str = "host:managed-local";
/// The ID of the simulator drawn while nothing reports the project's runtime.
const SIMULATOR_FALLBACK_ID: &str = "runtime:local";
/// The run target that selects the simulator.
const SIMULATOR_RUN_TARGET: &str = "simulator";
/// The protocols of the local I/O connections that the simulator's node draws.
const LOCAL_IO_PROTOCOLS: [&str; 6] = [
    "simulated",
    "loopback",
    "modbus_tcp",
    "mqtt",
    "ethercat",
    "gpio",
];

/// The node of a configured runtime target that did not answer: `unknown` while nothing answers
/// (it may be stopped or only unreachable, so it is never green and never `stopped`), an error
/// naming the missing or rejected token after a refusal. It keeps the target's endpoint for the
/// inspector's actions. `None` for a target without an endpoint and for one that answered.
pub(super) fn unanswered_peer(report: &RuntimeReport) -> Option<Topology> {
    let (state, detail, origin) = match &report.probe {
        TargetProbe::Unreachable => (
            "unknown",
            "Configured endpoint not reachable — open it in Devices & Connections to connect or \
             diagnose.",
            RuntimeOrigin::Unreachable,
        ),
        TargetProbe::AuthFailed(failure) => (
            "error",
            auth_failure_detail(*failure),
            RuntimeOrigin::AuthFailed(*failure),
        ),
        TargetProbe::MissingEndpoint | TargetProbe::Reachable(_) => return None,
    };
    let endpoint = report.endpoint.trim();
    if endpoint.is_empty() {
        return None;
    }
    let label = report.label.trim();
    let host_id = format!("fleet:{endpoint}");
    let name = if label.is_empty() { "runtime" } else { label };
    let runtime = Runtime {
        control_endpoint: Some(endpoint.to_owned()),
        mode: state.to_owned(),
        health: state.to_owned(),
        detail: detail.to_owned(),
        ..Runtime::drawn(format!("{host_id}:runtime"), name.to_owned(), origin)
    };
    let hostname = if label.is_empty() { endpoint } else { label };
    let host = Host {
        runtimes: vec![runtime],
        ..Host::drawn(&host_id, hostname, HostOrigin::Unanswered)
    };
    Some(Topology {
        schema_version: 3,
        hosts: vec![host],
        ..Topology::default()
    })
}

fn auth_failure_detail(failure: AuthFailure) -> &'static str {
    match failure {
        AuthFailure::Missing => "No auth token provided — this runtime requires one.",
        AuthFailure::Rejected => "Auth token rejected — check it and try again.",
    }
}

/// Draws the local simulator, on this computer, while nothing reports the project's runtime. It
/// is never started by being drawn: it is stopped until the user starts it, an error after a
/// failed start, and connected only while it runs. It holds the project's local I/O
/// connections, each connected only with live evidence.
pub(super) fn add_simulator(
    topology: &mut Topology,
    simulator: &Simulator,
    this_computer: Option<&str>,
) {
    let (health, detail) = match (&simulator.failure, simulator.state) {
        (Some(failure), _) => ("error", failure.as_str()),
        (None, SimulatorState::Running) => ("connected", "● Running"),
        (None, SimulatorState::Starting) => ("pending", "Starting Simulator…"),
        (None, SimulatorState::NotStarted) => {
            ("stopped", "Stopped — start the simulator to run it.")
        }
    };
    let runtime = Runtime {
        mode: "simulate".to_owned(),
        health: health.to_owned(),
        detail: detail.to_owned(),
        endpoints: simulator
            .devices
            .iter()
            .filter_map(|device| local_connection(device, simulator))
            .collect(),
        ..Runtime::drawn(
            SIMULATOR_FALLBACK_ID.to_owned(),
            "Simulator".to_owned(),
            RuntimeOrigin::SimulatorFallback,
        )
    };
    let index = this_computer_host(topology, this_computer).unwrap_or_else(|| {
        let host = Host::drawn(
            THIS_COMPUTER_HOST,
            "This computer",
            HostOrigin::ThisComputer,
        );
        topology.hosts.insert(0, host);
        0
    });
    topology.hosts[index].runtimes.insert(0, runtime);
}

/// A configured local I/O connection on the drawn simulator. It is connected only when the
/// simulator runs, its protocol reports `connected`, and, for simulated and loopback I/O, the
/// simulator reports I/O values (specification 25 §5.5).
fn local_connection(device: &ConfiguredDevice, simulator: &Simulator) -> Option<Endpoint> {
    let protocol = device.protocol.as_str();
    if !LOCAL_IO_PROTOCOLS.contains(&protocol) {
        return None;
    }
    let health = device.health.as_deref();
    let evidence = simulator.io_values || !matches!(protocol, "simulated" | "loopback");
    let (health, detail) =
        if simulator.state == SimulatorState::Running && health == Some("connected") && evidence {
            ("connected", "Connected")
        } else {
            match health {
                Some("error") => ("error", "Error"),
                Some("degraded") => ("degraded", "Degraded"),
                _ => ("pending", "Pending proof"),
            }
        };
    Some(Endpoint {
        id: device.id.clone(),
        kind: "field".to_owned(),
        protocol: device.protocol.clone(),
        name: device.name.clone(),
        address: None,
        role: Some("owned_driver".to_owned()),
        health: health.to_owned(),
        detail: detail.to_owned(),
        live: None,
        params: None,
        children: Vec::new(),
        owned: true,
        supports_test: false,
        source: None,
        connector: None,
    })
}

/// Marks every runtime that is a managed runtime, matched by control endpoint or else by name,
/// with the managed runtime and its lifecycle state, which also decides its health and detail.
/// A managed runtime that no runtime shows joins this computer's host.
pub(super) fn inject_managed(
    topology: &mut Topology,
    managed: &[ManagedRuntime],
    this_computer: Option<&str>,
) {
    if managed.is_empty() {
        return;
    }
    let mut shown = BTreeSet::new();
    for runtime in topology.runtimes_mut() {
        let endpoint = runtime
            .control_endpoint
            .as_deref()
            .filter(|endpoint| !endpoint.trim().is_empty());
        let local = endpoint
            .and_then(|endpoint| {
                managed
                    .iter()
                    .rev()
                    .find(|local| local.control_endpoint.as_deref() == Some(endpoint))
            })
            .or_else(|| {
                managed
                    .iter()
                    .rev()
                    .find(|local| local.name == runtime.name)
            });
        if let Some(local) = local {
            mark_managed(runtime, local);
            shown.insert(local.name.as_str());
        }
    }
    let fresh: Vec<Runtime> = managed
        .iter()
        .filter(|local| !shown.contains(local.name.as_str()))
        .map(|local| {
            let mut runtime = Runtime {
                mode: "managed".to_owned(),
                control_endpoint: local
                    .control_endpoint
                    .clone()
                    .filter(|endpoint| !endpoint.trim().is_empty()),
                ..Runtime::drawn(
                    format!("managed:{}", local.name),
                    local.name.clone(),
                    RuntimeOrigin::Managed,
                )
            };
            mark_managed(&mut runtime, local);
            runtime
        })
        .collect();
    if fresh.is_empty() {
        return;
    }
    let index = this_computer_host(topology, this_computer).unwrap_or_else(|| {
        let host = Host::drawn(MANAGED_HOST, "This computer", HostOrigin::ThisComputer);
        topology.hosts.push(host);
        topology.hosts.len() - 1
    });
    topology.hosts[index].runtimes.extend(fresh);
}

fn mark_managed(runtime: &mut Runtime, local: &ManagedRuntime) {
    let (health, detail) = match local.state {
        ManagedState::Running => ("connected", "Running (managed local runtime)."),
        ManagedState::Stopped => ("stopped", "Stopped — Start it from this node."),
        ManagedState::Starting => ("pending", "Starting managed local runtime…"),
        ManagedState::Stopping => ("pending", "Stopping managed local runtime…"),
        ManagedState::Unavailable => ("error", "Status unavailable — refresh before starting."),
    };
    health.clone_into(&mut runtime.health);
    detail.clone_into(&mut runtime.detail);
    runtime.managed = Some(local.clone());
}

/// This computer's host: the reported host with the ID `this_computer`, else a host drawn for
/// this computer.
fn this_computer_host(topology: &Topology, this_computer: Option<&str>) -> Option<usize> {
    topology
        .hosts
        .iter()
        .position(|host| this_computer == Some(host.id.as_str()))
        .or_else(|| {
            topology
                .hosts
                .iter()
                .position(|host| host.origin == HostOrigin::ThisComputer)
        })
}

/// A runtime never also appears as an external or shared system: such a system named like a
/// runtime or a managed runtime is removed, and so are the links to it, so that every link
/// still names two nodes that appear.
pub(super) fn remove_runtime_twins(topology: &mut Topology, managed: &[ManagedRuntime]) {
    let names: BTreeSet<String> = topology
        .runtimes()
        .map(|runtime| runtime.name.clone())
        .chain(managed.iter().map(|local| local.name.clone()))
        .collect();
    let mut removed = BTreeSet::new();
    topology.external.retain(|external| {
        let twin = names.contains(&external.name);
        if twin {
            removed.insert(external.id.clone());
        }
        !twin
    });
    topology.shared.retain(|shared| {
        let twin = names.contains(&shared.name);
        if twin {
            removed.insert(shared.id.clone());
        }
        !twin
    });
    topology
        .links
        .retain(|link| !removed.contains(&link.from) && !removed.contains(&link.to));
}

/// Marks each runtime with the session: whether the extension holds a session to it (the
/// simulator's is its running debug session), and whether it is the selected run target, which
/// is the simulator when nothing else is selected.
pub(super) fn mark_session(topology: &mut Topology, session: &Session, simulator: &Simulator) {
    let selected = session
        .run_target
        .as_deref()
        .map(str::trim)
        .filter(|target| !target.is_empty())
        .unwrap_or(SIMULATOR_RUN_TARGET);
    let running = simulator.state == SimulatorState::Running && simulator.failure.is_none();
    for runtime in topology.runtimes_mut() {
        let is_simulator = matches!(
            runtime.origin,
            RuntimeOrigin::Simulator | RuntimeOrigin::SimulatorFallback
        );
        let endpoint = runtime.control_endpoint.as_deref();
        runtime.attached = if is_simulator {
            running
        } else {
            endpoint.is_some() && endpoint == session.attached_endpoint.as_deref()
        };
        runtime.run_target = (is_simulator && selected == SIMULATOR_RUN_TARGET)
            || runtime
                .managed
                .as_ref()
                .is_some_and(|local| local.name == selected)
            || endpoint == Some(selected)
            || runtime.id == selected;
    }
}
