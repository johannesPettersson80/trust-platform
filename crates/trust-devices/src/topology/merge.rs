//! The merge of a round's snapshots (specification 25 §5.6): hosts and runtimes are unioned by
//! identity, the project's configuration is overlaid on its live runtime, and each runtime's
//! connector reports are attached to its endpoints.

use serde_json::Value;

use super::wire::{self, ConnectorReport};
use super::{
    identity, Endpoint, Host, Runtime, RuntimeOrigin, RuntimeReport, Topology, TopologyIssue,
};
use crate::texts::graph as texts;

/// Protocols of which a runtime has at most one endpoint: a configured endpoint of one of them is
/// the same endpoint as the live one of its runtime.
const SINGLETON_PROTOCOLS: [&str; 12] = [
    "simulated",
    "loopback",
    "ads",
    "opcua_client",
    "opcua",
    "ads_server",
    "web",
    "discovery",
    "mesh",
    "openot",
    "runtime_cloud",
    "realtime_t0",
];

/// Merges snapshots in order. The result uses the highest schema version; a host keeps the facts
/// of the first snapshot that names it; hosts, containers, runtimes and endpoints are unioned by
/// identity; links and external systems are deduplicated by identity, the first kept; a shared
/// system's `used_by` lists are unioned. The configuration of the project's runtime folds into
/// the live runtime that it describes.
pub(super) fn merge_snapshots(snapshots: &[Topology]) -> Topology {
    let mut merged = Topology::default();
    for snapshot in snapshots {
        let snapshot = identity::normalize(snapshot);
        merged.schema_version = merged.schema_version.max(snapshot.schema_version);
        for host in snapshot.hosts {
            merge_host(&mut merged.hosts, host);
        }
        for link in snapshot.links {
            if !merged.links.iter().any(|known| known.id == link.id) {
                merged.links.push(link);
            }
        }
        for shared in snapshot.shared {
            match merged.shared.iter_mut().find(|known| known.id == shared.id) {
                Some(known) => {
                    for runtime in shared.used_by {
                        if !known.used_by.contains(&runtime) {
                            known.used_by.push(runtime);
                        }
                    }
                }
                None => merged.shared.push(shared),
            }
        }
        for external in snapshot.external {
            if !merged.external.iter().any(|known| known.id == external.id) {
                merged.external.push(external);
            }
        }
        merged.issues.extend(snapshot.issues);
    }
    identity::normalize(&merged)
}

/// The snapshot that a runtime's answers give: its `fleet.topology` report, in which the first
/// runtime is its own, reached through `report.endpoint` and told apart by the report's stable
/// runtime ID or else by that endpoint, with the reports of `connectors.status` attached. Each
/// failed request and each invalid answer is an issue; `None` when there is no topology to show.
pub(super) fn answered_snapshot(
    report: &RuntimeReport,
    origin: RuntimeOrigin,
    project: bool,
    issues: &mut Vec<TopologyIssue>,
) -> Option<Topology> {
    let source = report.source();
    let decoded = report.topology.as_ref().map(|answer| match answer {
        Ok(value) => {
            wire::topology(value, RuntimeOrigin::Discovered).map_err(|error| error.to_string())
        }
        Err(error) => Err(error.to_string()),
    });
    if let Some(Err(reason)) = &decoded {
        let text = if project {
            texts::topology_unavailable(reason)
        } else {
            texts::peer_topology_failed(source, reason)
        };
        issues.push(TopologyIssue {
            item: None,
            project,
            text,
        });
    }
    let mut snapshot = decoded.and_then(Result::ok).map(|mut snapshot| {
        if let Some(own) = snapshot.own_runtime_mut() {
            own.origin = origin;
            own.control_endpoint = Some(report.endpoint.clone());
            own.instance = Some(
                report
                    .stable_id
                    .clone()
                    .filter(|stable_id| !stable_id.trim().is_empty())
                    .unwrap_or_else(|| report.endpoint.clone()),
            );
        }
        identity::normalize(&snapshot)
    });
    let own = snapshot
        .as_mut()
        .and_then(Topology::own_runtime_mut)
        .map(|own| own.id.clone());
    match (&report.connectors, snapshot.as_mut()) {
        (Some(Ok(answer)), Some(snapshot)) => {
            let reports = wire::connector_reports(answer);
            let claims = attach_connectors(snapshot, &reports);
            for (connector, claim) in reports.iter().zip(claims) {
                if let Err(reason) = &connector.status {
                    issues.push(TopologyIssue {
                        item: claim.or_else(|| own.clone()),
                        project,
                        text: texts::connector_status_failed(source, reason),
                    });
                }
            }
        }
        (Some(Err(error)), _) => issues.push(TopologyIssue {
            item: own,
            project,
            text: texts::connector_status_failed(source, &error.to_string()),
        }),
        _ => {}
    }
    snapshot
}

