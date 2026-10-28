//! The fleet view that the canvas draws from a merged topology (specification 25 §5.5): each
//! computer headed `This computer`, `Computer <IP>`, `Computer <host name>` or the configured label
//! of a target that did not answer, with its reachability as its health; each runtime with its
//! product name and its health rolled up from its own and its connections' health words; each
//! connection with its product name, whether the search dims it, and the target its browse pane
//! opens; the faults; and the summary. The health rules, headings and names are the panel
//! engine's own (`view::health`, `view::computers`, `view::runtimes` and `view::connections`).

use serde::Serialize;
use serde_json::{Map, Value};
use trust_runtime::fleet_topology::FleetEndpointChild;

use super::ads::DEFAULT_ADS_PORT;
use super::ads_tags;
use super::local::{CanvasBanner, CanvasFailure};
use super::views::{self, AdsClientSummaryView};
use super::wire;
use crate::inputs::Facts;
use crate::request::RunMode;
use crate::texts::graph as texts;
use crate::topology::{Container, Endpoint, Host, ManagedState, Runtime, RuntimeOrigin, Topology};
use crate::view::health::{self, Health};
use crate::view::lifecycle::Owner;
use crate::view::{address, computers, connections, runtimes};

/// The fleet view: the computers with their containers, runtimes and connections, the links, the
/// shared and external systems, the faults and the summary.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CanvasFleet {
    /// The computers, the project's own first.
    pub hosts: Vec<FleetHostView>,
    /// The links between connections, runtimes, shared and external systems.
    pub links: Vec<FleetLinkView>,
    /// The shared systems, then the external systems, as the nodes the canvas draws for them.
    pub external: Vec<ExternalView>,
    /// How many external systems the topology has.
    pub external_count: usize,
    /// How many shared systems the topology has.
    pub shared_count: usize,
    /// `<n> hosts · <n> runtimes · <n> endpoints`.
    pub summary: String,
    /// The connections that report an error or are degraded, and the issues that belong to an item.
    pub faults: Vec<FleetFault>,
    /// The banner: a hint or a failure, with its actions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub banner: Option<CanvasBanner>,
}

/// A computer.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FleetHostView {
    /// The host's identity.
    pub id: String,
    /// Its heading.
    pub hostname: String,
    /// Its supporting detail: the host name with its operating system and architecture.
    pub label: String,
    /// Its reachability: `connected` or `runtime_unreachable`.
    pub health: String,
    /// How many runtimes it has, in containers or not.
    pub runtime_count: usize,
    /// How many connections its runtimes have.
    pub endpoint_count: usize,
    /// The containers that run a runtime.
    pub containers: Vec<FleetContainerView>,
    /// The runtimes outside a container.
    pub runtimes: Vec<FleetRuntimeView>,
}

/// A container that runs a runtime.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FleetContainerView {
    /// The container's identity.
    pub id: String,
    /// Its name.
    pub name: String,
    /// The image it runs.
    pub image: String,
    /// Its state.
    pub status: String,
    /// Its runtimes.
    pub runtimes: Vec<FleetRuntimeView>,
}

/// A runtime.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FleetRuntimeView {
    /// The runtime's identity.
    pub id: String,
    /// Its product name: `Simulator` for the simulator, else its own name, else `Runtime`.
    pub name: String,
    /// Its mode.
    pub mode: String,
    /// Its health, rolled up from its own and its connections' health words.
    pub health: String,
    /// The sentence that explains its own health.
    pub detail: String,
    /// How many connections it has.
    pub endpoint_count: usize,
    /// The control endpoint it is reached through; none for the extension's own simulator.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub control_endpoint: Option<String>,
    /// Its connections.
    pub endpoints: Vec<FleetEndpointView>,
    /// Whether this computer manages it (`fleet.toml`): its node offers Start, Stop and Logs.
    pub managed: bool,
    /// The managed runtime's name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub managed_name: Option<String>,
    /// The managed runtime's lifecycle: `running`, `stopped`, `starting`, `stopping` or
    /// `unavailable`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lifecycle_state: Option<&'static str>,
    /// Whether the extension holds a session to it.
    pub attached: bool,
    /// Whether it is the selected run target.
    pub run_target: bool,
    /// `Identify`: whether it can be asked now, and why not.
    pub identify: CanvasActionState,
    /// The controls its node offers, the primary one first (specification 25 §5.5).
    pub controls: Vec<RuntimeNodeControl>,
    /// Whether a search can start from it, and why not (specification 25 §2).
    pub discovery: DiscoveryOrigin,
}

