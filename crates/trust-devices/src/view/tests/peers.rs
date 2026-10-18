//! The views of the tests of the connections, devices, links, search and issues: runtimes on other
//! computers that answer with their topology and connector status, the catalog that
//! `trust-runtime comm schema` answers, and lookups into a view.

use serde_json::{json, Value};
use trust_runtime::control::offline_comm_schema_json;

use super::fixtures::{answered, gathered, host, no_project, request, status, NOW_MS};
use crate::gather::{ProjectInputs, TargetReport};
use crate::request::{HostFacts, RunMode, ViewOptions};
use crate::targets::TargetOrigin;
use crate::view::project as project_view;
use crate::view::types::{Connection, Device, DevicesView, Link, Section};

/// The control endpoint of the runtime on another computer.
pub(super) const PEER: &str = "tcp://192.168.77.10:5680";

/// A workspace without a project, with the catalog that `trust-runtime comm schema` answers.
pub(super) fn with_catalog() -> ProjectInputs {
    ProjectInputs {
        catalog: offline_comm_schema_json(None),
        ..no_project()
    }
}

/// The runtime at `endpoint`, whose program is in `state`, answering with `topology` and, when
/// given, with the connector status `connectors`.
pub(super) fn peer(
    endpoint: &str,
    state: &str,
    topology: Value,
    connectors: Option<Value>,
) -> TargetReport {
    let mut target = answered(
        TargetOrigin::Configured,
        endpoint,
        None,
        status(state, "admin"),
        topology,
    );
    target.live.connectors = connectors.map(Ok);
    target
}

/// The view of `targets`, with the catalog, the extension's facts `host`, and the search and
/// filter of `options`.
pub(super) fn view_of(
    targets: Vec<TargetReport>,
    host: HostFacts,
    options: ViewOptions,
) -> DevicesView {
    let mut request = request(host);
    request.view = options;
    project_view(&gathered(with_catalog(), targets), &request, None, NOW_MS)
}

/// The view of one running runtime at `PEER` that answers with `topology` and `connectors`.
pub(super) fn peer_view(
    topology: Value,
    connectors: Option<Value>,
    options: ViewOptions,
) -> DevicesView {
    view_of(
        vec![peer(PEER, "running", topology, connectors)],
        host(RunMode::Simulate, ""),
        options,
    )
}

/// No search and no filter.
pub(super) fn plain() -> ViewOptions {
    ViewOptions::default()
}

/// A search for `text`.
pub(super) fn searched(text: &str) -> ViewOptions {
    ViewOptions {
        search: text.to_owned(),
        ..ViewOptions::default()
    }
}

/// A filter that hides `protocols`.
pub(super) fn filtered(protocols: &[&str]) -> ViewOptions {
    ViewOptions {
        hidden_protocols: protocols
            .iter()
            .map(|protocol| (*protocol).to_owned())
            .collect(),
        ..ViewOptions::default()
    }
}

/// The `connectors.status` answer of the TypeScript test: a healthy Modbus driver and a stale
/// MQTT one.
pub(super) fn connector_answer() -> Value {
    json!({
        "schema_version": 1,
        "connectors": [
            {"connector_id": "io:modbus-tcp", "protocol": "modbus_tcp", "kind": "process_image",
             "state": "ready", "health": "ok", "confidence": "confirmed",
             "point_counts": {"total": 4, "good": 4, "degraded": 0, "unavailable": 0}},
            {"connector_id": "io:mqtt", "protocol": "mqtt", "kind": "process_image",
             "state": "stale", "health": "degraded", "confidence": "port_reachable",
             "point_counts": {"total": 3, "good": 1, "degraded": 1, "unavailable": 1}},
        ],
    })
}

/// A topology of the runtime `Line runtime` on the computer `host:trust-pi`, with `endpoints`,
/// `links` and `external` systems.
pub(super) fn topology_of(endpoints: Vec<Value>, links: Vec<Value>, external: Vec<Value>) -> Value {
    json!({
        "schema_version": 4,
        "hosts": [{
            "host_id": "host:trust-pi", "hostname": "trust-pi", "arch": "aarch64", "os": "linux",
            "ips": ["192.168.77.10"], "containers": [],
            "runtimes": [{
                "runtime_id": "runtime-a", "name": "Line runtime", "mode": "online",
                "cycle_ms": 10, "health": "connected",
                "detail": "Runtime answered fleet.topology.",
                "endpoints": Value::Array(endpoints),
            }],
        }],
        "links": Value::Array(links), "shared": [], "external": Value::Array(external),
    })
}

/// An endpoint of `Line runtime`.
pub(super) fn endpoint_of(
    id: &str,
    protocol: &str,
    name: &str,
    health: &str,
    detail: &str,
) -> Value {
    json!({
        "id": id, "kind": "field", "protocol": protocol, "name": name, "health": health,
        "detail": detail, "owned": true, "supports_test": false,
    })
}

/// An outbound link from `from` to `to`.
pub(super) fn link_of(
    id: &str,
    from: &str,
    to: &str,
    protocol: &str,
    role: &str,
    status: &str,
) -> Value {
    json!({
        "id": id, "from": from, "to": to, "protocol": protocol, "role": role,
        "direction": "outbound", "same_host": false, "status": status, "secure": false,
        "detail": format!("detail of {id}"),
    })
}

/// An external system that `protocol` reaches.
pub(super) fn external_of(id: &str, name: &str, protocol: &str) -> Value {
    json!({
        "id": id, "kind": "server", "name": name, "via_protocol": [protocol],
        "direction": "outbound",
    })
}

/// Every connection of the view.
pub(super) fn connections(view: &DevicesView) -> Vec<&Connection> {
    view.computers
        .iter()
        .flat_map(|computer| &computer.runtimes)
        .flat_map(|runtime| &runtime.connections)
        .collect()
}

/// The connection named `name`.
pub(super) fn connection<'a>(view: &'a DevicesView, name: &str) -> &'a Connection {
    connections(view)
        .into_iter()
        .find(|connection| connection.name == name)
        .unwrap_or_else(|| panic!("a connection named {name}"))
}

/// The device named `name`.
pub(super) fn device<'a>(view: &'a DevicesView, name: &str) -> &'a Device {
    view.devices
        .iter()
        .find(|device| device.name == name)
        .unwrap_or_else(|| panic!("a device named {name}"))
}

/// The link whose status reads `word`.
pub(super) fn link_with<'a>(view: &'a DevicesView, word: &str) -> &'a Link {
    view.links
        .iter()
        .find(|link| {
            link.status
                .as_ref()
                .is_some_and(|status| status.word == word)
        })
        .unwrap_or_else(|| panic!("a link whose status reads {word}"))
}

/// The labels and values of a section's rows.
pub(super) fn pairs(section: &Section) -> Vec<(&str, &str)> {
    section
        .rows
        .iter()
        .map(|row| (row.label.as_str(), row.value.as_str()))
        .collect()
}

/// The value of the first row labelled `label`.
pub(super) fn value<'a>(sections: &'a [Section], label: &str) -> Option<&'a str> {
    sections
        .iter()
        .flat_map(|section| &section.rows)
        .find(|row| row.label == label)
        .map(|row| row.value.as_str())
}

/// The section headed `heading`.
pub(super) fn headed<'a>(sections: &'a [Section], heading: &str) -> &'a Section {
    sections
        .iter()
        .find(|section| section.heading.as_deref() == Some(heading))
        .unwrap_or_else(|| panic!("a section headed {heading}"))
}
