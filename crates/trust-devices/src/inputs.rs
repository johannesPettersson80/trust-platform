//! One gather turned into the inputs of the topology merge, and into the facts that the view reads
//! beside the topology: the run mode, the project target's answer, and each runtime's `status`
//! answer by its control endpoint. Nothing here reads a file, a socket or a clock.
//!
//! - The project's own runtime is the target that serves the panel's live data
//!   (`Gathered::live_primary`); the other configured targets and the managed runtimes are its
//!   peers, in their configured order.
//! - A target without an answer before the gather's deadline gives no report this round: it is
//!   never drawn as unreachable, and a managed runtime that did not answer is `Status
//!   unavailable` (specification 25 §5.1 and §6.5).
//! - The simulator's state comes from the debug session. A start failure that only says the
//!   local simulator or runtime is stopped is the neutral stopped state, not a failure, and its
//!   socket path is never shown (specification 25 §5.6).

use std::collections::BTreeMap;

use serde_json::{json, Value};
use trust_runtime_client::TargetProbe;

use crate::gather::{Gathered, LiveData, Reach, TargetReport};
use crate::request::{DebugFailure, DebugSessionKind, HostFacts, RunMode};
use crate::targets::TargetOrigin;
use crate::texts::graph as texts;
use crate::topology::{
    ConfiguredDevice, ManagedRuntime, ManagedState, RuntimeReport, Session, Simulator,
    SimulatorState, TopologyInputs, TopologyIssue,
};
use crate::view::status::RuntimeStatus;

/// What one view is projected from.
#[derive(Debug, Clone)]
pub(crate) struct Inputs {
    /// The inputs of the topology merge.
    pub(crate) topology: TopologyInputs,
    /// What the view reads beside the topology.
    pub(crate) facts: Facts,
    /// What the operator must be told about the project's own files.
    pub(crate) issues: Vec<TopologyIssue>,
}

/// What the view reads beside the topology.
#[derive(Debug, Clone, Default)]
pub(crate) struct Facts {
    /// Whether the project runs in the simulator or at its control endpoint.
    pub(crate) mode: RunMode,
    /// The simulator's lifecycle, with whether a start failure blocks it.
    pub(crate) simulator: SimulatorState,
    /// Whether the last start of the simulator failed.
    pub(crate) simulator_failed: bool,
    /// In online mode, the target of the project's control endpoint and how it answered.
    pub(crate) project_target: Option<ProjectTarget>,
    /// Each runtime's `status` answer, by the control endpoint it answered on.
    pub(crate) statuses: BTreeMap<String, RuntimeStatus>,
}

/// The target of the project's control endpoint.
#[derive(Debug, Clone)]
pub(crate) struct ProjectTarget {
    /// Its control endpoint; empty when none is set.
    pub(crate) endpoint: String,
    /// How it answered.
    pub(crate) reach: Reach,
}

/// Reads one gather and the extension's facts. `this_computer` is the host ID that this
/// computer's runtimes report, when the caller knows it; without it, the host of the project's
/// configuration and else the simulator's own report stand for this computer
/// (`topology::merge`).
pub(crate) fn read(gathered: &Gathered, host: &HostFacts, this_computer: Option<&str>) -> Inputs {
    let mut issues = Vec::new();
    let project = match &gathered.project.topology {
        Some(Ok(topology)) => Some(topology.clone()),
        Some(Err(error)) => {
            issues.push(project_issue(texts::topology_unavailable(error)));
            None
        }
        None => None,
    };
    if let Err(error) = &gathered.project.managed {
        issues.push(project_issue(texts::managed_runtimes_unavailable(error)));
    }
    let primary = gathered.live_primary();
    let simulator = simulator(gathered, host, primary);
    let topology = TopologyInputs {
        project,
        primary: primary.and_then(runtime_report),
        peers: gathered
            .targets
            .iter()
            .filter(|report| {
                matches!(
                    report.target.origin,
                    TargetOrigin::Managed | TargetOrigin::Configured
                )
            })
            .filter_map(runtime_report)
            .collect(),
        managed: gathered
            .targets
            .iter()
            .filter_map(managed_runtime)
            .collect(),
        session: Session {
            attached_endpoint: host
                .debug_session
                .as_ref()
                .filter(|session| session.kind == DebugSessionKind::Attach && !session.starting)
                .and_then(|session| session.endpoint.clone()),
            run_target: host.run_target.clone(),
        },
        this_computer: this_computer.map(str::to_owned),
        simulator,
    };
    let facts = Facts {
        mode: host.settings.mode,
        simulator: topology.simulator.state,
        simulator_failed: topology.simulator.failure.is_some(),
        project_target: gathered
            .targets
            .iter()
            .find(|report| report.target.origin == TargetOrigin::Project)
            .map(|report| ProjectTarget {
                endpoint: report.target.endpoint.clone(),
                reach: report.reach.clone(),
            }),
        statuses: statuses(gathered),
    };
    Inputs {
        topology,
        facts,
        issues,
    }
}

fn project_issue(text: String) -> TopologyIssue {
    TopologyIssue {
        item: None,
        project: true,
        text,
    }
}

