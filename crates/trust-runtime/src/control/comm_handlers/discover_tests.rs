use std::io::Read;
use std::net::TcpListener;
use std::thread;

use super::*;

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
