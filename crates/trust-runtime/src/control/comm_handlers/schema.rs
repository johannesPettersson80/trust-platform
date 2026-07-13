use std::path::Path;

use serde::Serialize;
use serde_json::json;

use super::contract::COMM_SCHEMA_VERSION;
use super::{ControlResponse, ControlState};

mod ads;
mod fields;

use ads::{ads_protocol_schema, ads_server_protocol_schema};
use fields::{
    discovery_fields, ethercat_fields, gpio_fields, loopback_fields, mesh_fields, modbus_fields,
    mqtt_fields, opcua_client_fields, opcua_fields, openot_fields, realtime_fields,
    runtime_cloud_fields, simulated_fields,
};

#[derive(Debug, Serialize)]
struct CommSchemaResponse {
    schema_version: u32,
    protocols: Vec<CommProtocolSchema>,
}

#[derive(Debug, Serialize)]
struct CommProtocolSchema {
    id: &'static str,
    driver: &'static str,
    title: &'static str,
    purpose: &'static str,
    availability: &'static str,
    category: &'static str,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    categories: Vec<&'static str>,
    config_home: &'static str,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    visible_when: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize)]
struct CommConfiguredInstance {
    id: String,
    driver: String,
    display_name: String,
    params: serde_json::Value,
}

struct IoProtocolSpec {
    id: &'static str,
    driver: &'static str,
    title: &'static str,
    purpose: &'static str,
    supports_test: bool,
    fields: Vec<CommFieldSchema>,
}

pub(super) fn handle_comm_schema(
    id: u64,
    params: Option<serde_json::Value>,
    state: &ControlState,
) -> ControlResponse {
    match comm_schema_value(params.as_ref(), state.project_root.as_deref()) {
        Ok(value) => ControlResponse::ok(id, value),
        Err(error) => {
            ControlResponse::error(id, format!("Communication schema build failed: {error}"))
        }
    }
}

pub(super) fn static_comm_schema_value(
    params: Option<&serde_json::Value>,
) -> Result<serde_json::Value, String> {
    comm_schema_value(params, None)
}

fn comm_schema_value(
    params: Option<&serde_json::Value>,
    project_root: Option<&Path>,
) -> Result<serde_json::Value, String> {
    let filter = params
        .and_then(|value| value.get("protocol"))
        .and_then(serde_json::Value::as_str)
        .map(normalize_protocol);
    let instances = project_root.map_or_else(Vec::new, configured_instances_from_root);
    let mut protocols = communication_protocol_schemas(&instances);
    if let Some(filter) = filter {
        protocols.retain(|protocol| protocol.id == filter);
    }
    serde_json::to_value(CommSchemaResponse {
        schema_version: COMM_SCHEMA_VERSION,
        protocols,
    })
    .map_err(|error| format!("Communication schema serialization failed: {error}"))
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

pub(super) fn supports_runtime_file_protocol(protocol: &str) -> bool {
    matches!(
        normalize_protocol(protocol).as_str(),
        "opcua"
            | "opcua_client"
            | "openot"
            | "discovery"
            | "mesh"
            | "realtime_t0"
            | "runtime_cloud"
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

fn configured_instances_from_root(root: &Path) -> Vec<CommConfiguredInstance> {
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
            IoProtocolSpec {
                id: "modbus_tcp",
                driver: "modbus-tcp",
                title: "Modbus TCP",
                purpose: "Read and write register-oriented devices or PLC endpoints.",
                supports_test: true,
                fields: modbus_fields(),
            },
            instances,
        ),
        protocol_schema(
            IoProtocolSpec {
                id: "mqtt",
                driver: "mqtt",
                title: "MQTT",
                purpose: "Publish and subscribe process I/O through a broker.",
                supports_test: true,
                fields: mqtt_fields(),
            },
            instances,
        ),
        protocol_schema(
            IoProtocolSpec {
                id: "ethercat",
                driver: "ethercat",
                title: "EtherCAT",
                purpose: "Wire deterministic fieldbus I/O through a real NIC.",
                supports_test: false,
                fields: ethercat_fields(),
            },
            instances,
        ),
        protocol_schema(
            IoProtocolSpec {
                id: "gpio",
                driver: "gpio",
                title: "GPIO",
                purpose: "Map local Linux/Pi pins to runtime I/O.",
                supports_test: false,
                fields: gpio_fields(),
            },
            instances,
        ),
        protocol_schema(
            IoProtocolSpec {
                id: "simulated",
                driver: "simulated",
                title: "Simulated I/O",
                purpose: "Try process I/O without hardware.",
                supports_test: false,
                fields: simulated_fields(),
            },
            instances,
        ),
        protocol_schema(
            IoProtocolSpec {
                id: "loopback",
                driver: "loopback",
                title: "Loopback I/O",
                purpose: "Echo outputs back into inputs for fast local sanity checks.",
                supports_test: false,
                fields: loopback_fields(),
            },
            instances,
        ),
    ]
}

fn communication_protocol_schemas(instances: &[CommConfiguredInstance]) -> Vec<CommProtocolSchema> {
    let mut protocols = Vec::with_capacity(15);
    protocols.extend(io_protocol_schemas(instances));
    protocols.extend([
        runtime_protocol_schema(
            "opcua",
            "OPC UA server",
            "Expose selected truST globals to OPC UA clients such as SCADA, HMI, or historians.",
            "supervisory_service",
            opcua_fields(),
        ),
        opcua_client_protocol_schema(),
        runtime_protocol_schema(
            "openot",
            "OpenOT",
            "Configure OpenOT evidence output; no evidence is published until a runtime reports one.",
            "supervisory_service",
            openot_fields(),
        ),
        runtime_protocol_schema(
            "discovery",
            "Discovery",
            "Find and pair truST runtimes on the network.",
            "peer_link",
            discovery_fields(),
        ),
        runtime_protocol_schema(
            "mesh",
            "Mesh / Zenoh",
            "Connect this runtime to selected peers or a Zenoh router. No live link is active until a runtime reports one.",
            "peer_link",
            mesh_fields(),
        ),
        runtime_protocol_schema(
            "realtime_t0",
            "Realtime T0",
            "Check and request host settings for deterministic same-host exchange. No live link is active until a runtime reports one.",
            "peer_link",
            realtime_fields(),
        ),
        runtime_protocol_schema(
            "runtime_cloud",
            "Runtime cloud / federation",
            "Configure federation policy and link preferences. No live link is active until a runtime reports one.",
            "peer_link",
            runtime_cloud_fields(),
        ),
        ads_server_protocol_schema(),
        ads_protocol_schema(),
    ]);
    protocols
}

fn protocol_schema(
    spec: IoProtocolSpec,
    instances: &[CommConfiguredInstance],
) -> CommProtocolSchema {
    let categories = if spec.id == "mqtt" {
        vec!["field_device", "supervisory_service"]
    } else {
        vec!["field_device"]
    };
    CommProtocolSchema {
        id: spec.id,
        driver: spec.driver,
        title: spec.title,
        purpose: spec.purpose,
        availability: "default",
        category: "field_device",
        categories,
        config_home: "io.toml",
        apply_mode: "file",
        lifecycle_effect: "restart_required",
        supports_test: spec.supports_test,
        supports_multi_instance: true,
        actions: io_protocol_actions(spec.id),
        fields: spec.fields,
        instances: instances
            .iter()
            .filter(|instance| driver_to_protocol(instance.driver.as_str()) == Some(spec.id))
            .cloned()
            .collect(),
    }
}

fn io_protocol_actions(protocol: &str) -> Vec<&'static str> {
    let mut actions = vec!["add", "edit", "upsert", "remove", "disable"];
    if matches!(protocol, "modbus_tcp" | "mqtt" | "ethercat" | "gpio") {
        actions.push("discover");
    }
    if protocol == "ethercat" {
        actions.push("browse_symbols");
    }
    actions
}

fn runtime_protocol_schema(
    id: &'static str,
    title: &'static str,
    purpose: &'static str,
    category: &'static str,
    fields: Vec<CommFieldSchema>,
) -> CommProtocolSchema {
    CommProtocolSchema {
        id,
        driver: "",
        title,
        purpose,
        availability: "default",
        category,
        categories: vec![category],
        config_home: "runtime.toml",
        apply_mode: "file",
        lifecycle_effect: "restart_required",
        supports_test: false,
        supports_multi_instance: false,
        actions: runtime_protocol_actions(id),
        fields,
        instances: Vec::new(),
    }
}

fn runtime_protocol_actions(protocol: &str) -> Vec<&'static str> {
    let mut actions = vec!["edit", "upsert", "remove", "disable"];
    if protocol == "discovery" {
        actions.push("discover");
    }
    if matches!(protocol, "opcua" | "ads_server") {
        actions.push("browse_symbols");
    }
    actions
}

