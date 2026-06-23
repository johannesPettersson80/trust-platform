#[test]
fn breakpoints_set_accepts_project_relative_source_path() {
    let source = r#"
PROGRAM Main
VAR
    run : BOOL := TRUE;
END_VAR
run := NOT run;
END_PROGRAM
"#;
    let mut state = hmi_test_state(source);
    let project_root = std::env::temp_dir().join("trust-breakpoints-project-relative");
    state.project_root = Some(project_root.clone());
    state.sources = SourceRegistry::new(vec![SourceFile {
        id: 1,
        path: project_root.join("src").join("main.st"),
        text: source.to_string(),
    }]);

    let response = handle_request_value(
        json!({
            "id": 101,
            "type": "breakpoints.set",
            "params": { "source": "src/main.st", "lines": [6] }
        }),
        &state,
        None,
    );

    assert!(
        response.ok,
        "project-relative breakpoint source should bind: {:?}",
        response.error
    );
    let result = response.result.expect("breakpoint result");
    assert_eq!(result.get("file_id").and_then(serde_json::Value::as_u64), Some(1));
}

#[test]
fn request_routing_contract_dispatches_core_handler_modules() {
    let source = r#"
PROGRAM Main
VAR
    run : BOOL := TRUE;
END_VAR
END_PROGRAM
"#;
    let state = hmi_test_state(source);
    let requests = vec![
        json!({"id": 1, "type": "status"}),
        json!({"id": 2, "type": "io.list"}),
        json!({"id": 3, "type": "debug.state"}),
        json!({"id": 4, "type": "var.forced"}),
        json!({"id": 5, "type": "restart", "params": { "mode": "warm" }}),
        json!({"id": 6, "type": "ads.status"}),
        json!({"id": 7, "type": "ads.route_plan", "params": ads_route_plan_params("trusted_same_host")}),
        json!({"id": 8, "type": "ads.identity", "params": { "target_ip": "127.0.0.1" }}),
        json!({"id": 9, "type": "ads.discover", "params": ads_discover_manual_params()}),
        json!({"id": 10, "type": "ads.route_add", "params": ads_route_add_params("untrusted_remote_plain_tcp")}),
        json!({"id": 11, "type": "ads.route_remove", "params": ads_route_remove_params()}),
        json!({"id": 12, "type": "ads.doctor", "params": ads_doctor_params(false)}),
        json!({"id": 13, "type": "ads.doctor.start", "params": ads_doctor_params(false)}),
        json!({"id": 14, "type": "ads.doctor.status", "params": { "job_id": "missing" }}),
        json!({"id": 15, "type": "ads.import_symbols", "params": ads_import_symbols_params()}),
        json!({"id": 16, "type": "ads.server.status"}),
        json!({"id": 17, "type": "ads.server.symbols"}),
        json!({"id": 18, "type": "ads.server.route_plan", "params": ads_route_plan_params("trusted_same_host")}),
        json!({"id": 19, "type": "ads.server.doctor", "params": {}}),
        json!({"id": 20, "type": "ads.server.doctor.start", "params": {}}),
        json!({"id": 21, "type": "ads.server.doctor.status", "params": { "job_id": "missing" }}),
        json!({"id": 22, "type": "comm.capabilities"}),
        json!({"id": 23, "type": "comm.schema"}),
        json!({"id": 24, "type": "comm.discover", "params": { "protocol": "modbus_tcp", "scope": { "cidr": "127.0.0.1/32", "timeout_ms": 1 }, "origin": "this_host", "passive": true }}),
        json!({"id": 25, "type": "comm.browse_symbols", "params": { "protocol": "ads", "snapshot": ads_symbol_snapshot_value() }}),
        json!({"id": 26, "type": "comm.apply", "params": { "protocol": "modbus_tcp", "dry_run": true, "params": { "address": "127.0.0.1:502" } }}),
        json!({"id": 27, "type": "comm.test", "params": { "protocol": "simulated", "params": {} }}),
        json!({"id": 28, "type": "fleet.topology"}),
    ];

    for request in requests {
        let response = handle_request_value(request.clone(), &state, None);
        assert_ne!(
            response.error.as_deref(),
            Some("unsupported request"),
            "request should be routed by module split: {request}"
        );
    }
}

#[test]
fn comm_test_reports_structured_results_without_external_network() {
    let source = r#"
PROGRAM Main
VAR
    run : BOOL := TRUE;
END_VAR
END_PROGRAM
"#;
    let mut state = hmi_test_state(source);

    let simulated = handle_request_value(
        json!({"id": 51, "type": "comm.test", "params": { "protocol": "simulated", "params": {} }}),
        &state,
        None,
    );
    assert!(simulated.ok, "simulated comm.test failed: {:?}", simulated.error);
    let result = simulated.result.expect("simulated result");
    assert_eq!(result.get("supported").and_then(serde_json::Value::as_bool), Some(true));
    assert_eq!(result.get("ok").and_then(serde_json::Value::as_bool), Some(true));

    let missing_address = handle_request_value(
        json!({"id": 52, "type": "comm.test", "params": { "protocol": "modbus_tcp", "params": {} }}),
        &state,
        None,
    );
    assert!(missing_address.ok, "field-error result should be structured");
    let result = missing_address.result.expect("missing-address result");
    assert_eq!(result.get("ok").and_then(serde_json::Value::as_bool), Some(false));
    assert!(result
        .get("field_errors")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|errors| errors.iter().any(|error| {
            error.get("field").and_then(serde_json::Value::as_str) == Some("address")
        })));

    let blocked_secret = handle_request_value(
        json!({
            "id": 53,
            "type": "comm.test",
            "auth": engineer_token("comm-test-secret", &mut state),
            "params": {
                "protocol": "mqtt",
                "credential_channel": "trusted_same_host",
                "params": {
                    "broker": "127.0.0.1:1883",
                    "username": "user",
                    "password": "do-not-log"
                }
            }
        }),
        &state,
        Some("10.0.0.20:50200"),
    );
    assert!(blocked_secret.ok, "secret block result should be structured");
    let result = blocked_secret.result.expect("blocked secret result");
    assert_eq!(result.get("ok").and_then(serde_json::Value::as_bool), Some(false));
    assert!(
        !serde_json::to_string(&result)
            .expect("result json")
            .contains("do-not-log")
    );

    for protocol in [
        "opcua",
        "openot",
        "mesh",
        "realtime_t0",
        "runtime_cloud",
        "ethercat",
        "gpio",
    ] {
        let response = handle_request_value(
            json!({"id": 54, "type": "comm.test", "params": { "protocol": protocol, "params": {} }}),
            &state,
            None,
        );
        assert!(response.ok, "unsupported protocol result should be structured: {protocol}");
        let result = response.result.expect("unsupported protocol result");
        assert_eq!(
            result.get("supported").and_then(serde_json::Value::as_bool),
            Some(false),
            "{protocol} should not advertise a dead Test button"
        );
    }
}

#[test]
fn comm_schema_reports_io_driver_fields_without_secret_defaults() {
    let source = r#"
PROGRAM Main
VAR
    run : BOOL := TRUE;
END_VAR
END_PROGRAM
"#;
    let mut state = hmi_test_state(source);
    let root = temp_dir("comm-schema");
    set_hmi_project_root(&mut state, &root);

    let response = handle_request_value(
        json!({"id": 44, "type": "comm.schema", "params": { "protocol": "mqtt" }}),
        &state,
        None,
    );

    assert!(response.ok, "comm.schema failed: {:?}", response.error);
    let result = response.result.expect("schema result");
    assert_eq!(result.pointer("/schema_version").and_then(serde_json::Value::as_u64), Some(4));
    let protocols = result
        .get("protocols")
        .and_then(serde_json::Value::as_array)
        .expect("protocols");
    assert_eq!(protocols.len(), 1);
    assert_eq!(protocols[0].get("id").and_then(serde_json::Value::as_str), Some("mqtt"));
    let fields = protocols[0]
        .get("fields")
        .and_then(serde_json::Value::as_array)
        .expect("fields");
    let password = fields
        .iter()
        .find(|field| field.get("id").and_then(serde_json::Value::as_str) == Some("password"))
        .expect("password field");
    assert_eq!(password.get("secret").and_then(serde_json::Value::as_bool), Some(true));
    assert!(password.get("default").is_some_and(serde_json::Value::is_null));
    assert!(
        !serde_json::to_string(&result)
            .expect("schema json")
            .contains("password\":\""),
        "schema must not contain a password value"
    );

    let simulated = handle_request_value(
        json!({"id": 441, "type": "comm.schema", "params": { "protocol": "simulated" }}),
        &state,
        None,
    );
    assert!(simulated.ok, "comm.schema simulated failed: {:?}", simulated.error);
    let result = simulated.result.expect("simulated schema result");
    let fields = result
        .pointer("/protocols/0/fields")
        .and_then(serde_json::Value::as_array)
        .expect("simulated fields");
    for field_id in ["input_count", "output_count", "scan_period_ms", "mode"] {
        assert!(
            fields
                .iter()
                .any(|field| field.get("id").and_then(serde_json::Value::as_str) == Some(field_id)),
            "missing simulated field {field_id}"
        );
    }
}

#[test]
fn comm_schema_reports_non_io_file_protocols() {
    let source = r#"
PROGRAM Main
VAR
    run : BOOL := TRUE;
END_VAR
END_PROGRAM
"#;
    let state = hmi_test_state(source);

    let response = handle_request_value(
        json!({"id": 144, "type": "comm.schema", "params": { "protocol": "opcua" }}),
        &state,
        None,
    );

    assert!(response.ok, "comm.schema opcua failed: {:?}", response.error);
    let result = response.result.expect("schema result");
    let protocols = result
        .get("protocols")
        .and_then(serde_json::Value::as_array)
        .expect("protocols");
    assert_eq!(protocols.len(), 1);
    let opcua = &protocols[0];
    assert_eq!(opcua.get("id").and_then(serde_json::Value::as_str), Some("opcua"));
    assert_eq!(
        opcua.get("apply_mode").and_then(serde_json::Value::as_str),
        Some("file")
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
        opcua
            .get("supports_multi_instance")
            .and_then(serde_json::Value::as_bool),
        Some(false)
    );
    let fields = opcua
        .get("fields")
        .and_then(serde_json::Value::as_array)
        .expect("fields");
    assert!(fields
        .iter()
        .any(|field| field.get("id").and_then(serde_json::Value::as_str) == Some("expose")));
}

#[test]
fn comm_apply_writes_runtime_toml_without_returning_secret_values() {
    let source = r#"
PROGRAM Main
VAR
    run : BOOL := TRUE;
END_VAR
END_PROGRAM
"#;
    let mut state = hmi_test_state(source);
    let root = temp_dir("comm-apply-runtime-file");
    set_hmi_project_root(&mut state, &root);
    write_file(
        &root.join("runtime.toml"),
        &crate::bundle_template::render_runtime_toml(&SmolStr::new("comm-runtime"), 10),
    );

    let response = handle_request_value(
        json!({
            "id": 145,
            "type": "comm.apply",
            "params": {
                "protocol": "opcua",
                "credential_channel": "trusted_same_host",
                "params": {
                    "enabled": true,
                    "listen": "127.0.0.1:4840",
                    "endpoint_path": "/",
                    "namespace_uri": "urn:trust:runtime",
                    "publish_interval_ms": 250,
                    "max_nodes": 128,
                    "expose": ["global.*"],
                    "security_policy": "basic256sha256",
                    "security_mode": "sign_and_encrypt",
                    "allow_anonymous": false,
                    "username": "operator",
                    "password": "must-not-return"
                }
            }
        }),
        &state,
        None,
    );

    assert!(response.ok, "runtime file apply failed: {:?}", response.error);
    let result = response.result.expect("runtime file result");
    assert_eq!(result.get("applied").and_then(serde_json::Value::as_bool), Some(true));
    assert_eq!(
        result.get("lifecycle_effect").and_then(serde_json::Value::as_str),
        Some("restart_required")
    );
    assert!(result.get("snippet").is_none_or(serde_json::Value::is_null));
    let runtime_text = fs::read_to_string(root.join("runtime.toml")).expect("read runtime.toml");
    assert!(runtime_text.contains("[runtime.opcua]"));
    assert!(runtime_text.contains("username = \"operator\""));
    assert!(runtime_text.contains("password = \"must-not-return\""));
    crate::config::validate_runtime_toml_text(&runtime_text)
        .expect("written runtime.toml should validate");
    assert!(
        !serde_json::to_string(&result)
            .expect("result json")
            .contains("must-not-return")
    );
}

#[test]
fn comm_apply_blocks_non_io_secret_fields_on_untrusted_channel() {
    let source = r#"
PROGRAM Main
VAR
    run : BOOL := TRUE;
END_VAR
END_PROGRAM
"#;
    let mut state = hmi_test_state(source);

    let response = handle_request_value(
        json!({
            "id": 146,
            "type": "comm.apply",
            "auth": engineer_token("comm-apply-runtime-secret", &mut state),
            "params": {
                "protocol": "mesh",
                "credential_channel": "trusted_same_host",
                "params": {
                    "enabled": true,
                    "role": "peer",
                    "listen": "0.0.0.0:5200",
                    "connect": [],
                    "tls": false,
                    "auth_token": "mesh-secret",
                    "publish": [],
                    "subscribe": {},
                    "zenohd_version": "1.7.2",
                    "plugin_versions": {}
                }
            }
        }),
        &state,
        Some("10.0.0.20:50200"),
    );

    assert!(response.ok, "blocked runtime file result should be structured");
    let result = response.result.expect("blocked result");
    assert_eq!(
        result.get("lifecycle_effect").and_then(serde_json::Value::as_str),
        Some("blocked")
    );
    let text = serde_json::to_string(&result).expect("result json");
    assert!(text.contains("Secret fields cannot be sent"));
    assert!(!text.contains("mesh-secret"));
}

#[test]
fn comm_apply_returns_field_errors_and_dry_run_does_not_write() {
    let source = r#"
PROGRAM Main
VAR
    run : BOOL := TRUE;
END_VAR
END_PROGRAM
"#;
    let mut state = hmi_test_state(source);
    let root = temp_dir("comm-apply-dry-run");
    set_hmi_project_root(&mut state, &root);
    write_file(
        &root.join("runtime.toml"),
        &crate::bundle_template::render_runtime_toml(&SmolStr::new("comm-dry-run"), 10),
    );

    let invalid = handle_request_value(
        json!({
            "id": 45,
            "type": "comm.apply",
            "params": {
                "protocol": "modbus_tcp",
                "dry_run": true,
                "params": { "address": "not-an-endpoint", "unit_id": 300 }
            }
        }),
        &state,
        None,
    );
    assert!(invalid.ok, "validation response should be structured");
    let result = invalid.result.expect("invalid apply result");
    assert_eq!(result.get("applied").and_then(serde_json::Value::as_bool), Some(false));
    assert_eq!(
        result.get("lifecycle_effect").and_then(serde_json::Value::as_str),
        Some("blocked")
    );
    let fields = result
        .get("field_errors")
        .and_then(serde_json::Value::as_array)
        .expect("field errors");
    assert!(fields
        .iter()
        .any(|error| error.get("field").and_then(serde_json::Value::as_str) == Some("address")));
    assert!(fields
        .iter()
        .any(|error| error.get("field").and_then(serde_json::Value::as_str) == Some("unit_id")));
    assert!(!root.join("io.toml").exists(), "invalid dry-run must not write");

    let valid = handle_request_value(
        json!({
            "id": 46,
            "type": "comm.apply",
            "params": {
                "protocol": "modbus_tcp",
                "dry_run": true,
                "params": {
                    "address": "127.0.0.1:502",
                    "unit_id": 1,
                    "input_start": 0,
                    "output_start": 10,
                    "timeout_ms": 500,
                    "on_error": "fault"
                }
            }
        }),
        &state,
        None,
    );
    assert!(valid.ok, "valid dry-run failed: {:?}", valid.error);
    let result = valid.result.expect("valid apply result");
    assert_eq!(
        result.get("lifecycle_effect").and_then(serde_json::Value::as_str),
        Some("validate_only")
    );
    assert!(result.get("snippet").is_none_or(serde_json::Value::is_null));
    assert!(!root.join("io.toml").exists(), "dry-run must not write io.toml");

    let modbus_hostname = handle_request_value(
        json!({
            "id": 47,
            "type": "comm.apply",
            "params": {
                "protocol": "modbus_tcp",
                "dry_run": true,
                "params": {
                    "address": "plc.local:502",
                    "unit_id": 1,
                    "input_start": 0,
                    "output_start": 10,
                    "timeout_ms": 500,
                    "on_error": "fault"
                }
            }
        }),
        &state,
        None,
    );
    assert!(modbus_hostname.ok, "hostname rejection should be structured");
    let result = modbus_hostname.result.expect("modbus hostname result");
    assert!(result
        .get("field_errors")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|errors| errors.iter().any(|error| {
            error.get("field").and_then(serde_json::Value::as_str) == Some("address")
        })));

    let mqtt_hostname = handle_request_value(
        json!({
            "id": 48,
            "type": "comm.apply",
            "params": {
                "protocol": "mqtt",
                "dry_run": true,
                "params": {
                    "broker": "broker.local:1883",
                    "topic_in": "line/in",
                    "topic_out": "line/out",
                    "reconnect_ms": 500,
                    "keep_alive_s": 5,
                    "allow_insecure_remote": true
                }
            }
        }),
        &state,
        None,
    );
    assert!(mqtt_hostname.ok, "mqtt hostname dry-run failed: {:?}", mqtt_hostname.error);
    let result = mqtt_hostname.result.expect("mqtt hostname result");
    assert_eq!(
        result.get("lifecycle_effect").and_then(serde_json::Value::as_str),
        Some("validate_only")
    );

    let mqtt_bad_broker_and_array = handle_request_value(
        json!({
            "id": 480,
            "type": "comm.apply",
            "params": {
                "protocol": "mqtt",
                "dry_run": true,
                "params": {
                    "broker": "bad broker:1883",
                    "topic_in": "line/in",
                    "topic_out": "line/out",
                    "reconnect_ms": 500,
                    "keep_alive_s": 5,
                    "tls_alpn": "not-an-array",
                    "allow_insecure_remote": true
                }
            }
        }),
        &state,
        None,
    );
    assert!(
        mqtt_bad_broker_and_array.ok,
        "mqtt bad broker result should be structured"
    );
    let result = mqtt_bad_broker_and_array
        .result
        .expect("mqtt bad broker result");
    let fields = result
        .get("field_errors")
        .and_then(serde_json::Value::as_array)
        .expect("field errors");
    assert!(fields
        .iter()
        .any(|error| error.get("field").and_then(serde_json::Value::as_str) == Some("broker")));
    assert!(fields
        .iter()
        .any(|error| error.get("field").and_then(serde_json::Value::as_str) == Some("tls_alpn")));

    let opcua_bad_array = handle_request_value(
        json!({
            "id": 481,
            "type": "comm.apply",
            "params": {
                "protocol": "opcua",
                "dry_run": true,
                "params": {
                    "enabled": true,
                    "listen": "127.0.0.1:4840",
                    "endpoint_path": "/",
                    "namespace_uri": "urn:trust:runtime",
                    "publish_interval_ms": 250,
                    "max_nodes": 128,
                    "expose": "global.*",
                    "security_policy": "basic256sha256",
                    "security_mode": "sign_and_encrypt",
                    "allow_anonymous": true
                }
            }
        }),
        &state,
        None,
    );
    assert!(
        opcua_bad_array.ok,
        "opcua bad array result should be structured"
    );
    let result = opcua_bad_array.result.expect("opcua bad array result");
    assert!(result
        .get("field_errors")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|errors| errors.iter().any(|error| {
            error.get("field").and_then(serde_json::Value::as_str) == Some("expose")
        })));

    let simulated_bad_bounds = handle_request_value(
        json!({
            "id": 482,
            "type": "comm.apply",
            "params": {
                "protocol": "simulated",
                "dry_run": true,
                "params": {
                    "input_count": 4097,
                    "output_count": 8,
                    "scan_period_ms": 0,
                    "mode": "invalid"
                }
            }
        }),
        &state,
        None,
    );
    assert!(
        simulated_bad_bounds.ok,
        "simulated bounds result should be structured"
    );
    let result = simulated_bad_bounds
        .result
        .expect("simulated bounds result");
    let fields = result
        .get("field_errors")
        .and_then(serde_json::Value::as_array)
        .expect("simulated field errors");
    for field_id in ["input_count", "scan_period_ms", "mode"] {
        assert!(
            fields
                .iter()
                .any(|error| error.get("field").and_then(serde_json::Value::as_str) == Some(field_id)),
            "missing simulated validation error for {field_id}"
        );
    }
}