/// Whether a search can start from a runtime: from one the extension is attached to or that runs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DiscoveryOrigin {
    /// Whether it can.
    pub ready: bool,
    /// Why it cannot.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Whether an action can run now, and why not.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CanvasActionState {
    /// Whether it can run.
    pub enabled: bool,
    /// Why it cannot.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<&'static str>,
}

/// A connection or service of a runtime.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FleetEndpointView {
    /// The connection's identity.
    pub id: String,
    /// `field`, `service` or `peer`.
    pub kind: String,
    /// The protocol.
    pub protocol: String,
    /// Its product name: `Simulated I/O` and `Loopback I/O` for local I/O, else its own name or
    /// its protocol.
    pub name: String,
    /// The runtime's role on it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    /// Its health word.
    pub health: String,
    /// The sentence that explains it.
    pub detail: String,
    /// Whether its runtime owns its configuration.
    pub owned: bool,
    /// The live evidence.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub live: Option<Value>,
    /// The connector status its runtime reported.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connector: Option<Value>,
    /// Its configuration without secrets.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
    /// The devices on its bus.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<FleetEndpointChild>,
    /// Whether the canvas's search dims it: a search is under way and none of its computer's,
    /// container's, runtime's and own words contain it (specification 25 §5.5).
    pub dimmed: bool,
    /// The target that its browse pane opens with, for an ADS or OPC UA client connection that
    /// holds connections (specification 25 §5.4); without one the pane opens with `params`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub browse_target: Option<Value>,
    /// Whether it needs attention: its health is neither good nor a quiet state, so a filter that
    /// hides it reports it (specification 25 §5.5).
    pub attention: bool,
    /// The ADS client's summary (specification 25 §5.13).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ads_summary: Option<AdsClientSummaryView>,
}

/// A link.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FleetLinkView {
    /// The link's identity.
    pub id: String,
    /// The node it starts at.
    pub from: String,
    /// The node it ends at.
    pub to: String,
    /// The protocol.
    pub protocol: String,
    /// The runtime's role on the link; empty when the report names none.
    pub role: String,
    /// Its health word.
    pub status: String,
    /// The sentence that explains it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    /// Whether it is protected.
    pub secure: bool,
}

/// A shared or external system, as a node of the canvas.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ExternalView {
    /// Its identity.
    pub id: String,
    /// Its name, or a shared system's address when it has no name.
    pub name: String,
    /// Its kind.
    pub kind: String,
}

/// A fault that the canvas lists and that focuses its item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FleetFault {
    /// The fault's identity.
    pub id: String,
    /// Its sentence.
    pub label: String,
    /// The item it focuses.
    pub target_node_id: String,
    /// `warning` or `error`.
    pub severity: &'static str,
    /// Whether it is the fault of the add form's refused result, which the canvas hides while the
    /// user edits the form (specification 25 §5.9).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub form: bool,
}

/// The fleet view of `topology`. `run_mode` decides whether the project's configured runtime is
/// the simulator's node (specification 25 §5.6); `query` is the canvas's search.
pub(super) fn project(
    topology: &Topology,
    run_mode: RunMode,
    query: &str,
    failure: Option<&CanvasFailure>,
) -> CanvasFleet {
    // The runtime a Connect found without a token, or with one it refused, offers Set auth token
    // (specification 25 §5.5); the Connect classifies it by the refusal's code (`workspace_permission`).
    let auth_endpoint = failure
        .filter(|failure| failure.kind == AUTH_FAILURE)
        .and_then(|failure| failure.endpoint.as_deref())
        .map(str::trim);
    let facts = Facts {
        mode: run_mode,
        ..Facts::default()
    };
    let search = Search::new(query);
    let this_computer = topology.this_computer.as_deref();
    let hosts: Vec<FleetHostView> = topology
        .hosts
        .iter()
        .map(|host| host_view(host, this_computer, &facts, &search, auth_endpoint))
        .collect();
    let runtime_count: usize = hosts.iter().map(|host| host.runtime_count).sum();
    let endpoint_count: usize = hosts.iter().map(|host| host.endpoint_count).sum();
    let mut faults: Vec<FleetFault> = hosts.iter().flat_map(connection_faults).collect();
    faults.extend(
        topology
            .issues
            .iter()
            .filter_map(|issue| issue.item.as_ref().map(|item| (item, &issue.text)))
            .map(|(item, text)| FleetFault {
                id: format!("issue:{item}"),
                label: text.clone(),
                target_node_id: item.clone(),
                severity: "error",
                form: false,
            }),
    );
    CanvasFleet {
        summary: format!(
            "{} · {} · {}",
            counted(hosts.len(), "host"),
            counted(runtime_count, "runtime"),
            counted(endpoint_count, "endpoint")
        ),
        hosts,
        links: topology
            .links
            .iter()
            .map(|link| FleetLinkView {
                id: link.id.clone().unwrap_or_default(),
                from: link.from.clone(),
                to: link.to.clone(),
                protocol: link.protocol.clone(),
                role: link.role.clone().unwrap_or_default(),
                status: link.status.clone(),
                detail: link.detail.clone(),
                secure: link.secure,
            })
            .collect(),
        external: topology
            .shared
            .iter()
            .map(|shared| ExternalView {
                id: shared.id.clone(),
                name: if shared.name.is_empty() {
                    shared.address.clone()
                } else {
                    shared.name.clone()
                },
                kind: shared.kind.clone(),
            })
            .chain(topology.external.iter().map(|external| ExternalView {
                id: external.id.clone(),
                name: external.name.clone(),
                kind: external.kind.clone(),
            }))
            .collect(),
        external_count: topology.external.len(),
        shared_count: topology.shared.len(),
        faults,
        banner: None,
    }
}

