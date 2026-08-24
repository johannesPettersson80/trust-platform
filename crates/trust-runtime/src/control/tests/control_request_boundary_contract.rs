fn control_request(
    kind: &str,
    params: Option<serde_json::Value>,
) -> ControlRequest {
    ControlRequest {
        id: 7,
        r#type: kind.into(),
        params,
        auth: None,
        request_id: None,
    }
}

#[test]
fn shutdown_prepares_success_before_signalling_resource_stop() {
    let state = hmi_test_state("PROGRAM Main END_PROGRAM");

    let response = program_handlers::handle_shutdown(41, &state);

    assert!(response.ok, "shutdown response must be successful");
    assert!(
        !state.resource.stop_requested(),
        "resource stop must remain deferred until the response write boundary"
    );
}

#[test]
fn web_shutdown_dispatch_defers_resource_stop_until_transport_completion() {
    let state = hmi_test_state("PROGRAM Main END_PROGRAM");

    let response = dispatch_web_control_request_port(
        json!({"id": 42, "type": "shutdown"}),
        &state,
        Some("web"),
        None,
    );
    assert!(response.ok, "web shutdown response must be successful");
    let mut responses = [response];
    let serialized = write_then_complete_web_control_responses_port(
        &mut responses,
        &state,
        |responses| {
            assert!(
                !state.resource.stop_requested(),
                "web dispatch must not stop the resource before the response write boundary"
            );
            serde_json::to_value(&responses[0]).expect("serialized web response")
        },
    );
    assert_eq!(serialized["id"], json!(42));
    assert_eq!(serialized["result"]["status"], json!("stopping"));
    assert!(
        state.resource.stop_requested(),
        "transport completion must apply the deferred resource stop"
    );
}

#[test]
fn access_capabilities_are_exact_for_each_role() {
    for (role, mutation_allowed, reason) in [
        (
            AccessRole::Viewer,
            false,
            Some("Viewer role — connect with an engineer token to write or force."),
        ),
        (
            AccessRole::Operator,
            false,
            Some("Operator role — connect with an engineer token to write or force."),
        ),
        (AccessRole::Engineer, true, None),
        (AccessRole::Admin, true, None),
    ] {
        let access = access_capabilities_json(role);
        assert_eq!(access["role"], json!(role.as_str()));
        assert_eq!(access["io"]["write"], json!(mutation_allowed));
        assert_eq!(access["io"]["force"], json!(mutation_allowed));
        assert_eq!(access["io"]["release"], json!(mutation_allowed));
        assert_eq!(access["hmi"]["write"], json!(mutation_allowed));
        assert_eq!(access["reason"], reason.map_or(serde_json::Value::Null, |text| json!(text)));
    }
}

#[test]
fn capability_reason_capitalization_handles_empty_ascii_and_unicode() {
    assert_eq!(capitalize_role(""), "");
    assert_eq!(capitalize_role("viewer"), "Viewer");
    assert_eq!(capitalize_role("operator"), "Operator");
    assert_eq!(capitalize_role("åskådare"), "Åskådare");
}

#[test]
fn attaching_access_mutates_only_object_results() {
    let mut object = ControlResponse::ok(1, json!({"status": "running"}));
    attach_access_capabilities(&mut object, AccessRole::Viewer);
    assert_eq!(
        object
            .result
            .as_ref()
            .and_then(|value| value.get("access"))
            .and_then(|value| value.get("role")),
        Some(&json!("viewer"))
    );

    let mut array = ControlResponse::ok(2, json!([]));
    attach_access_capabilities(&mut array, AccessRole::Admin);
    assert_eq!(array.result, Some(json!([])));

    let mut scalar = ControlResponse::ok(3, json!("running"));
    attach_access_capabilities(&mut scalar, AccessRole::Admin);
    assert_eq!(scalar.result, Some(json!("running")));

    let mut error = ControlResponse::error(4, "failed".into());
    attach_access_capabilities(&mut error, AccessRole::Admin);
    assert!(error.result.is_none());
}

