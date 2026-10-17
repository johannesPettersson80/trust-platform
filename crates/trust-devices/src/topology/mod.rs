//! The topology merge: identity of hosts, runtimes, endpoints and devices across the project, the
//! runtimes' reports and the finds, as specification 25 §5.6 states it.
//!
//! [`merge()`] is a pure function from one round of [`TopologyInputs`] to one [`Topology`]:
//!
//! 1. the project's own runtime, with the project's configuration overlaid on it;
//! 2. every other configured runtime, or a node drawn for it when it did not answer;
//! 3. the local simulator while nothing reports the project's runtime;
//! 4. the runtimes this computer manages, and the session's marks.
//!
//! The modules hold the rules: `wire` decodes what runtimes answer, `identity` gives every node
//! its owner-scoped ID, `merge` unions the snapshots and attaches connector reports, and
//! `synthetic` draws the nodes that no report gives.

mod identity;
mod merge;
mod synthetic;
mod wire;

pub(crate) use identity::encode;

use serde_json::Value;
use trust_runtime::connectors::{
    ConnectorHealth, ConnectorPointCounts, ConnectorState, DiscoveryConfidence,
};
use trust_runtime::fleet_topology::{FleetEndpointChild, FleetExternal, FleetLink, FleetShared};
use trust_runtime_client::{AuthFailure, ControlError, TargetProbe};

use crate::texts::graph as texts;

/// Everything one round of the topology is built from. The panel engine fills it from what it
/// gathered; nothing here reads a file, a socket or a clock.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TopologyInputs {
    /// The chosen project's configured topology, as `offline_fleet_topology_json` of
    /// `trust_runtime::control` builds it on this computer, when the project has a runtime.
    pub project: Option<Value>,
    /// What the project's own runtime target answered: the runtime at the project's control
    /// endpoint, or the simulator while it runs the project.
    pub primary: Option<RuntimeReport>,
    /// What every other configured runtime target answered: the fleet endpoints and the managed
    /// runtimes, in their configured order.
    pub peers: Vec<RuntimeReport>,
    /// The local simulator.
    pub simulator: Simulator,
    /// The runtimes this computer manages (`fleet.toml`), with their status.
    pub managed: Vec<ManagedRuntime>,
    /// The extension's session: the attached endpoint and the selected run target.
    pub session: Session,
    /// The host ID that this computer's runtimes report (`host:` and the sanitized host name,
    /// specification 11). When it is absent, the host of the project's configuration, which is
    /// built on this computer, stands for it, and else the host of the simulator's own report.
    pub this_computer: Option<String>,
}

/// What one runtime target answered this round.
#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeReport {
    /// The control endpoint the target is reached through, as configured.
    pub endpoint: String,
    /// The target's configured label; empty when it has none.
    pub label: String,
    /// Whether a runtime answered `status` on the endpoint and accepted the token.
    pub probe: TargetProbe,
    /// The `fleet.topology` answer; `None` when it was not requested or has not arrived.
    pub topology: Option<Result<Value, ControlError>>,
    /// The `connectors.status` answer; `None` when it was not requested or has not arrived.
    pub connectors: Option<Result<Value, ControlError>>,
    /// The stable runtime ID that the runtime's `status` or `fleet.topology` answer carries, when
    /// it carries one. No runtime reports one in these answers yet; without it, the endpoint
    /// tells the runtime apart.
    pub stable_id: Option<String>,
}

impl RuntimeReport {
    /// How the operator knows the target: its label, or its endpoint.
    fn source(&self) -> &str {
        let label = self.label.trim();
        if label.is_empty() {
            self.endpoint.trim()
        } else {
            label
        }
    }
}