/// Attaches connector reports to endpoints of the same protocol, each report to at most one
/// endpoint. Every endpoint first takes the report that names its connection, then the one whose
/// target matches its address, then the first one left of its protocol, so a weaker match never
/// takes a report that a stronger one claims. A disabled endpoint runs no connection and takes
/// none. An invalid report claims its endpoint but attaches nothing. Returns the endpoint that
/// each report claimed.
fn attach_connectors(snapshot: &mut Topology, reports: &[ConnectorReport]) -> Vec<Option<String>> {
    let mut endpoints: Vec<&mut Endpoint> = snapshot
        .runtimes_mut()
        .flat_map(|runtime| runtime.endpoints.iter_mut())
        .collect();
    let mut claims: Vec<Option<usize>> = vec![None; reports.len()];
    let mut claimed: Vec<bool> = endpoints
        .iter()
        .map(|endpoint| endpoint.health == "disabled")
        .collect();
    let rules: [fn(&Endpoint, &ConnectorReport) -> bool; 3] =
        [names_match, targets_match, |_, _| true];
    for rule in rules {
        for (index, endpoint) in endpoints.iter().enumerate() {
            if claimed[index] {
                continue;
            }
            let found = reports.iter().zip(&claims).position(|(report, claim)| {
                claim.is_none() && report.protocol == endpoint.protocol && rule(endpoint, report)
            });
            if let Some(report) = found {
                claims[report] = Some(index);
                claimed[index] = true;
            }
        }
    }
    for (report, claim) in reports.iter().zip(&claims) {
        if let (Ok(status), Some(index)) = (&report.status, claim) {
            endpoints[*index].connector = Some(status.clone());
        }
    }
    claims
        .iter()
        .map(|claim| claim.map(|index| endpoints[index].id.clone()))
        .collect()
}

fn names_match(endpoint: &Endpoint, report: &ConnectorReport) -> bool {
    report
        .name
        .as_deref()
        .is_some_and(|name| name == endpoint.name.trim())
}

/// Whether the report's target equals, contains or is contained in the endpoint's address or
/// its `endpoint_url`, `broker`, `host`, `listen` or `interface` parameter.
fn targets_match(endpoint: &Endpoint, report: &ConnectorReport) -> bool {
    let Some(target) = report.target.as_deref() else {
        return false;
    };
    let params = endpoint.params.as_ref();
    let parameters = ["endpoint_url", "broker", "host", "listen", "interface"]
        .into_iter()
        .map(|key| {
            params
                .and_then(|params| params.get(key))
                .and_then(Value::as_str)
        });
    std::iter::once(endpoint.address.as_deref())
        .chain(parameters)
        .flatten()
        .map(str::trim)
        .filter(|candidate| !candidate.is_empty())
        .any(|candidate| target.contains(candidate) || candidate.contains(target))
}