fn host_view(
    host: &Host,
    this_computer: Option<&str>,
    facts: &Facts,
    search: &Search,
    auth_endpoint: Option<&str>,
) -> FleetHostView {
    let heading = computers::heading(host, this_computer, None);
    let haystack = lower(&[&host.hostname, &host.os, &host.arch, &host.ips.join(" ")]);
    let bare: Vec<FleetRuntimeView> = host
        .runtimes
        .iter()
        .map(|runtime| runtime_view(runtime, facts, search, &haystack, auth_endpoint))
        .collect();
    let containers: Vec<FleetContainerView> = host
        .containers
        .iter()
        .map(|container| container_view(container, facts, search, &haystack, auth_endpoint))
        .collect();
    let all = || {
        bare.iter()
            .chain(containers.iter().flat_map(|container| &container.runtimes))
    };
    let running = host.all_runtimes().any(|runtime| rollup(runtime).running());
    FleetHostView {
        id: host.id.clone(),
        label: detail(host, &heading),
        hostname: heading,
        health: if health::host_reachable(host, running) {
            "connected"
        } else {
            "runtime_unreachable"
        }
        .to_owned(),
        runtime_count: all().count(),
        endpoint_count: all().map(|runtime| runtime.endpoint_count).sum(),
        containers,
        runtimes: bare,
    }
}

fn container_view(
    container: &Container,
    facts: &Facts,
    search: &Search,
    host: &str,
    auth_endpoint: Option<&str>,
) -> FleetContainerView {
    let haystack = format!(
        "{host} {}",
        lower(&[&container.name, &container.image, &container.status])
    );
    FleetContainerView {
        id: container.id.clone(),
        name: container.name.clone(),
        image: container.image.clone(),
        status: container.status.clone(),
        runtimes: container
            .runtimes
            .iter()
            .map(|runtime| runtime_view(runtime, facts, search, &haystack, auth_endpoint))
            .collect(),
    }
}

