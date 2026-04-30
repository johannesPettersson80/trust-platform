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
