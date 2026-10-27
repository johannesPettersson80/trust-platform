//! `trust-lsp.devicesTopology` (specification 25 §5.5 and §5.6): the canvas sends what the runtime
//! that serves its live data, the open project's configuration and every other configured runtime
//! answered, and gets back the merged topology and the fleet view it draws, with what the merged
//! topology says about the ADS devices that its last search found. Nothing here reads a file, a
//! socket or a clock.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use trust_runtime_client::{AuthFailure, ControlError, TargetProbe};

use super::ads_tags::{self, PortSelection};
use super::fleet::{self, CanvasFleet};
use super::local::{
    self, CanvasFailure, CanvasManaged, CanvasSession, CanvasSetup, CanvasSimulator,
};
use super::views;
use super::wire;
use super::CANVAS_SCHEMA_VERSION;
use crate::request::RunMode;
use crate::topology::{
    self, MergedReports, RuntimeOrigin, RuntimeReport, Topology, TopologyInputs, TopologyIssue,
};

/// What the canvas gathered this round.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct CanvasTopologyRequest {
    /// The open project's configured topology (`trust-runtime comm topology`), when it read one.
    pub project: Option<Value>,
    /// The runtime that serves the canvas's live data: the runtime target, or the extension's own
    /// simulator while the target does not answer.
    pub primary: Option<CanvasRuntime>,
    /// Every other configured runtime (`trust.runtime.fleetEndpoints`), in its configured order.
    pub peers: Vec<CanvasRuntime>,
    /// The control endpoint of the extension's own simulator while it serves the live data. Its
    /// report is the simulator's own, whose node names no control endpoint (specification 25 §5.1).
    pub simulator_endpoint: Option<String>,
    /// Whether the project runs in the simulator (`trust.runtime.mode`): the project's configured
    /// runtime is then named `Simulator` (specification 25 §5.6).
    pub run_mode: RunMode,
    /// The canvas's search: the connections it does not match are dimmed (specification 25 §5.5).
    pub search_query: String,
    /// What the canvas's last search found (`comm.discover` candidates with their `id`,
    /// `protocol`, `params` and `originRuntimeId`), so that the answer says which ADS devices the
    /// project already uses and which results can no longer be added.
    pub discovered: Vec<Value>,
    /// The local simulator, as the extension's lifecycle reports it.
    pub simulator: CanvasSimulator,
    /// The last failure of a start or a Connect.
    pub failure: Option<CanvasFailure>,
    /// The `comm.schema` answer of the runtime that serves the live data: the connections the
    /// drawn simulator holds, and the protocols that offer discovery.
    pub schema: Option<Value>,
    /// The `comm.capabilities` answer of that runtime: the health of each protocol.
    pub capabilities: Option<Value>,
    /// The runtimes this computer manages, with their state.
    pub managed: Vec<CanvasManaged>,
    /// The extension's session.
    pub session: CanvasSession,
    /// The canvas's add flow.
    pub setup: CanvasSetup,
}

/// What one runtime target answered.
#[derive(Debug, Clone, Deserialize)]
pub struct CanvasRuntime {
    /// The control endpoint the target is reached through.
    pub endpoint: String,
    /// The target's label; empty when it has none.
    #[serde(default)]
    pub label: String,
    /// How the target answered its probe.
    pub reach: CanvasReach,
    /// The `fleet.topology` answer; absent when it was not requested.
    #[serde(default)]
    pub topology: Option<Answer>,
    /// The `connectors.status` answer; absent when it was not requested.
    #[serde(default)]
    pub connectors: Option<Answer>,
}

/// How a target answered its probe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CanvasReach {
    /// A runtime answered `status` and accepted the token.
    Reachable,
    /// Nothing answered.
    Unreachable,
    /// A runtime answered and asked for a token the request did not carry.
    AuthMissing,
    /// A runtime answered and refused the token.
    AuthRejected,
    /// No endpoint is configured.
    MissingEndpoint,
}

/// One request's answer: its result, or why it failed.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Answer {
    /// The request's result.
    Ok(Value),
    /// Why the request failed.
    Error(String),
}

/// The merged topology and the fleet view the canvas draws.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CanvasTopology {
    /// [`CANVAS_SCHEMA_VERSION`].
    pub schema_version: u32,
    /// The project's own runtime merged with its configuration, in the `fleet.topology` shape with
    /// owner-scoped identities; `None` when neither gives a host.
    pub local: Option<Value>,
    /// The fleet view of `local` merged with every other configured runtime; `None` when no host
    /// is shown.
    pub fleet: Option<CanvasFleet>,
    /// The banner text of the reports that cannot be read (specification 25 §5.10): the project's
    /// own first, then those of the other runtimes after `Peer topology degraded:`.
    pub topology_error: Option<String>,
    /// Each ADS device of `discovered`: whether a configured connection already serves it, and
    /// the tags already imported from it per ADS port.
    pub discovered: Vec<DiscoveredAds>,
}

