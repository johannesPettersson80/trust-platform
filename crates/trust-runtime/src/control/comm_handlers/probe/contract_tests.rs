use std::net::TcpListener;
use std::thread;

use serde_json::{json, Value as JsonValue};

use super::*;

fn request(protocol: &str, params: JsonValue) -> CommTestResponse {
    test_request(CommTestRequest {
        protocol: protocol.to_string(),
        params,
        credential_channel: None,
    })
}

fn request_with_channel(
    protocol: &str,
    params: JsonValue,
    credential_channel: Option<&str>,
) -> CommTestResponse {
    test_request(CommTestRequest {
        protocol: protocol.to_string(),
        params,
        credential_channel: credential_channel.map(str::to_string),
    })
}

fn field_error_for(response: &CommTestResponse, field: &str) -> bool {
    response
        .field_errors
        .iter()
        .any(|error| error.field == field)
}

fn opcua_target_error(params: &JsonValue) -> CommFieldError {
    match opcua_client_target(params) {
        Ok(_) => panic!("invalid OPC UA target accepted: {params}"),
        Err(error) => error,
    }
}

fn spawn_one_shot_listener() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind listener");
    let address = listener.local_addr().expect("listener address").to_string();
    thread::spawn(move || {
        let _ = listener.accept();
    });
    address
}

fn unused_loopback_address() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind listener");
    let address = listener.local_addr().expect("listener address").to_string();
    drop(listener);
    address
}

#[test]
fn missing_test_params_fail_at_control_envelope_boundary() {
    let response = handle_comm_test(41, None);
    assert!(!response.ok);
    assert!(response.result.is_none());
    assert_eq!(response.error.as_deref(), Some("missing params"));
}

#[test]
fn missing_or_non_string_protocol_fails_deserialization() {
    for params in [
        json!({}),
        json!({ "protocol": null, "params": {} }),
        json!({ "protocol": 7, "params": {} }),
        json!({ "protocol": [], "params": {} }),
    ] {
        let response = handle_comm_test(42, Some(params));
        assert!(!response.ok);
        assert!(response.result.is_none());
        assert!(response
            .error
            .as_deref()
            .is_some_and(|error| error.starts_with("invalid comm.test payload:")));
    }
}

#[test]
fn response_schema_version_is_stable_for_every_result_class() {
    for response in [
        request("simulated", json!({})),
        request("ethercat", json!({})),
        request("not_registered", json!({})),
        request("modbus_tcp", json!({})),
    ] {
        assert_eq!(response.schema_version, COMM_SCHEMA_VERSION);
    }
}

#[test]
fn supported_protocol_set_is_exact() {
    for protocol in [
        "modbus_tcp",
        "mqtt",
        "opcua_client",
        "simulated",
        "loopback",
    ] {
        let response = request(protocol, json!({}));
        assert!(response.supported, "{protocol} must support Test");
    }
    for protocol in [
        "ethercat",
        "gpio",
        "opcua",
        "openot",
        "discovery",
        "mesh",
        "realtime_t0",
        "runtime_cloud",
        "ads_server",
        "ads",
        "not_registered",
    ] {
        let response = request(protocol, json!({}));
        assert!(!response.supported, "{protocol} must not support Test");
    }
}

#[test]
fn simulated_and_loopback_return_local_driver_evidence_without_target_data() {
    for protocol in ["simulated", "loopback"] {
        let response = request(protocol, json!({ "address": "must-not-be-used:1" }));
        assert!(response.supported);
        assert!(response.ok);
        assert_eq!(response.evidence, Some(json!({ "kind": "local_driver" })));
        assert!(response.error.is_none());
        assert!(response.field_errors.is_empty());
        assert!(!response.detail.contains("must-not-be-used"));
    }
}

#[test]
fn hardware_only_drivers_return_stable_unsupported_result() {
    for protocol in ["ethercat", "gpio"] {
        let response = request(protocol, json!({}));
        assert!(!response.supported);
        assert!(!response.ok);
        assert!(response.detail.contains("runtime hardware health"));
        assert!(response.error.is_none());
        assert!(response.evidence.is_none());
        assert!(response.field_errors.is_empty());
    }
}

