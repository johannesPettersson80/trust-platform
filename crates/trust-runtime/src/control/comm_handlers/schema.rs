use serde::Serialize;
use serde_json::json;

use super::contract::COMM_SCHEMA_VERSION;
use super::{ControlResponse, ControlState};

mod fields;

use fields::{
    discovery_fields, ethercat_fields, gpio_fields, loopback_fields, mesh_fields, modbus_fields,
    mqtt_fields, opcua_fields, openot_fields, realtime_fields, runtime_cloud_fields,
    simulated_fields,
};

#[derive(Debug, Serialize)]
struct CommSchemaResponse {
    schema_version: u32,
    family: &'static str,
    protocols: Vec<CommProtocolSchema>,
}

#[derive(Debug, Serialize)]
struct CommProtocolSchema {
    id: &'static str,
    driver: &'static str,
    title: &'static str,
    purpose: &'static str,
    apply_mode: &'static str,
    lifecycle_effect: &'static str,
    supports_test: bool,
    supports_multi_instance: bool,
    actions: Vec<&'static str>,
    fields: Vec<CommFieldSchema>,
    instances: Vec<CommConfiguredInstance>,
}

#[derive(Debug, Serialize)]
struct CommFieldSchema {
    id: &'static str,
    label: &'static str,
    #[serde(rename = "type")]
    field_type: &'static str,
    required: bool,
    advanced: bool,
    secret: bool,
    help: &'static str,
    default: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    validation: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    options: Option<Vec<&'static str>>,
}

#[derive(Debug, Clone, Serialize)]
struct CommConfiguredInstance {
    id: String,
    driver: String,
    display_name: String,
    params: serde_json::Value,
}

pub(super) fn handle_comm_schema(
    id: u64,
    params: Option<serde_json::Value>,
    state: &ControlState,
) -> ControlResponse {
    let filter = params
        .as_ref()
        .and_then(|value| value.get("protocol"))
        .and_then(serde_json::Value::as_str)
        .map(normalize_protocol);
    let instances = configured_instances(state);
    let mut protocols = communication_protocol_schemas(&instances);
    if let Some(filter) = filter {
        protocols.retain(|protocol| protocol.id == filter);
    }
    let response = CommSchemaResponse {
        schema_version: COMM_SCHEMA_VERSION,
        family: "io",
        protocols,
    };
    match serde_json::to_value(response) {
        Ok(value) => ControlResponse::ok(id, value),
        Err(error) => ControlResponse::error(
            id,
            format!("Communication schema serialization failed: {error}"),
        ),
    }
}

pub(super) fn normalize_protocol(value: &str) -> String {
    value.trim().replace('-', "_").to_ascii_lowercase()
}

pub(super) fn protocol_to_driver(protocol: &str) -> Option<&'static str> {
    match normalize_protocol(protocol).as_str() {
        "modbus_tcp" => Some("modbus-tcp"),
        "mqtt" => Some("mqtt"),
        "ethercat" => Some("ethercat"),
        "gpio" => Some("gpio"),
        "simulated" => Some("simulated"),
        "loopback" => Some("loopback"),
        _ => None,
    }
}

pub(super) fn supports_snippet_fallback(protocol: &str) -> bool {
    matches!(
        normalize_protocol(protocol).as_str(),
        "opcua" | "openot" | "discovery" | "mesh" | "realtime_t0" | "runtime_cloud"
    )
}

pub(super) fn driver_to_protocol(driver: &str) -> Option<&'static str> {
    match driver.trim().to_ascii_lowercase().as_str() {
        "modbus-tcp" | "modbus_tcp" => Some("modbus_tcp"),
        "mqtt" => Some("mqtt"),
        "ethercat" => Some("ethercat"),
        "gpio" => Some("gpio"),
        "simulated" | "sim" | "noop" => Some("simulated"),
        "loopback" => Some("loopback"),
        _ => None,
    }
}

fn configured_instances(state: &ControlState) -> Vec<CommConfiguredInstance> {
    let Some(root) = state.project_root.as_ref() else {
        return Vec::new();
    };
    let path = root.join("io.toml");
    let Ok(config) = crate::config::IoConfig::load(&path) else {
        return Vec::new();
    };
    config
        .drivers
        .iter()
        .enumerate()
        .filter_map(|(index, driver)| {
            let protocol = driver_to_protocol(driver.name.as_str())?;
            Some(CommConfiguredInstance {
                id: format!("{protocol}:{index}"),
                driver: driver.name.to_string(),
                display_name: instance_display_name(protocol, index, &driver.params),
                params: serde_json::to_value(&driver.params).unwrap_or_else(|_| json!({})),
            })
        })
        .collect()
}

fn io_protocol_schemas(instances: &[CommConfiguredInstance]) -> Vec<CommProtocolSchema> {
    vec![
        protocol_schema(
            "modbus_tcp",
            "modbus-tcp",
            "Modbus TCP",
            "Read and write register-oriented devices or PLC endpoints.",
            true,
            modbus_fields(),
            instances,
        ),
        protocol_schema(
            "mqtt",
            "mqtt",
            "MQTT",
            "Publish and subscribe process I/O through a broker.",
            true,
            mqtt_fields(),
            instances,
        ),
        protocol_schema(
            "ethercat",
            "ethercat",
            "EtherCAT",
            "Wire deterministic fieldbus I/O through a real NIC.",
            false,
            ethercat_fields(),
            instances,
        ),
        protocol_schema(
            "gpio",
            "gpio",
            "GPIO",
            "Map local Linux/Pi pins to runtime I/O.",
            false,
            gpio_fields(),
            instances,
        ),
        protocol_schema(
            "simulated",
            "simulated",
            "Simulated I/O",
            "Try process I/O without hardware.",
            false,
            simulated_fields(),
            instances,
        ),
        protocol_schema(
            "loopback",
            "loopback",
            "Loopback I/O",
            "Echo outputs back into inputs for fast local sanity checks.",
            false,
            loopback_fields(),
            instances,
        ),
    ]
}