/// The local simulator, as the extension reports its debug session.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Simulator {
    /// The control endpoint of the running simulator. A report from this endpoint is the
    /// simulator's own.
    pub endpoint: Option<String>,
    /// Whether the simulator is stopped, starting or running.
    pub state: SimulatorState,
    /// The message of the start failure that blocks it, when there is one. A stopped local
    /// control socket is a neutral stopped state, not a failure (specification 25 §5.6).
    pub failure: Option<String>,
    /// Whether the simulator reports I/O values: a field device is connected only then
    /// (specification 25 §5.5).
    pub io_values: bool,
    /// The connections that the catalog lists for the project, drawn on the simulator while
    /// nothing reports the project's runtime.
    pub devices: Vec<ConfiguredDevice>,
    /// Whether the project runs in the simulator (the `simulate` run mode). The project's
    /// configured runtime then stands for the simulator while the simulator does not report it:
    /// it shows the simulator's lifecycle and carries its session and run-target marks.
    pub runs_project: bool,
}

/// The lifecycle of the local simulator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SimulatorState {
    /// Not started: the simulator is never started without the user.
    #[default]
    NotStarted,
    /// Starting after the user asked.
    Starting,
    /// Running, proven by its lifecycle evidence.
    Running,
}

/// A connection that the catalog lists for the project (`comm.schema` instances).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfiguredDevice {
    /// The configured instance ID.
    pub id: String,
    /// Its display name.
    pub name: String,
    /// Its protocol ID, such as `modbus_tcp`.
    pub protocol: String,
    /// The health that `comm.capabilities` reports for the protocol, when it reports one.
    pub health: Option<String>,
}

/// A runtime that this computer manages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedRuntime {
    /// Its name in `fleet.toml`.
    pub name: String,
    /// Its control endpoint, when it has one.
    pub control_endpoint: Option<String>,
    /// Its lifecycle state.
    pub state: ManagedState,
}

/// The lifecycle state of a managed runtime (specification 25 §6.5). The gather reads it, and
/// the topology and the view show it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManagedState {
    /// Its control endpoint answers.
    Running,
    /// Nothing accepts a connection at its control endpoint.
    Stopped,
    /// A start has not reached `running` yet.
    Starting,
    /// A stop has not reached `stopped` yet.
    Stopping,
    /// Its status is unknown: `Status unavailable`, never shown as stopped.
    Unavailable,
}

impl ManagedState {
    /// The state that a fleet status names. A missing or unknown one is `Unavailable`.
    #[must_use]
    pub fn from_status(status: Option<&str>) -> Self {
        match status {
            Some("running") => Self::Running,
            Some("stopped") => Self::Stopped,
            Some("starting") => Self::Starting,
            Some("stopping") => Self::Stopping,
            _ => Self::Unavailable,
        }
    }
}

/// The extension's session, as it reports it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Session {
    /// The control endpoint of the runtime the extension is attached to, when it is.
    pub attached_endpoint: Option<String>,
    /// The selected run target: `simulator`, a managed runtime's name, a control endpoint or a
    /// runtime's ID. Absent or blank selects the simulator.
    pub run_target: Option<String>,
}

/// The merged topology that the view projects. Every node appears once under its owner-scoped
/// identity; every link names two nodes that appear.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Topology {
    /// The highest schema version among the reports.
    pub schema_version: u32,
    /// The computers, the project's own first.
    pub hosts: Vec<Host>,
    /// The links between endpoints, runtimes, shared systems and external systems.
    pub links: Vec<FleetLink>,
    /// The systems that several runtimes use, such as an MQTT broker.
    pub shared: Vec<FleetShared>,
    /// The systems outside the fleet that a runtime talks to or sees.
    pub external: Vec<FleetExternal>,
    /// What the operator must be told about this round's reports.
    pub issues: Vec<TopologyIssue>,
    /// The host that stands for this computer: the one with the ID that this computer's runtimes
    /// report, else the host of the project's configuration, else the host of the simulator's
    /// own report. `None` when no report gives it; a host drawn for this computer then stands
    /// for it.
    pub this_computer: Option<String>,
}

