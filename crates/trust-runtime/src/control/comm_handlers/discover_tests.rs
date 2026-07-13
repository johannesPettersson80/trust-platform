use std::io::Read;
use std::net::TcpListener;
use std::thread;

use super::*;

#[cfg(feature = "ads-wire")]
#[test]
fn ads_discovery_rejects_host_with_port_before_udp_identify() {
    let error = discover_value(
        json!({
            "protocol": "ads",
            "scope": { "host": "127.0.0.1:851" },
            "origin": "this_host",
            "passive": true
        }),
        None,
    )
    .expect_err("ADS discovery must reject host:port input before wire I/O");

    let actionable = error.to_ascii_lowercase();
    assert!(
        actionable.contains("host or ip only") && actionable.contains("ads port"),
        "expected an actionable bare-host error that points to the separate ADS port field, got: {error}"
    );
}

#[cfg(feature = "ads-wire")]
#[test]
fn ads_manual_identity_bypasses_udp_and_preserves_logical_port() {
    let value = discover_value(
        json!({
            "protocol": "ads",
            "scope": {
                "host": "192.0.2.5",
                "target_ams_net_id": "5.23.91.12.1.1",
                "ams_port": 852,
                "timeout_ms": 25
            },
            "origin": "this_host",
            "passive": true
        }),
        None,
    )
    .expect("a complete manual ADS identity must not depend on UDP Identify");

    assert_eq!(
        value
            .pointer("/candidates/0/source")
            .and_then(Value::as_str),
        Some("manual")
    );
    assert_eq!(
        value
            .pointer("/candidates/0/confidence")
            .and_then(Value::as_str),
        Some("declared")
    );
    assert_eq!(
        value
            .pointer("/candidates/0/params/host")
            .and_then(Value::as_str),
        Some("192.0.2.5")
    );
    assert_eq!(
        value
            .pointer("/candidates/0/params/ams_net_id")
            .and_then(Value::as_str),
        Some("5.23.91.12.1.1")
    );
    assert_eq!(
        value
            .pointer("/candidates/0/params/ams_port")
            .and_then(Value::as_u64),
        Some(852)
    );
}

#[cfg(feature = "ads-wire")]
#[test]
fn ads_manual_identity_does_not_fabricate_a_responding_logical_port() {
    let value = discover_value(
        json!({
            "protocol": "ads",
            "scope": {
                "host": "192.0.2.5",
                "target_ams_net_id": "5.23.91.12.1.1"
            },
            "origin": "this_host",
            "passive": true
        }),
        None,
    )
    .expect("manual ADS identity should remain available without UDP Identify");

    assert!(value
        .pointer("/candidates/0/params/ams_port")
        .is_some_and(Value::is_null));
    assert_eq!(
        value
            .pointer("/candidates/0/params/responding_ads_ports")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(0)
    );
    assert_eq!(
        value
            .pointer("/candidates/0/params/ads_service_status")
            .and_then(Value::as_str),
        Some("declared")
    );
}

#[cfg(feature = "ads-wire")]
#[test]
fn ads_manual_identity_rejects_malformed_ams_net_ids_before_candidate_creation() {
    for malformed in ["1.2.3.4.5", "1.2.3.not-a-number.5.6", "1.2.3.256.5.6"] {
        let error = discover_value(
            json!({
                "protocol": "ads",
                "scope": {
                    "host": "192.0.2.5",
                    "target_ams_net_id": malformed,
                    "ams_port": 852
                },
                "origin": "this_host",
                "passive": true
            }),
            None,
        )
        .expect_err("malformed manual AMS Net IDs must fail before candidate creation");

        assert!(
            error.contains("AMS Net ID must contain six numbers 0-255"),
            "expected actionable AMS Net ID guidance for {malformed:?}, got: {error}"
        );
    }
}

#[cfg(feature = "ads-wire")]
#[test]
fn ads_manual_identity_accepts_six_decimal_bytes() {
    let value = discover_value(
        json!({
            "protocol": "ads",
            "scope": {
                "host": "192.0.2.5",
                "target_ams_net_id": "10.20.30.40.1.1",
                "ams_port": 852
            },
            "origin": "this_host",
            "passive": true
        }),
        None,
    )
    .expect("a six-byte decimal AMS Net ID must remain valid");

    assert_eq!(
        value
            .pointer("/candidates/0/params/ams_net_id")
            .and_then(Value::as_str),
        Some("10.20.30.40.1.1")
    );
}

#[test]
fn ads_discovery_rejects_out_of_range_logical_ports_during_deserialization() {
    for port in [0_u64, 65_536] {
        let error = discover_value(
            json!({
                "protocol": "ads",
                "scope": {
                    "host": "192.0.2.5",
                    "target_ams_net_id": "5.23.91.12.1.1",
                    "ams_port": port
                },
                "origin": "this_host",
                "passive": true
            }),
            None,
        )
        .expect_err("ADS logical ports must be in the range 1..=65535");

        assert!(
            error.contains("invalid comm.discover payload"),
            "port {port} should fail the typed discovery contract, got: {error}"
        );
    }
}

