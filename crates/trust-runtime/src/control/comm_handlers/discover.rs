//! `comm.discover` (specification 11, "comm.discover"): one search of one protocol. This module
//! reads the request, applies its origin rules and builds the answer; the protocol's search is a
//! source of the one search (`crate::search`), asked once and without a deadline.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::super::{ControlResponse, ControlState};
use crate::search::sources::report::{CandidateSink, Deadline, DiscoverCandidate};
use crate::search::sources::scope::DiscoverScope;
use crate::search::sources::{ads, ethercat, gpio, modbus_tcp, mqtt, opcua, trust_runtimes};

const DISCOVER_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CommDiscoverRequest {
    protocol: String,
    #[serde(default = "empty_scope")]
    scope: Value,
    #[serde(default = "default_origin")]
    origin: DiscoverOrigin,
    #[serde(default = "default_passive")]
    passive: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum DiscoverOrigin {
    ThisHost,
    Runtime,
}

#[derive(Debug, Serialize)]
struct CommDiscoverResponse {
    schema_version: u32,
    protocol: String,
    origin: DiscoverOrigin,
    candidates: Vec<DiscoverCandidate>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    warnings: Vec<String>,
}

/// What one request found, in the order the search reported it.
#[derive(Default)]
struct Answer {
    candidates: Vec<DiscoverCandidate>,
    warnings: Vec<String>,
}

impl CandidateSink for Answer {
    fn found(&mut self, candidate: DiscoverCandidate) {
        self.candidates.push(candidate);
    }

    fn warn(&mut self, warning: String) {
        self.warnings.push(warning);
    }
}

pub(super) fn handle_comm_discover(
    id: u64,
    params: Option<Value>,
    state: &ControlState,
) -> ControlResponse {
    let Some(params) = params else {
        return ControlResponse::error(id, "missing comm.discover params".into());
    };
    match discover_value(params, Some(state)) {
        Ok(value) => ControlResponse::ok(id, value),
        Err(error) => ControlResponse::error(id, error),
    }
}

pub(super) fn discover_value(params: Value, state: Option<&ControlState>) -> Result<Value, String> {
    let request: CommDiscoverRequest = serde_json::from_value(params)
        .map_err(|error| format!("invalid comm.discover payload: {error}"))?;
    // The scope holds `ads_ports` and every protocol-specific key; an unknown key is rejected, and
    // a non-object scope is a request error (specification 11, "comm.discover").
    let mut scope = match request.scope {
        Value::Object(object) => object,
        other => {
            return Err(format!(
                "invalid comm.discover payload: scope must be an object, not {other}"
            ))
        }
    };
    let ads_ports = scope.remove("ads_ports");
    let ports = crate::search::sources::ads_ports::checked_ports(ads_ports.as_ref())
        .map_err(|error| format!("invalid comm.discover payload: {error}"))?;
    let scope: DiscoverScope = serde_json::from_value(Value::Object(scope))
        .map_err(|error| format!("invalid comm.discover payload: {error}"))?;
    let protocol = canonical_protocol(request.protocol.as_str());
    let mut answer = Answer::default();
    if !request.passive {
        answer.warn(
            "Active write probes are not supported; discovery ran in connect/read-only mode."
                .to_string(),
        );
    }
    // One request has no deadline: each probe ends by its own timeout.
    let deadline = Deadline::NONE;
    let in_runtime = request.origin == DiscoverOrigin::Runtime;
    match protocol.as_str() {
        "modbus_tcp" => modbus_tcp::discover(&scope, deadline, &mut answer)?,
        "opcua" => {
            answer.warn("OPC UA discovery is not exposed here because truST currently provides OPC UA server setup, not an OPC UA client writer.".to_string());
        }
        "opcua_client" => opcua::discover(&scope, deadline, &mut answer)?,
        "mqtt" => mqtt::discover(&scope, deadline, &mut answer)?,
        "ethercat" if !in_runtime => answer.warn(
            "EtherCAT discovery must run from the runtime host that owns the fieldbus adapter."
                .to_string(),
        ),
        "ethercat" => {
            let project_root = state.and_then(|state| state.project_root.as_deref());
            ethercat::discover(&scope, project_root, deadline, &mut answer)?;
        }
        "gpio" if !in_runtime => answer.warn(
            "GPIO discovery must run from the runtime host that owns the GPIO hardware."
                .to_string(),
        ),
        "gpio" => gpio::discover(deadline, &mut answer),
        "ads" if ports == crate::search::sources::ads_ports::DEFAULT_PORTS => {
            ads::discover(&scope, deadline, &mut answer)?
        }
        "ads" => ads::discover_with_ports(&scope, &ports, deadline, &mut answer)?,
        "discovery" => {
            // A runtime answers from its own discovery; anywhere else the request browses mDNS.
            let discovery = state
                .filter(|_| in_runtime)
                .map(|state| state.discovery.as_ref());
            trust_runtimes::discover(&scope, discovery, deadline, &mut answer)?;
        }
        other if known_protocol_without_discovery(other) => answer.warn(format!(
            "{} discovery is not available yet.",
            protocol_title(other)
        )),
        other => return Err(format!("comm.discover does not know protocol '{other}'")),
    }
    serde_json::to_value(CommDiscoverResponse {
        schema_version: DISCOVER_SCHEMA_VERSION,
        protocol,
        origin: request.origin,
        candidates: answer.candidates,
        warnings: answer.warnings,
    })
    .map_err(|error| format!("comm.discover serialization failed: {error}"))
}

fn default_origin() -> DiscoverOrigin {
    DiscoverOrigin::ThisHost
}

/// A `comm.discover` request with no scope.
fn empty_scope() -> Value {
    Value::Object(serde_json::Map::new())
}

fn default_passive() -> bool {
    true
}

fn canonical_protocol(protocol: &str) -> String {
    match protocol
        .trim()
        .to_ascii_lowercase()
        .replace('-', "_")
        .as_str()
    {
        "modbus_tcp" => "modbus_tcp".to_string(),
        "opcua_client" | "opc_ua_client" => "opcua_client".to_string(),
        "opcua" | "opc_ua" | "opcua_server" | "opc_ua_server" => "opcua".to_string(),
        "mqtt" | "mqtt_broker" => "mqtt".to_string(),
        "ads" | "ads_client" | "twincat" => "ads".to_string(),
        "discovery" | "trust" | "trust_runtime" | "trust_runtimes" | "mdns" => {
            "discovery".to_string()
        }
        other => other.to_string(),
    }
}

fn known_protocol_without_discovery(protocol: &str) -> bool {
    matches!(
        protocol,
        "ethercat"
            | "gpio"
            | "simulated"
            | "loopback"
            | "openot"
            | "opcua"
            | "mesh"
            | "runtime_cloud"
            | "realtime_t0"
            | "ads_server"
    )
}

fn protocol_title(protocol: &str) -> &'static str {
    match protocol {
        "ads_server" => "ADS server",
        "ethercat" => "EtherCAT",
        "gpio" => "GPIO",
        "loopback" => "Loopback",
        "mesh" => "Mesh",
        "openot" => "OpenOT",
        "opcua" => "OPC UA server",
        "opcua_client" => "OPC UA client",
        "realtime_t0" => "Realtime T0",
        "runtime_cloud" => "Runtime cloud",
        "simulated" => "Simulated I/O",
        _ => "Selected protocol",
    }
}