#[test]
fn credential_channel_classification_covers_complete_trust_partition() {
    for client in [
        None,
        Some("unix"),
        Some("loopback"),
        Some("127.0.0.1:1"),
        Some("127.255.255.255:65535"),
        Some("[::1]:1"),
    ] {
        assert_eq!(
            classify_comm_credential_channel(client),
            "trusted_same_host",
            "client={client:?}"
        );
    }
    for client in [
        Some(""),
        Some("local"),
        Some("localhost:1"),
        Some("192.0.2.1:1"),
        Some("[2001:db8::1]:1"),
        Some("127.0.0.1"),
        Some("tcp://127.0.0.1:1"),
        Some("UNIX"),
        Some(" unix"),
    ] {
        assert_eq!(
            classify_comm_credential_channel(client),
            "untrusted_remote_plain_tcp",
            "client={client:?}"
        );
    }
}

#[test]
fn credential_channel_is_injected_for_exact_communication_operations() {
    for kind in [
        "comm.apply",
        "comm.test",
        "comm.browse_symbols",
        "ads.import_symbols.apply",
    ] {
        let mut request = control_request(kind, None);
        stamp_comm_credential_channel(&mut request, Some("unix"));
        assert_eq!(
            request
                .params
                .as_ref()
                .and_then(|params| params.get("credential_channel")),
            Some(&json!("trusted_same_host")),
            "kind={kind}"
        );
    }
}

#[test]
fn credential_channel_replaces_caller_supplied_value() {
    let mut request = control_request(
        "comm.apply",
        Some(json!({
            "protocol": "modbus_tcp",
            "credential_channel": "caller-controlled"
        })),
    );
    stamp_comm_credential_channel(&mut request, Some("192.0.2.1:9000"));

    let params = request.params.expect("params");
    assert_eq!(params["protocol"], json!("modbus_tcp"));
    assert_eq!(
        params["credential_channel"],
        json!("untrusted_remote_plain_tcp")
    );
}

#[test]
fn credential_channel_does_not_rewrite_nonobject_params() {
    for params in [json!(null), json!([]), json!("params"), json!(7)] {
        let mut request = control_request("comm.test", Some(params.clone()));
        stamp_comm_credential_channel(&mut request, Some("unix"));
        assert_eq!(request.params, Some(params));
    }
}

#[test]
fn credential_channel_is_not_injected_into_other_operations() {
    for kind in [
        "status",
        "ads.import_symbols",
        "comm.schema",
        "comm.discover",
        "io.write",
    ] {
        let original = Some(json!({"credential_channel": "caller"}));
        let mut request = control_request(kind, original.clone());
        stamp_comm_credential_channel(&mut request, Some("unix"));
        assert_eq!(request.params, original, "kind={kind}");
    }
}

fn auth_state() -> ControlState {
    let mut state = hmi_test_state(
        r#"
PROGRAM Main
END_PROGRAM
"#,
    );
    state.auth_token = Arc::new(Mutex::new(Some(SmolStr::new("admin-token"))));
    state.control_requires_auth = true;
    state
}

#[test]
fn web_dispatch_injects_header_token_only_when_auth_member_is_absent() {
    let state = auth_state();
    let response = dispatch_web_control_request_port(
        json!({"id": 1, "type": "future.operation"}),
        &state,
        Some("127.0.0.1:9000"),
        Some("admin-token"),
    );

    assert!(!response.ok);
    assert_eq!(response.error.as_deref(), Some("unsupported request"));
    assert_ne!(response.error_code.as_deref(), Some("invalid_auth_token"));
    assert_ne!(response.error_code.as_deref(), Some("missing_auth_token"));
}

#[test]
fn web_dispatch_does_not_override_explicit_wrong_auth() {
    let state = auth_state();
    let response = dispatch_web_control_request_port(
        json!({
            "id": 2,
            "type": "future.operation",
            "auth": "wrong"
        }),
        &state,
        Some("127.0.0.1:9000"),
        Some("admin-token"),
    );

    assert!(!response.ok);
    assert_eq!(response.error_code.as_deref(), Some("invalid_auth_token"));
}

#[test]
fn web_dispatch_does_not_override_explicit_null_auth() {
    let state = auth_state();
    let response = dispatch_web_control_request_port(
        json!({
            "id": 3,
            "type": "future.operation",
            "auth": null
        }),
        &state,
        Some("127.0.0.1:9000"),
        Some("admin-token"),
    );

    assert!(!response.ok);
    assert_eq!(response.error_code.as_deref(), Some("missing_auth_token"));
}

