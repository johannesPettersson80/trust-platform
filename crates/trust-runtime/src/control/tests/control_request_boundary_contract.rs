#[test]
fn shutdown_prepares_success_before_signalling_resource_stop() {
    let state = control_test_state("PROGRAM Main END_PROGRAM");

    let response = program_handlers::handle_shutdown(41, &state);

    assert!(response.ok, "shutdown response must be successful");
    assert!(
        !state.resource.stop_requested() && !state.resource.program_stop_requested(),
        "resource stop must remain deferred until the response write boundary"
    );
}

#[test]
fn web_shutdown_dispatch_defers_resource_stop_until_transport_completion() {
    let state = control_test_state("PROGRAM Main END_PROGRAM");

    let response = dispatch_web_control_request_port(
        json!({"id": 42, "type": "shutdown"}),
        &state,
        Some("web"),
        None,
    );
    assert!(response.ok, "web shutdown response must be successful");
    let mut responses = [response];
    let serialized =
        write_then_complete_web_control_responses_port(&mut responses, &state, |responses| {
            assert!(
                !state.resource.stop_requested() && !state.resource.program_stop_requested(),
                "web dispatch must not stop the resource before the response write boundary"
            );
            serde_json::to_value(&responses[0]).expect("serialized web response")
        });
    assert_eq!(serialized["id"], json!(42));
    assert_eq!(serialized["result"]["status"], json!("stopping"));
    // Decision Q11 on computers (18 October 2026), specification 34 STATE-08: `shutdown` stops the
    // program and leaves the scan thread, so the process, running. Before it ended the thread.
    assert!(
        state.resource.program_stop_requested(),
        "transport completion must apply the deferred program stop"
    );
    assert!(
        !state.resource.stop_requested(),
        "a plain shutdown must not end the scan thread"
    );
}

#[test]
fn the_explicit_exit_ends_the_scan_thread_only_after_the_response_is_written() {
    // Specification 34 STATE-08: `shutdown` with `{"exit": true}` also ends the process. The reply
    // is written first, and the exit is requested after it. Wrong behaviour: the exit is
    // requested before the reply, or it is requested as the program stop alone.
    let state = control_test_state("PROGRAM Main END_PROGRAM");

    let response = dispatch_web_control_request_port(
        json!({"id": 43, "type": "shutdown", "params": {"exit": true}}),
        &state,
        Some("web"),
        None,
    );
    assert!(response.ok, "the explicit exit is answered like shutdown");
    let mut responses = [response];
    let serialized =
        write_then_complete_web_control_responses_port(&mut responses, &state, |responses| {
            assert!(
                !state.resource.stop_requested(),
                "the exit must wait for the response write boundary"
            );
            serde_json::to_value(&responses[0]).expect("serialized web response")
        });
    assert_eq!(serialized["result"]["status"], json!("stopping"));
    assert!(
        state.resource.stop_requested(),
        "transport completion must apply the exit"
    );
}

// The access-capability and credential-channel tests moved with the request pipeline to
// trust-runtime-services (`dispatch/contract_tests.rs`, `access/credential_channel/contract_tests.rs`).

fn auth_state() -> ControlState {
    let mut state = control_test_state(
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
    assert!(response
        .error
        .as_deref()
        .is_some_and(|error| error.contains("invalid request")));
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
        let response = handle_request_line(line, &state, Some("unix")).expect("response line");
        let value: serde_json::Value = serde_json::from_str(&response).expect("response JSON");
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
    let mut state = control_test_state(
        r#"
PROGRAM Main
END_PROGRAM
"#,
    );
    state.project_root = Some(PathBuf::from("/project"));
    state.resource_name = SmolStr::new("RESOURCE_A");

    let mut name = runtime_resource_name_port(&state);
    assert_eq!(name, "RESOURCE_A");
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
        control_request_required_role_port("ads.doctor", Some(&json!({"writes_enabled": true}))),
        AccessRole::Engineer
    );
}

#[test]
fn control_event_time_is_nonnegative_and_i64_bounded() {
    let time = control_event_time_now();
    assert!(time.as_nanos() >= 0);
    assert!(time.as_millis() >= 0);
}