/// A computer: its facts as the first report that names it gives them (see
/// [`FleetHost`](trust_runtime::fleet_topology::FleetHost)), and its runtimes.
#[derive(Debug, Clone, PartialEq)]
pub struct Host {
    /// The host's ID; host IDs are global.
    pub id: String,
    /// The host name, or the configured label of a target that did not answer.
    pub hostname: String,
    /// The board model.
    pub board: Option<String>,
    /// The processor architecture; empty for a drawn host.
    pub arch: String,
    /// The operating system; empty for a drawn host.
    pub os: String,
    /// The host's addresses.
    pub ips: Vec<String>,
    /// The processor temperature in degrees Celsius.
    pub temp_c: Option<f64>,
    /// The time since the host started, in seconds.
    pub uptime_s: Option<u64>,
    /// The one-minute load average.
    pub load: Option<f64>,
    /// Where the facts come from: `self`, `config` or `discovery`.
    pub source: Option<String>,
    /// When the host was last seen, in milliseconds since the Unix epoch.
    pub last_seen_ms: Option<u64>,
    /// Whether a report gives the host or the panel draws it.
    pub origin: HostOrigin,
    /// The containers that run a runtime.
    pub containers: Vec<Container>,
    /// The runtimes outside a container.
    pub runtimes: Vec<Runtime>,
}

/// Whether a report gives a host or the panel draws it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostOrigin {
    /// A runtime or the project's configuration reports it.
    Reported,
    /// This computer, drawn for the local simulator or the managed runtimes while no report
    /// gives it. It is always reachable.
    ThisComputer,
    /// Drawn for a configured runtime target that did not answer.
    Unanswered,
}

/// A container that runs a runtime (see
/// [`FleetContainer`](trust_runtime::fleet_topology::FleetContainer)).
#[derive(Debug, Clone, PartialEq)]
pub struct Container {
    /// The container's identity, scoped by its host.
    pub id: String,
    /// The container's name.
    pub name: String,
    /// The image it runs.
    pub image: String,
    /// The container's state.
    pub status: String,
    /// The restart policy.
    pub restart_policy: Option<String>,
    /// The processor limit.
    pub cpu_limit: Option<String>,
    /// The memory limit.
    pub mem_limit: Option<String>,
    /// The mounted paths.
    pub mounts: Option<Vec<String>>,
    /// Where the facts come from.
    pub source: Option<String>,
    /// The runtimes in the container.
    pub runtimes: Vec<Runtime>,
}

/// A runtime (see [`FleetRuntime`](trust_runtime::fleet_topology::FleetRuntime)), with how the
/// panel knows and reaches it.
#[derive(Debug, Clone, PartialEq)]
pub struct Runtime {
    /// The runtime's identity, scoped by its host and container.
    pub id: String,
    /// The runtime's name as reported; the view chooses the name it shows.
    pub name: String,
    /// The control endpoint: the one the panel reached it through, or the configured one.
    pub control_endpoint: Option<String>,
    /// The web server's listen address.
    pub web_listen: Option<String>,
    /// `online`, `simulate`, `stopped`, `unknown`, `error` or `managed`.
    pub mode: String,
    /// The cycle interval in milliseconds; 0 when unknown.
    pub cycle_ms: u64,
    /// The share of the cycle interval the last cycle used.
    pub load: Option<f64>,
    /// The runtime's health word.
    pub health: String,
    /// The sentence that explains the health.
    pub detail: String,
    /// Where the facts come from.
    pub source: Option<String>,
    /// When the runtime was last seen, in milliseconds since the Unix epoch.
    pub last_seen_ms: Option<u64>,
    /// The runtime's connections and services.
    pub endpoints: Vec<Endpoint>,
    /// What tells the runtime apart from another runtime of its host with the same runtime ID:
    /// the stable runtime ID it reports, else the control endpoint the panel reached it through.
    /// `None` for a runtime that no target reached, which its runtime ID identifies.
    pub instance: Option<String>,
    /// How the panel knows the runtime.
    pub origin: RuntimeOrigin,
    /// The managed runtime it is, with its lifecycle state, when this computer manages it.
    pub managed: Option<ManagedRuntime>,
    /// Whether the extension holds a session to it.
    pub attached: bool,
    /// Whether it is the selected run target: the simulator for `simulator`, else the runtime
    /// with the selected managed name, control endpoint or ID.
    pub run_target: bool,
}