/// What the merged topology says about one result of the last search.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DiscoveredAds {
    /// The search result's ID.
    pub id: String,
    /// Whether a configured ADS connection serves the device: its AMS Net ID and host do not
    /// contradict the device's, and it shares one of them. Always false for another protocol.
    pub on_canvas: bool,
    /// The tags already imported from an ADS device, per ADS port, each port and its paths
    /// sorted.
    pub imported_ads_symbols: Vec<PortSelection>,
    /// Why the result cannot be added now (specification 25 §2).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disabled_reason: Option<&'static str>,
    /// The runtime a device found by the search is added to: the runtime that found it while it is
    /// drawn, else the first runtime drawn; none when the runtime that found it is gone.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub draft_runtime_id: Option<String>,
    /// A runtime the search found, as the host the canvas adds for it (specification 25 §5.2).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host: Option<FoundHost>,
}

/// The host the canvas adds for a runtime a search found: its control endpoint, and its label, the
/// result's own label, else its name, else none.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FoundHost {
    /// The control endpoint the runtime announced.
    pub endpoint: String,
    /// The label the host gets; empty when the result has none.
    pub label: String,
}

/// Merges one round of the canvas's answers (specification 25 §5.6) and projects the fleet view.
#[must_use]
pub fn topology(request: &CanvasTopologyRequest) -> CanvasTopology {
    let simulator = local::simulator(
        &request.simulator,
        request.failure.as_ref(),
        request.schema.as_ref(),
        request.capabilities.as_ref(),
        request
            .simulator_endpoint
            .as_deref()
            .map(str::trim)
            .filter(|endpoint| !endpoint.is_empty())
            .map(str::to_owned),
        request.run_mode,
    );
    let inputs = TopologyInputs {
        project: request.project.clone(),
        primary: request.primary.as_ref().map(report),
        peers: request.peers.iter().map(report).collect(),
        simulator,
        managed: request.managed.iter().map(CanvasManaged::runtime).collect(),
        session: request.session.session(),
        ..TopologyInputs::default()
    };
    let MergedReports {
        mut local,
        topology: mut merged,
        nothing_local,
    } = topology::merge_reports(&inputs);
    topology::complete(&mut merged, nothing_local, &inputs);
    hide_simulator_endpoint(&mut local);
    hide_simulator_endpoint(&mut merged);
    local::show_connect_failure(&mut merged, request.failure.as_ref());
    if nothing_local {
        local::add_setup_devices(
            &mut merged,
            &request.setup,
            request.schema.as_ref(),
            &inputs.simulator,
        );
    }
    let topology_error = banner(&merged.issues);
    let configured = configured_connections(&merged);
    let mut fleet = fleet::project(
        &merged,
        request.run_mode,
        &request.search_query,
        request.failure.as_ref(),
    );
    let mut faults = local::faults(&merged, request.failure.as_ref(), &request.setup);
    faults.append(&mut fleet.faults);
    fleet.faults = faults;
    fleet.banner = local::banner(nothing_local, &inputs.simulator, topology_error.as_deref());
    fleet.summary = local::summary(&fleet, nothing_local, &inputs.simulator);
    let runtime_ids: Vec<&str> = fleet
        .hosts
        .iter()
        .flat_map(|host| {
            host.runtimes.iter().chain(
                host.containers
                    .iter()
                    .flat_map(|container| &container.runtimes),
            )
        })
        .map(|runtime| runtime.id.as_str())
        .collect();
    let discovering = discovering_protocols(request.schema.as_ref());
    let discovered = request
        .discovered
        .iter()
        .filter_map(|candidate| discovered(candidate, &configured, &runtime_ids, &discovering))
        .collect();
    CanvasTopology {
        schema_version: CANVAS_SCHEMA_VERSION,
        local: (!local.hosts.is_empty()).then(|| wire::topology_json(&local)),
        fleet: (!fleet.hosts.is_empty()).then_some(fleet),
        topology_error,
        discovered,
    }
}