fn communication_protocol_schemas(instances: &[CommConfiguredInstance]) -> Vec<CommProtocolSchema> {
    let mut protocols = Vec::with_capacity(12);
    protocols.extend(io_protocol_schemas(instances));
    protocols.extend([
        snippet_protocol_schema(
            "opcua",
            "OPC UA",
            "Let SCADA, HMI, or historian software read and write exposed PLC tags.",
            opcua_fields(),
        ),
        snippet_protocol_schema(
            "openot",
            "OpenOT",
            "Publish telemetry and evidence records from runtime variables.",
            openot_fields(),
        ),
        snippet_protocol_schema(
            "discovery",
            "Discovery",
            "Find and pair truST runtimes on the network.",
            discovery_fields(),
        ),
        snippet_protocol_schema(
            "mesh",
            "Mesh / Zenoh",
            "Share runtime data with peer, client, or router topology.",
            mesh_fields(),
        ),
        snippet_protocol_schema(
            "realtime_t0",
            "Realtime T0",
            "Configure Linux realtime posture for deterministic same-host exchange.",
            realtime_fields(),
        ),
        snippet_protocol_schema(
            "runtime_cloud",
            "Runtime cloud / federation",
            "Configure federation policy without pretending it is a live link.",
            runtime_cloud_fields(),
        ),
    ]);
    protocols
}

fn protocol_schema(
    id: &'static str,
    driver: &'static str,
    title: &'static str,
    purpose: &'static str,
    supports_test: bool,
    fields: Vec<CommFieldSchema>,
    instances: &[CommConfiguredInstance],
) -> CommProtocolSchema {
    CommProtocolSchema {
        id,
        driver,
        title,
        purpose,
        apply_mode: "native",
        lifecycle_effect: "restart_required",
        supports_test,
        supports_multi_instance: true,
        actions: vec!["add", "edit", "remove", "disable"],
        fields,
        instances: instances
            .iter()
            .filter(|instance| driver_to_protocol(instance.driver.as_str()) == Some(id))
            .cloned()
            .collect(),
    }
}

fn snippet_protocol_schema(
    id: &'static str,
    title: &'static str,
    purpose: &'static str,
    fields: Vec<CommFieldSchema>,
) -> CommProtocolSchema {
    CommProtocolSchema {
        id,
        driver: "",
        title,
        purpose,
        apply_mode: "snippet",
        lifecycle_effect: "deploy_required",
        supports_test: false,
        supports_multi_instance: false,
        actions: vec!["validate"],
        fields,
        instances: Vec::new(),
    }
}

fn instance_display_name(protocol: &str, index: usize, params: &toml::Value) -> String {
    let endpoint = params
        .get("address")
        .or_else(|| params.get("broker"))
        .and_then(toml::Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("");
    if endpoint.is_empty() {
        format!("{protocol} #{index}")
    } else {
        format!("{protocol} {endpoint}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_schema_protocols_match_io_driver_contract_names() {
        let protocols = io_protocol_schemas(&[]);

        for protocol in protocols {
            assert_eq!(
                protocol_to_driver(protocol.id),
                Some(protocol.driver),
                "{} must map to its IoDriverConfig name",
                protocol.id
            );
            assert_eq!(protocol.apply_mode, "native");
            assert!(protocol.supports_multi_instance);
            assert_eq!(protocol.actions, vec!["add", "edit", "remove", "disable"]);
        }
    }

    #[test]
    fn schema_defaults_cover_runtime_io_contract_fields() {
        let protocols = io_protocol_schemas(&[]);

        assert_fields(
            &protocols,
            "modbus_tcp",
            &[
                ("address", json!("127.0.0.1:502")),
                ("unit_id", json!(1)),
                ("input_start", json!(0)),
                ("output_start", json!(0)),
                ("timeout_ms", json!(500)),
                ("on_error", json!("fault")),
            ],
        );
        assert_fields(
            &protocols,
            "mqtt",
            &[
                ("broker", json!("127.0.0.1:1883")),
                ("client_id", json!("")),
                ("topic_in", json!("trust/io/in")),
                ("topic_out", json!("trust/io/out")),
                ("password", serde_json::Value::Null),
                ("tls", json!(false)),
                ("allow_insecure_remote", json!(false)),
                ("reconnect_ms", json!(500)),
                ("keep_alive_s", json!(5)),
                ("tls_alpn", json!([])),
            ],
        );
        for protocol in ["simulated", "loopback"] {
            assert_fields(
                &protocols,
                protocol,
                &[
                    ("input_count", json!(8)),
                    ("output_count", json!(8)),
                    ("scan_period_ms", json!(10)),
                ],
            );
        }
    }

    fn assert_fields(
        protocols: &[CommProtocolSchema],
        protocol_id: &str,
        expected: &[(&str, serde_json::Value)],
    ) {
        let protocol = protocols
            .iter()
            .find(|protocol| protocol.id == protocol_id)
            .unwrap_or_else(|| panic!("missing protocol {protocol_id}"));
        for (field_id, default) in expected {
            let field = protocol
                .fields
                .iter()
                .find(|field| field.id == *field_id)
                .unwrap_or_else(|| panic!("missing {protocol_id}.{field_id}"));
            assert_eq!(
                &field.default, default,
                "default drift for {protocol_id}.{field_id}"
            );
        }
    }
}