fn opcua_client_protocol_schema() -> CommProtocolSchema {
    CommProtocolSchema {
        id: "opcua_client",
        driver: "",
        title: "OPC UA client",
        purpose: "Read selected nodes from an external OPC UA server.",
        availability: "default",
        category: "peer_link",
        categories: vec!["peer_link"],
        config_home: "opcua_client.toml",
        apply_mode: "file",
        lifecycle_effect: "restart_required",
        supports_test: true,
        supports_multi_instance: true,
        actions: vec![
            "add",
            "edit",
            "upsert",
            "remove",
            "disable",
            "discover",
            "browse_symbols",
            "test",
        ],
        fields: opcua_client_fields(),
        instances: Vec::new(),
    }
}

fn instance_display_name(protocol: &str, index: usize, params: &toml::Value) -> String {
    fn protocol_display_name(protocol: &str) -> &str {
        match protocol {
            "modbus_tcp" => "Modbus TCP",
            "mqtt" => "MQTT broker",
            "ethercat" => "EtherCAT",
            "gpio" => "GPIO",
            "simulated" => "Simulated I/O",
            "loopback" => "Loopback I/O",
            _ => protocol,
        }
    }

    let endpoint = params
        .get("address")
        .or_else(|| params.get("broker"))
        .and_then(toml::Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("");
    let display = protocol_display_name(protocol);
    if endpoint.is_empty() {
        if index == 0 {
            display.to_string()
        } else {
            format!("{display} {}", index + 1)
        }
    } else {
        format!("{display} {endpoint}")
    }
}

#[cfg(test)]
mod tests;