/// What a target answered, as the topology merge reads it. A target without an answer this round
/// gives none, and so does a managed runtime that does not run: the managed runtimes are drawn
/// from their status.
fn runtime_report(report: &TargetReport) -> Option<RuntimeReport> {
    let probe = match &report.reach {
        Reach::Probe(probe) => probe.clone(),
        Reach::Managed(status) if status.state == ManagedState::Running => TargetProbe::Reachable(
            status
                .report
                .as_ref()
                .and_then(|report| report.control_status.clone())
                .unwrap_or_else(|| json!({})),
        ),
        Reach::Managed(_) | Reach::Unanswered => return None,
    };
    let LiveData {
        topology,
        connectors,
        ..
    } = &report.live;
    Some(RuntimeReport {
        endpoint: report.target.endpoint.clone(),
        label: report.target.label.clone().unwrap_or_default(),
        probe,
        topology: topology.clone(),
        connectors: connectors.clone(),
        stable_id: None,
    })
}

fn managed_runtime(report: &TargetReport) -> Option<ManagedRuntime> {
    let managed = report.target.managed.as_ref()?;
    let endpoint = report.target.endpoint.trim();
    Some(ManagedRuntime {
        name: managed.runtime.name.clone(),
        control_endpoint: (!endpoint.is_empty()).then(|| endpoint.to_owned()),
        state: report.managed_state().unwrap_or(ManagedState::Unavailable),
    })
}

/// The local simulator, from the debug session: starting while its launch starts, running while
/// it runs without a failure, else not started. The connections it draws while nothing reports
/// the project's runtime come from the catalog of the runtime that serves the live data.
fn simulator(gathered: &Gathered, host: &HostFacts, primary: Option<&TargetReport>) -> Simulator {
    let launch = host
        .debug_session
        .as_ref()
        .filter(|session| session.kind == DebugSessionKind::Launch);
    let failure = launch
        .and_then(|session| session.failure.as_ref())
        .filter(|failure| !is_neutral_stop(failure))
        .map(|failure| failure.message.clone());
    let state = match launch {
        Some(session) if session.starting => SimulatorState::Starting,
        Some(session) if session.failure.is_none() => SimulatorState::Running,
        _ => SimulatorState::NotStarted,
    };
    let devices = primary
        .and_then(|report| match &report.live.schema {
            Some(Ok(schema)) => Some(configured_devices(
                schema,
                report
                    .live
                    .capabilities
                    .as_ref()
                    .and_then(|reply| reply.as_ref().ok()),
            )),
            _ => None,
        })
        .unwrap_or_default();
    Simulator {
        endpoint: gathered
            .targets
            .iter()
            .find(|report| report.target.origin == TargetOrigin::Simulator)
            .map(|report| report.target.endpoint.clone()),
        state,
        failure,
        io_values: !host.io_state.is_empty(),
        devices,
        runs_project: host.settings.mode == RunMode::Simulate,
    }
}

/// Whether a start failure only says that the local simulator or runtime is stopped: a stale
/// runtime whose message says so, or says that a runtime at a local socket is not reachable.
/// That is the neutral stopped state, not a failure (specification 25 §5.6).
pub(crate) fn is_neutral_stop(failure: &DebugFailure) -> bool {
    if failure.kind != "stale_runtime" {
        return false;
    }
    let text = failure.message.to_ascii_lowercase();
    [
        "local runtime is stopped",
        "local simulator is stopped",
        "simulator is stopped",
    ]
    .iter()
    .any(|phrase| text.contains(phrase))
        || ((text.contains("runtime not reachable") || text.contains("runtime is not reachable"))
            && text.contains("unix://"))
}

/// The connections of a `comm.schema` answer's configured instances, each with the health that
/// `comm.capabilities` reports for its protocol.
fn configured_devices(schema: &Value, capabilities: Option<&Value>) -> Vec<ConfiguredDevice> {
    let health = |protocol: &str| {
        capabilities
            .and_then(|answer| answer.get("capabilities"))
            .and_then(Value::as_array)
            .and_then(|entries| {
                entries
                    .iter()
                    .find(|entry| entry.get("id").and_then(Value::as_str) == Some(protocol))
            })
            .and_then(|entry| entry.get("health"))
            .and_then(Value::as_str)
            .map(str::to_owned)
    };
    let text = |value: &Value, key: &str| {
        value
            .get(key)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned()
    };
    schema
        .get("protocols")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .flat_map(|protocol| {
            let id = text(protocol, "id");
            protocol
                .get("instances")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .map(move |instance| (id.clone(), instance))
        })
        .map(|(protocol, instance)| ConfiguredDevice {
            id: text(instance, "id"),
            name: text(instance, "display_name"),
            health: health(&protocol),
            protocol,
        })
        .collect()
}

/// Each runtime's `status` answer, by the control endpoint it answered on: the probe's answer,
/// or the one a managed runtime's status read carries.
fn statuses(gathered: &Gathered) -> BTreeMap<String, RuntimeStatus> {
    gathered
        .targets
        .iter()
        .filter_map(|report| {
            let status = match &report.reach {
                Reach::Probe(TargetProbe::Reachable(status)) => Some(status),
                Reach::Managed(managed) => managed
                    .report
                    .as_ref()
                    .and_then(|report| report.control_status.as_ref()),
                Reach::Probe(_) | Reach::Unanswered => None,
            }?;
            Some((
                report.target.endpoint.trim().to_owned(),
                RuntimeStatus::read(status),
            ))
        })
        .collect()
}