/// How the panel knows a runtime, which decides how it is reached (specification 25 §5.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeOrigin {
    /// It answered `fleet.topology` itself, through its control endpoint.
    Answered,
    /// The local simulator answered `fleet.topology` itself.
    Simulator,
    /// Only the project's configuration describes it.
    Configured,
    /// Another runtime saw it through discovery.
    Discovered,
    /// A configured target where nothing answered.
    Unreachable,
    /// A configured target whose runtime refused the token.
    AuthFailed(AuthFailure),
    /// The local simulator, drawn while nothing reports the project's runtime.
    SimulatorFallback,
    /// A managed runtime that no report shows.
    Managed,
}

/// One connection or service of a runtime (see
/// [`FleetEndpoint`](trust_runtime::fleet_topology::FleetEndpoint)), with its connector status.
#[derive(Debug, Clone, PartialEq)]
pub struct Endpoint {
    /// The endpoint's identity, scoped by its runtime.
    pub id: String,
    /// `field`, `service` or `peer`.
    pub kind: String,
    /// The protocol.
    pub protocol: String,
    /// The display name.
    pub name: String,
    /// The address it serves or reaches.
    pub address: Option<String>,
    /// The runtime's role on it.
    pub role: Option<String>,
    /// The endpoint's health word.
    pub health: String,
    /// The sentence that explains the health.
    pub detail: String,
    /// The live evidence.
    pub live: Option<Value>,
    /// The configuration without secrets.
    pub params: Option<Value>,
    /// The devices on a fieldbus.
    pub children: Vec<FleetEndpointChild>,
    /// Whether its runtime owns the configuration.
    pub owned: bool,
    /// Whether `comm.test` can test it.
    pub supports_test: bool,
    /// Where the facts come from.
    pub source: Option<String>,
    /// The connector status that its runtime reports for it.
    pub connector: Option<ConnectorStatus>,
}

/// The connector status of an endpoint, in the closed vocabularies of specification 11
/// ("Connector status and discovery truth contract").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectorStatus {
    /// The connector's ID.
    pub connector_id: String,
    /// The lifecycle state.
    pub state: ConnectorState,
    /// The health.
    pub health: ConnectorHealth,
    /// The discovery confidence.
    pub confidence: DiscoveryConfidence,
    /// The point counts.
    pub point_counts: ConnectorPointCounts,
}

/// Something the operator must be told about a report. Each is an error shown inline while the
/// rest of the topology stays visible (specification 25 §5.6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TopologyIssue {
    /// The item it belongs to: the endpoint an invalid connector report claimed, or the runtime
    /// whose report it is; `None` when the topology shows no node for it.
    pub item: Option<String>,
    /// Whether it concerns the project's own runtime rather than another configured runtime.
    pub project: bool,
    /// The sentence shown.
    pub text: String,
}