#[test]
fn comm_apply_writes_io_toml_and_preserves_unrelated_instances() {
    let source = r#"
PROGRAM Main
VAR
    run : BOOL := TRUE;
END_VAR
END_PROGRAM
"#;
    let mut state = hmi_test_state(source);
    let root = temp_dir("comm-apply-write");
    set_hmi_project_root(&mut state, &root);
	    write_file(
	        &root.join("io.toml"),
	        r#"
# existing plant I/O note must survive comm.apply edits
[io]
safe_state = [{ address = "%QX0.0", value = "FALSE" }]

[[io.drivers]]
name = "loopback"
params = {}
"#,
    );

    let response = handle_request_value(
        json!({
            "id": 47,
            "type": "comm.apply",
            "params": {
                "protocol": "modbus_tcp",
                "action": "add",
                "params": {
                    "address": "127.0.0.1:1502",
                    "unit_id": 2,
                    "input_start": 5,
                    "output_start": 7,
                    "timeout_ms": 750,
                    "on_error": "warn"
                }
            }
        }),
        &state,
        None,
    );

    assert!(response.ok, "comm.apply failed: {:?}", response.error);
    let result = response.result.expect("apply result");
    assert_eq!(result.get("applied").and_then(serde_json::Value::as_bool), Some(true));
    assert_eq!(
        result.get("lifecycle_effect").and_then(serde_json::Value::as_str),
        Some("restart_required")
    );
	    let text = fs::read_to_string(root.join("io.toml")).expect("read io.toml");
	    assert!(
	        text.contains("existing plant I/O note"),
	        "toml_edit writer should preserve surrounding comments: {text}"
	    );
	    assert!(text.contains("loopback"), "unrelated loopback instance should remain: {text}");
    assert!(text.contains("modbus-tcp"), "modbus instance should be written: {text}");
    assert!(text.contains("127.0.0.1:1502"));
    crate::config::validate_io_toml_text(&text).expect("written io.toml should validate");
}

#[test]
fn comm_apply_add_accepts_safe_state_only_io_toml() {
    let source = r#"
PROGRAM Main
VAR
    run : BOOL := TRUE;
END_VAR
END_PROGRAM
"#;
    let mut state = hmi_test_state(source);
    let root = temp_dir("comm-apply-safe-state-only");
    set_hmi_project_root(&mut state, &root);
    write_file(
        &root.join("io.toml"),
        r#"
# empty project I/O file created by setup
[io]
safe_state = [{ address = "%QX0.0", value = "FALSE" }]
"#,
    );

    let response = handle_request_value(
        json!({
            "id": 147,
            "type": "comm.apply",
            "params": {
                "protocol": "modbus_tcp",
                "action": "add",
                "params": {
                    "address": "127.0.0.1:1502",
                    "unit_id": 1,
                    "input_start": 0,
                    "output_start": 0,
                    "timeout_ms": 500,
                    "on_error": "warn"
                }
            }
        }),
        &state,
        None,
    );

    assert!(
        response.ok,
        "safe-state-only comm.apply failed: {:?}",
        response.error
    );
    let result = response.result.expect("safe-state-only result");
    assert_eq!(
        result.get("applied").and_then(serde_json::Value::as_bool),
        Some(true),
        "safe-state-only apply result: {result:#}"
    );
    let text = fs::read_to_string(root.join("io.toml")).expect("read io.toml");
    assert!(text.contains("empty project I/O file"));
    assert!(text.contains("modbus-tcp"));
    crate::config::validate_io_toml_text(&text).expect("safe-state-only io.toml should validate");
}

#[test]
fn comm_apply_ethercat_migrates_single_driver_bootstrap_to_multi_driver_topology() {
    let source = r#"
PROGRAM Main
VAR
    run : BOOL := TRUE;
END_VAR
END_PROGRAM
"#;
    let mut state = hmi_test_state(source);
    let root = temp_dir("comm-apply-ethercat-bootstrap");
    set_hmi_project_root(&mut state, &root);
    write_file(
        &root.join("runtime.toml"),
        &crate::bundle_template::render_runtime_toml(&SmolStr::new("line-a"), 10),
    );
    write_file(
        &root.join("io.toml"),
        r#"
# easy-setup projects start with a single placeholder driver
[io]
driver = "simulated"
params = {}
"#,
    );

    let response = handle_request_value(
        json!({
            "id": 148,
            "type": "comm.apply",
            "params": {
                "protocol": "ethercat",
                "action": "upsert",
                "params": {
                    "adapter": "eth1",
                    "timeout_ms": 250,
                    "cycle_warn_ms": 5,
                    "on_error": "fault",
                    "modules": [
                        { "model": "EK1100", "slot": 0, "channels": 1 },
                        { "model": "EL1008", "slot": 1, "channels": 8 }
                    ],
                    "mock_inputs": []
                }
            }
        }),
        &state,
        None,
    );

    assert!(
        response.ok,
        "ethercat bootstrap apply failed: {:?}",
        response.error
    );
    let result = response.result.expect("ethercat bootstrap result");
    assert_eq!(
        result.get("applied").and_then(serde_json::Value::as_bool),
        Some(true),
        "ethercat bootstrap apply result: {result:#}"
    );
    let text = fs::read_to_string(root.join("io.toml")).expect("read io.toml");
    assert!(
        !text.contains("\ndriver = \"simulated\""),
        "legacy single-driver key must be removed when writing canonical multi-driver TOML: {text}"
    );
    assert!(text.contains("[[io.drivers]]"), "{text}");
    assert!(text.contains("name = \"simulated\""), "{text}");
    assert!(text.contains("name = \"ethercat\""), "{text}");
    crate::config::validate_io_toml_text(&text).expect("ethercat bootstrap io.toml validates");

    let topology =
        crate::control::offline_fleet_topology_json(&root).expect("offline fleet topology");
    let endpoints = topology
        .pointer("/hosts/0/runtimes/0/endpoints")
        .and_then(serde_json::Value::as_array)
        .or_else(|| {
            topology
                .pointer("/hosts/0/containers/0/runtimes/0/endpoints")
                .and_then(serde_json::Value::as_array)
        })
        .expect("offline topology endpoints");
    let ethercat = endpoints
        .iter()
        .find(|endpoint| {
            endpoint.get("protocol").and_then(serde_json::Value::as_str) == Some("ethercat")
        })
        .expect("ethercat endpoint");
    assert!(
        ethercat
            .get("children")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|children| children.iter().any(|child| {
                child.get("kind").and_then(serde_json::Value::as_str) == Some("field_slave")
                    && child.get("model").and_then(serde_json::Value::as_str) == Some("EL1008")
                    && child.get("channels").and_then(serde_json::Value::as_u64) == Some(8)
            })),
        "ethercat endpoint should expose configured child slaves: {ethercat:#}"
    );
}

#[test]
fn comm_apply_accepts_empty_and_comment_only_io_toml_as_editable_bases() {
    let source = r#"
PROGRAM Main
VAR
    run : BOOL := TRUE;
END_VAR
END_PROGRAM
"#;

    for (case, initial) in [
        ("empty-drivers", "[io]\ndrivers = []\n"),
        ("comment-only", "# created by first-run setup\n"),
    ] {
        let mut state = hmi_test_state(source);
        let root = temp_dir(&format!("comm-apply-{case}"));
        set_hmi_project_root(&mut state, &root);
        write_file(&root.join("io.toml"), initial);

        let response = handle_request_value(
            json!({
                "id": 149,
                "type": "comm.apply",
                "params": {
                    "protocol": "ethercat",
                    "action": "upsert",
                    "params": {
                        "adapter": "mock",
                        "timeout_ms": 250,
                        "cycle_warn_ms": 5,
                        "on_error": "fault",
                        "modules": [
                            { "model": "EK1100", "slot": 0, "channels": 1 }
                        ],
                        "mock_inputs": []
                    }
                }
            }),
            &state,
            None,
        );

        assert!(response.ok, "{case} result should be structured: {:?}", response.error);
        let result = response.result.expect("apply result");
        assert_eq!(
            result.get("applied").and_then(serde_json::Value::as_bool),
            Some(true),
            "{case} apply result: {result:#}"
        );
        let text = fs::read_to_string(root.join("io.toml")).expect("read io.toml");
        assert!(text.contains("name = \"ethercat\""), "{case}: {text}");
        crate::config::validate_io_toml_text(&text)
            .unwrap_or_else(|error| panic!("{case} io.toml should validate: {error}\n{text}"));
    }
}

#[test]
fn comm_apply_remove_last_driver_deletes_io_toml() {
    let source = r#"
PROGRAM Main
VAR
    run : BOOL := TRUE;
END_VAR
END_PROGRAM
"#;
    let mut state = hmi_test_state(source);
    let root = temp_dir("comm-apply-remove-last");
    set_hmi_project_root(&mut state, &root);
    write_file(
        &root.join("io.toml"),
        r#"
[io]
safe_state = [{ address = "%QX0.0", value = "FALSE" }]

[[io.drivers]]
name = "loopback"
params = {}
"#,
    );

    let dry_run = handle_request_value(
        json!({
            "id": 490,
            "type": "comm.apply",
            "params": {
                "protocol": "loopback",
                "action": "remove",
                "instance_id": "loopback:0",
                "dry_run": true,
                "params": {}
            }
        }),
        &state,
        None,
    );
    assert!(dry_run.ok, "remove dry-run failed: {:?}", dry_run.error);
    let result = dry_run.result.expect("remove dry-run result");
    assert_eq!(
        result
            .get("lifecycle_effect")
            .and_then(serde_json::Value::as_str),
        Some("validate_only")
    );
    assert!(
        result
            .get("snippet")
            .is_none_or(serde_json::Value::is_null),
        "removing the last driver must not return invalid empty io.toml"
    );
    assert!(root.join("io.toml").exists(), "dry-run must not remove io.toml");

    let remove = handle_request_value(
        json!({
            "id": 491,
            "type": "comm.apply",
            "params": {
                "protocol": "loopback",
                "action": "remove",
                "instance_id": "loopback:0",
                "params": {}
            }
        }),
        &state,
        None,
    );
    assert!(remove.ok, "remove failed: {:?}", remove.error);
    let result = remove.result.expect("remove result");
    assert_eq!(
        result.get("applied").and_then(serde_json::Value::as_bool),
        Some(true)
    );
    assert_eq!(
        result
            .get("lifecycle_effect")
            .and_then(serde_json::Value::as_str),
        Some("restart_required")
    );
    assert!(
        !root.join("io.toml").exists(),
        "removing the last configured driver should turn I/O config off"
    );
}

#[test]
fn comm_apply_remove_one_instance_preserves_unrelated_instances() {
    let source = r#"
PROGRAM Main
VAR
    run : BOOL := TRUE;
END_VAR
END_PROGRAM
"#;
    let mut state = hmi_test_state(source);
    let root = temp_dir("comm-apply-remove-one");
    set_hmi_project_root(&mut state, &root);
    write_file(
        &root.join("io.toml"),
        r#"
[io]
safe_state = [{ address = "%QX0.0", value = "FALSE" }]

[[io.drivers]]
name = "loopback"
params = {}

[[io.drivers]]
name = "modbus-tcp"

[io.drivers.params]
address = "127.0.0.1:1502"
unit_id = 2
input_start = 5
output_start = 7
timeout_ms = 750
on_error = "warn"
"#,
    );

    let remove = handle_request_value(
        json!({
            "id": 492,
            "type": "comm.apply",
            "params": {
                "protocol": "modbus_tcp",
                "action": "remove",
                "instance_id": "modbus_tcp:1",
                "params": {}
            }
        }),
        &state,
        None,
    );

    assert!(remove.ok, "remove one instance failed: {:?}", remove.error);
    let result = remove.result.expect("remove one result");
    assert_eq!(
        result.get("applied").and_then(serde_json::Value::as_bool),
        Some(true)
    );
    let text = fs::read_to_string(root.join("io.toml")).expect("read io.toml");
    assert!(text.contains("loopback"), "loopback should remain: {text}");
    assert!(
        !text.contains("modbus-tcp"),
        "removed modbus instance should be gone: {text}"
    );
    crate::config::validate_io_toml_text(&text).expect("remaining io.toml should validate");
}

#[test]
fn comm_apply_edit_is_idempotent_and_preserves_unrelated_instances() {
    let source = r#"
PROGRAM Main
VAR
    run : BOOL := TRUE;
END_VAR
END_PROGRAM
"#;
    let mut state = hmi_test_state(source);
    let root = temp_dir("comm-apply-edit-idempotent");
    set_hmi_project_root(&mut state, &root);
    write_file(
        &root.join("io.toml"),
        r#"
[io]
safe_state = [{ address = "%QX0.0", value = "FALSE" }]

[[io.drivers]]
name = "loopback"
params = {}

[[io.drivers]]
name = "modbus-tcp"

[io.drivers.params]
address = "127.0.0.1:1502"
unit_id = 2
input_start = 5
output_start = 7
timeout_ms = 750
on_error = "warn"
"#,
    );

    let edit_payload = json!({
        "id": 493,
        "type": "comm.apply",
        "params": {
            "protocol": "modbus_tcp",
            "action": "edit",
            "instance_id": "modbus_tcp:1",
            "params": {
                "address": "127.0.0.1:2502",
                "unit_id": 3,
                "input_start": 10,
                "output_start": 20,
                "timeout_ms": 1000,
                "on_error": "fault"
            }
        }
    });

    for id in [493, 494] {
        let mut payload = edit_payload.clone();
        payload["id"] = json!(id);
        let response = handle_request_value(payload, &state, None);
        assert!(response.ok, "edit failed: {:?}", response.error);
        assert_eq!(
            response
                .result
                .as_ref()
                .and_then(|result| result.get("applied"))
                .and_then(serde_json::Value::as_bool),
            Some(true)
        );
    }

    let after_first_repeat = fs::read_to_string(root.join("io.toml")).expect("read io.toml");
    let repeat = handle_request_value(edit_payload, &state, None);
    assert!(repeat.ok, "repeat edit failed: {:?}", repeat.error);
    let after_second_repeat = fs::read_to_string(root.join("io.toml")).expect("read io.toml");

    assert_eq!(
        after_first_repeat, after_second_repeat,
        "same edit should be idempotent"
    );
    assert!(
        after_second_repeat.contains("loopback"),
        "unrelated loopback instance should remain: {after_second_repeat}"
    );
    assert!(
        after_second_repeat.contains("127.0.0.1:2502"),
        "edited modbus endpoint should be written: {after_second_repeat}"
    );
    assert!(
        !after_second_repeat.contains("127.0.0.1:1502"),
        "old modbus endpoint should be replaced: {after_second_repeat}"
    );
    crate::config::validate_io_toml_text(&after_second_repeat)
        .expect("edited io.toml should validate");
}

