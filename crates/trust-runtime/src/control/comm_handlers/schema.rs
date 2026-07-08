use std::path::Path;

use serde::Serialize;
use serde_json::json;

use super::contract::COMM_SCHEMA_VERSION;
use super::{ControlResponse, ControlState};

mod fields;

use fields::{
    ads_fields, ads_server_fields, discovery_fields, ethercat_fields, gpio_fields, loopback_fields,
    mesh_fields, modbus_fields, mqtt_fields, opcua_client_fields, opcua_fields, openot_fields,
    realtime_fields, runtime_cloud_fields, simulated_fields,
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
        runtime_protocol_schema(
            "ads_server",
            "ADS server",
            "Expose selected truST globals to TwinCAT or ADS clients.",
            "supervisory_service",
            ads_server_fields(),
        ),
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

fn ads_protocol_schema() -> CommProtocolSchema {
    CommProtocolSchema {
        id: "ads",
        driver: "",
        title: "ADS client",
        purpose: "Connect this runtime to a TwinCAT or ADS PLC.",
        availability: "default",
        category: "peer_link",
        categories: vec!["peer_link"],
        config_home: "ads.toml",
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
            "doctor",
            "route_script",
        ],
        fields: ads_fields(),
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
            assert_eq!(protocol.category, "field_device");
            assert_eq!(protocol.config_home, "io.toml");
            assert_eq!(protocol.apply_mode, "file");
            assert!(protocol.supports_multi_instance);
            let mut expected_actions = vec!["add", "edit", "upsert", "remove", "disable"];
            if matches!(protocol.id, "modbus_tcp" | "mqtt" | "ethercat" | "gpio") {
                expected_actions.push("discover");
            }
            if protocol.id == "ethercat" {
                expected_actions.push("browse_symbols");
            }
            assert_eq!(protocol.actions, expected_actions);
        }
    }

    fn field_by_id<'a>(protocol: &'a serde_json::Value, id: &str) -> &'a serde_json::Value {
        protocol
            .get("fields")
            .and_then(serde_json::Value::as_array)
            .and_then(|fields| {
                fields
                    .iter()
                    .find(|field| field.get("id").and_then(serde_json::Value::as_str) == Some(id))
            })
            .unwrap_or_else(|| panic!("missing field {id}"))
    }

    #[test]
    fn schema_v4_exposes_categories_config_homes_and_ads_protocols_without_profiles() {
        let value = static_comm_schema_value(None).expect("schema");
        assert_eq!(
            value
                .pointer("/schema_version")
                .and_then(serde_json::Value::as_u64),
            Some(4)
        );
        assert!(value.get("family").is_none());
        let protocols = value
            .get("protocols")
            .and_then(serde_json::Value::as_array)
            .expect("protocols");
        let by_id = |id: &str| {
            protocols
                .iter()
                .find(|protocol| protocol.get("id").and_then(serde_json::Value::as_str) == Some(id))
                .unwrap_or_else(|| panic!("missing protocol {id}"))
        };
        assert!(
            protocols
                .iter()
                .all(|protocol| protocol.get("profiles").is_none()),
            "comm.schema must not expose rejected device archetype profiles: {protocols:?}"
        );
        assert!(
            protocols.iter().all(|protocol| protocol
                .get("availability")
                .and_then(serde_json::Value::as_str)
                == Some("default")),
            "normal comm.schema protocols must be default-built unless explicitly marked optional_build: {protocols:?}"
        );

        let modbus = by_id("modbus_tcp");
        assert_eq!(
            modbus.get("category").and_then(serde_json::Value::as_str),
            Some("field_device")
        );
        assert_eq!(
            modbus
                .get("config_home")
                .and_then(serde_json::Value::as_str),
            Some("io.toml")
        );
        assert_eq!(
            modbus.get("apply_mode").and_then(serde_json::Value::as_str),
            Some("file")
        );

        let mqtt = by_id("mqtt");
        let categories = mqtt
            .get("categories")
            .and_then(serde_json::Value::as_array)
            .expect("mqtt categories");
        assert!(categories
            .iter()
            .any(|value| value.as_str() == Some("field_device")));
        assert!(categories
            .iter()
            .any(|value| value.as_str() == Some("supervisory_service")));

        let opcua = by_id("opcua");
        assert_eq!(
            opcua.get("title").and_then(serde_json::Value::as_str),
            Some("OPC UA server")
        );
        assert_eq!(
            opcua.get("category").and_then(serde_json::Value::as_str),
            Some("supervisory_service")
        );
        assert_eq!(
            opcua.get("config_home").and_then(serde_json::Value::as_str),
            Some("runtime.toml")
        );
        assert_eq!(
            opcua.get("apply_mode").and_then(serde_json::Value::as_str),
            Some("file")
        );
        let opcua_actions = opcua
            .get("actions")
            .and_then(serde_json::Value::as_array)
            .expect("opcua actions");
        assert!(
            !opcua_actions
                .iter()
                .any(|value| value.as_str() == Some("discover")),
            "OPC UA server must not advertise client-side discovery"
        );
        assert!(opcua_actions
            .iter()
            .any(|value| value.as_str() == Some("browse_symbols")));
        let opcua_expose = field_by_id(opcua, "expose");
        assert_eq!(
            opcua_expose
                .get("label")
                .and_then(serde_json::Value::as_str),
            Some("Exposed globals")
        );
        assert_eq!(
            opcua_expose.get("help").and_then(serde_json::Value::as_str),
            Some("Choose project globals to expose, or add a pattern such as global.*.")
        );

        let opcua_client = by_id("opcua_client");
        assert_eq!(
            opcua_client
                .get("title")
                .and_then(serde_json::Value::as_str),
            Some("OPC UA client")
        );
        assert_eq!(
            opcua_client
                .get("category")
                .and_then(serde_json::Value::as_str),
            Some("peer_link")
        );
        assert_eq!(
            opcua_client
                .get("config_home")
                .and_then(serde_json::Value::as_str),
            Some("opcua_client.toml")
        );
        assert_eq!(
            opcua_client
                .get("apply_mode")
                .and_then(serde_json::Value::as_str),
            Some("file")
        );

        let gpio = by_id("gpio");
        assert_eq!(
            field_by_id(gpio, "chip").pointer("/visible_when/field"),
            Some(&serde_json::Value::String("backend".to_string()))
        );
        assert_eq!(
            field_by_id(gpio, "chip").pointer("/visible_when/equals"),
            Some(&serde_json::Value::String("libgpiod".to_string()))
        );
        assert_eq!(
            field_by_id(gpio, "sysfs_base").pointer("/visible_when/field"),
            Some(&serde_json::Value::String("backend".to_string()))
        );
        assert_eq!(
            field_by_id(gpio, "sysfs_base").pointer("/visible_when/equals"),
            Some(&serde_json::Value::String("sysfs".to_string()))
        );
        assert_eq!(
            opcua_client
                .get("supports_test")
                .and_then(serde_json::Value::as_bool),
            Some(true)
        );
        let opcua_client_actions = opcua_client
            .get("actions")
            .and_then(serde_json::Value::as_array)
            .expect("opcua_client actions");
        for action in ["discover", "browse_symbols", "test"] {
            assert!(
                opcua_client_actions
                    .iter()
                    .any(|value| value.as_str() == Some(action)),
                "missing OPC UA client action {action}"
            );
        }

        let ethercat_actions = by_id("ethercat")
            .get("actions")
            .and_then(serde_json::Value::as_array)
            .expect("ethercat actions");
        for action in ["discover", "browse_symbols"] {
            assert!(
                ethercat_actions
                    .iter()
                    .any(|value| value.as_str() == Some(action)),
                "missing EtherCAT action {action}"
            );
        }

        let gpio_actions = by_id("gpio")
            .get("actions")
            .and_then(serde_json::Value::as_array)
            .expect("gpio actions");
        assert!(gpio_actions
            .iter()
            .any(|value| value.as_str() == Some("discover")));

        let ads = by_id("ads");
        assert_eq!(
            ads.get("category").and_then(serde_json::Value::as_str),
            Some("peer_link")
        );
        assert_eq!(
            ads.get("config_home").and_then(serde_json::Value::as_str),
            Some("ads.toml")
        );
        let ads_actions = ads
            .get("actions")
            .and_then(serde_json::Value::as_array)
            .expect("ads actions");
        for action in ["discover", "browse_symbols", "doctor", "route_script"] {
            assert!(
                ads_actions
                    .iter()
                    .any(|value| value.as_str() == Some(action)),
                "missing ADS action {action}"
            );
        }
        let ads_fields = ads
            .get("fields")
            .and_then(serde_json::Value::as_array)
            .expect("ads fields");
        let update_interval = ads_fields
            .iter()
            .find(|field| {
                field.get("id").and_then(serde_json::Value::as_str)
                    == Some("worker_tick_interval_ms")
            })
            .expect("ads update interval field");
        assert_eq!(
            update_interval
                .get("label")
                .and_then(serde_json::Value::as_str),
            Some("ADS link update interval (ms)")
        );
        let update_help = update_interval
            .get("help")
            .and_then(serde_json::Value::as_str)
            .expect("ads update interval help");
        assert!(update_help.contains("reads, writes, reconnects, and status updates"));
        assert!(!update_help.contains("worker tick"));

        let ads_server = by_id("ads_server");
        assert_eq!(
            ads_server
                .get("category")
                .and_then(serde_json::Value::as_str),
            Some("supervisory_service")
        );
        assert_eq!(
            ads_server
                .get("config_home")
                .and_then(serde_json::Value::as_str),
            Some("runtime.toml")
        );
        let ads_server_actions = ads_server
            .get("actions")
            .and_then(serde_json::Value::as_array)
            .expect("ads_server actions");
        assert!(ads_server_actions
            .iter()
            .any(|value| value.as_str() == Some("browse_symbols")));

        for protocol in ["mesh", "realtime_t0", "runtime_cloud"] {
            let purpose = by_id(protocol)
                .get("purpose")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_else(|| panic!("missing {protocol} purpose"));
            assert!(
                purpose.contains("No live link is active until a runtime reports one."),
                "{protocol} must use the shared configured-only note: {purpose}"
            );
            assert!(!purpose.contains("pretending"));
        }
        let runtime_cloud = by_id("runtime_cloud");
        assert_eq!(
            field_by_id(runtime_cloud, "wan_allow_write")
                .get("label")
                .and_then(serde_json::Value::as_str),
            Some("Allowed WAN writes")
        );

        let openot = by_id("openot");
        assert_eq!(
            field_by_id(openot, "path")
                .get("label")
                .and_then(serde_json::Value::as_str),
            Some("Evidence file")
        );
        assert_eq!(
            field_by_id(openot, "capacity")
                .get("label")
                .and_then(serde_json::Value::as_str),
            Some("Record capacity")
        );

        let realtime = by_id("realtime_t0");
        assert_eq!(
            field_by_id(realtime, "require_preempt_rt_kernel")
                .get("label")
                .and_then(serde_json::Value::as_str),
            Some("Require real-time kernel")
        );
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
                ("on_error", json!("fault")),
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