#[test]
fn modbus_discovery_reports_tcp_listener_as_port_reachable_only() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind test listener");
    let addr = listener.local_addr().expect("listener addr");
    let handle = thread::spawn(move || {
        let _ = listener.accept().map(|(mut stream, _)| {
            let mut buffer = [0u8; 1];
            let _ = stream.read(&mut buffer);
        });
    });

    let value = discover_value(
        json!({
            "protocol": "modbus_tcp",
            "scope": { "host": addr.to_string(), "timeout_ms": 250 },
            "origin": "this_host",
            "passive": true
        }),
        None,
    )
    .expect("discover value");
    let candidates = value
        .get("candidates")
        .and_then(Value::as_array)
        .expect("candidates");
    assert_eq!(candidates.len(), 1);
    let expected = addr.to_string();
    assert_eq!(
        candidates[0].get("confidence").and_then(Value::as_str),
        Some("port_reachable")
    );
    assert_ne!(
        candidates[0].get("confidence").and_then(Value::as_str),
        Some("confirmed")
    );
    assert_eq!(
        candidates[0]
            .get("params")
            .and_then(|params| params.get("address"))
            .and_then(Value::as_str),
        Some(expected.as_str())
    );
    assert_eq!(
        candidates[0].get("source").and_then(Value::as_str),
        Some("tcp_connect")
    );
    handle.join().expect("join listener");
}

#[test]
fn modbus_discovery_rejects_large_cidr() {
    let error = discover_value(
        json!({
            "protocol": "modbus_tcp",
            "scope": { "cidr": "10.0.0.0/16" },
            "origin": "this_host"
        }),
        None,
    )
    .expect_err("large scan must be rejected");
    assert!(error.contains("/24 or tighter"), "{error}");
}

#[test]
fn known_unimplemented_protocol_returns_warning_not_error() {
    let value = discover_value(
        json!({
            "protocol": "openot",
            "scope": {},
            "origin": "runtime"
        }),
        None,
    )
    .expect("known deferred protocol must not hard-error");
    assert_eq!(
        value
            .pointer("/candidates")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(0)
    );
    let warning = value
        .pointer("/warnings/0")
        .and_then(Value::as_str)
        .expect("warning");
    assert!(warning.contains("OpenOT discovery is not available yet"));
}

#[test]
fn ethercat_discovery_requires_runtime_origin() {
    let value = discover_value(
        json!({
            "protocol": "ethercat",
            "scope": { "adapter": "eth0" },
            "origin": "this_host"
        }),
        None,
    )
    .expect("runtime-only scan reports warning");
    assert_eq!(
        value
            .pointer("/candidates")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(0)
    );
    let warning = value
        .pointer("/warnings/0")
        .and_then(Value::as_str)
        .expect("warning");
    assert!(warning.contains("runtime host"));
}

#[test]
fn opcua_discovery_is_server_only_warning() {
    let value = discover_value(
        json!({
            "protocol": "opcua",
            "scope": { "host": "127.0.0.1:4840" },
            "origin": "this_host"
        }),
        None,
    )
    .expect("opcua server-only warning");
    assert_eq!(
        value
            .pointer("/candidates")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(0)
    );
    let warning = value
        .pointer("/warnings/0")
        .and_then(Value::as_str)
        .expect("warning");
    assert!(warning.contains("OPC UA server setup"));
}

#[test]
fn unknown_protocol_still_errors() {
    let error = discover_value(
        json!({
            "protocol": "made_up_protocol",
            "scope": {},
            "origin": "this_host"
        }),
        None,
    )
    .expect_err("unknown protocol should fail");
    assert!(
        error.contains("does not know protocol 'made_up_protocol'"),
        "{error}"
    );
}

#[test]
fn targeted_mqtt_discovery_reports_tcp_listener_as_port_reachable_only() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind test listener");
    let addr = listener.local_addr().expect("listener addr");
    let handle = thread::spawn(move || {
        let _ = listener.accept().map(|(mut stream, _)| {
            let mut buffer = [0u8; 1];
            let _ = stream.read(&mut buffer);
        });
    });

    let value = discover_value(
        json!({
            "protocol": "mqtt",
            "scope": { "host": addr.to_string(), "timeout_ms": 250 },
            "origin": "this_host",
            "passive": true
        }),
        None,
    )
    .expect("discover value");
    let expected_broker = addr.to_string();
    assert_eq!(
        value
            .pointer("/candidates/0/params/broker")
            .and_then(Value::as_str),
        Some(expected_broker.as_str())
    );
    assert_eq!(
        value
            .pointer("/candidates/0/confidence")
            .and_then(Value::as_str),
        Some("port_reachable")
    );
    assert_ne!(
        value
            .pointer("/candidates/0/confidence")
            .and_then(Value::as_str),
        Some("confirmed")
    );
    assert_eq!(
        value
            .pointer("/candidates/0/source")
            .and_then(Value::as_str),
        Some("tcp_connect")
    );
    handle.join().expect("join listener");
}

#[test]
fn runtime_ethercat_discovery_reports_mock_bus_modules() {
    let value = discover_value(
        json!({
            "protocol": "ethercat",
            "scope": { "adapter": "mock" },
            "origin": "runtime",
            "passive": true
        }),
        None,
    )
    .expect("ethercat mock discovery");
    let candidates = value
        .pointer("/candidates")
        .and_then(Value::as_array)
        .expect("candidates");
    assert!(
        candidates.iter().any(|candidate| candidate
            .pointer("/params/model")
            .and_then(Value::as_str)
            == Some("EL1008")),
        "mock EtherCAT bus should expose configured/discovered modules: {value}"
    );
    assert_eq!(
        candidates
            .iter()
            .find(|candidate| {
                candidate.pointer("/params/model").and_then(Value::as_str) == Some("EL1008")
            })
            .and_then(|candidate| candidate.get("source"))
            .and_then(Value::as_str),
        Some("ethercat_bus")
    );
}