#[test]
fn comm_apply_edit_requires_selected_instance() {
    let source = r#"
PROGRAM Main
VAR
    run : BOOL := TRUE;
END_VAR
END_PROGRAM
"#;
    let mut state = hmi_test_state(source);
    let root = temp_dir("comm-apply-edit-requires-instance");
    set_hmi_project_root(&mut state, &root);
    write_file(
        &root.join("io.toml"),
        r#"
[io]
safe_state = [{ address = "%QX0.0", value = "FALSE" }]

[[io.drivers]]
name = "modbus-tcp"

[io.drivers.params]
address = "127.0.0.1:1502"
unit_id = 2
input_start = 5
output_start = 7
timeout_ms = 750
on_error = "warn"
"#,
    );

    let response = handle_request_value(
        json!({
            "id": 497,
            "type": "comm.apply",
            "params": {
                "protocol": "modbus_tcp",
                "action": "edit",
                "params": {
                    "address": "127.0.0.1:2502",
                    "unit_id": 3,
                    "input_start": 10,
                    "output_start": 20,
                    "timeout_ms": 1000,
                    "on_error": "fault"
                }
            }
        }),
        &state,
        None,
    );

    assert!(
        response.ok,
        "missing instance result should be structured: {:?}",
        response.error
    );
    let result = response.result.expect("missing instance result");
    assert_eq!(
        result
            .get("lifecycle_effect")
            .and_then(serde_json::Value::as_str),
        Some("blocked")
    );
    assert!(result
        .get("field_errors")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|errors| errors.iter().any(|error| {
            error.get("field").and_then(serde_json::Value::as_str) == Some("instance_id")
        })));

    let text = fs::read_to_string(root.join("io.toml")).expect("read io.toml");
    assert!(
        text.contains("127.0.0.1:1502"),
        "edit without a selected instance must not rewrite first instance: {text}"
    );
    assert!(
        !text.contains("127.0.0.1:2502"),
        "blocked edit endpoint must not be written: {text}"
    );
}

#[test]
fn comm_apply_rejects_cross_protocol_instance_ids() {
    let source = r#"
PROGRAM Main
VAR
    run : BOOL := TRUE;
END_VAR
END_PROGRAM
"#;
    let mut state = hmi_test_state(source);
    let root = temp_dir("comm-apply-cross-protocol-instance");
    set_hmi_project_root(&mut state, &root);
    write_file(
        &root.join("io.toml"),
        r#"
[io]
safe_state = [{ address = "%QX0.0", value = "FALSE" }]

[[io.drivers]]
name = "loopback"
params = {}

[[io.drivers]]
name = "modbus-tcp"

[io.drivers.params]
address = "127.0.0.1:1502"
unit_id = 2
input_start = 5
output_start = 7
timeout_ms = 750
on_error = "warn"
"#,
    );

    let wrong_prefix = handle_request_value(
        json!({
            "id": 495,
            "type": "comm.apply",
            "params": {
                "protocol": "modbus_tcp",
                "action": "remove",
                "instance_id": "loopback:1",
                "params": {}
            }
        }),
        &state,
        None,
    );
    assert!(
        wrong_prefix.ok,
        "wrong-prefix result should be structured: {:?}",
        wrong_prefix.error
    );
    let result = wrong_prefix.result.expect("wrong-prefix result");
    assert_eq!(
        result
            .get("lifecycle_effect")
            .and_then(serde_json::Value::as_str),
        Some("blocked")
    );
    assert!(result
        .get("field_errors")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|errors| errors.iter().any(|error| {
            error.get("field").and_then(serde_json::Value::as_str) == Some("instance_id")
        })));

    let wrong_slot = handle_request_value(
        json!({
            "id": 496,
            "type": "comm.apply",
            "params": {
                "protocol": "modbus_tcp",
                "action": "edit",
                "instance_id": "modbus_tcp:0",
                "params": {
                    "address": "127.0.0.1:2502",
                    "unit_id": 3,
                    "input_start": 10,
                    "output_start": 20,
                    "timeout_ms": 1000,
                    "on_error": "fault"
                }
            }
        }),
        &state,
        None,
    );
    assert!(
        wrong_slot.ok,
        "wrong-slot result should be structured: {:?}",
        wrong_slot.error
    );
    let result = wrong_slot.result.expect("wrong-slot result");
    assert_eq!(
        result
            .get("lifecycle_effect")
            .and_then(serde_json::Value::as_str),
        Some("blocked")
    );

    let text = fs::read_to_string(root.join("io.toml")).expect("read io.toml");
    assert!(text.contains("loopback"), "loopback must remain: {text}");
    assert!(
        text.contains("127.0.0.1:1502"),
        "modbus endpoint must not be rewritten by a stale id: {text}"
    );
    assert!(
        !text.contains("127.0.0.1:2502"),
        "stale edit must not be applied: {text}"
    );
}

#[test]
fn comm_apply_blocks_secret_fields_on_untrusted_channel() {
    let source = r#"
PROGRAM Main
VAR
    run : BOOL := TRUE;
END_VAR
END_PROGRAM
"#;
    let mut state = hmi_test_state(source);
    let root = temp_dir("comm-apply-secret");
    set_hmi_project_root(&mut state, &root);

    let blocked = handle_request_value(
        json!({
            "id": 48,
            "type": "comm.apply",
            "auth": engineer_token("comm-apply-secret", &mut state),
            "params": {
                "protocol": "mqtt",
                "credential_channel": "trusted_same_host",
                "params": {
                    "broker": "127.0.0.1:1883",
                    "username": "user",
                    "password": "top-password-value",
                    "topic_in": "trust/io/in",
                    "topic_out": "trust/io/out",
                    "allow_insecure_remote": true
                }
            }
        }),
        &state,
        Some("10.0.0.20:50200"),
    );
    assert!(blocked.ok, "blocked result should be structured");
    let result = blocked.result.expect("blocked result");
    assert_eq!(
        result.get("lifecycle_effect").and_then(serde_json::Value::as_str),
        Some("blocked")
    );
    let text = serde_json::to_string(&result).expect("result json");
    assert!(text.contains("Secret fields cannot be sent"));
    assert!(!text.contains("top-password-value"));
    assert!(!root.join("io.toml").exists(), "blocked secret apply must not write");

    let trusted = handle_request_value(
        json!({
            "id": 49,
            "type": "comm.apply",
            "params": {
                "protocol": "mqtt",
                "credential_channel": "trusted_same_host",
                "dry_run": true,
                "params": {
                    "broker": "127.0.0.1:1883",
                    "username": "user",
                    "password": "secret",
                    "topic_in": "trust/io/in",
                    "topic_out": "trust/io/out",
                    "allow_insecure_remote": true
                }
            }
        }),
        &state,
        None,
    );
    assert!(trusted.ok, "trusted dry-run failed: {:?}", trusted.error);
    let result = trusted.result.expect("trusted result");
    assert_eq!(
        result.get("lifecycle_effect").and_then(serde_json::Value::as_str),
        Some("validate_only")
    );
}

#[test]
fn comm_apply_is_policy_gated_and_audited_without_secret_values() {
    let source = r#"
PROGRAM Main
VAR
    run : BOOL := TRUE;
END_VAR
END_PROGRAM
"#;
    let mut state = hmi_test_state(source);
    let root = temp_dir("comm-apply-audit");
    set_hmi_project_root(&mut state, &root);
    let (audit_tx, audit_rx) = std::sync::mpsc::channel();
    state.audit_tx = Some(audit_tx);

    let denied = handle_request_value(
        json!({
            "id": 50,
            "type": "comm.apply",
            "params": {
                "protocol": "mqtt",
                "credential_channel": "untrusted_remote_plain_tcp",
                "params": {
                    "broker": "127.0.0.1:1883",
                    "username": "user",
                    "password": "super-secret",
                    "allow_insecure_remote": true
                }
            }
        }),
        &state,
        Some("10.0.0.20:50200"),
    );
    assert!(!denied.ok, "remote viewer should not be allowed to mutate config");
    let audit = audit_rx
        .recv_timeout(std::time::Duration::from_secs(2))
        .expect("denied audit");
    assert_eq!(audit.request_type.as_str(), "comm.apply");
    assert!(!audit.ok);
    let details = audit.details.expect("audit details");
    assert_eq!(
        details.get("protocol").and_then(serde_json::Value::as_str),
        Some("mqtt")
    );
    assert_eq!(
        details
            .get("secret_fields_present")
            .and_then(serde_json::Value::as_bool),
        Some(true)
    );
    assert!(
        !serde_json::to_string(&details)
            .expect("audit details json")
            .contains("super-secret")
    );

    let accepted = handle_request_value(
        json!({
            "id": 51,
            "type": "comm.apply",
            "params": {
                "protocol": "mqtt",
                "credential_channel": "trusted_same_host",
                "dry_run": true,
                "params": {
                    "broker": "127.0.0.1:1883",
                    "username": "user",
                    "password": "accepted-secret",
                    "topic_in": "trust/io/in",
                    "topic_out": "trust/io/out",
                    "allow_insecure_remote": true
                }
            }
        }),
        &state,
        None,
    );
    assert!(
        accepted.ok,
        "trusted local engineer apply should be accepted: {:?}",
        accepted.error
    );
    let audit = audit_rx
        .recv_timeout(std::time::Duration::from_secs(2))
        .expect("accepted audit");
    assert_eq!(audit.request_type.as_str(), "comm.apply");
    assert!(audit.ok);
    let details = audit.details.expect("accepted audit details");
    assert_eq!(
        details.get("protocol").and_then(serde_json::Value::as_str),
        Some("mqtt")
    );
    assert_eq!(
        details.get("dry_run").and_then(serde_json::Value::as_bool),
        Some(true)
    );
    assert_eq!(
        details
            .get("secret_fields_present")
            .and_then(serde_json::Value::as_bool),
        Some(true)
    );
    assert!(
        !serde_json::to_string(&details)
            .expect("accepted audit details json")
            .contains("accepted-secret")
    );
}

#[test]
fn offline_comm_schema_apply_and_topology_work_without_runtime() {
    let schema = crate::control::offline_comm_schema_json(None).expect("offline comm schema");
    assert_eq!(
        schema
            .get("schema_version")
            .and_then(serde_json::Value::as_u64),
        Some(4)
    );
    let protocols = schema
        .get("protocols")
        .and_then(serde_json::Value::as_array)
        .expect("protocols");
    assert!(protocols.iter().any(|protocol| {
        protocol.get("id").and_then(serde_json::Value::as_str) == Some("modbus_tcp")
    }));
    assert!(protocols.iter().all(|protocol| {
        protocol
            .get("instances")
            .and_then(serde_json::Value::as_array)
            .is_some_and(Vec::is_empty)
    }));
    assert!(
        protocols.iter().all(|protocol| protocol.get("profiles").is_none()),
        "offline comm schema must not expose rejected device archetype profiles"
    );

    let root = temp_dir("offline-comm");
    write_file(
        &root.join("runtime.toml"),
        &crate::bundle_template::render_runtime_toml(&SmolStr::new("offline-line"), 10),
    );

    let add = crate::control::offline_comm_apply_json(
        &root,
        json!({
            "protocol": "modbus-tcp",
            "action": "add",
            "params": {
                "address": "127.0.0.1:502",
                "unit_id": 1,
                "input_start": 0,
                "output_start": 0,
                "timeout_ms": 500,
                "on_error": "warn"
            }
        }),
    )
    .expect("offline comm apply");
    assert_eq!(
        add.get("applied").and_then(serde_json::Value::as_bool),
        Some(true)
    );
    assert_eq!(
        add.get("lifecycle_effect")
            .and_then(serde_json::Value::as_str),
        Some("restart_required")
    );
    let text = fs::read_to_string(root.join("io.toml")).expect("read offline io.toml");
    assert!(text.contains("modbus-tcp"));
    crate::config::validate_io_toml_text(&text).expect("offline io.toml validates");

    let topology =
        crate::control::offline_fleet_topology_json(&root).expect("offline fleet topology");
    assert_eq!(
        topology
            .get("schema_version")
            .and_then(serde_json::Value::as_u64),
        Some(4)
    );
    let hosts = topology
        .get("hosts")
        .and_then(serde_json::Value::as_array)
        .expect("hosts");
    let runtime = hosts[0]
        .get("runtimes")
        .and_then(serde_json::Value::as_array)
        .and_then(|runtimes| runtimes.first())
        .expect("offline runtime");
    assert_eq!(
        runtime.get("health").and_then(serde_json::Value::as_str),
        Some("configured_policy")
    );
    assert_eq!(
        runtime.get("source").and_then(serde_json::Value::as_str),
        Some("config")
    );
    let endpoints = runtime
        .get("endpoints")
        .and_then(serde_json::Value::as_array)
        .expect("endpoints");
    let modbus = endpoints
        .iter()
        .find(|endpoint| {
            endpoint.get("protocol").and_then(serde_json::Value::as_str) == Some("modbus_tcp")
        })
        .expect("modbus endpoint");
    assert_eq!(
        modbus.get("health").and_then(serde_json::Value::as_str),
        Some("configured_policy")
    );
    assert!(modbus.get("live").is_none(), "offline topology must not invent live values");
    assert_eq!(
        modbus
            .get("params")
            .and_then(|params| params.get("address"))
            .and_then(serde_json::Value::as_str),
        Some("127.0.0.1:502")
    );
    let links = topology
        .get("links")
        .and_then(serde_json::Value::as_array)
        .expect("links");
    assert!(links.iter().any(|link| {
        link.get("protocol").and_then(serde_json::Value::as_str) == Some("modbus_tcp")
            && link.get("role").and_then(serde_json::Value::as_str) == Some("client")
            && link.get("id").and_then(serde_json::Value::as_str).is_some()
    }));
    let serialized = serde_json::to_string(&topology).expect("topology json");
    for forbidden in ["auth_token", "password", "source_ip", "source_cidr"] {
        assert!(
            !serialized.contains(forbidden),
            "offline topology leaked forbidden token {forbidden}: {serialized}"
        );
    }

    let opcua_apply = crate::control::offline_comm_apply_json(
        &root,
        json!({
            "protocol": "opcua",
            "action": "upsert",
            "params": {
                "enabled": true,
                "listen": "127.0.0.1:4840",
                "endpoint_path": "/",
                "namespace_uri": "urn:trust:runtime",
                "publish_interval_ms": 250,
                "max_nodes": 128,
                "expose": ["global.*"],
                "security_policy": "basic256sha256",
                "security_mode": "sign_and_encrypt",
                "allow_anonymous": true
            }
        }),
    )
    .expect("offline opcua apply");
    assert_eq!(
        opcua_apply
            .get("schema_version")
            .and_then(serde_json::Value::as_u64),
        Some(4)
    );
    assert_eq!(
        opcua_apply
            .get("applied")
            .and_then(serde_json::Value::as_bool),
        Some(true)
    );
    assert!(opcua_apply.get("snippet").is_none_or(serde_json::Value::is_null));
    let runtime_text = fs::read_to_string(root.join("runtime.toml")).expect("read runtime.toml");
    assert!(runtime_text.contains("[runtime.opcua]"));
    crate::config::validate_runtime_toml_text(&runtime_text)
        .expect("offline runtime.toml validates after comm.apply");

    let topology =
        crate::control::offline_fleet_topology_json(&root).expect("offline fleet topology with opcua");
    let endpoints = topology
        .pointer("/hosts/0/runtimes/0/endpoints")
        .and_then(serde_json::Value::as_array)
        .expect("endpoints with opcua");
    let opcua = endpoints
        .iter()
        .find(|endpoint| {
            endpoint.get("protocol").and_then(serde_json::Value::as_str) == Some("opcua")
        })
        .expect("opcua endpoint");
    assert_eq!(
        opcua.get("health").and_then(serde_json::Value::as_str),
        Some("configured_policy")
    );
    assert_eq!(
        opcua
            .get("params")
            .and_then(|params| params.get("listen"))
            .and_then(serde_json::Value::as_str),
        Some("127.0.0.1:4840")
    );

    let opcua_disable = crate::control::offline_comm_apply_json(
        &root,
        json!({
            "protocol": "opcua",
            "action": "disable",
            "params": {}
        }),
    )
    .expect("offline opcua disable");
    assert_eq!(
        opcua_disable
            .get("applied")
            .and_then(serde_json::Value::as_bool),
        Some(true)
    );
    let runtime_text = fs::read_to_string(root.join("runtime.toml")).expect("read runtime.toml");
    assert!(runtime_text.contains("[runtime.opcua]"));
    assert!(runtime_text.contains("enabled = false"));

    let remove = crate::control::offline_comm_apply_json(
        &root,
        json!({
            "protocol": "modbus_tcp",
            "action": "remove",
            "params": { "address": "127.0.0.1:502" }
        }),
    )
    .expect("offline comm remove");
    assert_eq!(
        remove.get("applied").and_then(serde_json::Value::as_bool),
        Some(true)
    );
    assert!(
        !root.join("io.toml").exists(),
        "offline remove should remove the last io.toml driver"
    );
}

