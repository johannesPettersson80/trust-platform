//! A merged topology written back in the shape of a `fleet.topology` report (specification 11,
//! "Offline communication operation and fleet-topology truth"), with the owner-scoped identities of
//! the merge and, on each endpoint, the connector status its runtime reported (specification 25
//! §5.5). The canvas reads the project's own runtime from it.

use serde_json::{json, Map, Value};
use trust_runtime::fleet_topology::{FleetContainer, FleetEndpoint, FleetHost, FleetRuntime};

use crate::topology::{ConnectorStatus, Container, Endpoint, Host, Runtime, Topology};

/// `topology` as a `fleet.topology` report.
pub(super) fn topology_json(topology: &Topology) -> Value {
    json!({
        "schema_version": topology.schema_version,
        "hosts": topology.hosts.iter().map(host_json).collect::<Vec<_>>(),
        "links": topology.links,
        "shared": topology.shared,
        "external": topology.external,
    })
}

fn host_json(host: &Host) -> Value {
    let wire = FleetHost {
        host_id: host.id.clone(),
        hostname: host.hostname.clone(),
        board: host.board.clone(),
        arch: host.arch.clone(),
        os: host.os.clone(),
        ips: host.ips.clone(),
        temp_c: host.temp_c,
        uptime_s: host.uptime_s,
        load: host.load,
        containers: Vec::new(),
        runtimes: Vec::new(),
        source: host.source.clone(),
        last_seen_ms: host.last_seen_ms,
    };
    with(
        object(&wire),
        [
            (
                "containers",
                Value::Array(host.containers.iter().map(container_json).collect()),
            ),
            (
                "runtimes",
                Value::Array(host.runtimes.iter().map(runtime_json).collect()),
            ),
        ],
    )
}

fn container_json(container: &Container) -> Value {
    let wire = FleetContainer {
        container_id: container.id.clone(),
        name: container.name.clone(),
        image: container.image.clone(),
        status: container.status.clone(),
        runtimes: Vec::new(),
        restart_policy: container.restart_policy.clone(),
        cpu_limit: container.cpu_limit.clone(),
        mem_limit: container.mem_limit.clone(),
        mounts: container.mounts.clone(),
        source: container.source.clone(),
    };
    with(
        object(&wire),
        [(
            "runtimes",
            Value::Array(container.runtimes.iter().map(runtime_json).collect()),
        )],
    )
}

fn runtime_json(runtime: &Runtime) -> Value {
    let wire = FleetRuntime {
        runtime_id: runtime.id.clone(),
        name: runtime.name.clone(),
        control_endpoint: runtime.control_endpoint.clone(),
        web_listen: runtime.web_listen.clone(),
        mode: runtime.mode.clone(),
        cycle_ms: runtime.cycle_ms,
        load: runtime.load,
        health: runtime.health.clone(),
        detail: runtime.detail.clone(),
        endpoints: Vec::new(),
        source: runtime.source.clone(),
        last_seen_ms: runtime.last_seen_ms,
    };
    with(
        object(&wire),
        [(
            "endpoints",
            Value::Array(runtime.endpoints.iter().map(endpoint_json).collect()),
        )],
    )
}

/// One endpoint in the report's shape, with its connector status when its runtime reported one.
pub(super) fn endpoint_json(endpoint: &Endpoint) -> Value {
    let wire = FleetEndpoint {
        id: endpoint.id.clone(),
        kind: endpoint.kind.clone(),
        protocol: endpoint.protocol.clone(),
        name: endpoint.name.clone(),
        address: endpoint.address.clone(),
        role: endpoint.role.clone(),
        health: endpoint.health.clone(),
        detail: endpoint.detail.clone(),
        live: endpoint.live.clone(),
        params: endpoint.params.clone(),
        children: endpoint.children.clone(),
        owned: endpoint.owned,
        supports_test: endpoint.supports_test,
        source: endpoint.source.clone(),
    };
    let object = object(&wire);
    match &endpoint.connector {
        Some(connector) => with(object, [("connector", connector_json(connector))]),
        None => Value::Object(object),
    }
}

/// A connector status in the closed vocabularies of specification 11 ("Connector status and
/// discovery truth contract").
pub(super) fn connector_json(connector: &ConnectorStatus) -> Value {
    json!({
        "connector_id": connector.connector_id,
        "state": connector.state,
        "health": connector.health,
        "confidence": connector.confidence,
        "point_counts": connector.point_counts,
    })
}

/// The JSON object of a report part; these plain data types always serialize to one.
fn object<T: serde::Serialize>(value: &T) -> Map<String, Value> {
    match serde_json::to_value(value) {
        Ok(Value::Object(object)) => object,
        _ => Map::new(),
    }
}

/// `object` with `fields` set.
fn with<const N: usize>(mut object: Map<String, Value>, fields: [(&str, Value); N]) -> Value {
    for (key, value) in fields {
        object.insert(key.to_owned(), value);
    }
    Value::Object(object)
}
