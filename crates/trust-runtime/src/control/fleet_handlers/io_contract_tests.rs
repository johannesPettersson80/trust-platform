use serde_json::{json, Value as JsonValue};
use smol_str::SmolStr;

use super::*;

fn params(source: &str) -> toml::Value {
    toml::from_str(source).expect("valid test TOML")
}

fn driver_config(name: &str, enabled: bool, params: toml::Value) -> IoDriverConfig {
    IoDriverConfig {
        name: SmolStr::new(name),
        params,
        enabled,
    }
}

fn driver_status(name: &str, health: IoDriverHealth) -> IoDriverStatus {
    IoDriverStatus {
        name: SmolStr::new(name),
        health,
    }
}

fn snapshot_entry(
    name: Option<&str>,
    address: &str,
    value: crate::io::IoSnapshotValue,
) -> IoSnapshotEntry {
    IoSnapshotEntry {
        name: name.map(SmolStr::new),
        address: IoAddress::parse(address).expect("valid I/O address"),
        value_type: None,
        value,
        source: None,
    }
}

fn endpoint_json(endpoint: &FleetEndpoint) -> JsonValue {
    serde_json::to_value(endpoint).expect("endpoint must serialize")
}

#[test]
fn fleet_io_contract_normalizes_modbus_hyphen_and_underscore_spellings() {
    assert_eq!(protocol_from_driver_name("modbus-tcp"), "modbus_tcp");
    assert_eq!(protocol_from_driver_name("modbus_tcp"), "modbus_tcp");
}

#[test]
fn fleet_io_contract_normalizes_case_whitespace_and_generic_hyphens() {
    assert_eq!(protocol_from_driver_name("  MQTT  "), "mqtt");
    assert_eq!(protocol_from_driver_name("CUSTOM-WIRE"), "custom_wire");
}

#[test]
fn fleet_io_contract_same_driver_name_uses_canonical_protocol() {
    assert!(same_driver_name(" MODBUS_TCP ", "modbus-tcp"));
    assert!(same_driver_name("MQTT", "mqtt"));
    assert!(!same_driver_name("mqtt", "modbus-tcp"));
}

#[test]
fn fleet_io_contract_driver_roles_are_protocol_specific() {
    assert_eq!(driver_role("modbus_tcp"), "client");
    assert_eq!(driver_role("mqtt"), "client");
    assert_eq!(driver_role("ethercat"), "master");
    assert_eq!(driver_role("gpio"), "owned_driver");
    assert_eq!(driver_role("custom"), "owned_driver");
}

#[test]
fn fleet_io_contract_driver_address_prefers_nonempty_address() {
    let value = params(
        r#"
address = " 192.0.2.10:502 "
broker = "mqtt.example:1883"
"#,
    );
    assert_eq!(
        driver_endpoint_address(&value),
        Some("192.0.2.10:502".to_string())
    );
}