#[test]
fn offline_comm_apply_creates_runtime_toml_when_absent() {
    let root = temp_dir("offline-comm-runtime-create");

    let opcua_apply = crate::control::offline_comm_apply_json(
        &root,
        json!({
            "protocol": "opcua",
            "action": "upsert",
            "params": {
                "enabled": true,
                "listen": "127.0.0.1:4840",
                "endpoint_path": "/",
                "namespace_uri": "urn:trust:runtime",
                "publish_interval_ms": 250,
                "max_nodes": 128,
                "expose": ["global.*"],
                "security_policy": "basic256sha256",
                "security_mode": "sign_and_encrypt",
                "allow_anonymous": true
            }
        }),
    )
    .expect("offline opcua apply creates runtime.toml");

    assert_eq!(
        opcua_apply
            .get("schema_version")
            .and_then(serde_json::Value::as_u64),
        Some(4)
    );
    assert_eq!(
        opcua_apply
            .get("applied")
            .and_then(serde_json::Value::as_bool),
        Some(true)
    );
    assert_eq!(
        opcua_apply
            .get("lifecycle_effect")
            .and_then(serde_json::Value::as_str),
        Some("restart_required")
    );
    let runtime_text = fs::read_to_string(root.join("runtime.toml")).expect("read runtime.toml");
    assert!(runtime_text.contains("[runtime.opcua]"));
    assert!(runtime_text.contains("listen = \"127.0.0.1:4840\""));
    crate::config::validate_runtime_toml_text(&runtime_text)
        .expect("created runtime.toml should validate");
}

#[test]
fn offline_comm_apply_writes_ads_runtime_and_ads_toml() {
    let root = temp_dir("offline-comm-ads");

    let apply = crate::control::offline_comm_apply_json(
        &root,
        json!({
            "protocol": "ads",
            "action": "upsert",
            "params": {
                "enabled": true,
                "config_path": "ads.toml",
                "worker_tick_interval_ms": 20,
                "connections": [{
                    "name": "line1",
                    "target_net_id": "5.23.91.12.1.1",
                    "host": "192.168.10.5",
                    "ams_port": 851,
                    "transport": "plain",
                    "insecure_transport": true,
                    "points": [{
                        "symbol": "MAIN.Temperature",
                        "var": "line1_temp",
                        "type": "REAL"
                    }]
                }]
            }
        }),
    )
    .expect("offline ads apply");

    assert_eq!(
        apply
            .get("schema_version")
            .and_then(serde_json::Value::as_u64),
        Some(4)
    );
    assert_eq!(
        apply.get("applied").and_then(serde_json::Value::as_bool),
        Some(true)
    );
    assert!(apply.get("snippet").is_none_or(serde_json::Value::is_null));

    let runtime_text = fs::read_to_string(root.join("runtime.toml")).expect("read runtime.toml");
    assert!(runtime_text.contains("[runtime.ads]"));
    crate::config::validate_runtime_toml_text(&runtime_text)
        .expect("runtime.toml with runtime.ads should validate");
    let ads_text = fs::read_to_string(root.join("ads.toml")).expect("read ads.toml");
    crate::ads::parse_ads_toml(ads_text.as_str()).expect("ads.toml should validate");

    let topology =
        crate::control::offline_fleet_topology_json(&root).expect("offline topology with ADS");
    let endpoints = topology
        .pointer("/hosts/0/runtimes/0/endpoints")
        .and_then(serde_json::Value::as_array)
        .expect("endpoints");
    assert!(endpoints.iter().any(|endpoint| {
        endpoint.get("protocol").and_then(serde_json::Value::as_str) == Some("ads")
            && endpoint
                .get("health")
                .and_then(serde_json::Value::as_str)
                == Some("configured_policy")
    }));
    let serialized = serde_json::to_string(&topology).expect("topology json");
    assert!(!serialized.contains("source_ip"));
    assert!(!serialized.contains("source_cidr"));
}

#[test]
fn comm_capabilities_control_request_reports_stable_protocol_statuses() {
    let source = r#"
PROGRAM Main
VAR
    run : BOOL := TRUE;
END_VAR
END_PROGRAM
"#;
    let state = hmi_test_state(source);

    let response = handle_request_value(json!({"id": 43, "type": "comm.capabilities"}), &state, None);

    assert!(
        response.ok,
        "comm.capabilities failed: {:?}",
        response.error
    );
    let result = response.result.expect("capabilities result");
    assert_eq!(
        result
            .get("schema_version")
            .and_then(serde_json::Value::as_u64),
        Some(4)
    );
    let capabilities = result
        .get("capabilities")
        .and_then(serde_json::Value::as_array)
        .expect("capabilities array");
    assert_eq!(capabilities.len(), 14);

    let by_id = |id: &str| {
        capabilities
            .iter()
            .find(|capability| capability.get("id").and_then(serde_json::Value::as_str) == Some(id))
            .unwrap_or_else(|| panic!("missing capability {id}"))
    };

    assert_eq!(
        by_id("ads").get("health").and_then(serde_json::Value::as_str),
        Some(if cfg!(feature = "ads-wire") {
            "not_configured"
        } else {
            "not_in_build"
        })
    );
    assert_eq!(
        by_id("runtime_cloud")
            .get("health")
            .and_then(serde_json::Value::as_str),
        Some("not_configured")
    );
    assert_ne!(
        by_id("runtime_cloud")
            .get("health")
            .and_then(serde_json::Value::as_str),
        Some("connected")
    );
    assert_eq!(
        by_id("modbus_tcp")
            .get("health")
            .and_then(serde_json::Value::as_str),
        Some("not_configured")
    );
    assert_eq!(
        by_id("openot")
            .get("platform")
            .and_then(serde_json::Value::as_str),
        Some("unix")
    );
    assert_eq!(
        by_id("gpio")
            .get("platform")
            .and_then(serde_json::Value::as_str),
        Some("linux")
    );
    assert_eq!(
        by_id("ethercat")
            .get("platform")
            .and_then(serde_json::Value::as_str),
        Some("unix")
    );
    assert_eq!(
        by_id("realtime_t0")
            .get("platform")
            .and_then(serde_json::Value::as_str),
        Some("linux")
    );
}

#[test]
fn default_visible_comm_schema_protocols_are_built_on_this_platform() {
    let source = r#"
PROGRAM Main
VAR
    run : BOOL := TRUE;
END_VAR
END_PROGRAM
"#;
    let state = hmi_test_state(source);

    let schema = handle_request_value(json!({"id": 45, "type": "comm.schema"}), &state, None);
    assert!(schema.ok, "comm.schema failed: {:?}", schema.error);
    let schema = schema.result.expect("schema result");
    let protocols = schema
        .get("protocols")
        .and_then(serde_json::Value::as_array)
        .expect("schema protocols");

    let capabilities =
        handle_request_value(json!({"id": 46, "type": "comm.capabilities"}), &state, None);
    assert!(
        capabilities.ok,
        "comm.capabilities failed: {:?}",
        capabilities.error
    );
    let capabilities = capabilities.result.expect("capabilities result");
    let capabilities = capabilities
        .get("capabilities")
        .and_then(serde_json::Value::as_array)
        .expect("capabilities");

    for protocol in protocols {
        let id = protocol
            .get("id")
            .and_then(serde_json::Value::as_str)
            .expect("schema protocol id");
        let availability = protocol
            .get("availability")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("default");
        if availability == "optional_build" {
            continue;
        }
        assert_eq!(
            availability, "default",
            "unknown availability for protocol {id}"
        );

        let capability = capabilities
            .iter()
            .find(|capability| {
                capability.get("id").and_then(serde_json::Value::as_str) == Some(id)
            })
            .unwrap_or_else(|| panic!("missing capability for default schema protocol {id}"));
        if !capability_platform_matches_this_runtime(capability) {
            continue;
        }
        assert_eq!(
            capability
                .get("built")
                .and_then(serde_json::Value::as_bool),
            Some(true),
            "default schema protocol {id} must be built in the official default runtime"
        );
    }
}

fn capability_platform_matches_this_runtime(capability: &serde_json::Value) -> bool {
    match capability
        .get("platform")
        .and_then(serde_json::Value::as_str)
    {
        Some("linux") => cfg!(target_os = "linux"),
        Some("unix") => cfg!(unix),
        Some(_) => false,
        None => true,
    }
}

fn test_ads_client_config() -> crate::ads::AdsClientConfig {
    let mut route = trust_ads_core::AdsRoute::new(
        "line1",
        trust_ads_core::AmsNetId::new("5.23.91.12.1.1"),
        "192.168.77.20",
        851,
    );
    route.local_net_id = Some(trust_ads_core::AmsNetId::new("192.168.77.10.1.1"));
    route.security.transport = trust_ads_core::TransportSecurity::Plain;
    crate::ads::AdsClientConfig {
        connections: vec![crate::ads::AdsConnectionConfig {
            route,
            points: Vec::new(),
        }],
    }
}

fn connected_ads_status() -> crate::ads::diagnostics::AdsStatusReport {
    crate::ads::diagnostics::AdsStatusReport {
        schema_version: crate::ads::diagnostics::ADS_DIAGNOSTICS_SCHEMA_VERSION,
        role: crate::ads::diagnostics::DoctorRole::Client,
        overall: crate::ads::diagnostics::AdsStatusOverall::Healthy,
        runtime_identity_hash: None,
        deployed_ads_config_hash: None,
        connections: vec![crate::ads::diagnostics::AdsConnectionStatus {
            name: "line1".to_string(),
            target: Some(crate::ads::diagnostics::TargetIdentity {
                name: Some("TwinCAT".to_string()),
                ip: "192.168.77.20".to_string(),
                ams_net_id: "5.23.91.12.1.1".to_string(),
                ams_port: 851,
                tc_version: Some("3.1".to_string()),
            }),
            state: crate::ads::diagnostics::AdsConnectionStatusState::Connected,
            point_count: 2,
            degraded_points: 0,
            last_good_value_ms: Some(1234),
            symbol_version: Some(7),
            summary: "connected".to_string(),
        }],
        summary: "ADS connections healthy.".to_string(),
    }
}

fn apply_runtime_config_to_control_settings(
    state: &mut ControlState,
    runtime: &crate::config::RuntimeConfig,
) {
    state.resource_name = runtime.resource_name.clone();
    let mut settings = state.settings.lock().expect("settings lock");
    settings.cycle_interval = runtime.cycle_interval;
    settings.web.enabled = runtime.web.enabled;
    settings.web.listen = runtime.web.listen.clone();
    settings.web.auth = SmolStr::new(match runtime.web.auth {
        crate::config::WebAuthMode::Local => "local",
        crate::config::WebAuthMode::Token => "token",
    });
    settings.web.tls = runtime.web.tls;
    settings.discovery.enabled = runtime.discovery.enabled;
    settings.discovery.service_name = runtime.discovery.service_name.clone();
    settings.discovery.advertise = runtime.discovery.advertise;
    settings.discovery.interfaces = runtime.discovery.interfaces.clone();
    settings.discovery.host_group = runtime.discovery.host_group.clone();
    settings.mesh.enabled = runtime.mesh.enabled;
    settings.mesh.role = runtime.mesh.role;
    settings.mesh.listen = runtime.mesh.listen.clone();
    settings.mesh.connect = runtime.mesh.connect.clone();
    settings.mesh.tls = runtime.mesh.tls;
    settings.mesh.auth_token = runtime.mesh.auth_token.clone();
    settings.mesh.publish = runtime.mesh.publish.clone();
    settings.mesh.subscribe = runtime.mesh.subscribe.clone();
    settings.opcua.enabled = runtime.opcua.enabled;
    settings.opcua.listen = runtime.opcua.listen.clone();
    settings.opcua.endpoint_path = runtime.opcua.endpoint_path.clone();
    settings.opcua.namespace_uri = runtime.opcua.namespace_uri.clone();
    settings.opcua.publish_interval_ms = runtime.opcua.publish_interval_ms;
    settings.opcua.max_nodes = runtime.opcua.max_nodes;
    settings.opcua.expose = runtime.opcua.expose.clone();
    settings.opcua.security_policy =
        SmolStr::new(runtime.opcua.security.policy.as_config_value());
    settings.opcua.security_mode = SmolStr::new(runtime.opcua.security.mode.as_config_value());
    settings.opcua.allow_anonymous = runtime.opcua.security.allow_anonymous;
    settings.opcua.username_set = runtime.opcua.username.is_some();
}

