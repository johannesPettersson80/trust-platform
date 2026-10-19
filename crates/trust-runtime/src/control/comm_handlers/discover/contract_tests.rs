use serde_json::{json, Value};

use super::*;

fn discover(protocol: &str) -> Result<Value, String> {
    discover_value(
        json!({
            "protocol": protocol,
            "scope": {},
            "origin": "this_host",
            "passive": true
        }),
        None,
    )
}

#[test]
fn request_defaults_are_this_host_and_passive() {
    let value =
        discover_value(json!({ "protocol": "openot" }), None).expect("deferred discovery response");
    assert_eq!(value["schema_version"], DISCOVER_SCHEMA_VERSION);
    assert_eq!(value["protocol"], "openot");
    assert_eq!(value["origin"], "this_host");
    assert_eq!(value["candidates"], json!([]));
    let warnings = value["warnings"].as_array().expect("warnings");
    assert_eq!(warnings.len(), 1);
    assert!(!warnings[0]
        .as_str()
        .expect("warning string")
        .contains("Active write probes"));
}

#[test]
fn active_request_remains_read_only_and_reports_stable_warning() {
    let value = discover_value(
        json!({
            "protocol": "openot",
            "scope": {},
            "origin": "runtime",
            "passive": false
        }),
        None,
    )
    .expect("active request is downgraded safely");
    let warnings = value["warnings"].as_array().expect("warnings");
    assert_eq!(
        warnings[0],
        "Active write probes are not supported; discovery ran in connect/read-only mode."
    );
    assert!(warnings[1]
        .as_str()
        .expect("deferred warning")
        .contains("OpenOT discovery is not available yet"));
}

#[test]
fn request_requires_nonempty_string_protocol() {
    for params in [
        json!({}),
        json!({ "protocol": null }),
        json!({ "protocol": 7 }),
        json!({ "protocol": [] }),
        json!({ "protocol": {} }),
    ] {
        let error = discover_value(params, None).expect_err("malformed protocol accepted");
        assert!(
            error.starts_with("invalid comm.discover payload:"),
            "{error}"
        );
    }
    for protocol in ["", " ", "\t\r\n"] {
        let error = discover(protocol).expect_err("blank protocol accepted");
        assert!(error.contains("does not know protocol"), "{error}");
    }
}

#[test]
fn nonobject_scope_and_unknown_keys_are_rejected() {
    for scope in [
        Value::Null,
        json!(7),
        json!("scope"),
        json!([]),
        json!(true),
    ] {
        let error = discover_value(json!({ "protocol": "openot", "scope": scope }), None)
            .expect_err("nonobject scope accepted");
        assert!(
            error.starts_with("invalid comm.discover payload:"),
            "{error}"
        );
    }

    for params in [
        json!({ "protocol": "openot", "pasive": true }),
        json!({
            "protocol": "openot",
            "scope": { "timeot_ms": 50 }
        }),
    ] {
        let error = discover_value(params, None).expect_err("unknown key accepted");
        assert!(
            error.starts_with("invalid comm.discover payload:"),
            "{error}"
        );
    }
}

#[test]
fn origin_vocabulary_is_closed() {
    for origin in ["this_host", "runtime"] {
        let value = discover_value(json!({ "protocol": "openot", "origin": origin }), None)
            .expect("valid origin");
        assert_eq!(value["origin"], origin);
    }
    for origin in ["host", "local", "remote", "", "RUNTIME"] {
        let error = discover_value(json!({ "protocol": "openot", "origin": origin }), None)
            .expect_err("unknown origin accepted");
        assert!(
            error.starts_with("invalid comm.discover payload:"),
            "{error}"
        );
    }
}

#[test]
fn canonical_protocol_aliases_are_exact() {
    for (alias, expected) in [
        (" MODBUS-TCP ", "modbus_tcp"),
        ("modbus_tcp", "modbus_tcp"),
        ("OPC-UA", "opcua"),
        ("opc_ua_server", "opcua"),
        ("opcua_server", "opcua"),
        ("OPC-UA-CLIENT", "opcua_client"),
        ("opc_ua_client", "opcua_client"),
        ("mqtt_broker", "mqtt"),
        ("ADS-CLIENT", "ads"),
        ("TwinCAT", "ads"),
        ("trust", "discovery"),
        ("trust-runtime", "discovery"),
        ("trust_runtimes", "discovery"),
        ("mDNS", "discovery"),
    ] {
        assert_eq!(canonical_protocol(alias), expected, "alias {alias}");
    }
}

