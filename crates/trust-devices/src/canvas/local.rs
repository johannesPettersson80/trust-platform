//! What the canvas draws from the extension's own facts (specification 25 §5.1, §5.5, §5.6 and
//! §5.9): the local simulator's lifecycle, the managed runtimes and the session, which the merge
//! of the panel engine draws (`topology::complete`); and the canvas's own items: a Connect failure
//! on the runtime it was for, the draft of the add form and the first-run device on the drawn
//! simulator, the faults of a start failure and of a refused form, and the banner and summary of
//! the canvas.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::fleet::{CanvasFleet, FleetFault};
use crate::inputs::{configured_devices, is_neutral_stop};
use crate::request::{DebugFailure, RunMode};
use crate::texts::connections as connection_texts;
use crate::texts::graph as texts;
use crate::texts::issues as issue_texts;
use crate::topology::{
    Endpoint, ManagedRuntime, ManagedState, Session, Simulator, SimulatorState, Topology,
    SIMULATOR_FALLBACK_ID,
};

/// The protocols of the drawn simulator's connections; the add form's draft is one of them, else
/// simulated I/O.
const LOCAL_PROTOCOLS: [&str; 6] = [
    "simulated",
    "loopback",
    "modbus_tcp",
    "mqtt",
    "ethercat",
    "gpio",
];
/// The kind of failure that only says the runtime answered with its debugging disabled: its node
/// keeps its own health (specification 25 §5.1).
const DEBUG_DISABLED: &str = "debug_disabled";

/// The local simulator as the extension's lifecycle reports it.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct CanvasSimulator {
    /// Whether the lifecycle reports a running session.
    pub running: bool,
    /// The session's state: `running`, `connected` or `stopped`.
    pub runtime_state: String,
    /// Whether a start is under way.
    pub starting: bool,
    /// Whether the simulator reports I/O values.
    pub io_values: bool,
}

/// The last failure of a start or a Connect.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct CanvasFailure {
    /// `missing_binary`, `port_conflict`, `workspace_permission`, `failed_spawn`,
    /// `stale_runtime` or `debug_disabled`.
    pub kind: String,
    /// The text for the user.
    pub message: String,
    /// More of it, when there is more.
    pub detail: Option<String>,
    /// The control endpoint of the runtime a Connect failure belongs to.
    pub endpoint: Option<String>,
}

/// A runtime that this computer manages, as `trust-runtime fleet` lists it.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct CanvasManaged {
    /// Its name in `fleet.toml`.
    pub name: String,
    /// Its control endpoint.
    pub control_endpoint: String,
    /// `running`, `stopped`, `starting`, `stopping`, or anything else for an unknown state.
    pub state: String,
}

/// The extension's session.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct CanvasSession {
    /// The control endpoint of the runtime the extension is attached to.
    pub attached_endpoint: Option<String>,
    /// The selected run target: `simulator`, a managed runtime's name, a control endpoint or a
    /// runtime's ID.
    pub run_target: Option<String>,
}

/// The canvas's add flow.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct CanvasSetup {
    /// The step: `welcome`, `runtime_live`, `intent`, `add_device` or `connected`.
    pub stage: String,
    /// The protocol of the form that is open.
    pub active_protocol: String,
    /// Whether the user asked for a first device.
    pub device_requested: bool,
    /// The last result of the form, as `comm.apply` answers it.
    pub apply_result: Option<Value>,
}

/// The banner of the canvas: a hint or a failure, with its actions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CanvasBanner {
    /// `info` or `error`.
    pub kind: &'static str,
    /// The sentence.
    pub text: String,
    /// The actions it offers.
    pub actions: Vec<CanvasBannerAction>,
}

/// An action of the banner.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CanvasBannerAction {
    /// Its label.
    pub label: &'static str,
    /// What the canvas does: `startLocalSimulator`, `stopLocalSimulator` or `openRuntimeLogs`.
    pub action: &'static str,
}

impl CanvasManaged {
    /// The managed runtime the merge draws.
    pub(super) fn runtime(&self) -> ManagedRuntime {
        let endpoint = self.control_endpoint.trim();
        ManagedRuntime {
            name: self.name.clone(),
            control_endpoint: (!endpoint.is_empty()).then(|| endpoint.to_owned()),
            state: ManagedState::from_status(Some(self.state.as_str())),
        }
    }
}

impl CanvasSession {
    /// The session the merge marks.
    pub(super) fn session(&self) -> Session {
        Session {
            attached_endpoint: self.attached_endpoint.clone(),
            run_target: self.run_target.clone(),
        }
    }
}