#[test]
fn unsupported_protocols_return_no_evidence_or_field_errors() {
    for protocol in [
        "opcua",
        "openot",
        "discovery",
        "mesh",
        "realtime_t0",
        "runtime_cloud",
        "ads_server",
        "ads",
        "unknown",
    ] {
        let response = request(protocol, json!({ "opaque_input": "must-not-echo" }));
        assert!(!response.ok);
        assert!(response.evidence.is_none());
        assert!(response.error.is_none());
        assert!(response.field_errors.is_empty());
        let serialized = serde_json::to_string(&response).expect("serialize response");
        assert!(!serialized.contains("must-not-echo"));
    }
}

#[test]
fn protocol_aliases_normalize_before_dispatch() {
    for alias in [" SIMULATED ", "simulated", "Simulated"] {
        let response = request(alias, json!({}));
        assert_eq!(response.protocol, "simulated");
        assert!(response.supported);
        assert!(response.ok);
    }
    let response = request(" MODBUS-TCP ", json!({}));
    assert_eq!(response.protocol, "modbus_tcp");
    assert!(response.supported);
}

#[test]
fn blank_protocol_is_a_field_error_not_an_unknown_capability() {
    for protocol in ["", " ", "\t\r\n"] {
        let response = request(protocol, json!({}));
        assert!(field_error_for(&response, "protocol"));
        assert!(!response.supported);
        assert!(!response.ok);
    }
}

#[test]
fn non_object_params_are_rejected_at_the_test_boundary() {
    for params in [
        JsonValue::Null,
        json!(false),
        json!(7),
        json!("target"),
        json!([]),
    ] {
        let response = request("modbus_tcp", params);
        assert!(field_error_for(&response, "params"));
        assert!(response.evidence.is_none());
    }
}

#[test]
fn nested_registered_secret_keys_are_detected_case_insensitively() {
    for key in [
        "password",
        "AUTH_TOKEN",
        "Token",
        " secret ",
        "client_secret",
    ] {
        let params = json!({
            "outer": [
                {
                    "inner": {
                        (key): "credential-sentinel"
                    }
                }
            ]
        });
        assert!(
            secret_values_present(&params),
            "secret key {key} was missed"
        );
    }
}

#[test]
fn empty_or_non_string_secret_values_do_not_create_credentials() {
    for value in [
        json!(""),
        json!(" \t "),
        JsonValue::Null,
        json!(false),
        json!(0),
        json!([]),
        json!({}),
    ] {
        assert!(!secret_values_present(&json!({ "password": value })));
    }
}

#[test]
fn nonempty_secret_requires_exact_trusted_same_host_channel() {
    for channel in [
        None,
        Some(""),
        Some("trusted"),
        Some("TRUSTED_SAME_HOST"),
        Some("trusted_same_host "),
        Some("untrusted_remote"),
    ] {
        let response = request_with_channel(
            "simulated",
            json!({ "password": "credential-sentinel" }),
            channel,
        );
        assert!(!response.ok, "channel {channel:?} accepted a credential");
        assert!(response.supported);
        assert!(field_error_for(&response, "password"));
        assert_eq!(response.detail, "Connection test was blocked.");
        assert!(response.evidence.is_none());
        assert!(response.error.is_none());
    }

    let trusted = request_with_channel(
        "simulated",
        json!({ "password": "credential-sentinel" }),
        Some("trusted_same_host"),
    );
    assert!(trusted.ok);
    assert!(trusted.field_errors.is_empty());
}

#[test]
fn blocked_secret_response_never_echoes_secret_or_params() {
    let response = request_with_channel(
        "mqtt",
        json!({
            "broker": "broker.internal:1883",
            "password": "credential-sentinel",
            "nested": { "token": "nested-sentinel" }
        }),
        None,
    );
    let serialized = serde_json::to_string(&response).expect("serialize blocked response");
    assert!(!serialized.contains("credential-sentinel"));
    assert!(!serialized.contains("nested-sentinel"));
    assert!(!serialized.contains("broker.internal"));
}

#[test]
fn modbus_target_requires_a_nonempty_string_address() {
    for params in [
        json!({}),
        json!({ "address": "" }),
        json!({ "address": " \t " }),
        json!({ "address": 502 }),
        json!({ "address": false }),
    ] {
        let error = modbus_target(&params).expect_err("invalid Modbus target accepted");
        assert_eq!(error.field, "address");
    }
}

#[test]
fn modbus_target_trims_and_defaults_port_502() {
    for (address, expected) in [
        ("plc.internal", "plc.internal:502"),
        ("  plc.internal  ", "plc.internal:502"),
        ("127.0.0.1", "127.0.0.1:502"),
    ] {
        assert_eq!(
            modbus_target(&json!({ "address": address })).expect("Modbus target"),
            expected
        );
    }
}