fn runtime_view(
    runtime: &Runtime,
    facts: &Facts,
    search: &Search,
    parent: &str,
    auth_endpoint: Option<&str>,
) -> FleetRuntimeView {
    let haystack = format!(
        "{parent} {}",
        lower(&[
            &runtime.name,
            &runtime.mode,
            &runtime.health,
            &runtime.detail
        ])
    );
    let health = health_word(rollup(runtime));
    let endpoints: Vec<FleetEndpointView> = runtime
        .endpoints
        .iter()
        .map(|endpoint| endpoint_view(endpoint, search, &haystack, &runtime.health))
        .collect();
    let control_endpoint = runtime
        .control_endpoint
        .as_deref()
        .map(str::trim)
        .filter(|endpoint| !endpoint.is_empty());
    // A Connect refused for its token names the runtime it tried (§5.1). The project's configured
    // runtime, which stands for the simulator while the project runs in it (§5.6), is then the
    // runtime connected to, under its own name: no Start fixes a refused token.
    let auth_token_required = runtime.managed.is_none()
        && !matches!(
            runtime.origin,
            RuntimeOrigin::Simulator | RuntimeOrigin::SimulatorFallback
        )
        && control_endpoint.is_some_and(|endpoint| auth_endpoint == Some(endpoint));
    let owner = if auth_token_required {
        Owner::Session
    } else {
        runtimes::owner(runtime, facts)
    };
    let controls = runtime_controls(&NodeFacts {
        owner: match owner {
            Owner::Managed(_) => NodeOwner::Managed,
            Owner::Simulator => NodeOwner::Simulator,
            Owner::Session => NodeOwner::Session,
        },
        health,
        attached: runtime.attached,
        control_endpoint,
        auth_token_required,
    });
    let name = runtimes::display_name(runtime, owner);
    let discovery_ready = runtime.attached || health == "connected";
    let discovery = DiscoveryOrigin {
        ready: discovery_ready,
        reason: (!discovery_ready)
            .then(|| format!("Start or connect {name} before scanning from it.")),
    };
    FleetRuntimeView {
        id: runtime.id.clone(),
        name,
        mode: runtime.mode.clone(),
        health: health.to_owned(),
        detail: runtime.detail.clone(),
        endpoint_count: endpoints.len(),
        control_endpoint: runtime
            .control_endpoint
            .as_deref()
            .map(str::trim)
            .filter(|endpoint| !endpoint.is_empty())
            .map(str::to_owned),
        endpoints,
        managed: runtime.managed.is_some(),
        managed_name: runtime.managed.as_ref().map(|local| local.name.clone()),
        lifecycle_state: runtime.managed.as_ref().map(|local| match local.state {
            ManagedState::Running => "running",
            ManagedState::Stopped => "stopped",
            ManagedState::Starting => "starting",
            ManagedState::Stopping => "stopping",
            ManagedState::Unavailable => "unavailable",
        }),
        attached: runtime.attached,
        run_target: runtime.run_target,
        identify: identify(runtime),
        controls,
        discovery,
    }
}

/// `Identify` of a runtime (specification 25 §5.12): it can be asked of a runtime that answered
/// this round, the extension's simulator included; any other runtime did not answer, and nothing
/// is sent to it. The runtime itself refuses a role below Operator and a missing identify output.
fn identify(runtime: &Runtime) -> CanvasActionState {
    let answered = matches!(
        runtime.origin,
        RuntimeOrigin::Answered | RuntimeOrigin::Simulator
    );
    CanvasActionState {
        enabled: answered,
        reason: (!answered).then_some(texts::IDENTIFY_UNREACHABLE),
    }
}

fn endpoint_view(
    endpoint: &Endpoint,
    search: &Search,
    runtime: &str,
    runtime_health: &str,
) -> FleetEndpointView {
    let haystack = format!(
        "{runtime} {}",
        lower(&[
            &endpoint.name,
            &endpoint.protocol,
            &endpoint.kind,
            endpoint.role.as_deref().unwrap_or_default(),
            &endpoint.detail,
        ])
    );
    FleetEndpointView {
        id: endpoint.id.clone(),
        kind: endpoint.kind.clone(),
        protocol: endpoint.protocol.clone(),
        name: connections::display_name(endpoint),
        role: endpoint.role.clone(),
        health: endpoint.health.clone(),
        detail: endpoint.detail.clone(),
        owned: endpoint.owned,
        live: endpoint.live.clone(),
        connector: endpoint.connector.as_ref().map(wire::connector_json),
        params: endpoint.params.clone(),
        children: endpoint.children.clone(),
        dimmed: search.dims(&haystack),
        browse_target: endpoint
            .params
            .as_ref()
            .and_then(|params| browse_target(&endpoint.protocol, params)),
        attention: needs_attention(&endpoint.health),
        ads_summary: (endpoint.protocol == "ads").then(|| {
            views::ads_summary(
                endpoint.params.as_ref(),
                &endpoint.health,
                &endpoint.detail,
                runtime_health,
            )
        }),
    }
}

/// Whether a connection's health word asks for attention: any word but a good or quiet one
/// (`connected`, `configured`, `configured_policy`, `disabled`, `stopped`, `simulate`) or none.
fn needs_attention(health: &str) -> bool {
    !matches!(
        health,
        "" | "connected" | "configured" | "configured_policy" | "disabled" | "stopped" | "simulate"
    ) && !health.trim().is_empty()
}

/// The canvas's search, trimmed and in lower case; empty while none is under way.
struct Search(String);

impl Search {
    fn new(query: &str) -> Self {
        Self(query.trim().to_lowercase())
    }