impl CanvasFailure {
    /// Whether the failure blocks: a start failure that only says that the local simulator or
    /// runtime is stopped is the neutral stopped state (specification 25 §5.6).
    pub(super) fn active(failure: Option<&Self>) -> Option<&Self> {
        failure.filter(|failure| {
            let message = match failure.detail.as_deref() {
                Some(detail) => format!("{} {detail}", failure.message),
                None => failure.message.clone(),
            };
            !is_neutral_stop(&DebugFailure {
                kind: failure.kind.clone(),
                message,
            })
        })
    }
}

/// The simulator that the merge draws: starting while a start is under way, running while the
/// lifecycle proves it runs, else not started; with the blocking failure of a start (a failed
/// Connect names its runtime's endpoint and stays on that runtime), the connections that the
/// catalog lists with the health of their protocol, and whether it reports I/O values.
pub(super) fn simulator(
    facts: &CanvasSimulator,
    failure: Option<&CanvasFailure>,
    schema: Option<&Value>,
    capabilities: Option<&Value>,
    endpoint: Option<String>,
    run_mode: RunMode,
) -> Simulator {
    let failure = CanvasFailure::active(failure).filter(|failure| failure.endpoint.is_none());
    let proven = facts.running && matches!(facts.runtime_state.as_str(), "running" | "connected");
    let state = if facts.starting {
        SimulatorState::Starting
    } else if proven {
        SimulatorState::Running
    } else {
        SimulatorState::NotStarted
    };
    Simulator {
        endpoint,
        state,
        failure: failure.map(|failure| failure.message.clone()),
        io_values: facts.io_values,
        devices: schema
            .map(|schema| configured_devices(schema, capabilities))
            .unwrap_or_default(),
        runs_project: run_mode == RunMode::Simulate,
    }
}

/// A Connect that failed stays on the runtime it was for (specification 25 §5.1): that runtime's
/// node shows the failure, so its inspector offers the recovery. A runtime that answered with its
/// debugging disabled keeps its own health.
pub(super) fn show_connect_failure(topology: &mut Topology, failure: Option<&CanvasFailure>) {
    let Some(failure) = failure.filter(|failure| failure.kind != DEBUG_DISABLED) else {
        return;
    };
    let Some(endpoint) = failure.endpoint.as_deref() else {
        return;
    };
    for host in &mut topology.hosts {
        let runtimes = host.runtimes.iter_mut().chain(
            host.containers
                .iter_mut()
                .flat_map(|container| container.runtimes.iter_mut()),
        );
        for runtime in
            runtimes.filter(|runtime| runtime.control_endpoint.as_deref() == Some(endpoint))
        {
            "error".clone_into(&mut runtime.health);
            failure.message.clone_into(&mut runtime.detail);
        }
    }
}

/// The add flow's items on the drawn simulator: the draft of the open form, while its protocol has
/// no connection there, and the first device that the user asked for while the catalog lists none.
pub(super) fn add_setup_devices(
    topology: &mut Topology,
    setup: &CanvasSetup,
    schema: Option<&Value>,
    simulator: &Simulator,
) {
    let Some(runtime) = topology
        .hosts
        .iter_mut()
        .flat_map(|host| host.runtimes.iter_mut())
        .find(|runtime| runtime.id == SIMULATOR_FALLBACK_ID)
    else {
        return;
    };
    let mut added = Vec::new();
    if setup.stage == "add_device" {
        let protocol = local_protocol(&setup.active_protocol);
        if !runtime
            .endpoints
            .iter()
            .any(|endpoint| endpoint.protocol == protocol)
        {
            let name = protocol_title(schema, protocol)
                .unwrap_or_else(|| connection_texts::protocol_name(protocol));
            added.push(local_endpoint(
                format!("draft:{protocol}"),
                protocol,
                name,
                "pending",
                texts::DRAFT_SETUP,
            ));
        }
    }
    let requested = setup.device_requested || setup.stage == "connected";
    if requested && simulator.devices.is_empty() {
        let connected = simulator.state == SimulatorState::Running && simulator.io_values;
        added.push(local_endpoint(
            FIRST_RUN_DEVICE_ID.to_owned(),
            "simulated",
            texts::FIRST_RUN_DEVICE.to_owned(),
            if connected { "connected" } else { "pending" },
            if connected {
                texts::CONNECTED
            } else {
                texts::WAITING_FOR_RUNTIME_IO
            },
        ));
    }
    runtime.endpoints.splice(0..0, added);
}

/// The ID of the first device that the user asked for.
const FIRST_RUN_DEVICE_ID: &str = "first-run:simulated";

