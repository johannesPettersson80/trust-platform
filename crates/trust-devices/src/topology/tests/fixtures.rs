//! The reports of the TypeScript tests that these tests port, as the JSON that runtimes send.

use std::collections::BTreeSet;

use serde_json::{json, Value};
use trust_runtime_client::TargetProbe;

use crate::topology::{wire, Endpoint, Runtime, RuntimeOrigin, RuntimeReport, Topology};

/// The options of `legacyMeshSnapshot`: one host with one runtime whose mesh endpoint links to one
/// configured mesh peer, and a shared MQTT broker that the runtime uses.
pub(super) struct Mesh {
    schema: u64,
    host: &'static str,
    runtime: &'static str,
    peer: &'static str,
    status: &'static str,
    detail: &'static str,
    container: Option<&'static str>,
}

impl Mesh {
    pub(super) fn new(
        schema: u64,
        host: &'static str,
        runtime: &'static str,
        peer: &'static str,
        status: &'static str,
        detail: &'static str,
    ) -> Self {
        Self {
            schema,
            host,
            runtime,
            peer,
            status,
            detail,
            container: None,
        }
    }

    pub(super) fn in_container(mut self, container: &'static str) -> Self {
        self.container = Some(container);
        self
    }

    /// The report as a runtime of the schema version sends it: version 3 adds the link's `id`
    /// and `role`, version 4 adds `source` everywhere and the endpoint's `role`.
    pub(super) fn report(&self) -> Value {
        let external_id = "external:mesh:0";
        let endpoint_id = format!("endpoint:{}:mesh", self.runtime);
        let v4 = self.schema == 4;
        let mut endpoint = json!({
            "id": endpoint_id, "kind": "peer", "protocol": "mesh", "name": "Mesh / Zenoh",
            "health": self.status, "detail": self.detail, "owned": true, "supports_test": false,
        });
        let mut runtime = json!({
            "runtime_id": self.runtime, "name": self.runtime, "mode": "online", "cycle_ms": 10,
            "health": "connected", "detail": "Running.",
        });
        let mut link = json!({
            "from": endpoint_id, "to": external_id, "protocol": "mesh", "direction": "outbound",
            "same_host": false, "status": self.status, "secure": true, "detail": self.detail,
        });
        let mut host = json!({
            "host_id": self.host, "hostname": self.host, "arch": "x86_64", "os": "linux",
            "ips": [], "containers": [], "runtimes": [],
        });
        let mut external = json!({
            "id": external_id, "kind": "peer", "name": self.peer, "via_protocol": ["mesh"],
            "direction": "outbound",
        });
        if self.schema >= 3 {
            link["id"] = json!(format!(
                "link:mesh:peer:{}:{}",
                legacy_id_part(&endpoint_id),
                legacy_id_part(external_id)
            ));
            link["role"] = json!("peer");
        }
        if v4 {
            endpoint["role"] = json!("peer");
            endpoint["source"] = json!("self");
            runtime["source"] = json!("self");
            host["source"] = json!("self");
            external["source"] = json!("config");
        }
        runtime["endpoints"] = json!([endpoint]);
        match self.container {
            Some(container) => {
                let mut frame = json!({
                    "container_id": container, "name": container, "image": "runtime:latest",
                    "status": "running", "runtimes": [runtime],
                });
                if v4 {
                    frame["source"] = json!("self");
                }
                host["containers"] = json!([frame]);
            }
            None => host["runtimes"] = json!([runtime]),
        }
        json!({
            "schema_version": self.schema,
            "hosts": [host],
            "links": [link],
            "shared": [{
                "id": "shared:mqtt:broker", "kind": "broker", "name": "MQTT broker",
                "address": "127.0.0.1:1883", "used_by": [self.runtime],
            }],
            "external": [external],
        })
    }
}

/// `legacyFleetIdPart`: how older runtimes sanitized the parts of a link ID.
fn legacy_id_part(value: &str) -> String {
    let sanitized: String = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let trimmed = sanitized.trim_matches('-');
    if trimmed.is_empty() {
        "unknown".to_owned()
    } else {
        trimmed.to_owned()
    }
}