#[test]
fn fleet_topology_reports_runtime_hosts_endpoints_and_links_without_secrets() {
    let source = r#"
PROGRAM Main
VAR
    run : BOOL := TRUE;
END_VAR
END_PROGRAM
"#;
    let mut state = hmi_test_state_with_ads_status(source, connected_ads_status());
    let project_root = temp_dir("fleet-topology");
    write_file(
        &project_root.join("io.toml"),
        r#"
	[io]
	safe_state = [{ address = "%QX0.0", value = "FALSE" }]

	[[io.drivers]]
	name = "modbus-tcp"
	params = { address = "127.0.0.1:1502", unit_id = 1, input_start = 0, output_start = 0, timeout_ms = 500, on_error = "fault" }

	[[io.drivers]]
	name = "ethercat"
	params = { adapter = "eth0", on_error = "fault" }

	[[io.drivers]]
	name = "mqtt"
	params = { broker = "mqtt://broker.local:1883", topic_in = "trust/io/in", topic_out = "trust/io/out", password = "mqtt-secret", tls = true }
	"#,
    );
    set_hmi_project_root(&mut state, &project_root);
    *state.ads_client_config.lock().expect("ads client config") = Some(test_ads_client_config());
    let now_ns = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64;
    {
        let mut settings = state.settings.lock().expect("settings lock");
        settings.web.enabled = true;
        settings.web.listen = SmolStr::new("127.0.0.1:8080");
        settings.discovery.enabled = true;
        settings.discovery.advertise = true;
        settings.discovery.host_group = Some(SmolStr::new("cell-a"));
        settings.mesh.enabled = true;
        settings.mesh.listen = SmolStr::new("127.0.0.1:7447");
        settings.mesh.connect = vec![SmolStr::new("tcp/10.0.0.2:7447")];
        settings.mesh.tls = true;
        settings.mesh.auth_token = Some(SmolStr::new("mesh-secret"));
        settings.runtime_cloud.link_preferences =
            vec![crate::config::RuntimeCloudLinkPreferenceRule {
                source: SmolStr::new("runtime:RESOURCE"),
                target: SmolStr::new("runtime:peer"),
                transport: crate::config::RuntimeCloudPreferredTransport::Realtime,
            }];
    }
    state
        .web_listener_bound
        .store(true, std::sync::atomic::Ordering::Relaxed);
    *state.mesh_topology.lock().expect("mesh topology lock") =
        Some(crate::mesh::MeshTopologyEvidence::for_test(
            true,
            &["peer-runtime"],
            now_ns,
        ));
    state.discovery.replace_entries(vec![crate::discovery::DiscoveryEntry {
        id: SmolStr::new("peer-runtime-1234"),
        name: SmolStr::new("peer-runtime"),
        addresses: vec!["10.0.0.2".parse::<std::net::IpAddr>().unwrap()],
        web_port: Some(8080),
        web_tls: false,
        mesh_port: Some(7447),
        control: Some(SmolStr::new("tcp://10.0.0.2:9900")),
        host_group: Some(SmolStr::new("cell-a")),
        last_seen_ns: now_ns,
    }]);
    state
        .io_health
        .lock()
        .expect("io health lock")
        .extend([
            crate::io::IoDriverStatus {
                name: SmolStr::new("modbus-tcp"),
                health: crate::io::IoDriverHealth::Ok,
            },
            crate::io::IoDriverStatus {
                name: SmolStr::new("ethercat"),
                health: crate::io::IoDriverHealth::Ok,
            },
            crate::io::IoDriverStatus {
                name: SmolStr::new("mqtt"),
                health: crate::io::IoDriverHealth::Degraded {
                    error: SmolStr::new("broker slow"),
                },
            },
        ]);
    *state.io_snapshot.lock().expect("io snapshot lock") = Some(crate::io::IoSnapshot {
        inputs: vec![crate::io::IoSnapshotEntry {
            name: Some(SmolStr::new("Input0")),
            address: crate::io::IoAddress::parse("%IX0.0").expect("input address"),
            value: crate::io::IoSnapshotValue::Value(crate::value::Value::Bool(true)),
        }],
        outputs: vec![crate::io::IoSnapshotEntry {
            name: Some(SmolStr::new("Output0")),
            address: crate::io::IoAddress::parse("%QX0.0").expect("output address"),
            value: crate::io::IoSnapshotValue::Value(crate::value::Value::Bool(false)),
        }],
        memory: Vec::new(),
    });
    state
        .io_snapshot_seen_ms
        .store(now_ns / 1_000_000, std::sync::atomic::Ordering::Relaxed);

    let response = handle_request_value(json!({"id": 46, "type": "fleet.topology"}), &state, None);
    assert!(response.ok, "fleet.topology failed: {:?}", response.error);
    let result = response.result.expect("topology result");

    assert_eq!(
        result
            .get("schema_version")
            .and_then(serde_json::Value::as_u64),
        Some(4)
    );
    let hosts = result
        .get("hosts")
        .and_then(serde_json::Value::as_array)
        .expect("hosts array");
    assert_eq!(hosts.len(), 2);
    assert!(
        hosts[0].get("uptime_s").is_some() || !cfg!(target_os = "linux"),
        "linux hosts should expose real uptime when /proc is available"
    );
    let runtime = hosts[0]
        .get("runtimes")
        .and_then(serde_json::Value::as_array)
        .filter(|runtimes| !runtimes.is_empty())
        .or_else(|| {
            hosts[0]
                .get("containers")
                .and_then(serde_json::Value::as_array)
                .and_then(|containers| containers.first())
                .and_then(|container| container.get("runtimes"))
                .and_then(serde_json::Value::as_array)
        })
        .and_then(|runtimes| runtimes.first())
        .expect("runtime entry");
    assert_eq!(
        runtime
            .get("runtime_id")
            .and_then(serde_json::Value::as_str),
        Some("RESOURCE")
    );
    let endpoints = runtime
        .get("endpoints")
        .and_then(serde_json::Value::as_array)
        .expect("endpoint array");
    let endpoint = |protocol: &str| {
        endpoints
            .iter()
            .find(|endpoint| {
                endpoint.get("protocol").and_then(serde_json::Value::as_str) == Some(protocol)
            })
            .unwrap_or_else(|| panic!("missing endpoint {protocol}"))
    };
    assert_eq!(
        endpoint("modbus_tcp")
            .get("health")
            .and_then(serde_json::Value::as_str),
        Some("connected")
    );
    assert_eq!(
        endpoint("modbus_tcp")
            .get("address")
            .and_then(serde_json::Value::as_str),
        Some("127.0.0.1:1502")
    );
    assert_eq!(
        endpoint("modbus_tcp")
            .get("role")
            .and_then(serde_json::Value::as_str),
        Some("client")
    );
    assert!(
        endpoint("modbus_tcp").get("live").is_some(),
        "I/O endpoint should include the real I/O snapshot summary"
    );
    assert_eq!(
        endpoint("ethercat")
            .get("role")
            .and_then(serde_json::Value::as_str),
        Some("master")
    );
    assert_eq!(
        endpoint("ethercat")
            .get("health")
            .and_then(serde_json::Value::as_str),
        Some("connected")
    );
    assert_eq!(
        endpoint("mqtt")
            .get("role")
            .and_then(serde_json::Value::as_str),
        Some("client")
    );
    assert_eq!(
        endpoint("mqtt")
            .get("health")
            .and_then(serde_json::Value::as_str),
        Some("degraded")
    );
    assert_eq!(
        endpoint("web")
            .get("health")
            .and_then(serde_json::Value::as_str),
        Some("connected")
    );
    assert_eq!(
        endpoint("web")
            .get("detail")
            .and_then(serde_json::Value::as_str),
        Some("Web listener successfully bound during runtime startup.")
    );
    assert_eq!(
        endpoint("mesh")
            .get("role")
            .and_then(serde_json::Value::as_str),
        Some("peer")
    );
    assert_eq!(
        endpoint("mesh")
            .get("health")
            .and_then(serde_json::Value::as_str),
        Some("connected")
    );
    assert_eq!(
        endpoint("ads")
            .get("role")
            .and_then(serde_json::Value::as_str),
        Some("client")
    );
    assert_eq!(
        endpoint("ads")
            .get("address")
            .and_then(serde_json::Value::as_str),
        Some("192.168.77.10.1.1")
    );
    assert_eq!(
        endpoint("ads")
            .get("health")
            .and_then(serde_json::Value::as_str),
        Some("connected")
    );
    assert_eq!(
        endpoint("ads")
            .get("live")
            .and_then(|live| live.get("value"))
            .and_then(|value| value.get("connected"))
            .and_then(serde_json::Value::as_u64),
        Some(1)
    );

    let links = result
        .get("links")
        .and_then(serde_json::Value::as_array)
        .expect("links array");
    for link in links {
        assert!(
            link.get("id")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|value| !value.is_empty()),
            "link id should be non-empty: {link:?}"
        );
        assert!(
            link.get("role")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|value| !value.is_empty()),
            "link role should be non-empty: {link:?}"
        );
    }
    assert!(links.iter().any(|link| {
        link.get("protocol").and_then(serde_json::Value::as_str) == Some("mesh")
            && link.get("secure").and_then(serde_json::Value::as_bool) == Some(true)
    }));
    assert!(links.iter().any(|link| {
        link.get("protocol").and_then(serde_json::Value::as_str) == Some("realtime")
            && link.get("same_host").and_then(serde_json::Value::as_bool) == Some(true)
    }));
    assert!(links.iter().any(|link| {
        link.get("protocol").and_then(serde_json::Value::as_str) == Some("discovery")
            && link.get("to").and_then(serde_json::Value::as_str) == Some("runtime:peer-runtime")
    }));
    assert!(links.iter().any(|link| {
        link.get("protocol").and_then(serde_json::Value::as_str) == Some("modbus_tcp")
            && link.get("role").and_then(serde_json::Value::as_str) == Some("client")
            && link.get("to").and_then(serde_json::Value::as_str)
                == Some("external:modbus:127.0.0.1:1502")
            && link.get("status").and_then(serde_json::Value::as_str) == Some("connected")
    }));
    assert!(links.iter().any(|link| {
        link.get("protocol").and_then(serde_json::Value::as_str) == Some("ethercat")
            && link.get("role").and_then(serde_json::Value::as_str) == Some("master")
            && link.get("to").and_then(serde_json::Value::as_str)
                == Some("external:ethercat:eth0")
            && link.get("status").and_then(serde_json::Value::as_str) == Some("connected")
            && link.get("detail").is_none()
    }));
    assert!(links.iter().any(|link| {
        link.get("protocol").and_then(serde_json::Value::as_str) == Some("ads")
            && link.get("role").and_then(serde_json::Value::as_str) == Some("client")
            && link.get("to").and_then(serde_json::Value::as_str)
                == Some("external:ads:5.23.91.12.1.1")
            && link.get("status").and_then(serde_json::Value::as_str) == Some("connected")
            && link.get("secure").and_then(serde_json::Value::as_bool) == Some(false)
    }));

    let shared = result
        .get("shared")
        .and_then(serde_json::Value::as_array)
        .expect("shared array");
    assert!(shared.iter().any(|node| {
        node.get("kind").and_then(serde_json::Value::as_str) == Some("broker")
            && node.get("address").and_then(serde_json::Value::as_str)
                == Some("mqtt://broker.local:1883")
    }));

    let external = result
        .get("external")
        .and_then(serde_json::Value::as_array)
        .expect("external array");
    assert!(external.iter().any(|node| {
        node.get("id").and_then(serde_json::Value::as_str)
            == Some("external:modbus:127.0.0.1:1502")
            && node.get("kind").and_then(serde_json::Value::as_str) == Some("device")
    }));
    assert!(external.iter().any(|node| {
        node.get("id").and_then(serde_json::Value::as_str) == Some("external:ethercat:eth0")
            && node.get("kind").and_then(serde_json::Value::as_str) == Some("fieldbus")
    }));
    assert!(external.iter().any(|node| {
        node.get("id").and_then(serde_json::Value::as_str)
            == Some("external:ads:5.23.91.12.1.1")
            && node.get("kind").and_then(serde_json::Value::as_str) == Some("plc")
            && node.get("name").and_then(serde_json::Value::as_str)
                == Some("TwinCAT 5.23.91.12.1.1")
    }));

    let discovered = result
        .get("discovered")
        .and_then(serde_json::Value::as_array)
        .expect("discovered array");
    assert_eq!(discovered.len(), 1);
    assert_eq!(
        discovered[0].get("name").and_then(serde_json::Value::as_str),
        Some("peer-runtime")
    );

    let serialized = serde_json::to_string(&result).expect("topology json");
    assert!(!serialized.contains("mesh-secret"));
    assert!(!serialized.contains("mqtt-secret"));
}

#[test]
fn fleet_topology_uses_project_config_for_roles_and_counterparts() {
    let source = r#"
PROGRAM Main
VAR
    run : BOOL := TRUE;
END_VAR
END_PROGRAM
"#;
    let mut state = hmi_test_state(source);
    let demo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/network_canvas_demo")
        .canonicalize()
        .expect("network canvas demo path");
    let project_root = temp_dir("fleet-topology-role-counterparts");
    fs::copy(demo_root.join("runtime.toml"), project_root.join("runtime.toml"))
        .expect("copy runtime.toml");
    write_file(
        &project_root.join("io.toml"),
        r#"
[io]
safe_state = [{ address = "%QX0.0", value = "FALSE" }]

[[io.drivers]]
name = "modbus-tcp"
params = { address = "192.168.1.50:502", unit_id = 1, input_start = 0, output_start = 0, timeout_ms = 500, on_error = "warn" }

[[io.drivers]]
name = "ethercat"
params = { adapter = "eth1", timeout_ms = 250, cycle_warn_ms = 5, on_error = "fault", modules = [{ model = "EK1100", slot = 0, channels = 1 }, { model = "EL1008", slot = 1, channels = 8 }] }
"#,
    );
    let runtime_config =
        crate::config::RuntimeConfig::load(project_root.join("runtime.toml")).expect("runtime.toml");
    let io = crate::config::IoConfig::load(project_root.join("io.toml")).expect("io.toml");
    assert_eq!(io.drivers.len(), 2, "test project should expose the two I/O drivers");
    apply_runtime_config_to_control_settings(&mut state, &runtime_config);
    {
        let mut settings = state.settings.lock().expect("settings lock");
        settings.mesh.enabled = true;
        settings.mesh.role = crate::config::MeshRole::Peer;
        settings.mesh.listen = SmolStr::new("127.0.0.1:7447");
        settings.mesh.connect = vec![SmolStr::new("tcp/10.0.0.2:7447")];
        settings.opcua.enabled = true;
        settings.opcua.listen = SmolStr::new("127.0.0.1:4840");
    }
    set_hmi_project_root(&mut state, &project_root);

    let response = handle_request_value(json!({"id": 48, "type": "fleet.topology"}), &state, None);
    assert!(response.ok, "fleet.topology failed: {:?}", response.error);
    let result = response.result.expect("topology result");
    assert_eq!(
        result
            .get("schema_version")
            .and_then(serde_json::Value::as_u64),
        Some(4)
    );
    let hosts = result
        .get("hosts")
        .and_then(serde_json::Value::as_array)
        .expect("hosts array");
    let runtime = hosts[0]
        .get("runtimes")
        .and_then(serde_json::Value::as_array)
        .filter(|runtimes| !runtimes.is_empty())
        .or_else(|| {
            hosts[0]
                .get("containers")
                .and_then(serde_json::Value::as_array)
                .and_then(|containers| containers.first())
                .and_then(|container| container.get("runtimes"))
                .and_then(serde_json::Value::as_array)
        })
        .and_then(|runtimes| runtimes.first())
        .expect("runtime entry");
    assert_eq!(
        runtime
            .get("runtime_id")
            .and_then(serde_json::Value::as_str),
        Some(runtime_config.resource_name.as_str())
    );
    let endpoints = runtime
        .get("endpoints")
        .and_then(serde_json::Value::as_array)
        .expect("endpoints");
    let endpoint = |protocol: &str| {
        endpoints
            .iter()
            .find(|endpoint| {
                endpoint.get("protocol").and_then(serde_json::Value::as_str) == Some(protocol)
            })
            .unwrap_or_else(|| panic!("missing endpoint {protocol}"))
    };
    assert_eq!(
        endpoint("modbus_tcp")
            .get("role")
            .and_then(serde_json::Value::as_str),
        Some("client")
    );
    assert_eq!(
        endpoint("modbus_tcp")
            .get("address")
            .and_then(serde_json::Value::as_str),
        Some("192.168.1.50:502")
    );
    assert_eq!(
        endpoint("ethercat")
            .get("role")
            .and_then(serde_json::Value::as_str),
        Some("master")
    );
    let ethercat_children = endpoint("ethercat")
        .get("children")
        .and_then(serde_json::Value::as_array)
        .expect("ethercat children");
    assert!(ethercat_children.iter().any(|child| {
        child.get("kind").and_then(serde_json::Value::as_str) == Some("field_slave")
            && child.get("model").and_then(serde_json::Value::as_str) == Some("EL1008")
            && child.get("channels").and_then(serde_json::Value::as_u64) == Some(8)
            && child.get("source").and_then(serde_json::Value::as_str) == Some("config")
    }));
    assert_eq!(
        endpoint("opcua")
            .get("role")
            .and_then(serde_json::Value::as_str),
        Some("server")
    );
    assert_eq!(
        endpoint("mesh")
            .get("role")
            .and_then(serde_json::Value::as_str),
        Some("peer")
    );

    let links = result
        .get("links")
        .and_then(serde_json::Value::as_array)
        .expect("links");
    assert!(links.iter().any(|link| {
        link.get("protocol").and_then(serde_json::Value::as_str) == Some("modbus_tcp")
            && link.get("role").and_then(serde_json::Value::as_str) == Some("client")
            && link.get("to").and_then(serde_json::Value::as_str)
                == Some("external:modbus:192.168.1.50:502")
    }));
    assert!(links.iter().any(|link| {
        link.get("protocol").and_then(serde_json::Value::as_str) == Some("ethercat")
            && link.get("role").and_then(serde_json::Value::as_str) == Some("master")
            && link.get("to").and_then(serde_json::Value::as_str)
                == Some("external:ethercat:eth1")
    }));

    let external = result
        .get("external")
        .and_then(serde_json::Value::as_array)
        .expect("external");
    assert!(external.iter().any(|node| {
        node.get("id").and_then(serde_json::Value::as_str)
            == Some("external:modbus:192.168.1.50:502")
            && node.get("kind").and_then(serde_json::Value::as_str) == Some("device")
    }));
    assert!(external.iter().any(|node| {
        node.get("id").and_then(serde_json::Value::as_str) == Some("external:ethercat:eth1")
            && node.get("kind").and_then(serde_json::Value::as_str) == Some("fieldbus")
    }));
}

#[test]
fn fleet_topology_does_not_mark_enabled_services_green_without_bound_evidence() {
    let source = r#"
PROGRAM Main
VAR
    run : BOOL := TRUE;
END_VAR
END_PROGRAM
"#;
    let state = hmi_test_state(source);
    {
        let mut settings = state.settings.lock().expect("settings lock");
        settings.web.enabled = true;
        settings.web.listen = SmolStr::new("127.0.0.1:8080");
        settings.opcua.enabled = true;
        settings.opcua.listen = SmolStr::new("127.0.0.1:4840");
        settings.opcua.expose = vec![SmolStr::new("global.*")];
    }

    let response = handle_request_value(json!({"id": 47, "type": "fleet.topology"}), &state, None);
    assert!(response.ok, "fleet.topology failed: {:?}", response.error);
    let result = response.result.expect("topology result");
    let hosts = result
        .get("hosts")
        .and_then(serde_json::Value::as_array)
        .expect("hosts array");
    let endpoints = hosts[0]
        .get("runtimes")
        .and_then(serde_json::Value::as_array)
        .filter(|runtimes| !runtimes.is_empty())
        .or_else(|| {
            hosts[0]
                .get("containers")
                .and_then(serde_json::Value::as_array)
                .and_then(|containers| containers.first())
                .and_then(|container| container.get("runtimes"))
                .and_then(serde_json::Value::as_array)
        })
        .and_then(|runtimes| runtimes.first())
        .and_then(|runtime| runtime.get("endpoints"))
        .and_then(serde_json::Value::as_array)
        .expect("endpoint array");
    let endpoint = |protocol: &str| {
        endpoints
            .iter()
            .find(|endpoint| {
                endpoint.get("protocol").and_then(serde_json::Value::as_str) == Some(protocol)
            })
            .unwrap_or_else(|| panic!("missing endpoint {protocol}"))
    };

    assert_eq!(
        endpoint("web")
            .get("health")
            .and_then(serde_json::Value::as_str),
        Some("configured_policy")
    );
    assert_eq!(
        endpoint("opcua")
            .get("health")
            .and_then(serde_json::Value::as_str),
        Some("configured_policy")
    );
}

#[test]
fn ads_import_symbols_control_request_shapes_cached_snapshot() {
    let source = r#"
PROGRAM Main
VAR
    run : BOOL := TRUE;
END_VAR
END_PROGRAM
"#;
    let state = hmi_test_state(source);

    let response = handle_request_value(
        json!({
            "id": 51,
            "type": "ads.import_symbols",
            "params": ads_import_symbols_params(),
        }),
        &state,
        None,
    );

    assert!(
        response.ok,
        "ads.import_symbols failed: {:?}",
        response.error
    );
    let result = response.result.expect("import-symbols result");
    assert_eq!(
        result
            .get("connection_name")
            .and_then(serde_json::Value::as_str),
        Some("line1")
    );
    assert_eq!(
        result
            .get("snapshot")
            .and_then(|snapshot| snapshot.get("route_name"))
            .and_then(serde_json::Value::as_str),
        Some("line1")
    );
    let candidates = result
        .get("candidates")
        .and_then(serde_json::Value::as_array)
        .expect("candidates");
    assert_eq!(candidates.len(), 2);
    assert_eq!(
        candidates[0]
            .get("suggested_var")
            .and_then(serde_json::Value::as_str),
        Some("line1_gvl_lineready")
    );
    assert_eq!(
        candidates[1]
            .get("suggested_var")
            .and_then(serde_json::Value::as_str),
        Some("line1_main_temperature")
    );
    assert_eq!(
        candidates[1]
            .get("access")
            .and_then(serde_json::Value::as_str),
        Some("read")
    );
    assert_eq!(
        candidates[1]
            .get("mode")
            .and_then(serde_json::Value::as_str),
        Some("poll")
    );
    assert_eq!(
        candidates[1]
            .get("selected")
            .and_then(serde_json::Value::as_bool),
        Some(true)
    );
    assert_eq!(
        candidates[0]
            .get("selected")
            .and_then(serde_json::Value::as_bool),
        Some(false)
    );
    assert_eq!(
        result
            .get("groups")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(2)
    );
}