    /// Whether the search dims an item whose words, in lower case, are `haystack`.
    fn dims(&self, haystack: &str) -> bool {
        !self.0.is_empty() && !haystack.contains(&self.0)
    }
}

/// The target that the browse pane of a client connection opens with (specification 25 §5.4): an
/// OPC UA client browses its first configured connection, and an ADS client its first configured
/// connection with every configured connection, the ADS ports of the connections that serve the
/// same device, the tags already imported from that device, and its own ADS port, 851 when it names
/// none. `None` for another protocol, or when no first connection is configured.
fn browse_target(protocol: &str, params: &Value) -> Option<Value> {
    let connections = params.get("connections")?.as_array()?;
    let first = connections.first()?.as_object()?;
    match protocol {
        "opcua_client" => Some(Value::Object(first.clone())),
        "ads" => {
            let configured = Value::Array(
                connections
                    .iter()
                    .filter(|connection| connection.is_object())
                    .cloned()
                    .collect(),
            );
            let mut ports: Vec<u16> = ads_tags::device_connections(&configured, first)
                .into_iter()
                .filter_map(|connection| connection.get("ams_port").and_then(ads_tags::ads_port))
                .collect();
            ports.sort_unstable();
            ports.dedup();
            let mut target: Map<String, Value> = first.clone();
            let own_port = first
                .get("ams_port")
                .and_then(ads_tags::ads_port)
                .unwrap_or(DEFAULT_ADS_PORT);
            // The first connection's own tags count on the port it is browsed at.
            let mut with_own_port = configured.clone();
            if let Some(own) = with_own_port
                .as_array_mut()
                .and_then(|connections| connections.first_mut())
                .and_then(Value::as_object_mut)
            {
                own.insert("ams_port".to_owned(), Value::from(own_port));
            }
            target.insert(
                "imported_ads_symbols".to_owned(),
                serde_json::to_value(ads_tags::imported_selections(&with_own_port, first))
                    .unwrap_or_default(),
            );
            target.insert("responding_ads_ports".to_owned(), Value::from(ports));
            target.insert("connections".to_owned(), configured);
            target.insert("ams_port".to_owned(), Value::from(own_port));
            Some(Value::Object(target))
        }
        _ => None,
    }
}

/// The connections of a computer that report an error or are degraded, as faults that name the
/// connection and its detail (specification 25 §5.5).
fn connection_faults(host: &FleetHostView) -> Vec<FleetFault> {
    host.runtimes
        .iter()
        .chain(
            host.containers
                .iter()
                .flat_map(|container| &container.runtimes),
        )
        .flat_map(|runtime| &runtime.endpoints)
        .filter_map(|endpoint| {
            let severity = match endpoint.health.as_str() {
                "error" => "error",
                "degraded" => "warning",
                _ => return None,
            };
            Some(FleetFault {
                id: format!("fleet:{}", endpoint.id),
                label: format!("{}: {}", endpoint.name, endpoint.detail),
                target_node_id: endpoint.id.clone(),
                severity,
                form: false,
            })
        })
        .collect()
}

/// A runtime's health rolled up from its own and its connections' raw health words
/// (specification 25 §5.5).
fn rollup(runtime: &Runtime) -> Health {
    health::runtime_rollup(
        &runtime.health,
        runtime
            .endpoints
            .iter()
            .map(|endpoint| endpoint.health.as_str()),
    )
}

/// The health word of a rolled-up health, in the vocabulary the canvas draws.
/// The kind of a Connect failure whose refusal said that a token is missing or was rejected (the
/// run-target Connect, `trust-lsp.runTargets`, classifies it by the refusal's code).
const AUTH_FAILURE: &str = "workspace_permission";

/// One control a runtime node offers (specification 25 §5.5): the message the canvas sends, its
/// label, whether it is the node's primary control, and whether it can run now.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RuntimeNodeControl {
    /// `startLocalSimulator`, `stopLocalSimulator`, `managedStart`, `managedStop`,
    /// `runtimeConnect`, `runtimeDisconnect`, `setAuthToken`, `setAsRunTarget`, `openRuntimeLogs`,
    /// `openRuntimeSettings`, or `none` for a control that shows progress only.
    pub action: &'static str,
    /// The button's text.
    pub label: &'static str,
    /// `primary` or `secondary`.
    pub kind: &'static str,
    /// Whether it can run now.
    pub enabled: bool,
}

