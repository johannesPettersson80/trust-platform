//! The `fleet.topology` report: the host, containers, runtimes, endpoints and links that a
//! runtime reports, with the peers it discovered (specification 11, "Offline communication
//! operation and fleet-topology truth"). The control handlers write it. Tools read the reports
//! of schema versions 2, 3 and 4 with
//! [`FleetTopology::decode`](crate::fleet_topology::FleetTopology::decode) (specification 25,
//! 5.6).

use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The schema version that this runtime writes.
pub const FLEET_TOPOLOGY_SCHEMA_VERSION: u32 = 4;

/// The oldest schema version that [`FleetTopology::decode`] reads.
pub const OLDEST_READABLE_SCHEMA_VERSION: u32 = 2;

/// A `fleet.topology` report.
///
/// A field that an older schema version lacks reads as absent or empty: version 2 links have no
/// `id` and no `role`, and its hosts, containers, runtimes, endpoints and external systems have
/// no `source`. Fields that this runtime does not know are ignored.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FleetTopology {
    /// The schema version of the report.
    pub schema_version: u32,
    /// The reporting runtime's own host first, then the hosts it discovered.
    #[serde(default)]
    pub hosts: Vec<FleetHost>,
    /// The links between endpoints, runtimes, shared systems and external systems.
    #[serde(default)]
    pub links: Vec<FleetLink>,
    /// Systems that several runtimes use, such as an MQTT broker.
    #[serde(default)]
    pub shared: Vec<FleetShared>,
    /// Systems outside the fleet that a runtime talks to or sees.
    #[serde(default)]
    pub external: Vec<FleetExternal>,
    /// Runtimes seen by discovery; omitted when there are none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub discovered: Vec<FleetDiscovered>,
}

impl FleetTopology {
    /// Reads a `fleet.topology` report of schema version 2, 3 or 4.
    ///
    /// # Errors
    ///
    /// The value is not an object, has no integer `schema_version`, names a version this runtime
    /// does not read, lacks a required field, or has a field of the wrong type.
    pub fn decode(value: &Value) -> Result<Self, TopologyDecodeError> {
        let Some(object) = value.as_object() else {
            return Err(TopologyDecodeError::NotAnObject);
        };
        let Some(version) = object.get("schema_version").and_then(Value::as_u64) else {
            return Err(TopologyDecodeError::NoSchemaVersion);
        };
        let readable =
            u64::from(OLDEST_READABLE_SCHEMA_VERSION)..=u64::from(FLEET_TOPOLOGY_SCHEMA_VERSION);
        if !readable.contains(&version) {
            return Err(TopologyDecodeError::UnsupportedSchemaVersion(version));
        }
        Self::deserialize(value).map_err(|error| TopologyDecodeError::Malformed(error.to_string()))
    }
}

/// Why a value is not a `fleet.topology` report that this runtime reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TopologyDecodeError {
    /// The value is not a JSON object.
    NotAnObject,
    /// The report has no `schema_version`, or it is not a non-negative integer.
    NoSchemaVersion,
    /// The report has a schema version that this runtime does not read.
    UnsupportedSchemaVersion(u64),
    /// A required field is missing or a field has the wrong type; the decoder's reason.
    Malformed(String),
}

impl fmt::Display for TopologyDecodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotAnObject => formatter.write_str("the fleet topology is not a JSON object"),
            Self::NoSchemaVersion => {
                formatter.write_str("the fleet topology has no schema version")
            }
            Self::UnsupportedSchemaVersion(version) => write!(
                formatter,
                "fleet topology schema version {version} is not supported; versions \
                 {OLDEST_READABLE_SCHEMA_VERSION} to {FLEET_TOPOLOGY_SCHEMA_VERSION} are"
            ),
            Self::Malformed(reason) => {
                write!(formatter, "the fleet topology is malformed: {reason}")
            }
        }
    }
}

impl std::error::Error for TopologyDecodeError {}

/// A computer that runs runtimes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FleetHost {
    /// The host's stable ID.
    pub host_id: String,
    /// The host name.
    pub hostname: String,
    /// The board model, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub board: Option<String>,
    /// The processor architecture, such as `x86_64`; `unknown` for a discovered host.
    pub arch: String,
    /// The operating system, such as `linux`; `unknown` for a discovered host.
    pub os: String,
    /// The host's addresses.
    #[serde(default)]
    pub ips: Vec<String>,
    /// The processor temperature in degrees Celsius, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temp_c: Option<f64>,
    /// The time since the host started, in seconds, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uptime_s: Option<u64>,
    /// The one-minute load average, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub load: Option<f64>,
    /// The containers on the host that run a runtime.
    #[serde(default)]
    pub containers: Vec<FleetContainer>,
    /// The runtimes that run on the host outside a container.
    #[serde(default)]
    pub runtimes: Vec<FleetRuntime>,
    /// Where the facts come from: `self`, `config` or `discovery`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// When the host was last seen, in milliseconds since the Unix epoch.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_seen_ms: Option<u64>,
}

/// A container that runs a runtime.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FleetContainer {
    /// The container's ID.
    pub container_id: String,
    /// The container's name.
    pub name: String,
    /// The image it runs, or `unknown`.
    pub image: String,
    /// The container's state, such as `running`.
    pub status: String,
    /// The runtimes in the container.
    #[serde(default)]
    pub runtimes: Vec<FleetRuntime>,
    /// The restart policy, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub restart_policy: Option<String>,
    /// The processor limit, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpu_limit: Option<String>,
    /// The memory limit, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mem_limit: Option<String>,
    /// The mounted paths, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mounts: Option<Vec<String>>,
    /// Where the facts come from.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