#[test]
fn web_dispatch_does_not_override_malformed_auth_member() {
    let state = auth_state();
    let response = dispatch_web_control_request_port(
        json!({
            "id": 4,
            "type": "future.operation",
            "auth": 7
        }),
        &state,
        Some("127.0.0.1:9000"),
        Some("admin-token"),
    );

    assert!(!response.ok);
    assert_eq!(response.error_code, None);
    assert!(
        response
            .error
            .as_deref()
            .is_some_and(|error| error.contains("invalid request"))
    );
}

#[test]
fn web_dispatch_without_payload_or_header_token_reports_missing_auth() {
    let state = auth_state();
    let response = dispatch_web_control_request_port(
        json!({"id": 5, "type": "status"}),
        &state,
        Some("127.0.0.1:9000"),
        None,
    );
    assert_eq!(response.error_code.as_deref(), Some("missing_auth_token"));
}

#[test]
fn request_line_always_returns_one_serialized_response() {
    let state = auth_state();
    let response = handle_request_line(
        r#"{"id":6,"type":"future.operation","auth":"admin-token"}"#,
        &state,
        Some("unix"),
    )
    .expect("response line");
    let value: serde_json::Value = serde_json::from_str(&response).expect("response JSON");

    assert_eq!(value["id"], json!(6));
    assert_eq!(value["ok"], json!(false));
    assert_eq!(value["error"], json!("unsupported request"));
}

#[test]
fn malformed_request_line_uses_id_zero_and_stable_error_prefix() {
    let state = auth_state();
    for line in ["", "{", "not json", "[] trailing"] {
        let response =
            handle_request_line(line, &state, Some("unix")).expect("response line");
        let value: serde_json::Value =
            serde_json::from_str(&response).expect("response JSON");
        assert_eq!(value["id"], json!(0), "line={line:?}");
        assert_eq!(value["ok"], json!(false), "line={line:?}");
        assert!(
            value["error"]
                .as_str()
                .is_some_and(|error| error.starts_with("invalid request:")),
            "line={line:?}, response={value}"
        );
    }
}

#[test]
fn structurally_invalid_request_value_uses_id_zero() {
    let state = auth_state();
    for value in [
        serde_json::Value::Null,
        json!([]),
        json!("request"),
        json!({}),
        json!({"id": "one", "type": "status"}),
        json!({"id": 1, "type": null}),
    ] {
        let response = handle_request_value(value.clone(), &state, Some("unix"));
        assert!(!response.ok, "value={value}");
        assert!(
            response
                .error
                .as_deref()
                .is_some_and(|error| error.starts_with("invalid request:")),
            "value={value}, error={:?}",
            response.error
        );
    }
}

#[test]
fn control_ports_return_owned_project_root_and_resource_name() {
    let mut state = hmi_test_state(
        r#"
PROGRAM Main
END_PROGRAM
"#,
    );
    state.project_root = Some(PathBuf::from("/project"));
    state.resource_name = SmolStr::new("RESOURCE_A");

    let mut root = hmi_asset_project_root_port(&state).expect("project root");
    let mut name = runtime_resource_name_port(&state);
    assert_eq!(name, "RESOURCE_A");
    root.push("changed");
    name = SmolStr::new("CHANGED");

    assert_eq!(state.project_root, Some(PathBuf::from("/project")));
    assert_eq!(state.resource_name, "RESOURCE_A");
    assert_eq!(name, "CHANGED");
}

#[test]
fn required_role_port_matches_registry_authority() {
    assert_eq!(
        control_request_required_role_port("status", None),
        AccessRole::Viewer
    );
    assert_eq!(
        control_request_required_role_port("io.write", None),
        AccessRole::Engineer
    );
    assert_eq!(
        control_request_required_role_port("future.operation", None),
        AccessRole::Admin
    );
    assert_eq!(
        control_request_required_role_port(
            "ads.doctor",
            Some(&json!({"writes_enabled": true}))
        ),
        AccessRole::Engineer
    );
}

#[test]
fn control_event_time_is_nonnegative_and_i64_bounded() {
    let time = control_event_time_now();
    assert!(time.as_nanos() >= 0);
    assert!(time.as_millis() >= 0);
}