/// Builds the topology of one round (specification 25 §5.6). The project's own runtime and its
/// configuration come first, then every other configured runtime; the local simulator stands in
/// while neither reports the project's runtime; then the managed runtimes join this computer and
/// the session marks its runtimes.
#[must_use]
pub fn merge(inputs: &TopologyInputs) -> Topology {
    let origin = |report: &RuntimeReport| {
        if inputs.simulator.endpoint.as_deref() == Some(report.endpoint.as_str()) {
            RuntimeOrigin::Simulator
        } else {
            RuntimeOrigin::Answered
        }
    };
    let mut issues = Vec::new();
    let mut this_computer = inputs.this_computer.clone();

    let mut local = Vec::new();
    if let Some(primary) = &inputs.primary
        && matches!(primary.probe, TargetProbe::Reachable(_))
        && let Some(snapshot) =
            merge::answered_snapshot(primary, origin(primary), true, &mut issues)
    {
        local.push(snapshot);
    }
    if let Some(project) = &inputs.project {
        match wire::topology(project, RuntimeOrigin::Configured) {
            Ok(snapshot) => {
                if this_computer.is_none() {
                    this_computer = snapshot.hosts.first().map(|host| host.id.clone());
                }
                local.push(snapshot);
            }
            Err(error) => issues.push(TopologyIssue {
                item: None,
                project: true,
                text: texts::topology_unavailable(&error.to_string()),
            }),
        }
    }
    let local = merge::merge_snapshots(&local);
    let nothing_local = local.hosts.is_empty();

    let mut snapshots = vec![local];
    for peer in &inputs.peers {
        let snapshot = if matches!(peer.probe, TargetProbe::Reachable(_)) {
            merge::answered_snapshot(peer, origin(peer), false, &mut issues)
        } else {
            synthetic::unanswered_peer(peer)
        };
        snapshots.extend(snapshot);
    }
    let mut topology = merge::merge_snapshots(&snapshots);

    let this_computer = this_computer.or_else(|| {
        topology
            .hosts
            .iter()
            .find(|host| {
                host.all_runtimes()
                    .any(|runtime| runtime.origin == RuntimeOrigin::Simulator)
            })
            .map(|host| host.id.clone())
    });
    let this_computer = this_computer.as_deref();
    if nothing_local {
        synthetic::add_simulator(&mut topology, &inputs.simulator, this_computer);
    }
    synthetic::stand_in_for_simulator(&mut topology, &inputs.simulator);
    synthetic::inject_managed(&mut topology, &inputs.managed, this_computer);
    synthetic::remove_runtime_twins(&mut topology, &inputs.managed);
    synthetic::mark_session(&mut topology, &inputs.session, &inputs.simulator);
    topology.issues = issues;
    topology.this_computer = this_computer.map(str::to_owned);
    topology
}

impl Topology {
    /// Every runtime: on each host, the runtimes outside containers, then those in its
    /// containers.
    pub fn runtimes(&self) -> impl Iterator<Item = &Runtime> {
        self.hosts.iter().flat_map(Host::all_runtimes)
    }

    fn runtimes_mut(&mut self) -> impl Iterator<Item = &mut Runtime> {
        self.hosts.iter_mut().flat_map(|host| {
            host.runtimes.iter_mut().chain(
                host.containers
                    .iter_mut()
                    .flat_map(|container| container.runtimes.iter_mut()),
            )
        })
    }

    /// The runtime that answers a report itself: the first runtime of its first host, outside a
    /// container or in its first container.
    fn own_runtime_mut(&mut self) -> Option<&mut Runtime> {
        let host = self.hosts.first_mut()?;
        if host.runtimes.is_empty() {
            host.containers.first_mut()?.runtimes.first_mut()
        } else {
            host.runtimes.first_mut()
        }
    }
}

impl Host {
    /// All the host's runtimes: those outside containers, then those in its containers.
    pub fn all_runtimes(&self) -> impl Iterator<Item = &Runtime> {
        self.runtimes.iter().chain(
            self.containers
                .iter()
                .flat_map(|container| container.runtimes.iter()),
        )
    }

    /// A host that the panel draws, with no reported facts.
    fn drawn(id: &str, hostname: &str, origin: HostOrigin) -> Self {
        Self {
            id: id.to_owned(),
            hostname: hostname.to_owned(),
            board: None,
            arch: String::new(),
            os: String::new(),
            ips: Vec::new(),
            temp_c: None,
            uptime_s: None,
            load: None,
            source: None,
            last_seen_ms: None,
            origin,
            containers: Vec::new(),
            runtimes: Vec::new(),
        }
    }
}

impl Runtime {
    /// A runtime that the panel draws: no reported facts, no endpoints and no marks yet.
    fn drawn(id: String, name: String, origin: RuntimeOrigin) -> Self {
        Self {
            id,
            name,
            control_endpoint: None,
            web_listen: None,
            mode: String::new(),
            cycle_ms: 0,
            load: None,
            health: String::new(),
            detail: String::new(),
            source: None,
            last_seen_ms: None,
            endpoints: Vec::new(),
            instance: None,
            origin,
            managed: None,
            attached: false,
            run_target: false,
        }
    }
}

#[cfg(test)]
pub(crate) mod tests;