#[test]
fn canonical_protocol_preserves_unknown_normalized_identity() {
    assert_eq!(canonical_protocol(" Custom-Protocol "), "custom_protocol");
    assert_eq!(canonical_protocol("UNKNOWN"), "unknown");
}

#[test]
fn known_deferred_protocols_return_stable_titled_warning() {
    for (protocol, title) in [
        ("simulated", "Simulated I/O"),
        ("loopback", "Loopback"),
        ("openot", "OpenOT"),
        ("mesh", "Mesh"),
        ("runtime_cloud", "Runtime cloud"),
        ("realtime_t0", "Realtime T0"),
        ("ads_server", "ADS server"),
    ] {
        let value = discover(protocol).expect("known deferred protocol");
        assert_eq!(value["protocol"], protocol);
        assert_eq!(value["candidates"], json!([]));
        assert_eq!(
            value["warnings"][0],
            format!("{title} discovery is not available yet.")
        );
    }
}

#[test]
fn known_without_discovery_registry_is_exact() {
    for protocol in [
        "ethercat",
        "gpio",
        "simulated",
        "loopback",
        "openot",
        "opcua",
        "mesh",
        "runtime_cloud",
        "realtime_t0",
        "ads_server",
    ] {
        assert!(
            known_protocol_without_discovery(protocol),
            "missing known protocol {protocol}"
        );
    }
    for protocol in [
        "modbus_tcp",
        "mqtt",
        "opcua_client",
        "ads",
        "discovery",
        "unknown",
    ] {
        assert!(
            !known_protocol_without_discovery(protocol),
            "active/unknown protocol classified as deferred: {protocol}"
        );
    }
}

#[test]
fn protocol_titles_are_stable_for_every_deferred_identity() {
    for (protocol, title) in [
        ("ads_server", "ADS server"),
        ("ethercat", "EtherCAT"),
        ("gpio", "GPIO"),
        ("loopback", "Loopback"),
        ("mesh", "Mesh"),
        ("openot", "OpenOT"),
        ("opcua", "OPC UA server"),
        ("opcua_client", "OPC UA client"),
        ("realtime_t0", "Realtime T0"),
        ("runtime_cloud", "Runtime cloud"),
        ("simulated", "Simulated I/O"),
    ] {
        assert_eq!(protocol_title(protocol), title);
    }
    assert_eq!(protocol_title("unknown"), "Selected protocol");
}

#[test]
fn malformed_timeout_types_are_request_errors() {
    for malformed in [
        json!(-1),
        json!(1.5),
        json!("150"),
        json!(true),
        json!([]),
        json!({}),
    ] {
        let error = discover_value(
            json!({
                "protocol": "openot",
                "scope": { "timeout_ms": malformed }
            }),
            None,
        )
        .expect_err("malformed timeout accepted");
        assert!(
            error.starts_with("invalid comm.discover payload:"),
            "{error}"
        );
    }
}

#[test]
fn runtime_only_hardware_discovery_rejects_this_host_origin_without_hardware_io() {
    for protocol in ["ethercat", "gpio"] {
        let value = discover_value(
            json!({
                "protocol": protocol,
                "origin": "this_host",
                "scope": { "adapter": "must-not-open" }
            }),
            None,
        )
        .expect("runtime-origin warning");
        assert_eq!(value["candidates"], json!([]));
        assert!(value["warnings"][0]
            .as_str()
            .expect("warning")
            .contains("runtime host"));
    }
}

#[test]
fn opcua_server_discovery_never_calls_client_discovery() {
    let value = discover_value(
        json!({
            "protocol": "opcua",
            "scope": { "host": "must-not-resolve.invalid:4840" },
            "origin": "this_host"
        }),
        None,
    )
    .expect("server setup warning");
    assert_eq!(value["candidates"], json!([]));
    assert!(value["warnings"][0]
        .as_str()
        .expect("warning")
        .contains("OPC UA server setup"));
}