#[test]
fn modbus_target_preserves_explicit_valid_port() {
    for address in ["plc.internal:1502", "127.0.0.1:502", "[::1]:1502"] {
        assert_eq!(
            modbus_target(&json!({ "address": address })).expect("Modbus target"),
            address
        );
    }
}

#[test]
fn modbus_target_defaults_bracketed_ipv6_port() {
    assert_eq!(
        modbus_target(&json!({ "address": "[::1]" })).expect("IPv6 Modbus target"),
        "[::1]:502"
    );
}

#[test]
fn modbus_target_rejects_malformed_or_non_authority_inputs() {
    for address in [
        "http://plc.internal",
        "plc.internal:0",
        "plc.internal:notaport",
        "user@plc.internal:502",
        "plc.internal:502/path",
        "plc.internal:502?query",
        "plc.internal:502#fragment",
        "::1",
    ] {
        assert!(
            modbus_target(&json!({ "address": address })).is_err(),
            "invalid Modbus target accepted: {address}"
        );
    }
}

#[test]
fn mqtt_target_requires_nonempty_string_broker() {
    for params in [
        json!({}),
        json!({ "broker": "" }),
        json!({ "broker": " \t " }),
        json!({ "broker": 1883 }),
        json!({ "broker": false }),
    ] {
        let error = mqtt_target(&params).expect_err("invalid MQTT target accepted");
        assert_eq!(error.field, "broker");
    }
}

#[test]
fn mqtt_target_accepts_address_alias_only_when_broker_is_absent() {
    assert_eq!(
        mqtt_target(&json!({ "address": "mqtt.internal" })).expect("address alias"),
        "mqtt.internal:1883"
    );
    assert_eq!(
        mqtt_target(&json!({
            "broker": "primary.internal:1884",
            "address": "ignored.internal:1885"
        }))
        .expect("broker precedence"),
        "primary.internal:1884"
    );
}

#[test]
fn mqtt_target_uses_scheme_specific_default_ports() {
    for (broker, expected) in [
        ("mqtt://broker.internal", "broker.internal:1883"),
        ("tcp://broker.internal", "broker.internal:1883"),
        ("mqtts://broker.internal", "broker.internal:8883"),
        ("ssl://broker.internal", "broker.internal:8883"),
    ] {
        assert_eq!(
            mqtt_target(&json!({ "broker": broker })).expect("MQTT target"),
            expected
        );
    }
}

#[test]
fn mqtt_target_preserves_explicit_valid_ports() {
    for (broker, expected) in [
        ("broker.internal:1884", "broker.internal:1884"),
        ("mqtt://broker.internal:1884", "broker.internal:1884"),
        ("ssl://broker.internal:8884", "broker.internal:8884"),
        ("[::1]:1883", "[::1]:1883"),
    ] {
        assert_eq!(
            mqtt_target(&json!({ "broker": broker })).expect("MQTT target"),
            expected
        );
    }
}

#[test]
fn mqtt_target_rejects_unsupported_or_non_authority_inputs() {
    for broker in [
        "http://broker.internal",
        "ws://broker.internal",
        "broker.internal:0",
        "broker.internal:notaport",
        "user:pass@broker.internal:1883",
        "broker.internal:1883/path",
        "broker.internal:1883?query",
        "broker.internal:1883#fragment",
        "::1",
    ] {
        assert!(
            mqtt_target(&json!({ "broker": broker })).is_err(),
            "invalid MQTT target accepted: {broker}"
        );
    }
}

#[test]
fn timeout_defaults_only_when_field_is_absent() {
    assert_eq!(timeout_ms(&json!({}), "timeout_ms"), 500);
    assert_eq!(timeout_ms(&json!({ "other": 7 }), "timeout_ms"), 500);
}

#[test]
fn tcp_probe_clamps_effective_timeout_into_closed_bounds() {
    let target = unused_loopback_address();
    for (requested, expected) in [(0, 1), (1, 1), (5_000, 5_000), (50_000, 5_000)] {
        let response = request(
            "modbus_tcp",
            json!({ "address": target, "timeout_ms": requested }),
        );
        assert_eq!(
            response
                .evidence
                .as_ref()
                .and_then(|value| value["timeout_ms"].as_u64()),
            Some(expected)
        );
    }
}