/// `fleetTopology()` of `network-canvas.test.ts`, at schema version 4: one runtime with a healthy
/// Modbus meter, a degraded MQTT endpoint linked to a shared broker, and a mesh endpoint linked
/// to one configured peer.
pub(super) fn fleet_topology() -> Value {
    json!({
        "schema_version": 4,
        "hosts": [{
            "host_id": "host:trust-pi", "hostname": "trust-pi", "arch": "aarch64", "os": "linux",
            "ips": ["192.168.77.10"], "containers": [],
            "runtimes": [{
                "runtime_id": "runtime-a", "name": "Line runtime", "web_listen": "0.0.0.0:8080",
                "mode": "simulate", "cycle_ms": 10, "health": "connected",
                "detail": "Runtime answered fleet.topology.",
                "endpoints": [
                    {
                        "id": "endpoint:runtime-a:modbus_tcp", "kind": "field",
                        "protocol": "modbus_tcp", "name": "Modbus meter", "role": "owned_driver",
                        "health": "connected", "detail": "Driver is healthy.", "owned": true,
                        "supports_test": true,
                    },
                    {
                        "id": "endpoint:runtime-a:mqtt", "kind": "field", "protocol": "mqtt",
                        "name": "MQTT broker", "role": "owned_driver", "health": "degraded",
                        "detail": "Broker connection refused.", "owned": true,
                        "supports_test": true,
                    },
                    {
                        "id": "endpoint:runtime-a:mesh", "kind": "peer", "protocol": "mesh",
                        "name": "Mesh / Zenoh", "role": "peer", "health": "degraded",
                        "detail": "One configured peer.", "owned": true, "supports_test": false,
                    },
                ],
            }],
        }],
        "links": [
            {
                "id": "link:mqtt:broker", "from": "endpoint:runtime-a:mqtt",
                "to": "shared:mqtt:broker", "protocol": "mqtt", "role": "publish_subscribe",
                "direction": "publish_subscribe", "same_host": false,
                "status": "configured_policy", "secure": false,
                "detail": "MQTT broker referenced by io.toml",
            },
            {
                "id": "link:mesh:peer", "from": "endpoint:runtime-a:mesh",
                "to": "external:runtime-a:mesh:0", "protocol": "mesh", "direction": "outbound",
                "same_host": false, "status": "degraded", "secure": true,
                "detail": "tcp/192.168.77.11:7447",
            },
        ],
        "shared": [{
            "id": "shared:mqtt:broker", "kind": "broker", "name": "MQTT broker",
            "address": "127.0.0.1:1883", "used_by": ["runtime-a"],
        }],
        "external": [{
            "id": "external:runtime-a:mesh:0", "kind": "peer", "name": "tcp/192.168.77.11:7447",
            "via_protocol": ["mesh"], "direction": "outbound",
        }],
    })
}

/// A report read as one runtime's own snapshot.
pub(super) fn snapshot(report: &Value) -> Topology {
    wire::topology(report, RuntimeOrigin::Answered).expect("the report is readable")
}

/// What a target at `endpoint` answered: `topology` for `fleet.topology`, and no connector
/// status.
pub(super) fn answered(endpoint: &str, topology: Value) -> RuntimeReport {
    RuntimeReport {
        endpoint: endpoint.to_owned(),
        label: String::new(),
        probe: TargetProbe::Reachable(json!({"state": "running"})),
        topology: Some(Ok(topology)),
        connectors: None,
        stable_id: None,
    }
}

/// A target that did not answer, or refused the token.
pub(super) fn unanswered(endpoint: &str, label: &str, probe: TargetProbe) -> RuntimeReport {
    RuntimeReport {
        endpoint: endpoint.to_owned(),
        label: label.to_owned(),
        probe,
        topology: None,
        connectors: None,
        stable_id: None,
    }
}

pub(super) fn runtime<'a>(topology: &'a Topology, name: &str) -> &'a Runtime {
    topology
        .runtimes()
        .find(|runtime| runtime.name == name)
        .unwrap_or_else(|| panic!("a runtime named {name}"))
}

pub(super) fn endpoint<'a>(runtime: &'a Runtime, protocol: &str) -> &'a Endpoint {
    runtime
        .endpoints
        .iter()
        .find(|endpoint| endpoint.protocol == protocol)
        .unwrap_or_else(|| panic!("a {protocol} endpoint on {}", runtime.name))
}

pub(super) fn runtime_ids(topology: &Topology) -> Vec<String> {
    topology
        .runtimes()
        .map(|runtime| runtime.id.clone())
        .collect()
}

pub(super) fn endpoint_ids(topology: &Topology) -> Vec<String> {
    topology
        .runtimes()
        .flat_map(|runtime| runtime.endpoints.iter().map(|endpoint| endpoint.id.clone()))
        .collect()
}

pub(super) fn link_ids(topology: &Topology) -> Vec<String> {
    topology
        .links
        .iter()
        .map(|link| link.id.clone().unwrap_or_default())
        .collect()
}

pub(super) fn external_ids(topology: &Topology) -> Vec<String> {
    topology
        .external
        .iter()
        .map(|external| external.id.clone())
        .collect()
}

/// How many different values `ids` holds.
pub(super) fn distinct(ids: &[String]) -> usize {
    ids.iter().collect::<BTreeSet<_>>().len()
}