/// The faults that the canvas adds to the connections' own (specification 25 §5.5 and §5.9): a
/// blocking start or Connect failure, on the runtime it was for or else the drawn simulator, and a
/// form that the runtime refused, on its draft. The refused form's fault belongs to the form: the
/// canvas hides it while the user edits the form, as it hides no other fault.
pub(super) fn faults(
    topology: &Topology,
    failure: Option<&CanvasFailure>,
    setup: &CanvasSetup,
) -> Vec<FleetFault> {
    let mut faults = Vec::new();
    if let Some(failure) = CanvasFailure::active(failure) {
        let target = failure
            .endpoint
            .as_deref()
            .and_then(|endpoint| {
                topology
                    .runtimes()
                    .find(|runtime| runtime.control_endpoint.as_deref() == Some(endpoint))
            })
            .map_or_else(
                || SIMULATOR_FALLBACK_ID.to_owned(),
                |runtime| runtime.id.clone(),
            );
        faults.push(FleetFault {
            id: format!("runtime:{}", failure.kind),
            label: failure.message.clone(),
            target_node_id: target,
            severity: "error",
            form: false,
        });
    }
    if let Some(result) = &setup.apply_result
        && result.get("lifecycle_effect").and_then(Value::as_str) == Some("blocked")
    {
        let protocol = result
            .get("protocol")
            .and_then(Value::as_str)
            .unwrap_or_default();
        faults.push(FleetFault {
            id: format!("apply:{protocol}"),
            label: result
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            target_node_id: format!("draft:{}", local_protocol(protocol)),
            severity: "error",
            form: true,
        });
    }
    faults
}

/// The banner of the canvas. While the drawn simulator stands for the project, it follows the
/// simulator: a blocking failure with `Retry` and `Open logs`, else the reports that cannot be
/// read, else `Simulator is stopped.` with `Start simulator` or `Simulator running.` with `Stop
/// simulator`. Otherwise only the reports that cannot be read (specification 25 §5.10).
pub(super) fn banner(
    simulator_drawn: bool,
    simulator: &Simulator,
    topology_error: Option<&str>,
) -> Option<CanvasBanner> {
    let error = |text: &str, actions: Vec<CanvasBannerAction>| CanvasBanner {
        kind: "error",
        text: text.to_owned(),
        actions,
    };
    if simulator_drawn && let Some(failure) = &simulator.failure {
        return Some(error(
            failure,
            vec![
                CanvasBannerAction {
                    label: issue_texts::RETRY,
                    action: "startLocalSimulator",
                },
                CanvasBannerAction {
                    label: issue_texts::OPEN_LOGS,
                    action: "openRuntimeLogs",
                },
            ],
        ));
    }
    if let Some(text) = topology_error {
        return Some(error(text, Vec::new()));
    }
    if !simulator_drawn {
        return None;
    }
    let (text, label, action) = match simulator.state {
        SimulatorState::NotStarted => (
            issue_texts::SIMULATOR_IS_STOPPED,
            issue_texts::START_SIMULATOR,
            "startLocalSimulator",
        ),
        SimulatorState::Running => (
            issue_texts::SIMULATOR_RUNNING,
            issue_texts::STOP_SIMULATOR,
            "stopLocalSimulator",
        ),
        SimulatorState::Starting => return None,
    };
    Some(CanvasBanner {
        kind: "info",
        text: text.to_owned(),
        actions: vec![CanvasBannerAction { label, action }],
    })
}

/// The summary under the canvas: the computers, runtimes and connections it draws; while the drawn
/// simulator is the only runtime and does not run, it says so instead of counting its connections.
pub(super) fn summary(fleet: &CanvasFleet, simulator_drawn: bool, simulator: &Simulator) -> String {
    let runtimes: usize = fleet.hosts.iter().map(|host| host.runtime_count).sum();
    if simulator_drawn && runtimes == 1 && simulator.state != SimulatorState::Running {
        return format!(
            "{} · 1 runtime · {}",
            connection_texts::counted(fleet.hosts.len(), "host"),
            texts::SIMULATOR_STOPPED_SUMMARY
        );
    }
    fleet.summary.clone()
}

/// A protocol of the drawn simulator's connections: `protocol` trimmed, `-` as `_`, in lower case,
/// when it is one, else simulated I/O.
fn local_protocol(protocol: &str) -> &'static str {
    let normalized = protocol.trim().replace('-', "_").to_lowercase();
    LOCAL_PROTOCOLS
        .into_iter()
        .find(|known| *known == normalized)
        .unwrap_or("simulated")
}

/// The title that the catalog gives `protocol`.
fn protocol_title(schema: Option<&Value>, protocol: &str) -> Option<String> {
    schema?
        .get("protocols")?
        .as_array()?
        .iter()
        .find(|entry| entry.get("id").and_then(Value::as_str) == Some(protocol))?
        .get("title")?
        .as_str()
        .map(str::to_owned)
}

fn local_endpoint(
    id: String,
    protocol: &str,
    name: String,
    health: &str,
    detail: &str,
) -> Endpoint {
    Endpoint {
        id,
        kind: "field".to_owned(),
        protocol: protocol.to_owned(),
        name,
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
    }
}
