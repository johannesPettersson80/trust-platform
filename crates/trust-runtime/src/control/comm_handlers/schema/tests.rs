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
        ads.get("title").and_then(serde_json::Value::as_str),
        Some("Read from ADS")
    );
    assert_eq!(
        ads.get("purpose").and_then(serde_json::Value::as_str),
        Some("Read selected variables from another ADS device.")
    );
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
            field.get("id").and_then(serde_json::Value::as_str) == Some("worker_tick_interval_ms")
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
        ads_server.get("title").and_then(serde_json::Value::as_str),
        Some("Share over ADS")
    );
    assert_eq!(
        ads_server
            .get("purpose")
            .and_then(serde_json::Value::as_str),
        Some("Let ADS clients read selected truST values.")
    );
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