fn merge_host(hosts: &mut Vec<Host>, host: Host) {
    let Some(known) = hosts.iter_mut().find(|candidate| candidate.id == host.id) else {
        hosts.push(host);
        return;
    };
    merge_runtimes(&mut known.runtimes, host.runtimes);
    for container in host.containers {
        let same = known
            .containers
            .iter_mut()
            .find(|candidate| candidate.id == container.id);
        match same {
            Some(same) => merge_runtimes(&mut same.runtimes, container.runtimes),
            None => known.containers.push(container),
        }
    }
}

/// Unions runtimes by identity. The project's configured runtime also folds into the live runtime
/// it describes, so that the live runtime replaces the stopped overlay instead of becoming its
/// twin.
fn merge_runtimes(known: &mut Vec<Runtime>, extra: Vec<Runtime>) {
    for runtime in extra {
        let found = known
            .iter()
            .position(|candidate| candidate.id == runtime.id)
            .or_else(|| {
                known
                    .iter()
                    .position(|candidate| describes(&runtime, candidate))
            });
        match found {
            Some(index) => merge_endpoints(&mut known[index], runtime),
            None => known.push(runtime),
        }
    }
}

/// Whether the project's configured runtime `configured` describes the live runtime `live` of its
/// host: the runtime that the local simulator reports, whose resource name differs from the
/// configured one, or the project's own runtime answering under the configured runtime ID.
fn describes(configured: &Runtime, live: &Runtime) -> bool {
    configured.origin == RuntimeOrigin::Configured
        && match live.origin {
            RuntimeOrigin::Simulator => true,
            RuntimeOrigin::Answered => {
                identity::raw_runtime_id(&live.id) == identity::raw_runtime_id(&configured.id)
            }
            _ => false,
        }
}

/// Unions endpoints by identity, and folds a configured endpoint into the live endpoint it
/// configures, so that no live endpoint appears twice. A configured endpoint that a running
/// runtime does not run yet stays on it and says that a restart applies it; the runtime keeps its
/// live state.
fn merge_endpoints(runtime: &mut Runtime, extra: Runtime) {
    let from_configuration = extra.origin == RuntimeOrigin::Configured;
    let waiting = from_configuration
        && matches!(
            runtime.origin,
            RuntimeOrigin::Answered | RuntimeOrigin::Simulator
        );
    for mut endpoint in extra.endpoints {
        let found = runtime
            .endpoints
            .iter()
            .position(|known| known.id == endpoint.id)
            .or_else(|| {
                runtime
                    .endpoints
                    .iter()
                    .position(|known| replaces(known, &endpoint, from_configuration))
            });
        if let Some(index) = found {
            fill(&mut runtime.endpoints[index], endpoint);
        } else {
            if waiting && endpoint.health == "configured_policy" {
                texts::RESTART_TO_APPLY.clone_into(&mut endpoint.detail);
            }
            runtime.endpoints.push(endpoint);
        }
    }
}

/// Whether `other` is the same endpoint as the live endpoint `known` of its runtime: the
/// configured form of a singleton protocol's endpoint, or, from the project's configuration, the
/// same configured connection (`identity::connection_key`), as when the simulator reports the
/// project's runtime under another resource name.
fn replaces(known: &Endpoint, other: &Endpoint, from_configuration: bool) -> bool {
    if known.protocol != other.protocol {
        return false;
    }
    let configured =
        other.source.as_deref() == Some("config") || other.health == "configured_policy";
    (configured && SINGLETON_PROTOCOLS.contains(&known.protocol.as_str()))
        || (from_configuration && same_connection(&known.id, &other.id))
}

fn same_connection(known: &str, other: &str) -> bool {
    identity::connection_key(known)
        .is_some_and(|key| identity::connection_key(other).as_deref() == Some(key.as_str()))
}

/// Completes `known` with what only `other` has.
fn fill(known: &mut Endpoint, other: Endpoint) {
    if known.params.is_none() {
        known.params = other.params;
    }
    if known.connector.is_none() {
        known.connector = other.connector;
    }
    if known.children.is_empty() {
        known.children = other.children;
    }
    known.supports_test |= other.supports_test;
    known.owned |= other.owned;
}