#[test]
fn ads_import_symbols_rejects_snapshot_for_different_connection() {
    let source = r#"
PROGRAM Main
VAR
    run : BOOL := TRUE;
END_VAR
END_PROGRAM
"#;
    let state = hmi_test_state(source);
    let mut params = ads_import_symbols_params();
    params
        .get_mut("snapshot")
        .and_then(serde_json::Value::as_object_mut)
        .expect("snapshot object")
        .insert("route_name".to_string(), json!("other"));

    let response = handle_request_value(
        json!({
            "id": 52,
            "type": "ads.import_symbols",
            "params": params,
        }),
        &state,
        None,
    );

    assert!(!response.ok);
    assert!(response
        .error
        .as_deref()
        .unwrap_or_default()
        .contains("does not match import connection"));
}

#[cfg(not(feature = "ads-wire"))]
#[test]
fn ads_import_symbols_live_without_wire_reports_requirement() {
    let source = r#"
PROGRAM Main
VAR
    run : BOOL := TRUE;
END_VAR
END_PROGRAM
"#;
    let state = hmi_test_state(source);

    let response = handle_request_value(
        json!({
            "id": 53,
            "type": "ads.import_symbols",
            "params": {
                "connection_name": "line1",
                "target": {
                    "name": "CX-1234",
                    "ip": "192.168.10.5",
                    "ams_net_id": "5.23.91.12.1.1",
                    "ams_port": 851,
                    "tc_version": "3.1.4024",
                }
            },
        }),
        &state,
        None,
    );

    assert!(!response.ok);
    assert!(response
        .error
        .as_deref()
        .unwrap_or_default()
        .contains("ads-wire build"));
}

#[test]
fn ads_control_requests_reject_missing_params_for_parameterized_commands() {
    let source = r#"
PROGRAM Main
VAR
    run : BOOL := TRUE;
END_VAR
END_PROGRAM
"#;
    let state = hmi_test_state(source);

    for kind in [
        "ads.discover",
        "ads.identity",
        "ads.doctor",
        "ads.doctor.start",
        "ads.doctor.status",
        "ads.route_plan",
        "ads.route_add",
        "ads.route_remove",
        "ads.import_symbols",
        "ads.server.doctor.status",
        "ads.server.route_plan",
    ] {
        let response = handle_request_value(json!({"id": 54, "type": kind}), &state, None);
        assert!(!response.ok, "{kind} without params should fail");
        assert_eq!(
            response.error.as_deref(),
            Some("missing params"),
            "{kind} should use the shared missing-params contract"
        );
    }
}

#[test]
fn ads_identity_control_request_rejects_invalid_target_ip() {
    let source = r#"
PROGRAM Main
VAR
    run : BOOL := TRUE;
END_VAR
END_PROGRAM
"#;
    let state = hmi_test_state(source);

    let response = handle_request_value(
        json!({
            "id": 55,
            "type": "ads.identity",
            "params": { "target_ip": "not-an-ip" },
        }),
        &state,
        None,
    );

    assert!(!response.ok);
    assert!(response
        .error
        .as_deref()
        .unwrap_or_default()
        .contains("invalid target IP"));
}

#[cfg(not(feature = "ads-wire"))]
#[test]
fn ads_doctor_control_request_reports_wire_requirement_without_ads_wire() {
    let source = r#"
PROGRAM Main
VAR
    run : BOOL := TRUE;
END_VAR
END_PROGRAM
"#;
    let state = hmi_test_state(source);

    let response = handle_request_value(
        json!({
            "id": 47,
            "type": "ads.doctor",
            "params": ads_doctor_params(false),
        }),
        &state,
        None,
    );

    assert!(!response.ok);
    assert!(response
        .error
        .as_deref()
        .unwrap_or_default()
        .contains("ads-wire build"));
}

#[test]
fn ads_doctor_status_reports_unknown_job() {
    let source = r#"
PROGRAM Main
VAR
    run : BOOL := TRUE;
END_VAR
END_PROGRAM
"#;
    let state = hmi_test_state(source);

    let response = handle_request_value(
        json!({
            "id": 48,
            "type": "ads.doctor.status",
            "params": { "job_id": "missing" },
        }),
        &state,
        None,
    );

    assert!(!response.ok);
    assert_eq!(
        response.error_code.as_deref(),
        Some("ads_doctor_job_not_found")
    );
}

