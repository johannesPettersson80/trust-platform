//! What runtimes answer, decoded: `fleet.topology` reports through the decoder of
//! `trust_runtime::fleet_topology`, and `connectors.status` one report at a time, so that one
//! invalid report never hides the others (specification 11, "Connector status and discovery
//! truth contract").

use serde::de::DeserializeOwned;
use serde_json::{Map, Value};
use trust_runtime::connectors::ConnectorPointCounts;
use trust_runtime::fleet_topology::{
    FleetContainer, FleetEndpoint, FleetHost, FleetRuntime, FleetTopology, TopologyDecodeError,
};

use super::{
    ConnectorStatus, Container, Endpoint, Host, HostOrigin, Runtime, RuntimeOrigin, Topology,
};
use crate::texts::graph as texts;

/// Reads a `fleet.topology` report of schema version 2, 3 or 4; every runtime in it gets
/// `origin`.
pub(super) fn topology(
    value: &Value,
    origin: RuntimeOrigin,
) -> Result<Topology, TopologyDecodeError> {
    let report = FleetTopology::decode(value)?;
    Ok(Topology {
        schema_version: report.schema_version,
        hosts: report
            .hosts
            .into_iter()
            .map(|reported| host(reported, origin))
            .collect(),
        links: report.links,
        shared: report.shared,
        external: report.external,
        issues: Vec::new(),
        this_computer: None,
    })
}

fn host(host: FleetHost, origin: RuntimeOrigin) -> Host {
    Host {
        id: host.host_id,
        hostname: host.hostname,
        board: host.board,
        arch: host.arch,
        os: host.os,
        ips: host.ips,
        temp_c: host.temp_c,
        uptime_s: host.uptime_s,
        load: host.load,
        source: host.source,
        last_seen_ms: host.last_seen_ms,
        origin: HostOrigin::Reported,
        containers: host
            .containers
            .into_iter()
            .map(|reported| container(reported, origin))
            .collect(),
        runtimes: host
            .runtimes
            .into_iter()
            .map(|reported| runtime(reported, origin))
            .collect(),
    }
}

fn container(container: FleetContainer, origin: RuntimeOrigin) -> Container {
    Container {
        id: container.container_id,
        name: container.name,
        image: container.image,
        status: container.status,
        restart_policy: container.restart_policy,
        cpu_limit: container.cpu_limit,
        mem_limit: container.mem_limit,
        mounts: container.mounts,
        source: container.source,
        runtimes: container
            .runtimes
            .into_iter()
            .map(|reported| runtime(reported, origin))
            .collect(),
    }
}

fn runtime(runtime: FleetRuntime, origin: RuntimeOrigin) -> Runtime {
    Runtime {
        id: runtime.runtime_id,
        name: runtime.name,
        control_endpoint: runtime.control_endpoint,
        web_listen: runtime.web_listen,
        mode: runtime.mode,
        cycle_ms: runtime.cycle_ms,
        load: runtime.load,
        health: runtime.health,
        detail: runtime.detail,
        source: runtime.source,
        last_seen_ms: runtime.last_seen_ms,
        endpoints: runtime.endpoints.into_iter().map(endpoint).collect(),
        instance: None,
        origin,
        managed: None,
        attached: false,
        run_target: false,
    }
}

fn endpoint(endpoint: FleetEndpoint) -> Endpoint {
    Endpoint {
        id: endpoint.id,
        kind: endpoint.kind,
        protocol: endpoint.protocol,
        name: endpoint.name,
        address: endpoint.address,
        role: endpoint.role,
        health: endpoint.health,
        detail: endpoint.detail,
        live: endpoint.live,
        params: endpoint.params,
        children: endpoint.children,
        owned: endpoint.owned,
        supports_test: endpoint.supports_test,
        source: endpoint.source,
        connector: None,
    }
}

/// One report of a `connectors.status` answer, as far as the panel reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ConnectorReport {
    /// The endpoint protocol it describes: an ADS report of a supervisory server describes
    /// `ads_server`, an OPC UA report of a supervisory client `opcua_client`.
    pub(super) protocol: String,
    /// The connection name it reports (`display_name`).
    pub(super) name: Option<String>,
    /// The target it reports (`endpoint`).
    pub(super) target: Option<String>,
    /// The status, or why the report is invalid.
    pub(super) status: Result<ConnectorStatus, String>,
}

/// The reports of a `connectors.status` answer that name a protocol, in their order. An answer
/// without a report list has none.
pub(super) fn connector_reports(answer: &Value) -> Vec<ConnectorReport> {
    answer
        .get("connectors")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_object)
        .filter_map(connector_report)
        .collect()
}

fn connector_report(report: &Map<String, Value>) -> Option<ConnectorReport> {
    let protocol = match (text(report, "protocol")?, text(report, "kind")) {
        ("ads", Some("supervisory_server")) => "ads_server",
        ("opcua", Some("supervisory_client")) => "opcua_client",
        (protocol, _) => protocol,
    };
    Some(ConnectorReport {
        protocol: protocol.to_owned(),
        name: text(report, "display_name").map(str::to_owned),
        target: text(report, "endpoint").map(str::to_owned),
        status: status(report),
    })
}

/// A report's status. State, health and confidence must be values of the closed vocabularies;
/// a missing or malformed point count reads as zero.
fn status(report: &Map<String, Value>) -> Result<ConnectorStatus, String> {
    let counts = report.get("point_counts");
    Ok(ConnectorStatus {
        connector_id: text(report, "connector_id")
            .unwrap_or("connector")
            .to_owned(),
        state: vocabulary(report, "state")?,
        health: vocabulary(report, "health")?,
        confidence: vocabulary(report, "confidence")?,
        point_counts: ConnectorPointCounts {
            total: count(counts, "total"),
            good: count(counts, "good"),
            degraded: count(counts, "degraded"),
            unavailable: count(counts, "unavailable"),
        },
    })
}

fn vocabulary<T: DeserializeOwned>(report: &Map<String, Value>, field: &str) -> Result<T, String> {
    let value = report.get(field);
    value
        .and_then(Value::as_str)
        .and_then(|word| serde_json::from_value(Value::String(word.trim().to_owned())).ok())
        .ok_or_else(|| match value {
            None => texts::connector_value_missing(field),
            Some(Value::String(word)) => texts::connector_value_unknown(field, word),
            Some(other) => texts::connector_value_unknown(field, &other.to_string()),
        })
}

fn count(counts: Option<&Value>, key: &str) -> usize {
    counts
        .and_then(|counts| counts.get(key))
        .and_then(Value::as_u64)
        .and_then(|count| usize::try_from(count).ok())
        .unwrap_or(0)
}

/// The trimmed text of a string field; `None` when it is missing, not a string, or blank.
fn text<'a>(report: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    report
        .get(key)?
        .as_str()
        .map(str::trim)
        .filter(|text| !text.is_empty())
}