/// A runtime.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FleetRuntime {
    /// The runtime's ID.
    pub runtime_id: String,
    /// The runtime's resource name.
    pub name: String,
    /// The control endpoint, when the report names one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub control_endpoint: Option<String>,
    /// The web server's listen address, when the web server is enabled.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub web_listen: Option<String>,
    /// `online`, `simulate`, `stopped` or `unknown`.
    pub mode: String,
    /// The cycle interval in milliseconds; 0 when unknown.
    pub cycle_ms: u64,
    /// The share of the cycle interval that the last cycle used, from 0 to 1, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub load: Option<f64>,
    /// The runtime's health word, such as `connected` or `configured_policy`.
    pub health: String,
    /// The sentence that explains the health.
    pub detail: String,
    /// The runtime's connections and services.
    #[serde(default)]
    pub endpoints: Vec<FleetEndpoint>,
    /// Where the facts come from.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// When the runtime was last seen, in milliseconds since the Unix epoch.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_seen_ms: Option<u64>,
}

/// One connection or service of a runtime.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FleetEndpoint {
    /// The endpoint's ID, unique within its runtime.
    pub id: String,
    /// `field`, `service` or `peer`.
    pub kind: String,
    /// The protocol, such as `modbus_tcp` or `opcua`.
    pub protocol: String,
    /// The display name.
    pub name: String,
    /// The address it serves or reaches, when it has one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
    /// The runtime's role on it, such as `client`, `server` or `master`, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    /// The endpoint's health word.
    pub health: String,
    /// The sentence that explains the health.
    pub detail: String,
    /// The live evidence, such as samples and their time, when the runtime has it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub live: Option<Value>,
    /// The endpoint's configuration without secrets, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
    /// The devices on a fieldbus, such as EtherCAT terminals; omitted when there are none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<FleetEndpointChild>,
    /// Whether this runtime owns the endpoint's configuration.
    pub owned: bool,
    /// Whether `comm.test` can test the endpoint.
    pub supports_test: bool,
    /// Where the facts come from.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

/// A device on a fieldbus endpoint.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FleetEndpointChild {
    /// The device's ID, unique within its endpoint.
    pub id: String,
    /// The device's kind, such as `field_slave`.
    pub kind: String,
    /// The display name.
    pub name: String,
    /// The position on the bus, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slot: Option<u16>,
    /// The model, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// The number of channels, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channels: Option<u16>,
    /// The device's health word.
    pub health: String,
    /// The sentence that explains the health.
    pub detail: String,
    /// Where the facts come from.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

/// A link between two nodes of the report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FleetLink {
    /// The link's ID; absent in schema version 2.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// The ID of the node the link starts at.
    pub from: String,
    /// The ID of the node the link ends at.
    pub to: String,
    /// The protocol.
    pub protocol: String,
    /// The runtime's role on the link, such as `client` or `publish_subscribe`; absent in
    /// schema version 2.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    /// The direction of the traffic, such as `outbound`.
    pub direction: String,
    /// Whether both ends are on the same host.
    pub same_host: bool,
    /// The link's health word, from evidence.
    pub status: String,
    /// Whether the link is protected, for example by TLS.
    pub secure: bool,
    /// The sentence or address that explains the link, when there is one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// A system that several runtimes use, such as an MQTT broker.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FleetShared {
    /// The system's ID.
    pub id: String,
    /// The system's kind, such as `broker`.
    pub kind: String,
    /// The display name.
    pub name: String,
    /// The system's address.
    pub address: String,
    /// The IDs of the runtimes that use it.
    #[serde(default)]
    pub used_by: Vec<String>,
}

/// A system outside the fleet that a runtime talks to or sees.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FleetExternal {
    /// The system's ID.
    pub id: String,
    /// The system's kind, such as `peer` or `runtime`.
    pub kind: String,
    /// The display name.
    pub name: String,
    /// The protocols that reach it.
    #[serde(default)]
    pub via_protocol: Vec<String>,
    /// The direction of the traffic, such as `outbound` or `observed`.
    pub direction: String,
    /// Where the facts come from.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

/// A runtime seen by discovery.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FleetDiscovered {
    /// The ID of the discovered node.
    pub id: String,
    /// The node's kind, such as `runtime`.
    pub kind: String,
    /// The runtime's name.
    pub name: String,
    /// The addresses it announced.
    #[serde(default)]
    pub addresses: Vec<String>,
    /// The protocols that announced it.
    #[serde(default)]
    pub via_protocol: Vec<String>,
    /// The direction, such as `observed`.
    pub direction: String,
    /// Whether the project has adopted the runtime.
    pub adopted: bool,
    /// The control endpoint it announced, when it announced one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub control: Option<String>,
    /// The web port it announced, when it announced one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub web_port: Option<u16>,
    /// Whether its web server uses TLS, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub web_tls: Option<bool>,
    /// The mesh port it announced, when it announced one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mesh_port: Option<u16>,
    /// The host group it announced, when it announced one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host_group: Option<String>,
    /// When it was last seen, in milliseconds since the Unix epoch.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_seen_ms: Option<u64>,
    /// Where the facts come from.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

#[cfg(test)]
mod tests;