/// The protocols that the catalog says offer discovery (`actions` holds `discover`).
fn discovering_protocols(schema: Option<&Value>) -> Vec<String> {
    schema
        .and_then(|schema| schema.get("protocols"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|protocol| {
            protocol
                .get("actions")
                .and_then(Value::as_array)
                .is_some_and(|actions| actions.iter().any(|action| action == "discover"))
        })
        .filter_map(|protocol| {
            protocol
                .get("id")
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .collect()
}

/// Every configured client connection of the topology: the `connections` of every endpoint's
/// parameters, in the topology's order.
fn configured_connections(topology: &Topology) -> Value {
    Value::Array(
        topology
            .runtimes()
            .flat_map(|runtime| &runtime.endpoints)
            .filter_map(|endpoint| endpoint.params.as_ref())
            .filter_map(|params| params.get("connections").and_then(Value::as_array))
            .flatten()
            .cloned()
            .collect(),
    )
}

/// What the canvas says about the result `candidate` of the last search: for an ADS device,
/// whether `configured` serves it and the tags imported from it; for every result, why it cannot
/// be added now, when the runtime that found it (`originRuntimeId`) is not among `runtimes` or its
/// protocol is not among `discovering`. `None` for a result without an ID.
fn discovered(
    candidate: &Value,
    configured: &Value,
    runtimes: &[&str],
    discovering: &[String],
) -> Option<DiscoveredAds> {
    let id = candidate.get("id").and_then(Value::as_str)?.to_owned();
    let protocol = candidate
        .get("protocol")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let origin_drawn = candidate
        .get("originRuntimeId")
        .and_then(Value::as_str)
        .filter(|origin| !origin.is_empty())
        .is_none_or(|origin| runtimes.contains(&origin));
    let disabled_reason = views::disabled_reason(
        origin_drawn,
        discovering.iter().any(|known| known == protocol),
    );
    let draft_runtime_id = match candidate
        .get("originRuntimeId")
        .and_then(Value::as_str)
        .filter(|origin| !origin.is_empty())
    {
        Some(origin) => origin_drawn.then(|| origin.to_owned()),
        None => runtimes.first().map(|&runtime| runtime.to_owned()),
    };
    let host = found_host(candidate);
    if protocol != "ads" {
        return Some(DiscoveredAds {
            id,
            on_canvas: false,
            imported_ads_symbols: Vec::new(),
            disabled_reason,
            draft_runtime_id,
            host,
        });
    }
    let device = candidate
        .get("params")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    Some(DiscoveredAds {
        id,
        on_canvas: !ads_tags::device_connections(configured, &device).is_empty(),
        imported_ads_symbols: ads_tags::imported_selections(configured, &device),
        disabled_reason,
        draft_runtime_id,
        host,
    })
}

/// The host for a result that announced a control endpoint (`params.control_endpoint`).
fn found_host(candidate: &Value) -> Option<FoundHost> {
    let params = candidate.get("params");
    let text = |value: Option<&Value>| {
        value
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .map(str::to_owned)
    };
    let endpoint = text(params.and_then(|params| params.get("control_endpoint")))?;
    let label = text(candidate.get("label"))
        .or_else(|| text(params.and_then(|params| params.get("name"))))
        .unwrap_or_default();
    Some(FoundHost { endpoint, label })
}

/// The merge's report of one target. A failed request reads as the reason it gave.
fn report(runtime: &CanvasRuntime) -> RuntimeReport {
    RuntimeReport {
        endpoint: runtime.endpoint.trim().to_owned(),
        label: runtime.label.trim().to_owned(),
        probe: match runtime.reach {
            CanvasReach::Reachable => TargetProbe::Reachable(Value::Object(serde_json::Map::new())),
            CanvasReach::Unreachable => TargetProbe::Unreachable,
            CanvasReach::AuthMissing => TargetProbe::AuthFailed(AuthFailure::Missing),
            CanvasReach::AuthRejected => TargetProbe::AuthFailed(AuthFailure::Rejected),
            CanvasReach::MissingEndpoint => TargetProbe::MissingEndpoint,
        },
        topology: runtime.topology.as_ref().map(answer),
        connectors: runtime.connectors.as_ref().map(answer),
        stable_id: None,
    }
}

fn answer(answer: &Answer) -> Result<Value, ControlError> {
    match answer {
        Answer::Ok(value) => Ok(value.clone()),
        Answer::Error(reason) => Err(ControlError::Transport(reason.clone())),
    }
}

/// The extension's own simulator is started and stopped by the extension, never connected to: its
/// node names no control endpoint, so that it keeps the simulator's Start and Stop (specification
/// 25 §5.1).
fn hide_simulator_endpoint(topology: &mut Topology) {
    for host in &mut topology.hosts {
        let runtimes = host.runtimes.iter_mut().chain(
            host.containers
                .iter_mut()
                .flat_map(|container| container.runtimes.iter_mut()),
        );
        for runtime in runtimes.filter(|runtime| runtime.origin == RuntimeOrigin::Simulator) {
            runtime.control_endpoint = None;
        }
    }
}

/// The banner text of this round's issues (specification 25 §5.10): the project's own, then the
/// other runtimes' after `Peer topology degraded:`, joined with `; `.
fn banner(issues: &[TopologyIssue]) -> Option<String> {
    let texts = |project: bool| -> Vec<&str> {
        issues
            .iter()
            .filter(|issue| issue.project == project)
            .map(|issue| issue.text.as_str())
            .collect()
    };
    let mut parts: Vec<String> = texts(true).into_iter().map(str::to_owned).collect();
    let peers = texts(false);
    if !peers.is_empty() {
        parts.push(format!("Peer topology degraded: {}", peers.join("; ")));
    }
    (!parts.is_empty()).then(|| parts.join("; "))
}