/// Whose process a runtime node stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NodeOwner {
    /// The extension's own simulator, which the extension starts and stops.
    Simulator,
    /// A runtime this computer manages (`fleet.toml`), started and stopped through its fleet.
    Managed,
    /// Any other runtime: the extension owns only its session.
    Session,
}

/// What decides a runtime node's controls.
struct NodeFacts<'a> {
    /// Whose process the node stands for.
    owner: NodeOwner,
    /// Its health word.
    health: &'a str,
    /// Whether the extension holds a session to it.
    attached: bool,
    /// The control endpoint it is reached through.
    control_endpoint: Option<&'a str>,
    /// Whether its last Connect needs a token: none was given or the one given was refused.
    auth_token_required: bool,
}

/// The controls of a runtime node (specification 25 §5.5), the primary one first. A runtime whose
/// process the extension owns, its simulator or a managed runtime, is started and stopped; any
/// other runtime is never stopped, only connected and disconnected. A Connect that needs a token
/// makes Set auth token the primary control, with Connect kept as the retry. Every node can be set
/// as the run target and opens its settings; logs exist for the simulator and managed runtimes.
fn runtime_controls(node: &NodeFacts<'_>) -> Vec<RuntimeNodeControl> {
    let control = |action, label, kind, enabled| RuntimeNodeControl {
        action,
        label,
        kind,
        enabled,
    };
    let primary = if node.owner == NodeOwner::Managed {
        match node.health {
            "connected" => control("managedStop", "Stop", "primary", true),
            "stopped" => control("managedStart", "Start", "primary", true),
            "pending" => control("none", "Updating…", "primary", false),
            _ => control("managedStart", "Start", "primary", false),
        }
    } else if node.owner == NodeOwner::Simulator {
        match node.health {
            "pending" => control("none", "Starting…", "primary", false),
            "connected" => control("stopLocalSimulator", "Stop", "primary", true),
            _ => control("startLocalSimulator", "Start", "primary", true),
        }
    } else if node.attached {
        control("runtimeDisconnect", "Disconnect", "primary", true)
    } else if node.auth_token_required && node.control_endpoint.is_some() {
        control("setAuthToken", "Set auth token", "primary", true)
    } else {
        control(
            "runtimeConnect",
            "Connect",
            "primary",
            node.control_endpoint.is_some(),
        )
    };
    let mut controls = Vec::new();
    if node.auth_token_required && node.control_endpoint.is_some() {
        controls.push(if primary.action == "setAuthToken" {
            control("runtimeConnect", "Connect", "secondary", true)
        } else {
            control("setAuthToken", "Set auth token", "secondary", true)
        });
    }
    controls.insert(0, primary);
    controls.push(control(
        "setAsRunTarget",
        "Set as run target",
        "secondary",
        true,
    ));
    if node.owner != NodeOwner::Session {
        controls.push(control("openRuntimeLogs", "Logs", "secondary", true));
    }
    controls.push(control(
        "openRuntimeSettings",
        "Settings",
        "secondary",
        true,
    ));
    controls
}

fn health_word(health: Health) -> &'static str {
    match health {
        Health::Error => "error",
        Health::Degraded => "degraded",
        Health::Connected => "connected",
        Health::Simulate => "simulate",
        Health::Stopped => "stopped",
        Health::Pending => "pending",
        Health::Unknown => "unknown",
    }
}

/// The supporting detail of a computer: its host name and its operating system and architecture,
/// joined by ` · `; without them, `local computer` for this computer, else its first address
/// that is not a loopback address, else its heading.
fn detail(host: &Host, heading: &str) -> String {
    let os = host.os.trim();
    let arch = host.arch.trim();
    let system = if os.is_empty() && arch.is_empty() {
        String::new()
    } else {
        format!("{os}/{arch}")
    };
    let parts: Vec<&str> = [host.hostname.trim(), system.as_str()]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect();
    if !parts.is_empty() {
        return parts.join(" · ");
    }
    if heading == texts::THIS_COMPUTER {
        return "local computer".to_owned();
    }
    address::remote_address(host).unwrap_or_else(|| heading.to_owned())
}

/// `parts` in lower case, joined by spaces.
fn lower(parts: &[&str]) -> String {
    parts.join(" ").to_lowercase()
}

/// `count noun`, the noun pluralized by a trailing `s`.
fn counted(count: usize, noun: &str) -> String {
    format!("{count} {noun}{}", if count == 1 { "" } else { "s" })
}

#[cfg(test)]
#[path = "fleet_controls_tests.rs"]
mod controls_tests;