#[test]
fn fleet_io_contract_driver_address_falls_back_to_broker() {
    let value = params(r#"broker = " mqtt.example:1883 ""#);
    assert_eq!(
        driver_endpoint_address(&value),
        Some("mqtt.example:1883".to_string())
    );
}

#[test]
fn fleet_io_contract_empty_address_does_not_hide_nonempty_broker() {
    let value = params(
        r#"
address = " "
broker = "mqtt.example:1883"
"#,
    );
    assert_eq!(
        driver_endpoint_address(&value),
        Some("mqtt.example:1883".to_string())
    );
}

#[test]
fn fleet_io_contract_driver_address_rejects_empty_or_non_string_values() {
    assert_eq!(driver_endpoint_address(&params(r#"address = "  ""#)), None);
    assert_eq!(driver_endpoint_address(&params("address = 42")), None);
    assert_eq!(driver_endpoint_address(&params("")), None);
}

#[test]
fn fleet_io_contract_display_name_uses_protocol_titles() {
    let empty = params("");
    assert_eq!(driver_display_name("modbus_tcp", 0, &empty), "Modbus TCP");
    assert_eq!(driver_display_name("mqtt", 0, &empty), "MQTT broker");
    assert_eq!(driver_display_name("ethercat", 0, &empty), "EtherCAT");
    assert_eq!(driver_display_name("gpio", 0, &empty), "GPIO");
    assert_eq!(driver_display_name("simulated", 0, &empty), "Simulated I/O");
    assert_eq!(driver_display_name("loopback", 0, &empty), "Loopback I/O");
}

#[test]
fn fleet_io_contract_display_name_appends_one_based_ordinal_after_first() {
    let empty = params("");
    assert_eq!(driver_display_name("mqtt", 1, &empty), "MQTT broker 2");
    assert_eq!(driver_display_name("custom", 4, &empty), "custom 5");
}

#[test]
fn fleet_io_contract_display_name_uses_trimmed_endpoint_address() {
    let value = params(r#"broker = " broker.local:1883 ""#);
    assert_eq!(
        driver_display_name("mqtt", 7, &value),
        "MQTT broker broker.local:1883"
    );
}

#[test]
fn fleet_io_contract_bool_param_accepts_only_boolean_true() {
    assert!(bool_param(&params("enabled = true"), "enabled"));
    assert!(!bool_param(&params("enabled = false"), "enabled"));
    assert!(!bool_param(&params(r#"enabled = "true""#), "enabled"));
    assert!(!bool_param(&params(""), "enabled"));
}

#[test]
fn fleet_io_contract_secret_key_vocabulary_is_case_insensitive() {
    for key in [
        "password",
        "PASSWORD",
        "Auth_Token",
        "token",
        "credential",
        "credentials",
        "secret",
        "Client_Secret",
        "private_key",
        "source_ip",
        "SOURCE_CIDR",
        "allowed_clients",
        "clients",
    ] {
        assert!(is_secret_param_key(key), "{key}");
    }
}

#[test]
fn fleet_io_contract_public_keys_are_not_classified_as_secrets() {
    for key in [
        "host",
        "address",
        "broker",
        "username",
        "certificate",
        "client_id",
    ] {
        assert!(!is_secret_param_key(key), "{key}");
    }
}

#[test]
fn fleet_io_contract_redaction_covers_all_credential_aliases() {
    let value = params(
        r#"
password = "p"
auth_token = "a"
token = "t"
credential = "c"
credentials = "cs"
secret = "s"
client_secret = "client"
private_key = "key"
host = "plc.local"
"#,
    );
    assert_eq!(
        redacted_toml_params(&value),
        json!({
            "password": "<redacted>",
            "auth_token": "<redacted>",
            "token": "<redacted>",
            "credential": "<redacted>",
            "credentials": "<redacted>",
            "secret": "<redacted>",
            "client_secret": "<redacted>",
            "private_key": "<redacted>",
            "host": "plc.local",
        })
    );
}

#[test]
fn fleet_io_contract_redaction_covers_access_control_fields() {
    let value = params(
        r#"
source_ip = "192.0.2.10"
source_cidr = "192.0.2.0/24"
allowed_clients = ["one", "two"]
clients = [{ name = "one" }]
public_name = "server"
"#,
    );
    assert_eq!(
        redacted_toml_params(&value),
        json!({
            "source_ip": "<redacted>",
            "source_cidr": "<redacted>",
            "allowed_clients": "<redacted>",
            "clients": "<redacted>",
            "public_name": "server",
        })
    );
}

#[test]
fn fleet_io_contract_redaction_is_recursive_inside_nested_tables() {
    let value = params(
        r#"
[outer]
host = "plc.local"
password = "deep-password"
[outer.inner]
token = "deep-token"
port = 502
"#,
    );
    assert_eq!(
        redacted_toml_params(&value),
        json!({
            "outer": {
                "host": "plc.local",
                "password": "<redacted>",
                "inner": {
                    "token": "<redacted>",
                    "port": 502,
                }
            }
        })
    );
}

#[test]
fn fleet_io_contract_redaction_is_recursive_inside_array_tables() {
    let value = params(
        r#"
[[connections]]
name = "one"
password = "one-password"

[[connections]]
name = "two"
client_secret = "two-secret"
"#,
    );
    assert_eq!(
        redacted_toml_params(&value),
        json!({
            "connections": [
                {"name": "one", "password": "<redacted>"},
                {"name": "two", "client_secret": "<redacted>"},
            ]
        })
    );
}

#[test]
fn fleet_io_contract_redaction_preserves_non_secret_toml_shapes() {
    let value = params(
        r#"
string = "value"
integer = 42
float = 1.5
boolean = true
datetime = 2026-07-30T12:34:56Z
array = [1, 2, 3]
[table]
value = "nested"
"#,
    );
    assert_eq!(
        redacted_toml_params(&value),
        json!({
            "string": "value",
            "integer": 42,
            "float": 1.5,
            "boolean": true,
            "datetime": "2026-07-30T12:34:56Z",
            "array": [1, 2, 3],
            "table": {"value": "nested"},
        })
    );
}

#[test]
fn fleet_io_contract_driver_health_maps_healthy_state() {
    assert_eq!(
        driver_health(&driver_status("mqtt", IoDriverHealth::Ok)),
        ("connected".to_string(), "Driver is healthy.".to_string())
    );
}

#[test]
fn fleet_io_contract_driver_health_retains_degraded_detail() {
    let (health, detail) = driver_health(&driver_status(
        "mqtt",
        IoDriverHealth::Degraded {
            error: SmolStr::new("slow broker"),
        },
    ));
    assert_eq!(health, "degraded");
    assert!(detail.contains("slow broker"));
}

#[test]
fn fleet_io_contract_driver_health_retains_fault_detail() {
    let (health, detail) = driver_health(&driver_status(
        "mqtt",
        IoDriverHealth::Faulted {
            error: SmolStr::new("broker lost"),
        },
    ));
    assert_eq!(health, "error");
    assert!(detail.contains("broker lost"));
}

#[test]
fn fleet_io_contract_health_binding_accepts_protocol_aliases() {
    let statuses = [driver_status("MODBUS_TCP", IoDriverHealth::Ok)];
    let bound = driver_health_for_config(&statuses, 0, "modbus-tcp").expect("bound health");
    assert_eq!(bound.health, IoDriverHealth::Ok);
}

#[test]
fn fleet_io_contract_health_binding_rejects_unrelated_protocol() {
    let statuses = [driver_status("mqtt", IoDriverHealth::Ok)];
    assert!(driver_health_for_config(&statuses, 0, "modbus-tcp").is_none());
}

#[test]
fn fleet_io_contract_health_binding_uses_matching_protocol_ordinal() {
    let statuses = [
        driver_status("mqtt", IoDriverHealth::Ok),
        driver_status(
            "modbus-tcp",
            IoDriverHealth::Degraded {
                error: SmolStr::new("first"),
            },
        ),
        driver_status(
            "modbus_tcp",
            IoDriverHealth::Faulted {
                error: SmolStr::new("second"),
            },
        ),
    ];
    let bound = driver_health_for_config(&statuses, 1, "modbus-tcp").expect("second Modbus health");
    assert_eq!(
        bound.health,
        IoDriverHealth::Faulted {
            error: SmolStr::new("second")
        }
    );
}

#[test]
fn fleet_io_contract_enabled_config_without_health_is_configured_policy() {
    let config = driver_config("mqtt", true, params(r#"broker = "broker:1883""#));
    let endpoint = endpoint_from_driver_config(
        "runtime",
        0,
        &config,
        None,
        None,
        0,
        LIVE_CONFIGURED_DRIVER_DETAILS,
    );
    assert_eq!(endpoint.health, "configured_policy");
    assert!(endpoint.detail.contains("no live driver health"));
    assert!(endpoint.owned);
    assert!(endpoint.live.is_none());
}

#[test]
fn fleet_io_contract_disabled_config_is_not_live_or_testable() {
    let config = driver_config("mqtt", false, params(r#"broker = "broker:1883""#));
    let endpoint = endpoint_from_driver_config(
        "runtime",
        0,
        &config,
        Some(&driver_status("mqtt", IoDriverHealth::Ok)),
        Some(&IoSnapshot::default()),
        123,
        LIVE_CONFIGURED_DRIVER_DETAILS,
    );
    assert_eq!(endpoint.health, "disabled");
    assert!(!endpoint.supports_test);
    assert!(endpoint.live.is_none());
}

#[test]
fn fleet_io_contract_enabled_modbus_alias_is_testable() {
    for name in ["modbus-tcp", "modbus_tcp", "MODBUS-TCP"] {
        let config = driver_config(name, true, params(r#"address = "plc:502""#));
        let endpoint = endpoint_from_driver_config(
            "runtime",
            0,
            &config,
            None,
            None,
            0,
            LIVE_CONFIGURED_DRIVER_DETAILS,
        );
        assert!(endpoint.supports_test, "{name}");
    }
}

#[test]
fn fleet_io_contract_enabled_mqtt_alias_is_testable() {
    for name in ["mqtt", "MQTT"] {
        let config = driver_config(name, true, params(r#"broker = "broker:1883""#));
        let endpoint = endpoint_from_driver_config(
            "runtime",
            0,
            &config,
            None,
            None,
            0,
            LIVE_CONFIGURED_DRIVER_DETAILS,
        );
        assert!(endpoint.supports_test, "{name}");
    }
}

#[test]
fn fleet_io_contract_non_testable_driver_does_not_advertise_test() {
    let config = driver_config("ethercat", true, params(""));
    let endpoint = endpoint_from_driver_config(
        "runtime",
        0,
        &config,
        None,
        None,
        0,
        LIVE_CONFIGURED_DRIVER_DETAILS,
    );
    assert!(!endpoint.supports_test);
}

#[test]
fn fleet_io_contract_configured_endpoint_uses_stable_config_index() {
    let config = driver_config("mqtt", true, params(""));
    let endpoint = endpoint_from_driver_config(
        "runtime",
        3,
        &config,
        None,
        None,
        0,
        LIVE_CONFIGURED_DRIVER_DETAILS,
    );
    assert_eq!(endpoint.id, "endpoint:runtime:mqtt:3");
    assert_eq!(endpoint.protocol, "mqtt");
    assert_eq!(endpoint.role.as_deref(), Some("client"));
}

#[test]
fn fleet_io_contract_configured_endpoint_redacts_params_before_serialization() {
    let config = driver_config(
        "mqtt",
        true,
        params(
            r#"
broker = "broker:1883"
password = "do-not-serialize"
"#,
        ),
    );
    let endpoint = endpoint_from_driver_config(
        "runtime",
        0,
        &config,
        None,
        None,
        0,
        LIVE_CONFIGURED_DRIVER_DETAILS,
    );
    let payload = endpoint_json(&endpoint).to_string();
    assert!(!payload.contains("do-not-serialize"), "{payload}");
    assert!(payload.contains("<redacted>"), "{payload}");
}

#[test]
fn fleet_io_contract_health_only_endpoint_uses_canonical_protocol_and_role() {
    let endpoint = endpoint_from_driver_health(
        "runtime",
        0,
        &driver_status("MODBUS-TCP", IoDriverHealth::Ok),
        None,
        0,
    );
    assert_eq!(endpoint.protocol, "modbus_tcp");
    assert_eq!(endpoint.role.as_deref(), Some("client"));
    assert_eq!(endpoint.health, "connected");
}

#[test]
fn fleet_io_contract_health_only_protocol_alias_advertises_test() {
    for name in ["modbus-tcp", "modbus_tcp", "MODBUS-TCP", "mqtt", "MQTT"] {
        let endpoint = endpoint_from_driver_health(
            "runtime",
            0,
            &driver_status(name, IoDriverHealth::Ok),
            None,
            0,
        );
        assert!(endpoint.supports_test, "{name}");
    }
}

#[test]
fn fleet_io_contract_disabled_config_does_not_consume_live_health_ordinal() {
    let configs = [
        driver_config("modbus-tcp", false, params("")),
        driver_config("mqtt", true, params("")),
    ];
    let health = [driver_status("mqtt", IoDriverHealth::Ok)];
    let endpoints = io_endpoints("runtime", &health, &configs, None, 0);
    assert_eq!(endpoints.len(), 2);
    assert_eq!(endpoints[0].health, "disabled");
    assert_eq!(endpoints[1].health, "connected");
}

#[test]
fn fleet_io_contract_configured_endpoint_order_is_preserved() {
    let configs = [
        driver_config("mqtt", true, params("")),
        driver_config("modbus-tcp", true, params("")),
        driver_config("gpio", false, params("")),
    ];
    let endpoints = io_endpoints("runtime", &[], &configs, None, 0);
    assert_eq!(
        endpoints
            .iter()
            .map(|endpoint| endpoint.protocol.as_str())
            .collect::<Vec<_>>(),
        ["mqtt", "modbus_tcp", "gpio"]
    );
    assert_eq!(endpoints[0].id, "endpoint:runtime:mqtt");
    assert_eq!(endpoints[1].id, "endpoint:runtime:modbus_tcp:1");
    assert_eq!(endpoints[2].id, "endpoint:runtime:gpio:2");
}

#[test]
fn fleet_io_contract_health_only_order_is_preserved_without_config() {
    let health = [
        driver_status("mqtt", IoDriverHealth::Ok),
        driver_status("modbus-tcp", IoDriverHealth::Ok),
    ];
    let endpoints = io_endpoints("runtime", &health, &[], None, 0);
    assert_eq!(
        endpoints
            .iter()
            .map(|endpoint| endpoint.protocol.as_str())
            .collect::<Vec<_>>(),
        ["mqtt", "modbus_tcp"]
    );
}

#[test]
fn fleet_io_contract_snapshot_without_timestamp_has_no_live_evidence() {
    assert_eq!(io_snapshot_live(Some(&IoSnapshot::default()), 0), None);
}

#[test]
fn fleet_io_contract_timestamp_without_snapshot_has_no_live_evidence() {
    assert_eq!(io_snapshot_live(None, 42), None);
}

#[test]
fn fleet_io_contract_snapshot_live_reports_counts_timestamp_and_sample() {
    let snapshot = IoSnapshot {
        scan: Some(7),
        forced: Vec::new(),
        inputs: vec![snapshot_entry(
            Some("Input0"),
            "%IX0.0",
            crate::io::IoSnapshotValue::Value(crate::value::Value::Bool(true)),
        )],
        outputs: vec![snapshot_entry(
            Some("Output0"),
            "%QW2",
            crate::io::IoSnapshotValue::Unresolved,
        )],
        memory: vec![snapshot_entry(
            None,
            "%MD4",
            crate::io::IoSnapshotValue::Error("unavailable".to_string()),
        )],
    };
    assert_eq!(
        io_snapshot_live(Some(&snapshot), 123),
        Some(json!({
            "value": {
                "inputs": 1,
                "outputs": 1,
                "memory": 1,
                "sample": [
                    {
                        "direction": "input",
                        "name": "Input0",
                        "address": "%IX0.0",
                        "value": "Bool(true)",
                    },
                    {
                        "direction": "output",
                        "name": "Output0",
                        "address": "%QW2",
                        "value": "unresolved",
                    },
                    {
                        "direction": "memory",
                        "name": null,
                        "address": "%MD4",
                        "value": {"error": "unavailable"},
                    },
                ],
            },
            "last_seen_ms": 123,
        }))
    );
}

#[test]
fn fleet_io_contract_snapshot_sample_is_capped_at_eight_in_direction_order() {
    let snapshot = IoSnapshot {
        scan: None,
        forced: Vec::new(),
        inputs: (0..5)
            .map(|index| {
                snapshot_entry(
                    Some(format!("I{index}").as_str()),
                    format!("%IB{index}").as_str(),
                    crate::io::IoSnapshotValue::Unresolved,
                )
            })
            .collect(),
        outputs: (0..5)
            .map(|index| {
                snapshot_entry(
                    Some(format!("Q{index}").as_str()),
                    format!("%QB{index}").as_str(),
                    crate::io::IoSnapshotValue::Unresolved,
                )
            })
            .collect(),
        memory: vec![snapshot_entry(
            Some("M0"),
            "%MB0",
            crate::io::IoSnapshotValue::Unresolved,
        )],
    };
    let sample = io_snapshot_sample(&snapshot);
    assert_eq!(sample.len(), 8);
    assert!(sample[..5].iter().all(|item| item["direction"] == "input"));
    assert!(sample[5..].iter().all(|item| item["direction"] == "output"));
    assert_eq!(sample[7]["name"], "Q2");
}

#[test]
fn fleet_io_contract_formats_all_fixed_io_address_sizes() {
    let cases = [
        ("%IX1.2", "%IX1.2"),
        ("%IB2", "%IB2"),
        ("%QW4", "%QW4"),
        ("%MD8", "%MD8"),
        ("%IL16", "%IL16"),
    ];
    for (source, expected) in cases {
        assert_eq!(
            format_io_address(&IoAddress::parse(source).expect("valid address")),
            expected
        );
    }
}

#[test]
fn fleet_io_contract_formats_fixed_byte_array_as_byte_address() {
    let address = IoAddress {
        area: IoArea::Output,
        size: crate::io::IoSize::Bytes(16),
        byte: 24,
        bit: 0,
        path: vec![24],
        wildcard: false,
    };
    assert_eq!(format_io_address(&address), "%QB24");
}

#[test]
fn fleet_io_contract_formats_wildcard_with_explicit_bit_size() {
    let address = IoAddress::parse("%I*").expect("wildcard address");
    assert_eq!(format_io_address(&address), "%IX*");
}

#[test]
fn fleet_io_contract_format_value_distinguishes_value_error_and_unresolved() {
    let value = snapshot_entry(
        None,
        "%IB0",
        crate::io::IoSnapshotValue::Value(crate::value::Value::USInt(7)),
    );
    let error = snapshot_entry(
        None,
        "%IB0",
        crate::io::IoSnapshotValue::Error("bad read".to_string()),
    );
    let unresolved = snapshot_entry(None, "%IB0", crate::io::IoSnapshotValue::Unresolved);
    assert_eq!(format_io_value(&value), json!("USInt(7)"));
    assert_eq!(format_io_value(&error), json!({"error": "bad read"}));
    assert_eq!(format_io_value(&unresolved), json!("unresolved"));
}