#[cfg(test)]
#[path = "discover_tests.rs"]
mod discover_tests;

#[cfg(test)]
#[path = "discover/contract_tests.rs"]
mod contract_tests;

#[cfg(test)]
mod ads_ports_scope_tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn ads_scope_ports_default_merge_and_reject_invalid() {
        // Specification 11, "comm.discover": `scope.ads_ports` is accepted, defaults when absent,
        // merges duplicates, and rejects an invalid value by naming it.
        assert_eq!(
            crate::search::sources::ads_ports::checked_ports(None).expect("default ports"),
            vec![301, 501, 851, 852, 853, 854]
        );
        discover_value(
            json!({ "protocol": "openot", "scope": { "ads_ports": [851, 301, 851] } }),
            None,
        )
        .expect("the ads_ports scope key is accepted");

        for bad in [json!(0), json!(65536), json!("852"), json!(501.5)] {
            let error = discover_value(
                json!({ "protocol": "openot", "scope": { "ads_ports": [bad] } }),
                None,
            )
            .expect_err("an invalid ADS port must be a request error");
            assert!(
                error.starts_with("invalid comm.discover payload:"),
                "{error}"
            );
            assert!(
                error.contains(&bad.to_string()),
                "the error must name {bad}: {error}"
            );
        }
    }
}