#[test]
fn malformed_timeout_is_a_field_error_not_a_silent_default() {
    let target = unused_loopback_address();
    for malformed in [
        json!(-1),
        json!(1.5),
        json!("50"),
        json!(true),
        json!([]),
        json!({}),
    ] {
        let response = request(
            "modbus_tcp",
            json!({ "address": target, "timeout_ms": malformed }),
        );
        assert!(field_error_for(&response, "timeout_ms"));
        assert!(response.evidence.is_none());
    }
}

#[test]
fn successful_tcp_probe_reports_claim_bounded_evidence() {
    for protocol in ["modbus_tcp", "mqtt"] {
        let address = spawn_one_shot_listener();
        let params = if protocol == "modbus_tcp" {
            json!({ "address": address, "timeout_ms": 250 })
        } else {
            json!({ "broker": address, "timeout_ms": 250 })
        };
        let response = request(protocol, params);
        assert!(response.supported);
        assert!(response.ok, "{}: {}", protocol, response.detail);
        let evidence = response.evidence.expect("TCP success evidence");
        assert_eq!(evidence["target"], address);
        assert_eq!(evidence["resolved"], address);
        assert_eq!(evidence["timeout_ms"], 250);
        assert_eq!(evidence.as_object().expect("evidence object").len(), 3);
    }
}

#[test]
fn refused_tcp_probe_is_a_completed_negative_without_field_error() {
    for protocol in ["modbus_tcp", "mqtt"] {
        let address = unused_loopback_address();
        let params = if protocol == "modbus_tcp" {
            json!({ "address": address, "timeout_ms": 50 })
        } else {
            json!({ "broker": address, "timeout_ms": 50 })
        };
        let response = request(protocol, params);
        assert!(response.supported);
        assert!(!response.ok);
        assert!(response.error.is_none());
        assert!(response.field_errors.is_empty());
        assert!(response.evidence.is_some());
    }
}

#[test]
fn unresolvable_target_is_a_field_error_with_bounded_evidence() {
    for (protocol, params, target_field) in [
        (
            "modbus_tcp",
            json!({ "address": "invalid host name:502", "timeout_ms": 25 }),
            "address",
        ),
        (
            "mqtt",
            json!({ "broker": "invalid host name:1883", "timeout_ms": 25 }),
            "broker",
        ),
    ] {
        let response = request(protocol, params);
        assert!(response.supported);
        assert!(!response.ok);
        assert!(field_error_for(&response, target_field));
        assert_eq!(response.detail, "Address could not be resolved.");
        assert_eq!(
            response
                .evidence
                .as_ref()
                .and_then(|value| value["timeout_ms"].as_u64()),
            Some(25)
        );
    }
}

#[test]
fn protocol_specific_tcp_details_do_not_overclaim_handshake() {
    assert_eq!(
        probe_success_detail("modbus_tcp"),
        "Modbus device port is reachable."
    );
    assert_eq!(
        probe_success_detail("mqtt"),
        "MQTT broker port is reachable."
    );
    assert_eq!(
        probe_failure_prefix("modbus_tcp"),
        "Modbus device port is not reachable"
    );
    assert_eq!(
        probe_failure_prefix("mqtt"),
        "MQTT broker port is not reachable"
    );
}

#[test]
fn opcua_target_requires_endpoint_or_host_string() {
    for params in [
        json!({}),
        json!({ "endpoint_url": "" }),
        json!({ "endpoint_url": " \t " }),
        json!({ "endpoint_url": 4840 }),
        json!({ "host": false }),
    ] {
        let error = opcua_target_error(&params);
        assert_eq!(error.field, "endpoint_url");
    }
}

#[test]
fn opcua_target_normalizes_bare_host_port_and_path() {
    for (params, expected) in [
        (
            json!({ "endpoint_url": "server.internal" }),
            "opc.tcp://server.internal:4840",
        ),
        (
            json!({ "endpoint_url": "server.internal:4841" }),
            "opc.tcp://server.internal:4841",
        ),
        (
            json!({ "endpoint_url": "server.internal/trust" }),
            "opc.tcp://server.internal/trust",
        ),
        (
            json!({ "host": " server.internal " }),
            "opc.tcp://server.internal:4840",
        ),
    ] {
        let target = opcua_client_target(&params).expect("OPC UA target");
        assert_eq!(target.endpoint_url, expected);
    }
}

#[test]
fn opcua_target_preserves_canonical_url_after_trimming() {
    let target = opcua_client_target(&json!({
        "endpoint_url": "  opc.tcp://server.internal:4841/trust  "
    }))
    .expect("canonical OPC UA target");
    assert_eq!(target.endpoint_url, "opc.tcp://server.internal:4841/trust");
}