#[cfg(not(feature = "ads-wire"))]
#[test]
fn ads_doctor_start_and_status_expose_failed_job_without_ads_wire() {
    let source = r#"
PROGRAM Main
VAR
    run : BOOL := TRUE;
END_VAR
END_PROGRAM
"#;
    let state = hmi_test_state(source);

    let start = handle_request_value(
        json!({
            "id": 49,
            "type": "ads.doctor.start",
            "params": ads_doctor_params(false),
        }),
        &state,
        None,
    );
    assert!(start.ok, "ads.doctor.start failed: {:?}", start.error);
    let job_id = start
        .result
        .as_ref()
        .and_then(|result| result.get("job_id"))
        .and_then(serde_json::Value::as_str)
        .expect("job id")
        .to_string();

    let mut final_status = None;
    for _ in 0..30 {
        let status = handle_request_value(
            json!({
                "id": 50,
                "type": "ads.doctor.status",
                "params": { "job_id": job_id },
            }),
            &state,
            None,
        );
        assert!(status.ok, "ads.doctor.status failed: {:?}", status.error);
        let result = status.result.expect("job status");
        if result
            .get("state")
            .and_then(serde_json::Value::as_str)
            == Some("failed")
        {
            final_status = Some(result);
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    let final_status = final_status.expect("doctor job should fail without ads-wire");
    assert!(final_status
        .get("error")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .contains("ads-wire build"));
}

#[test]
fn ads_route_remove_returns_removal_artifact() {
    let source = r#"
PROGRAM Main
VAR
    run : BOOL := TRUE;
END_VAR
END_PROGRAM
"#;
    let state = hmi_test_state(source);

    let response = handle_request_value(
        json!({
            "id": 46,
            "type": "ads.route_remove",
            "params": ads_route_remove_params(),
        }),
        &state,
        None,
    );

    assert!(response.ok, "ads.route_remove failed: {:?}", response.error);
    let result = response.result.expect("ads.route_remove result");
    assert_eq!(
        result.get("status").and_then(serde_json::Value::as_str),
        Some("artifact")
    );
    assert_eq!(
        result
            .get("artifact")
            .and_then(|artifact| artifact.get("kind"))
            .and_then(serde_json::Value::as_str),
        Some("removal_powershell")
    );
    assert!(result
        .get("artifact")
        .and_then(|artifact| artifact.get("content"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .contains("Removed ADS route"));
}

#[test]
fn ads_discover_control_request_supports_manual_target_without_broadcast() {
    let source = r#"
PROGRAM Main
VAR
    run : BOOL := TRUE;
END_VAR
END_PROGRAM
"#;
    let state = hmi_test_state(source);

    let response = handle_request_value(
        json!({
            "id": 40,
            "type": "ads.discover",
            "params": ads_discover_manual_params(),
        }),
        &state,
        None,
    );

    assert!(response.ok, "ads.discover failed: {:?}", response.error);
    let result = response.result.expect("ads.discover result");
    let targets = result.as_array().expect("discovery results");
    assert_eq!(targets.len(), 1);
    assert_eq!(
        targets[0]
            .get("source")
            .and_then(serde_json::Value::as_str),
        Some("manual")
    );
    assert_eq!(
        targets[0]
            .get("target")
            .and_then(|target| target.get("ams_net_id"))
            .and_then(serde_json::Value::as_str),
        Some("5.23.91.12.1.1")
    );
}

#[cfg(not(feature = "ads-wire"))]
#[test]
fn ads_discover_control_request_reports_wire_requirement_without_manual_ams_id() {
    let source = r#"
PROGRAM Main
VAR
    run : BOOL := TRUE;
END_VAR
END_PROGRAM
"#;
    let state = hmi_test_state(source);

    let response = handle_request_value(
        json!({
            "id": 40,
            "type": "ads.discover",
            "params": { "target": "192.168.10.5" },
        }),
        &state,
        None,
    );

    assert!(!response.ok);
    assert!(response
        .error
        .as_deref()
        .unwrap_or_default()
        .contains("ads-wire build"));
}

#[test]
fn ads_route_add_rejects_untrusted_channel_without_echoing_secret() {
    let source = r#"
PROGRAM Main
VAR
    run : BOOL := TRUE;
END_VAR
END_PROGRAM
"#;
    let state = hmi_test_state(source);

    let response = handle_request_value(
        json!({
            "id": 44,
            "type": "ads.route_add",
            "params": ads_route_add_params("untrusted_remote_plain_tcp"),
        }),
        &state,
        None,
    );

    assert!(!response.ok);
    assert_eq!(
        response.error_code.as_deref(),
        Some("untrusted_credential_channel")
    );
    let serialized = serde_json::to_string(&response).expect("serialize response");
    assert!(!serialized.contains("not-persisted"));
}

#[cfg(not(feature = "ads-wire"))]
#[test]
fn ads_route_add_trusted_channel_reports_wire_requirement_without_echoing_secret() {
    let source = r#"
PROGRAM Main
VAR
    run : BOOL := TRUE;
END_VAR
END_PROGRAM
"#;
    let state = hmi_test_state(source);

    let response = handle_request_value(
        json!({
            "id": 45,
            "type": "ads.route_add",
            "params": ads_route_add_params("trusted_same_host"),
        }),
        &state,
        None,
    );

    assert!(!response.ok);
    assert!(response
        .error
        .as_deref()
        .unwrap_or_default()
        .contains("ads-wire build"));
    let serialized = serde_json::to_string(&response).expect("serialize response");
    assert!(!serialized.contains("not-persisted"));
}

#[test]
fn ads_identity_control_request_derives_runtime_host_source_identity() {
    let source = r#"
PROGRAM Main
VAR
    run : BOOL := TRUE;
END_VAR
END_PROGRAM
"#;
    let state = hmi_test_state(source);

    let response = handle_request_value(
        json!({
            "id": 41,
            "type": "ads.identity",
            "params": { "target_ip": "127.0.0.1" },
        }),
        &state,
        None,
    );

    assert!(response.ok, "ads.identity failed: {:?}", response.error);
    let result = response.result.expect("ads.identity result");
    assert_eq!(
        result.get("chosen_ip").and_then(serde_json::Value::as_str),
        Some("127.0.0.1")
    );
    assert_eq!(
        result
            .get("ams_net_id")
            .and_then(serde_json::Value::as_str),
        Some("127.0.0.1.1.1")
    );
    assert_eq!(
        result
            .get("classification")
            .and_then(serde_json::Value::as_str),
        Some("loopback")
    );
}

#[test]
fn ads_status_control_request_returns_runtime_ads_status_schema() {
    let source = r#"
PROGRAM Main
VAR
    run : BOOL := TRUE;
END_VAR
END_PROGRAM
"#;
    let state = hmi_test_state(source);

    let response = handle_request_value(json!({"id": 42, "type": "ads.status"}), &state, None);

    assert!(response.ok, "ads.status failed: {:?}", response.error);
    let result = response.result.expect("ads.status result");
    assert_eq!(
        result
            .get("schema_version")
            .and_then(serde_json::Value::as_u64),
        Some(2)
    );
    assert_eq!(
        result.get("role").and_then(serde_json::Value::as_str),
        Some("client")
    );
    assert_eq!(
        result.get("overall").and_then(serde_json::Value::as_str),
        Some("disabled")
    );
    assert_eq!(
        result
            .get("connections")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(0)
    );
}

#[test]
fn ads_route_plan_control_request_returns_runtime_identity_artifacts() {
    let source = r#"
PROGRAM Main
VAR
    run : BOOL := TRUE;
END_VAR
END_PROGRAM
"#;
    let state = hmi_test_state(source);

    let response = handle_request_value(
        json!({
            "id": 43,
            "type": "ads.route_plan",
            "params": ads_route_plan_params("untrusted_remote_plain_tcp"),
        }),
        &state,
        None,
    );

    assert!(response.ok, "ads.route_plan failed: {:?}", response.error);
    let result = response.result.expect("ads.route_plan result");
    assert_eq!(
        result
            .get("automatic_route")
            .and_then(serde_json::Value::as_str),
        Some("disabled_untrusted_channel")
    );
    assert_eq!(
        result
            .get("local")
            .and_then(|local| local.get("ams_net_id"))
            .and_then(serde_json::Value::as_str),
        Some("192.168.10.20.1.1")
    );
    let artifacts = result
        .get("artifacts")
        .and_then(serde_json::Value::as_array)
        .expect("route artifacts");
    assert_eq!(artifacts.len(), 4);
    assert!(artifacts.iter().any(|artifact| artifact
        .get("kind")
        .and_then(serde_json::Value::as_str)
        == Some("static_routes_xml")));
}

#[cfg(feature = "ads-server")]
#[test]
fn ads_server_status_control_request_returns_server_surface() {
    let state = hmi_test_state(ads_server_runtime_source());
    *state.ads_server_config.lock().expect("ads server config") =
        Some(ads_server_runtime_config());

    let response = handle_request_value(json!({"id": 44, "type": "ads.server.status"}), &state, None);

    assert!(response.ok, "ads.server.status failed: {:?}", response.error);
    let result = response.result.expect("ads.server.status result");
    assert_eq!(result.get("role").and_then(serde_json::Value::as_str), Some("server"));
    assert_eq!(
        result
            .get("ams_net_id")
            .and_then(serde_json::Value::as_str),
        Some("127.0.0.1.1.1")
    );
    assert_eq!(
        result
            .get("exposed_count")
            .and_then(serde_json::Value::as_u64),
        Some(1)
    );
    assert_eq!(
        result
            .get("writable_count")
            .and_then(serde_json::Value::as_u64),
        Some(1)
    );
}

#[cfg(feature = "ads-server")]
#[test]
fn ads_server_symbols_control_request_returns_exposed_snapshot() {
    let state = hmi_test_state(ads_server_runtime_source());
    *state.ads_server_config.lock().expect("ads server config") =
        Some(ads_server_runtime_config());

    let response = handle_request_value(json!({"id": 45, "type": "ads.server.symbols"}), &state, None);

    assert!(response.ok, "ads.server.symbols failed: {:?}", response.error);
    let result = response.result.expect("ads.server.symbols result");
    assert_eq!(
        result
            .get("symbols")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(1)
    );
}

#[test]
fn ads_server_route_plan_uses_server_wording() {
    let source = r#"
PROGRAM Main
VAR
    run : BOOL := TRUE;
END_VAR
END_PROGRAM
"#;
    let state = hmi_test_state(source);

    let response = handle_request_value(
        json!({
            "id": 46,
            "type": "ads.server.route_plan",
            "params": ads_route_plan_params("trusted_same_host"),
        }),
        &state,
        None,
    );

    assert!(response.ok, "ads.server.route_plan failed: {:?}", response.error);
    let result = response.result.expect("ads.server.route_plan result");
    let manual = result
        .get("artifacts")
        .and_then(serde_json::Value::as_array)
        .expect("artifacts")
        .iter()
        .find(|artifact| {
            artifact
                .get("kind")
                .and_then(serde_json::Value::as_str)
                == Some("manual_steps")
        })
        .and_then(|artifact| artifact.get("content"))
        .and_then(serde_json::Value::as_str)
        .expect("manual content");
    assert!(manual.contains("truST ADS server"));
    assert!(!manual.contains("ADS Error 1861"));
}

#[cfg(feature = "ads-server")]
#[test]
fn ads_server_doctor_control_request_returns_server_report_without_external_proof() {
    let state = hmi_test_state(ads_server_runtime_source());
    *state.ads_server_config.lock().expect("ads server config") =
        Some(ads_server_runtime_config());

    let response = handle_request_value(
        json!({
            "id": 47,
            "type": "ads.server.doctor",
            "params": {},
        }),
        &state,
        None,
    );

    assert!(response.ok, "ads.server.doctor failed: {:?}", response.error);
    let result = response.result.expect("ads.server.doctor result");
    assert_eq!(result.get("role").and_then(serde_json::Value::as_str), Some("server"));
    assert_eq!(
        result
            .get("production_ready")
            .and_then(serde_json::Value::as_bool),
        Some(false)
    );
    assert!(
        !result
            .get("evidence")
            .and_then(|evidence| evidence.get("external_client_verified"))
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false)
    );
}

#[cfg(feature = "ads-server")]
#[test]
fn ads_server_status_reflects_last_external_doctor_evidence_without_overclaiming() {
    let state = hmi_test_state(ads_server_runtime_source());
    *state.ads_server_config.lock().expect("ads server config") =
        Some(ads_server_runtime_config());

    let doctor = handle_request_value(
        json!({
            "id": 147,
            "type": "ads.server.doctor",
            "params": {
                "external_client": {
                    "kind": "pyads",
                    "name": "ci-pyads",
                    "timestamp_ms": 1781234567999_u64
                }
            },
        }),
        &state,
        None,
    );
    assert!(doctor.ok, "ads.server.doctor failed: {:?}", doctor.error);
    assert_eq!(
        doctor
            .result
            .as_ref()
            .and_then(|report| report.get("production_ready"))
            .and_then(serde_json::Value::as_bool),
        Some(false)
    );

    let status = handle_request_value(
        json!({
            "id": 148,
            "type": "ads.server.status",
        }),
        &state,
        None,
    );
    assert!(status.ok, "ads.server.status failed: {:?}", status.error);
    let result = status.result.expect("status result");
    assert_eq!(
        result
            .get("external_client_verified")
            .and_then(serde_json::Value::as_bool),
        Some(true)
    );
    assert_eq!(
        result
            .get("external_client_kind")
            .and_then(serde_json::Value::as_str),
        Some("pyads")
    );
    assert_eq!(
        result
            .get("production_ready")
            .and_then(serde_json::Value::as_bool),
        Some(false)
    );
    assert_eq!(
        result
            .get("proof_status")
            .and_then(serde_json::Value::as_str),
        Some("external_client_verified")
    );
}

#[cfg(feature = "ads-server")]
#[test]
fn ads_server_doctor_start_and_status_use_job_poll_surface() {
    let state = hmi_test_state(ads_server_runtime_source());
    *state.ads_server_config.lock().expect("ads server config") =
        Some(ads_server_runtime_config());

    let start = handle_request_value(
        json!({
            "id": 48,
            "type": "ads.server.doctor.start",
            "params": {},
        }),
        &state,
        None,
    );
    assert!(start.ok, "ads.server.doctor.start failed: {:?}", start.error);
    let job_id = start
        .result
        .as_ref()
        .and_then(|value| value.get("job_id"))
        .and_then(serde_json::Value::as_str)
        .expect("job id")
        .to_string();
    assert!(job_id.starts_with("ads-server-doctor-"));

    let mut final_status = None;
    for _ in 0..20 {
        let status = handle_request_value(
            json!({
                "id": 49,
                "type": "ads.server.doctor.status",
                "params": { "job_id": job_id },
            }),
            &state,
            None,
        );
        assert!(status.ok, "ads.server.doctor.status failed: {:?}", status.error);
        let result = status.result.expect("job status");
        if result.get("state").and_then(serde_json::Value::as_str) != Some("running") {
            final_status = Some(result);
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }

    let final_status = final_status.expect("server doctor job completed");
    assert_eq!(
        final_status
            .get("state")
            .and_then(serde_json::Value::as_str),
        Some("complete")
    );
    assert_eq!(
        final_status
            .get("report")
            .and_then(|report| report.get("role"))
            .and_then(serde_json::Value::as_str),
        Some("server")
    );
}

#[test]
fn debug_program_and_io_handlers_preserve_behavior() {
    let source = r#"
PROGRAM Main
VAR
    run : BOOL := TRUE;
END_VAR
END_PROGRAM
"#;
    let state = hmi_test_state(source);

    let pause = handle_request_value(json!({"id": 1, "type": "pause"}), &state, None);
    assert!(pause.ok, "pause should succeed: {:?}", pause.error);

    let debug_state = handle_request_value(json!({"id": 2, "type": "debug.state"}), &state, None);
    assert!(
        debug_state.ok,
        "debug.state should succeed: {:?}",
        debug_state.error
    );

    let restart = handle_request_value(
        json!({"id": 3, "type": "restart", "params": { "mode": "warm" }}),
        &state,
        None,
    );
    assert!(restart.ok, "restart should succeed: {:?}", restart.error);
    assert_eq!(
        state.pending_restart.lock().ok().and_then(|guard| *guard),
        Some(RestartMode::Warm)
    );

    let io_write = handle_request_value(
        json!({
            "id": 4,
            "type": "io.write",
            "params": { "address": "%QX0.0", "value": "true" }
        }),
        &state,
        None,
    );
    assert!(io_write.ok, "io.write should succeed: {:?}", io_write.error);
    assert_eq!(
        io_write
            .result
            .as_ref()
            .and_then(|result| result.get("status"))
            .and_then(serde_json::Value::as_str),
        Some("queued")
    );
}

#[test]
fn io_read_marks_forced_io_rows() {
    let source = r#"
PROGRAM Main
VAR
    run : BOOL := TRUE;
END_VAR
END_PROGRAM
"#;
    let state = hmi_test_state(source);
    let output = crate::io::IoAddress::parse("%QX0.0").expect("output address");
    *state.io_snapshot.lock().expect("snapshot lock") = Some(crate::io::IoSnapshot {
        inputs: Vec::new(),
        outputs: vec![crate::io::IoSnapshotEntry {
            name: Some(SmolStr::new("OUT0")),
            address: output.clone(),
            value: crate::io::IoSnapshotValue::Value(crate::value::Value::Bool(false)),
        }],
        memory: Vec::new(),
    });

    let force = handle_request_value(
        json!({
            "id": 40,
            "type": "io.force",
            "params": { "address": "%QX0.0", "value": "TRUE" }
        }),
        &state,
        None,
    );
    assert!(force.ok, "io.force should succeed: {:?}", force.error);

    let read = handle_request_value(json!({"id": 41, "type": "io.read"}), &state, None);
    assert!(read.ok, "io.read should succeed: {:?}", read.error);
    let forced = read
        .result
        .as_ref()
        .and_then(|result| result.get("snapshot"))
        .and_then(|snapshot| snapshot.get("outputs"))
        .and_then(serde_json::Value::as_array)
        .and_then(|outputs| outputs.first())
        .and_then(|entry| entry.get("forced"))
        .and_then(serde_json::Value::as_bool);
    assert_eq!(forced, Some(true), "forced row should be marked: {read:?}");

    let release = handle_request_value(
        json!({
            "id": 42,
            "type": "io.unforce",
            "params": { "address": "%QX0.0" }
        }),
        &state,
        None,
    );
    assert!(
        release.ok,
        "io.unforce should succeed: {:?}",
        release.error
    );
}

#[test]
fn status_reports_execution_backend_selection_and_metrics_tag() {
    let source = r#"
PROGRAM Main
VAR
    run : BOOL := TRUE;
END_VAR
END_PROGRAM
"#;
    let state = hmi_test_state(source);

    let status = handle_request_value(json!({"id": 30, "type": "status"}), &state, None);
    assert!(status.ok, "status should succeed: {:?}", status.error);
    let result = status.result.expect("status result");
    assert_eq!(
        result
            .get("execution_backend")
            .and_then(serde_json::Value::as_str),
        Some("vm")
    );
    assert_eq!(
        result
            .get("execution_backend_source")
            .and_then(serde_json::Value::as_str),
        Some("default")
    );
    assert_eq!(
        result
            .get("metrics")
            .and_then(|metrics| metrics.get("execution_backend"))
            .and_then(serde_json::Value::as_str),
        Some("vm")
    );
}

#[test]
fn status_and_config_get_report_same_backend_selection() {
    let source = r#"
PROGRAM Main
VAR
    run : BOOL := TRUE;
END_VAR
END_PROGRAM
"#;
    let state = hmi_test_state(source);

    let status = handle_request_value(json!({"id": 31, "type": "status"}), &state, None);
    assert!(status.ok, "status should succeed: {:?}", status.error);
    let status_result = status.result.expect("status result");
    let status_backend = status_result
        .get("execution_backend")
        .and_then(serde_json::Value::as_str)
        .expect("status execution_backend");
    let status_source = status_result
        .get("execution_backend_source")
        .and_then(serde_json::Value::as_str)
        .expect("status execution_backend_source");

    let config_get = handle_request_value(json!({"id": 32, "type": "config.get"}), &state, None);
    assert!(config_get.ok, "config.get should succeed: {:?}", config_get.error);
    let config_result = config_get.result.expect("config.get result");
    let config_backend = config_result
        .get("runtime.execution_backend")
        .and_then(serde_json::Value::as_str)
        .expect("config execution_backend");
    let config_source = config_result
        .get("runtime.execution_backend_source")
        .and_then(serde_json::Value::as_str)
        .expect("config execution_backend_source");

    assert_eq!(status_backend, config_backend);
    assert_eq!(status_source, config_source);
}

#[test]
fn runtime_status_projection_contract_reports_resource_metrics_realtime_and_io_health() {
    let source = r#"
PROGRAM Main
VAR
    run : BOOL := TRUE;
END_VAR
END_PROGRAM
"#;
    let state = hmi_test_state(source);

    {
        let mut settings = state.settings.lock().expect("settings lock");
        settings.simulation.enabled = true;
        settings.simulation.time_scale = 4;
        settings.simulation.mode_label = SmolStr::new("accelerated");
        settings.simulation.warning = SmolStr::new("simulation clock scaled");
    }
    {
        let mut metrics = state.metrics.lock().expect("metrics lock");
        metrics.record_cycle(Duration::from_millis(12));
        metrics.record_overrun(&SmolStr::new("Main"), 2);
        metrics.record_fault();
        metrics.record_call("function_block", &SmolStr::new("Pump"), Duration::from_millis(4));
    }
    {
        let mut realtime = state.realtime_status.lock().expect("realtime status lock");
        realtime.requested.enabled = true;
        realtime.requested.require_preempt_rt_kernel = true;
        realtime.requested.lock_memory = true;
        realtime.requested.scheduler = crate::linux_rt::LinuxRtSchedulerPolicy::Fifo;
        realtime.requested.priority = 80;
        realtime.requested.cpu_affinity = vec![1, 2];
        realtime.requested.strict = true;
        realtime.kernel_realtime = Some(true);
        realtime.active_scheduler = Some(crate::linux_rt::LinuxRtSchedulerPolicy::Fifo);
        realtime.active_priority = Some(80);
        realtime.active_cpu_affinity = vec![1, 2];
        realtime.memory_locked_kb = Some(4096);
        realtime.memory_lock_applied = true;
        realtime.affinity_applied_by_runtime = true;
        realtime.scheduler_applied_by_runtime = true;
        realtime.active = true;
        realtime.warnings = vec![SmolStr::new("rt warning")];
        realtime.errors = vec![SmolStr::new("rt error")];
    }
    state
        .io_health
        .lock()
        .expect("io health lock")
        .extend([
            crate::io::IoDriverStatus {
                name: SmolStr::new("fieldbus"),
                health: crate::io::IoDriverHealth::Ok,
            },
            crate::io::IoDriverStatus {
                name: SmolStr::new("simulated"),
                health: crate::io::IoDriverHealth::Degraded {
                    error: SmolStr::new("slow cycle"),
                },
            },
        ]);

    let status = handle_request_value(json!({"id": 33, "type": "status"}), &state, None);
    assert!(status.ok, "status should succeed: {:?}", status.error);
    let result = status.result.expect("status result");

    assert_eq!(
        result.get("state").and_then(serde_json::Value::as_str),
        Some("ready")
    );
    assert_eq!(
        result.get("resource").and_then(serde_json::Value::as_str),
        Some("RESOURCE")
    );
    assert_eq!(
        result.get("plc_name").and_then(serde_json::Value::as_str),
        Some("RESOURCE")
    );
    assert_eq!(
        result.get("control_mode").and_then(serde_json::Value::as_str),
        Some("debug")
    );
    assert_eq!(
        result
            .get("simulation_mode")
            .and_then(serde_json::Value::as_str),
        Some("accelerated")
    );
    assert_eq!(
        result
            .get("simulation_enabled")
            .and_then(serde_json::Value::as_bool),
        Some(true)
    );
    assert_eq!(
        result
            .get("simulation_time_scale")
            .and_then(serde_json::Value::as_u64),
        Some(4)
    );
    assert_eq!(
        result
            .get("simulation_warning")
            .and_then(serde_json::Value::as_str),
        Some("simulation clock scaled")
    );

    let metrics = result.get("metrics").expect("metrics object");
    assert_eq!(
        metrics
            .get("cycle_ms")
            .and_then(|cycle| cycle.get("last"))
            .and_then(serde_json::Value::as_f64),
        Some(12.0)
    );
    assert_eq!(
        metrics
            .get("cycle_ms")
            .and_then(|cycle| cycle.get("window_samples"))
            .and_then(serde_json::Value::as_u64),
        Some(1)
    );
    assert_eq!(
        metrics.get("overruns").and_then(serde_json::Value::as_u64),
        Some(2)
    );
    assert_eq!(
        metrics.get("faults").and_then(serde_json::Value::as_u64),
        Some(1)
    );
    assert_eq!(
        metrics
            .get("profiling")
            .and_then(|profiling| profiling.get("enabled"))
            .and_then(serde_json::Value::as_bool),
        Some(true)
    );
    assert_eq!(
        metrics
            .get("profiling")
            .and_then(|profiling| profiling.get("top"))
            .and_then(serde_json::Value::as_array)
            .and_then(|top| top.first())
            .and_then(|entry| entry.get("key"))
            .and_then(serde_json::Value::as_str),
        Some("function_block:Pump")
    );

    let realtime = result.get("realtime").expect("realtime object");
    assert_eq!(
        realtime.get("profile").and_then(serde_json::Value::as_str),
        Some("preempt-rt")
    );
    assert_eq!(
        realtime
            .get("requested")
            .and_then(|requested| requested.get("scheduler"))
            .and_then(serde_json::Value::as_str),
        Some("fifo")
    );
    assert_eq!(
        realtime
            .get("observed")
            .and_then(|observed| observed.get("scheduler"))
            .and_then(serde_json::Value::as_str),
        Some("fifo")
    );
    assert_eq!(
        realtime
            .get("observed")
            .and_then(|observed| observed.get("memory_locked_kb"))
            .and_then(serde_json::Value::as_u64),
        Some(4096)
    );
    assert_eq!(
        realtime.get("active").and_then(serde_json::Value::as_bool),
        Some(true)
    );
    assert_eq!(
        realtime
            .get("warnings")
            .and_then(serde_json::Value::as_array)
            .and_then(|items| items.first())
            .and_then(serde_json::Value::as_str),
        Some("rt warning")
    );
    assert_eq!(
        realtime
            .get("errors")
            .and_then(serde_json::Value::as_array)
            .and_then(|items| items.first())
            .and_then(serde_json::Value::as_str),
        Some("rt error")
    );

    let io_drivers = result
        .get("io_drivers")
        .and_then(serde_json::Value::as_array)
        .expect("io driver statuses");
    assert_eq!(io_drivers.len(), 2);
    assert_eq!(
        io_drivers[0].get("status").and_then(serde_json::Value::as_str),
        Some("ok")
    );
    assert_eq!(
        io_drivers[1].get("status").and_then(serde_json::Value::as_str),
        Some("degraded")
    );
    assert_eq!(
        io_drivers[1].get("error").and_then(serde_json::Value::as_str),
        Some("slow cycle")
    );
}

#[test]
fn runtime_health_projection_contract_marks_faulted_driver_unhealthy() {
    let source = r#"
PROGRAM Main
VAR
    run : BOOL := TRUE;
END_VAR
END_PROGRAM
"#;
    let state = hmi_test_state(source);
    state
        .io_health
        .lock()
        .expect("io health lock")
        .push(crate::io::IoDriverStatus {
            name: SmolStr::new("fieldbus"),
            health: crate::io::IoDriverHealth::Faulted {
                error: SmolStr::new("wire break"),
            },
        });

    let health = handle_request_value(json!({"id": 34, "type": "health"}), &state, None);
    assert!(health.ok, "health should succeed: {:?}", health.error);
    let result = health.result.expect("health result");

    assert_eq!(
        result.get("ok").and_then(serde_json::Value::as_bool),
        Some(false)
    );
    assert_eq!(
        result.get("state").and_then(serde_json::Value::as_str),
        Some("ready")
    );
    assert!(result.get("fault").is_some_and(serde_json::Value::is_null));
    assert_eq!(
        result
            .get("io_drivers")
            .and_then(serde_json::Value::as_array)
            .and_then(|drivers| drivers.first())
            .and_then(|driver| driver.get("status"))
            .and_then(serde_json::Value::as_str),
        Some("faulted")
    );
    assert_eq!(
        result
            .get("io_drivers")
            .and_then(serde_json::Value::as_array)
            .and_then(|drivers| drivers.first())
            .and_then(|driver| driver.get("error"))
            .and_then(serde_json::Value::as_str),
        Some("wire break")
    );
}

#[test]
fn config_set_reports_field_level_diagnostics_for_unknown_and_type_errors() {
    let source = r#"
PROGRAM Main
VAR
    run : BOOL := TRUE;
END_VAR
END_PROGRAM
"#;
    let state = hmi_test_state(source);

    let unknown = handle_request_value(
        json!({
            "id": 20,
            "type": "config.set",
            "params": { "unknown.key": true }
        }),
        &state,
        None,
    );
    assert!(!unknown.ok);
    assert_eq!(
        unknown.error.as_deref(),
        Some("unknown config key 'unknown.key'")
    );

    let invalid_type = handle_request_value(
        json!({
            "id": 21,
            "type": "config.set",
            "params": { "web.enabled": "yes" }
        }),
        &state,
        None,
    );
    assert!(!invalid_type.ok);
    assert!(invalid_type
        .error
        .as_deref()
        .unwrap_or_default()
        .contains("invalid config value for 'web.enabled': expected boolean"));

    let valid_extended_transport = handle_request_value(
        json!({
            "id": 22,
            "type": "config.set",
            "params": {
                "runtime_cloud.links.transports": [
                    {
                        "source": "runtime-a",
                        "target": "runtime-b",
                        "transport": "mqtt"
                    }
                ]
            }
        }),
        &state,
        None,
    );
    assert!(
        valid_extended_transport.ok,
        "extended runtime cloud transport must be accepted"
    );

    let invalid_transport = handle_request_value(
        json!({
            "id": 23,
            "type": "config.set",
            "params": {
                "runtime_cloud.links.transports": [
                    {
                        "source": "runtime-a",
                        "target": "runtime-b",
                        "transport": "udp"
                    }
                ]
            }
        }),
        &state,
        None,
    );
    assert!(!invalid_transport.ok);
    assert!(invalid_transport
        .error
        .as_deref()
        .unwrap_or_default()
        .contains("invalid runtime.cloud.links.transports[].transport 'udp'"));
}

#[test]
fn config_set_reports_cross_field_auth_diagnostic() {
    let source = r#"
PROGRAM Main
VAR
    run : BOOL := TRUE;
END_VAR
END_PROGRAM
"#;
    let state = hmi_test_state(source);
    let response = handle_request_value(
        json!({
            "id": 22,
            "type": "config.set",
            "params": { "web.auth": "token" }
        }),
        &state,
        None,
    );
    assert!(!response.ok);
    assert!(response
        .error
        .as_deref()
        .unwrap_or_default()
        .contains("invalid config value for 'web.auth': token mode requires control.auth_token"));
}

#[test]
fn config_set_rejects_runtime_backend_switch_during_live_control() {
    let source = r#"
PROGRAM Main
VAR
    run : BOOL := TRUE;
END_VAR
END_PROGRAM
"#;
    let state = hmi_test_state(source);

    let response = handle_request_value(
        json!({
            "id": 24,
            "type": "config.set",
            "params": { "runtime.execution_backend": "vm" }
        }),
        &state,
        None,
    );
    assert!(!response.ok);
    assert_eq!(
        response.error.as_deref(),
        Some(
            "runtime.execution_backend is startup-only; change it via runtime.toml/service posture and restart"
        )
    );

    let status = handle_request_value(json!({"id": 25, "type": "status"}), &state, None);
    assert!(status.ok, "status should succeed: {:?}", status.error);
    let result = status.result.expect("status result");
    assert_eq!(
        result
            .get("execution_backend")
            .and_then(serde_json::Value::as_str),
        Some("vm")
    );
    assert_eq!(
        result
            .get("realtime")
            .and_then(|value| value.get("profile"))
            .and_then(serde_json::Value::as_str),
        Some("disabled")
    );
}

#[test]
fn status_and_config_get_surface_realtime_defaults() {
    let source = r#"
PROGRAM Main
END_PROGRAM
"#;
    let state = hmi_test_state(source);

    let status = handle_request_value(json!({"id": 26, "type": "status"}), &state, None);
    assert!(status.ok, "status should succeed: {:?}", status.error);
    let status_result = status.result.expect("status result");
    assert_eq!(
        status_result
            .get("realtime")
            .and_then(|value| value.get("enabled"))
            .and_then(serde_json::Value::as_bool),
        Some(false)
    );
    assert_eq!(
        status_result
            .get("realtime")
            .and_then(|value| value.get("requested"))
            .and_then(|value| value.get("scheduler"))
            .and_then(serde_json::Value::as_str),
        Some("other")
    );

    let config_get = handle_request_value(json!({"id": 27, "type": "config.get"}), &state, None);
    assert!(config_get.ok, "config.get should succeed: {:?}", config_get.error);
    let config_result = config_get.result.expect("config.get result");
    assert_eq!(
        config_result
            .get("realtime.profile")
            .and_then(serde_json::Value::as_str),
        Some("disabled")
    );
    assert_eq!(
        config_result
            .get("realtime.scheduler")
            .and_then(serde_json::Value::as_str),
        Some("other")
    );
}

#[test]
fn invalid_and_malformed_requests_return_negative_responses() {
    let source = r#"
PROGRAM Main
VAR
    run : BOOL := TRUE;
END_VAR
END_PROGRAM
"#;
    let state = hmi_test_state(source);

    let invalid_line = handle_request_line("{invalid-json", &state, None)
        .expect("invalid request should still return response line");
    let invalid_json: serde_json::Value =
        serde_json::from_str(&invalid_line).expect("parse invalid response");
    let invalid_error = invalid_json
        .get("error")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    assert!(invalid_error.starts_with("invalid request:"));

    let unsupported =
        handle_request_value(json!({"id": 10, "type": "does.not.exist"}), &state, None);
    assert!(!unsupported.ok);
    assert_eq!(unsupported.error.as_deref(), Some("unsupported request"));

    let malformed_io = handle_request_value(
        json!({"id": 11, "type": "io.write", "params": { "address": "%QX0.0" }}),
        &state,
        None,
    );
    assert!(!malformed_io.ok);
    assert!(malformed_io
        .error
        .as_deref()
        .unwrap_or_default()
        .contains("invalid params"));

    let invalid_restart = handle_request_value(
        json!({"id": 12, "type": "restart", "params": { "mode": "sideways" }}),
        &state,
        None,
    );
    assert!(!invalid_restart.ok);
    assert_eq!(
        invalid_restart.error.as_deref(),
        Some("invalid restart mode")
    );
}

#[test]
fn rbac_authorization_matrix_enforces_sensitive_endpoint_roles() {
    let source = r#"
PROGRAM Main
VAR
    run : BOOL := TRUE;
END_VAR
END_PROGRAM
"#;
    let mut state = hmi_test_state(source);
    state.auth_token = Arc::new(Mutex::new(Some(SmolStr::new("admin-token"))));
    state.control_requires_auth = true;
    let pairing_path = pairing_file("matrix");
    let store = Arc::new(PairingStore::load(pairing_path.clone()));
    state.pairing = Some(store.clone());

    let viewer_code = store.start_pairing();
    let viewer_token = store
        .claim(&viewer_code.code, Some(AccessRole::Viewer))
        .expect("viewer token");
    let operator_code = store.start_pairing();
    let operator_token = store
        .claim(&operator_code.code, Some(AccessRole::Operator))
        .expect("operator token");
    let engineer_code = store.start_pairing();
    let engineer_token = store
        .claim(&engineer_code.code, Some(AccessRole::Engineer))
        .expect("engineer token");

    let viewer_status = handle_request_value(
        json!({"id": 50, "type": "status", "auth": viewer_token}),
        &state,
        None,
    );
    assert!(viewer_status.ok, "viewer should read status");

    let viewer_restart = handle_request_value(
        json!({"id": 51, "type": "restart", "auth": viewer_token, "params": {"mode": "warm"}}),
        &state,
        None,
    );
    assert!(!viewer_restart.ok, "viewer must not restart runtime");
    assert!(viewer_restart
        .error
        .as_deref()
        .is_some_and(|msg| msg.contains("requires role operator")));

    let operator_restart = handle_request_value(
        json!({"id": 52, "type": "restart", "auth": operator_token, "params": {"mode": "warm"}}),
        &state,
        None,
    );
    assert!(operator_restart.ok, "operator should restart runtime");

    let operator_config = handle_request_value(
        json!({"id": 53, "type": "config.set", "auth": operator_token, "params": {"log.level": "debug"}}),
        &state,
        None,
    );
    assert!(!operator_config.ok, "operator must not write config");
    assert!(operator_config
        .error
        .as_deref()
        .is_some_and(|msg| msg.contains("requires role engineer")));

    let operator_hmi_write = handle_request_value(
        json!({
            "id": 531,
            "type": "hmi.write",
            "auth": operator_token,
            "params": { "id": "resource/RESOURCE/program/Main/field/run", "value": false }
        }),
        &state,
        None,
    );
    assert!(
        !operator_hmi_write.ok,
        "operator must not write HMI targets"
    );
    assert!(operator_hmi_write
        .error
        .as_deref()
        .is_some_and(|msg| msg.contains("requires role engineer")));

    let engineer_write = handle_request_value(
        json!({
            "id": 54,
            "type": "io.write",
            "auth": engineer_token,
            "params": { "address": "%QX0.0", "value": "true" }
        }),
        &state,
        None,
    );
    assert!(engineer_write.ok, "engineer should write I/O");

    let engineer_hmi_write = handle_request_value(
        json!({
            "id": 541,
            "type": "hmi.write",
            "auth": engineer_token,
            "params": { "id": "resource/RESOURCE/program/Main/field/run", "value": false }
        }),
        &state,
        None,
    );
    assert!(
        !engineer_hmi_write.ok,
        "engineer write should still be gated by read-only defaults"
    );
    assert_eq!(
        engineer_hmi_write.error.as_deref(),
        Some("hmi.write disabled in read-only mode")
    );

    let engineer_pair_start = handle_request_value(
        json!({"id": 55, "type": "pair.start", "auth": engineer_token}),
        &state,
        None,
    );
    assert!(!engineer_pair_start.ok, "engineer must not start pairing");
    assert!(engineer_pair_start
        .error
        .as_deref()
        .is_some_and(|msg| msg.contains("requires role admin")));

    let admin_set_auth = handle_request_value(
        json!({
            "id": 56,
            "type": "config.set",
            "auth": "admin-token",
            "params": { "control.auth_token": "new-admin-token" }
        }),
        &state,
        None,
    );
    assert!(admin_set_auth.ok, "admin should update auth token");

    let unauthorized = handle_request_value(
        json!({"id": 57, "type": "status", "auth": "invalid-token"}),
        &state,
        None,
    );
    assert!(!unauthorized.ok);
    assert_eq!(unauthorized.error.as_deref(), Some("unauthorized"));

    let _ = std::fs::remove_file(pairing_path);
}

#[test]
fn unauthenticated_remote_control_defaults_to_viewer_without_admin_token() {
    let source = r#"
PROGRAM Main
VAR
    run : BOOL := TRUE;
END_VAR
END_PROGRAM
"#;
    let state = hmi_test_state(source);

    let remote_client = Some("127.0.0.1:55001");
    let status = handle_request_value(json!({"id": 901, "type": "status"}), &state, remote_client);
    assert!(status.ok, "viewer fallback should read status");

    let denied = handle_request_value(
        json!({
            "id": 902,
            "type": "config.set",
            "params": { "log.level": "debug" }
        }),
        &state,
        remote_client,
    );
    assert!(!denied.ok, "viewer fallback must not write config");
    assert!(denied
        .error
        .as_deref()
        .is_some_and(|msg| msg.contains("requires role engineer")));
}

#[test]
fn historian_query_and_alert_control_requests_return_contract_payloads() {
    let source = r#"
PROGRAM Main
VAR
    run : BOOL := TRUE;
END_VAR
END_PROGRAM
"#;
    let mut state = hmi_test_state(source);
    let history_path = temp_history_path("historian");
    let hook_path = temp_history_path("hook");
    let historian = HistorianService::new(
        HistorianConfig {
            enabled: true,
            sample_interval_ms: 1,
            mode: RecordingMode::All,
            include: Vec::new(),
            history_path: history_path.clone(),
            max_entries: 500,
            prometheus_enabled: true,
            prometheus_path: SmolStr::new("/metrics"),
            alerts: vec![AlertRule {
                name: SmolStr::new("run_high"),
                variable: SmolStr::new("Main.run"),
                above: Some(0.5),
                below: None,
                debounce_samples: 1,
                hook: Some(SmolStr::new(hook_path.to_string_lossy())),
            }],
        },
        None,
    )
    .expect("historian");
    let (snapshot_tx, snapshot_rx) = std::sync::mpsc::channel();
    state
        .resource
        .send_command(ResourceCommand::Snapshot {
            respond_to: snapshot_tx,
        })
        .expect("request runtime snapshot");
    let snapshot = snapshot_rx
        .recv_timeout(std::time::Duration::from_millis(250))
        .expect("snapshot");
    historian
        .capture_snapshot_at(&snapshot, 1_000)
        .expect("capture initial");
    state.historian = Some(historian);

    let query = handle_request_value(
        json!({ "id": 80, "type": "historian.query", "params": { "limit": 20 } }),
        &state,
        None,
    );
    assert!(
        query.ok,
        "historian.query should succeed: {:?}",
        query.error
    );
    let items = query
        .result
        .as_ref()
        .and_then(|value| value.get("items"))
        .and_then(serde_json::Value::as_array)
        .expect("items");
    assert!(!items.is_empty());

    let alerts = handle_request_value(
        json!({ "id": 81, "type": "historian.alerts", "params": { "limit": 20 } }),
        &state,
        None,
    );
    assert!(
        alerts.ok,
        "historian.alerts should succeed: {:?}",
        alerts.error
    );
    let alert_items = alerts
        .result
        .as_ref()
        .and_then(|value| value.get("items"))
        .and_then(serde_json::Value::as_array)
        .expect("alerts");
    assert!(!alert_items.is_empty());

    let _ = std::fs::remove_file(history_path);
    let _ = std::fs::remove_file(hook_path);
}

fn ads_route_plan_params(channel: &str) -> serde_json::Value {
    json!({
        "route_name": "trust-runtime-line-controller-1",
        "target": {
            "name": "CX-1234",
            "ip": "192.168.10.5",
            "ams_net_id": "5.23.91.12.1.1",
            "ams_port": 851,
            "tc_version": "3.1.4024",
        },
        "local": {
            "host_name": "line-controller-1",
            "chosen_ip": "192.168.10.20",
            "ams_net_id": "192.168.10.20.1.1",
            "nic": "eth0",
            "candidates": [],
            "classification": "lan",
        },
        "channel": channel,
    })
}

fn ads_discover_manual_params() -> serde_json::Value {
    json!({
        "target": "192.168.10.5",
        "target_ams_net_id": "5.23.91.12.1.1",
        "ams_port": 851,
        "target_name": "CX-1234",
        "include_broadcast": false,
    })
}

fn ads_route_add_params(channel: &str) -> serde_json::Value {
    let mut params = ads_route_plan_params(channel);
    let object = params.as_object_mut().expect("route params object");
    object.insert(
        "credentials".to_string(),
        json!({
            "username": "Administrator",
            "password": "not-persisted",
        }),
    );
    params
}

fn ads_route_remove_params() -> serde_json::Value {
    json!({
        "route_name": "trust-runtime-line-controller-1",
        "target": {
            "name": "CX-1234",
            "ip": "192.168.10.5",
            "ams_net_id": "5.23.91.12.1.1",
            "ams_port": 851,
            "tc_version": "3.1.4024",
        },
    })
}

fn ads_doctor_params(writes_enabled: bool) -> serde_json::Value {
    json!({
        "target_ip": "192.168.10.5",
        "target_identity": {
            "name": "CX-1234",
            "ip": "192.168.10.5",
            "ams_net_id": "5.23.91.12.1.1",
            "ams_port": 851,
            "tc_version": "3.1.4024",
        },
        "expected_target_ams_net_id": "5.23.91.12.1.1",
        "ams_port": 851,
        "local_identity": {
            "host_name": "line-controller-1",
            "chosen_ip": "192.168.10.20",
            "ams_net_id": "192.168.10.20.1.1",
            "nic": "eth0",
            "candidates": [],
            "classification": "lan",
        },
        "selected_symbol": "MAIN.Temperature",
        "writes_enabled": writes_enabled,
    })
}

fn ads_import_symbols_params() -> serde_json::Value {
    json!({
        "connection_name": "line1",
        "include_patterns": ["*Temperature"],
        "name_prefix": "line1_",
        "snapshot": {
            "schema_version": 1,
            "route_name": "line1",
            "symbols": [
                {
                    "name": "MAIN.Temperature",
                    "data_type": {
                        "source_name": "REAL",
                        "iec_type": "REAL"
                    },
                    "index_group": 16416,
                    "index_offset": 0,
                    "byte_size": 4,
                    "flags": ["read"]
                },
                {
                    "name": "GVL.LineReady",
                    "data_type": {
                        "source_name": "BOOL",
                        "iec_type": "BOOL"
                    },
                    "index_group": 16416,
                    "index_offset": 4,
                    "byte_size": 1,
                    "flags": ["read", "write"]
                }
            ]
        }
    })
}

fn ads_symbol_snapshot_value() -> serde_json::Value {
    ads_import_symbols_params()
        .get("snapshot")
        .cloned()
        .expect("snapshot fixture")
}

#[cfg(feature = "ads-server")]
fn ads_server_runtime_config() -> crate::ads::server::AdsServerRuntimeConfig {
    crate::ads::server::AdsServerRuntimeConfig {
        enabled: true,
        listen: Some(SmolStr::new("127.0.0.1")),
        ads_port: 851,
        ams_net_id: Some(trust_ads_core::AmsNetId::new("127.0.0.1.1.1")),
        insecure_transport: true,
        writes_enabled: true,
        expose: vec![SmolStr::new("global.*")],
        writable: vec![SmolStr::new("global.setpoint")],
        clients: vec![crate::ads::server::AdsServerClientConfig {
            ams_net_id: trust_ads_core::AmsNetId::new("5.23.91.12.1.1"),
            source: crate::ads::server::AdsServerSourcePin::Cidr(SmolStr::new("127.0.0.0/8")),
        }],
        ..crate::ads::server::AdsServerRuntimeConfig::default()
    }
}

#[cfg(feature = "ads-server")]
fn ads_server_runtime_source() -> &'static str {
    r#"
CONFIGURATION Config
VAR_GLOBAL
    setpoint : REAL := 12.5;
END_VAR
RESOURCE CommRes ON PLC
    TASK MainTask (INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM Main WITH MainTask : Main;
END_RESOURCE
END_CONFIGURATION
PROGRAM Main
VAR_EXTERNAL
    setpoint : REAL;
END_VAR
setpoint := setpoint;
END_PROGRAM
"#
}