#[test]
fn opcua_security_defaults_to_none_and_accepts_normalized_aliases() {
    let default =
        opcua_client_target(&json!({ "host": "server.internal" })).expect("default OPC UA target");
    assert_eq!(default.security.policy.as_config_value(), "none");
    assert_eq!(default.security.mode.as_config_value(), "none");

    for (policy, mode, expected_policy, expected_mode) in [
        (
            " Basic256-Sha256 ",
            " Sign-And-Encrypt ",
            "basic256sha256",
            "sign_and_encrypt",
        ),
        (
            "AES128_SHA256_RSAOAEP",
            "SIGN",
            "aes128sha256rsaoaep",
            "sign",
        ),
    ] {
        let target = opcua_client_target(&json!({
            "host": "server.internal",
            "security_policy": policy,
            "security_mode": mode
        }))
        .expect("normalized OPC UA security");
        assert_eq!(target.security.policy.as_config_value(), expected_policy);
        assert_eq!(target.security.mode.as_config_value(), expected_mode);
    }
}

#[test]
fn opcua_rejects_unknown_security_policy_and_mode_separately() {
    let policy = opcua_target_error(&json!({
        "host": "server.internal",
        "security_policy": "unknown"
    }));
    assert_eq!(policy.field, "security_policy");

    let mode = opcua_target_error(&json!({
        "host": "server.internal",
        "security_mode": "unknown"
    }));
    assert_eq!(mode.field, "security_mode");
}

#[test]
fn opcua_anonymous_auth_is_default_and_disables_username_requirement() {
    let target = opcua_client_target(&json!({
        "host": "server.internal",
        "username": "",
        "password": ""
    }))
    .expect("anonymous OPC UA target");
    assert!(matches!(
        target.auth,
        crate::opcua::OpcUaClientAuthConfig::Anonymous
    ));
    assert!(target.security.allow_anonymous);
}

#[test]
fn opcua_username_auth_aliases_trim_required_credentials() {
    for auth in ["username", "user_name", "user", " USER "] {
        let target = opcua_client_target(&json!({
            "host": "server.internal",
            "auth": auth,
            "username": " operator ",
            "password": " credential "
        }))
        .expect("username OPC UA target");
        match target.auth {
            crate::opcua::OpcUaClientAuthConfig::UserName { username, password } => {
                assert_eq!(username.as_str(), "operator");
                assert_eq!(password.as_str(), "credential");
            }
            crate::opcua::OpcUaClientAuthConfig::Anonymous => {
                panic!("username alias {auth} selected anonymous auth")
            }
        }
        assert!(!target.security.allow_anonymous);
    }
}

#[test]
fn opcua_username_auth_reports_missing_fields_separately() {
    let missing_username = opcua_target_error(&json!({
        "host": "server.internal",
        "auth": "username",
        "password": "credential"
    }));
    assert_eq!(missing_username.field, "username");

    let missing_password = opcua_target_error(&json!({
        "host": "server.internal",
        "auth": "username",
        "username": "operator"
    }));
    assert_eq!(missing_password.field, "password");
}

#[test]
fn opcua_rejects_unknown_auth_mode() {
    let error = opcua_target_error(&json!({
        "host": "server.internal",
        "auth": "certificate"
    }));
    assert_eq!(error.field, "auth");
}

#[test]
fn opcua_trust_server_certificate_is_explicit_and_defaults_false() {
    let default =
        opcua_client_target(&json!({ "host": "server.internal" })).expect("default OPC UA target");
    assert!(!default.trust_server_certificate);

    let opted_in = opcua_client_target(&json!({
        "host": "server.internal",
        "trust_server_certificate": true
    }))
    .expect("trusted OPC UA target");
    assert!(opted_in.trust_server_certificate);
}

#[test]
fn probe_value_returns_structured_json_for_local_and_unsupported_protocols() {
    let local = probe_value(" LOOPBACK ", json!({}));
    assert_eq!(local["schema_version"], COMM_SCHEMA_VERSION);
    assert_eq!(local["protocol"], "loopback");
    assert_eq!(local["supported"], true);
    assert_eq!(local["ok"], true);

    let unsupported = probe_value("not-registered", json!({}));
    assert_eq!(unsupported["protocol"], "not_registered");
    assert_eq!(unsupported["supported"], false);
    assert_eq!(unsupported["ok"], false);
    assert!(unsupported.get("evidence").is_none());
}
