use std::sync::atomic::Ordering;

use serde_json::{json, Map, Value};
use smol_str::SmolStr;

use super::*;
use crate::control::tests::hmi_test_state;
use crate::settings::RuntimeSettings;

const SOURCE: &str = r#"
PROGRAM Main
VAR
    run : BOOL := TRUE;
END_VAR
END_PROGRAM
"#;

fn state() -> ControlState {
    hmi_test_state(SOURCE)
}

fn success_result(response: ControlResponse) -> Value {
    assert!(response.ok, "unexpected config error: {:?}", response.error);
    response.result.expect("successful result")
}

fn rejected_error(response: ControlResponse) -> String {
    assert!(!response.ok, "request unexpectedly succeeded");
    assert!(response.result.is_none());
    response.error.expect("rejection error")
}

fn set(state: &ControlState, params: Value) -> ControlResponse {
    handle_config_set(1, Some(params), state)
}

fn settings_snapshot(state: &ControlState) -> RuntimeSettings {
    state.settings.lock().expect("settings").clone()
}

#[test]
fn config_handler_contract_missing_and_nonobject_payloads_are_distinct() {
    assert_eq!(
        rejected_error(handle_config_set(1, None, &state())),
        "missing params"
    );
    for value in [
        Value::Null,
        json!(true),
        json!(1),
        json!("value"),
        json!([]),
    ] {
        assert_eq!(
            rejected_error(handle_config_set(1, Some(value), &state())),
            "invalid config payload: params must be an object"
        );
    }
}

#[test]
fn config_handler_contract_empty_object_is_successful_noop() {
    let state = state();
    let before = settings_snapshot(&state);
    let result = success_result(set(&state, json!({})));
    assert_eq!(result, json!({"updated": [], "restart_required": []}));
    let after = settings_snapshot(&state);
    assert_eq!(after.log_level, before.log_level);
    assert_eq!(after.web.enabled, before.web.enabled);
    assert_eq!(after.watchdog, before.watchdog);
}

#[test]
fn config_handler_contract_unknown_key_rejects_without_partial_settings_mutation() {
    let state = state();
    let before = settings_snapshot(&state);
    let error = rejected_error(set(
        &state,
        json!({
            "log.level": "debug",
            "unknown.key": true,
            "watchdog.enabled": true,
        }),
    ));
    assert_eq!(error, "unknown config key 'unknown.key'");
    let after = settings_snapshot(&state);
    assert_eq!(after.log_level, before.log_level);
    assert_eq!(after.watchdog, before.watchdog);
}

#[test]
fn config_handler_contract_type_error_rejects_without_partial_settings_mutation() {
    let state = state();
    let before = settings_snapshot(&state);
    let error = rejected_error(set(
        &state,
        json!({
            "log.level": "debug",
            "watchdog.enabled": "yes",
        }),
    ));
    assert!(error.contains("watchdog.enabled"), "{error}");
    let after = settings_snapshot(&state);
    assert_eq!(after.log_level, before.log_level);
    assert_eq!(after.watchdog, before.watchdog);
}

#[test]
fn config_handler_contract_startup_only_error_rolls_back_live_fields() {
    let state = state();
    let before = settings_snapshot(&state);
    let debug_before = state.debug_enabled.load(Ordering::Relaxed);
    let error = rejected_error(set(
        &state,
        json!({
            "control.debug_enabled": !debug_before,
            "log.level": "debug",
            "runtime.execution_backend": "vm",
        }),
    ));
    assert!(error.contains("runtime.execution_backend is startup-only"));
    assert_eq!(settings_snapshot(&state).log_level, before.log_level);
    assert_eq!(state.debug_enabled.load(Ordering::Relaxed), debug_before);
}

#[test]
fn config_handler_contract_startup_only_key_set_is_closed() {
    for key in [
        "runtime.execution_backend",
        "runtime.execution_backend_source",
        "realtime.profile",
        "realtime.enabled",
        "realtime.require_preempt_rt_kernel",
        "realtime.lock_memory",
        "realtime.scheduler",
        "realtime.priority",
        "realtime.cpu_affinity",
        "realtime.strict",
    ] {
        let mut params = Map::new();
        params.insert(key.to_string(), json!(true));
        let message = rejected_error(set(&state(), Value::Object(params)));
        assert_eq!(
            message,
            format!(
                "{key} is startup-only; change it via runtime.toml/service posture and restart"
            )
        );
    }
}

#[test]
fn config_handler_contract_immediate_updates_have_no_restart_claim() {
    let state = state();
    let result = success_result(set(
        &state,
        json!({
            "control.auth_token": "  new-control-token  ",
            "control.debug_enabled": true,
            "fault.policy": "restart",
            "log.level": " debug ",
            "retain.save_interval_ms": 50,
            "watchdog.action": "halt",
            "watchdog.enabled": true,
            "watchdog.timeout_ms": 25,
        }),
    ));
    assert_eq!(
        result,
        json!({
            "updated": [
                "control.auth_token",
                "control.debug_enabled",
                "fault.policy",
                "log.level",
                "retain.save_interval_ms",
                "watchdog.action",
                "watchdog.enabled",
                "watchdog.timeout_ms",
            ],
            "restart_required": [],
        })
    );
    let settings = settings_snapshot(&state);
    assert_eq!(settings.log_level, "debug");
    assert!(settings.watchdog.enabled);
    assert_eq!(settings.watchdog.timeout.as_millis(), 25);
    assert_eq!(
        *state.auth_token.lock().expect("auth token"),
        Some(SmolStr::new("new-control-token"))
    );
    assert!(state.debug_enabled.load(Ordering::Relaxed));
}

#[test]
fn config_handler_contract_restart_updates_are_exact_lexical_subset() {
    let state = state();
    let result = success_result(set(
        &state,
        json!({
            "control.auth_token": "admin-token",
            "control.mode": "DEBUG",
            "discovery.advertise": true,
            "discovery.enabled": true,
            "discovery.interfaces": [" eth0 "],
            "discovery.service_name": " plant/runtime ",
            "mesh.auth_token": " mesh-token ",
            "mesh.connect": [" tcp/peer:7447 "],
            "mesh.enabled": true,
            "mesh.listen": " 127.0.0.1:7447 ",
            "mesh.plugin_versions": {"storage": " 1.0 "},
            "mesh.publish": [" trust/demo/** "],
            "mesh.role": "ROUTER",
            "mesh.subscribe": {" trust/in ": " Input "},
            "mesh.tls": true,
            "mesh.zenohd_version": " 1.7.2 ",
            "retain.mode": "file",
            "runtime_cloud.links.transports": [
                {"source": "a", "target": "b", "transport": "mesh"}
            ],
            "runtime_cloud.profile": "plant",
            "runtime_cloud.wan.allow_write": [
                {"action": "write", "target": "b"}
            ],
            "web.auth": "TOKEN",
            "web.enabled": true,
            "web.listen": " 127.0.0.1:8080 ",
            "web.tls": true,
        }),
    ));
    let updated = result["updated"].as_array().expect("updated list");
    let restart = result["restart_required"].as_array().expect("restart list");
    assert_eq!(updated.len(), 24);
    assert_eq!(restart.len(), 23);
    assert!(updated.windows(2).all(|pair| {
        pair[0].as_str().expect("updated key") < pair[1].as_str().expect("updated key")
    }));
    assert!(restart.windows(2).all(|pair| {
        pair[0].as_str().expect("restart key") < pair[1].as_str().expect("restart key")
    }));
    assert!(updated.contains(&json!("control.auth_token")));
    assert!(!restart.contains(&json!("control.auth_token")));
    for key in restart {
        assert!(updated.contains(key), "{key}");
    }
}

#[test]
fn config_handler_contract_setting_token_and_token_web_auth_is_order_independent() {
    let first_state = state();
    let mut first = Map::new();
    first.insert("web.auth".to_string(), json!("token"));
    first.insert("control.auth_token".to_string(), json!("secret-token"));
    assert!(set(&first_state, Value::Object(first)).ok);
    assert_eq!(settings_snapshot(&first_state).web.auth, "token");

    let second_state = state();
    let mut second = Map::new();
    second.insert("control.auth_token".to_string(), json!("secret-token"));
    second.insert("web.auth".to_string(), json!("token"));
    assert!(set(&second_state, Value::Object(second)).ok);
    assert_eq!(settings_snapshot(&second_state).web.auth, "token");
}

#[test]
fn config_handler_contract_token_web_auth_requires_final_control_token() {
    let state = state();
    let error = rejected_error(set(&state, json!({"web.auth": "token"})));
    assert_eq!(
        error,
        "invalid config value for 'web.auth': token mode requires control.auth_token"
    );
    assert_eq!(settings_snapshot(&state).web.auth, "local");
}

#[test]
fn config_handler_contract_clearing_token_rejects_when_final_web_auth_is_token() {
    let state = state();
    assert!(
        set(
            &state,
            json!({
                "control.auth_token": "secret-token",
                "web.auth": "token",
            })
        )
        .ok
    );
    let before = settings_snapshot(&state);
    let error = rejected_error(set(&state, json!({"control.auth_token": null})));
    assert!(error.contains("web.auth"), "{error}");
    assert_eq!(settings_snapshot(&state).web.auth, before.web.auth);
    assert_eq!(
        state.auth_token.lock().expect("auth").as_deref(),
        Some("secret-token")
    );
}

#[test]
fn config_handler_contract_tcp_required_auth_rejects_null_token() {
    let mut state = state();
    state.control_requires_auth = true;
    *state.auth_token.lock().expect("auth") = Some(SmolStr::new("required-token"));
    let error = rejected_error(set(&state, json!({"control.auth_token": null})));
    assert_eq!(error, "auth token required for tcp endpoints");
    assert_eq!(
        state.auth_token.lock().expect("auth").as_deref(),
        Some("required-token")
    );
}

#[test]
fn config_handler_contract_local_auth_allows_token_clear() {
    let state = state();
    *state.auth_token.lock().expect("auth") = Some(SmolStr::new("old-token"));
    let result = success_result(set(&state, json!({"control.auth_token": null})));
    assert_eq!(
        result,
        json!({"updated": ["control.auth_token"], "restart_required": []})
    );
    assert_eq!(*state.auth_token.lock().expect("auth"), None);
}

#[test]
fn config_handler_contract_invalid_sibling_does_not_commit_secret() {
    let secret = "new-secret-must-not-commit";
    let state = state();
    let error = rejected_error(set(
        &state,
        json!({
            "control.auth_token": secret,
            "web.auth": "unsupported",
        }),
    ));
    assert!(!error.contains(secret), "{error}");
    assert_eq!(*state.auth_token.lock().expect("auth"), None);
    assert_eq!(settings_snapshot(&state).web.auth, "local");
}

#[test]
fn config_handler_contract_web_auth_and_control_mode_are_canonicalized() {
    let state = state();
    let result = success_result(set(
        &state,
        json!({
            "control.auth_token": "token",
            "control.mode": " DeBuG ",
            "web.auth": " ToKeN ",
        }),
    ));
    assert_eq!(settings_snapshot(&state).web.auth, "token");
    assert_eq!(
        *state.control_mode.lock().expect("control mode"),
        ControlMode::Debug
    );
    let restart_required = result["restart_required"]
        .as_array()
        .expect("restart_required array");
    assert!(restart_required.contains(&json!("control.mode")));
    assert!(restart_required.contains(&json!("web.auth")));
}

#[test]
fn config_handler_contract_mesh_token_set_and_clear_never_echo_value() {
    let secret = "mesh-secret-must-not-echo";
    let state = state();
    let set_result = success_result(set(&state, json!({"mesh.auth_token": secret})));
    assert!(!set_result.to_string().contains(secret));
    assert_eq!(
        settings_snapshot(&state).mesh.auth_token.as_deref(),
        Some(secret)
    );
    let clear_result = success_result(set(&state, json!({"mesh.auth_token": null})));
    assert!(!clear_result.to_string().contains(secret));
    assert_eq!(settings_snapshot(&state).mesh.auth_token, None);
}

#[test]
fn config_handler_contract_collection_values_are_trimmed_before_commit() {
    let state = state();
    assert!(
        set(
            &state,
            json!({
                "discovery.interfaces": [" eth0 ", " wlan0 "],
                "mesh.connect": [" tcp/peer:7447 "],
                "mesh.plugin_versions": {" storage ": " 1.0 "},
                "mesh.publish": [" trust/out/** "],
                "mesh.subscribe": {" trust/in/** ": " InputAlias "},
            })
        )
        .ok
    );
    let settings = settings_snapshot(&state);
    assert_eq!(settings.discovery.interfaces, ["eth0", "wlan0"]);
    assert_eq!(settings.mesh.connect, ["tcp/peer:7447"]);
    assert_eq!(settings.mesh.publish, ["trust/out/**"]);
    assert_eq!(
        settings
            .mesh
            .plugin_versions
            .get("storage")
            .map(SmolStr::as_str),
        Some("1.0")
    );
    assert_eq!(
        settings
            .mesh
            .subscribe
            .get("trust/in/**")
            .map(SmolStr::as_str),
        Some("InputAlias")
    );
}

#[test]
fn config_handler_contract_runtime_cloud_rules_commit_canonical_values() {
    let state = state();
    assert!(
        set(
            &state,
            json!({
                "runtime_cloud.profile": " WAN ",
                "runtime_cloud.wan.allow_write": [
                    {"action": " write ", "target": " peer "}
                ],
                "runtime_cloud.links.transports": [
                    {"source": " a ", "target": " b ", "transport": "MODBUS_TCP"}
                ],
            })
        )
        .ok
    );
    let settings = settings_snapshot(&state);
    assert_eq!(settings.runtime_cloud.profile, RuntimeCloudProfile::Wan);
    assert_eq!(settings.runtime_cloud.wan_allow_write[0].action, "write");
    assert_eq!(settings.runtime_cloud.wan_allow_write[0].target, "peer");
    assert_eq!(settings.runtime_cloud.link_preferences[0].source, "a");
    assert_eq!(settings.runtime_cloud.link_preferences[0].target, "b");
    assert_eq!(
        settings.runtime_cloud.link_preferences[0]
            .transport
            .as_str(),
        "modbus-tcp"
    );
}

#[test]
fn config_handler_contract_config_get_never_returns_secret_values() {
    let control_secret = "control-secret-value";
    let mesh_secret = "mesh-secret-value";
    let state = state();
    *state.auth_token.lock().expect("auth") = Some(SmolStr::new(control_secret));
    state.settings.lock().expect("settings").mesh.auth_token = Some(SmolStr::new(mesh_secret));
    let result = success_result(handle_config_get(2, &state));
    let payload = result.to_string();
    assert!(!payload.contains(control_secret), "{payload}");
    assert!(!payload.contains(mesh_secret), "{payload}");
    assert_eq!(result["control.auth_token_set"], true);
    assert_eq!(result["control.auth_token_length"], control_secret.len());
    assert_eq!(result["mesh.auth_token_set"], true);
    assert!(result.get("control.auth_token").is_none());
    assert!(result.get("mesh.auth_token").is_none());
}

#[test]
fn config_handler_contract_config_get_absent_credentials_report_presence_only() {
    let state = state();
    let result = success_result(handle_config_get(2, &state));
    assert_eq!(result["control.auth_token_set"], false);
    assert_eq!(result["control.auth_token_length"], Value::Null);
    assert_eq!(result["mesh.auth_token_set"], false);
}

#[test]
fn config_handler_contract_config_get_uses_canonical_enum_spellings() {
    let state = state();
    *state.control_mode.lock().expect("control mode") = ControlMode::Production;
    let result = success_result(handle_config_get(2, &state));
    assert_eq!(result["watchdog.action"], "safe_halt");
    assert_eq!(result["fault.policy"], "safe_halt");
    assert_eq!(result["retain.mode"], "none");
    assert_eq!(result["control.mode"], "production");
    assert_eq!(result["mesh.role"], "peer");
    assert_eq!(result["runtime_cloud.profile"], "dev");
    assert_eq!(result["runtime.execution_backend"], "vm");
    assert_eq!(result["runtime.execution_backend_source"], "default");
}

#[test]
fn config_handler_contract_get_and_set_agree_for_mutable_scalar_values() {
    let state = state();
    assert!(
        set(
            &state,
            json!({
                "control.mode": "debug",
                "fault.policy": "halt",
                "log.level": "trace",
                "retain.mode": "file",
                "watchdog.action": "restart",
                "watchdog.enabled": true,
                "watchdog.timeout_ms": 77,
            })
        )
        .ok
    );
    let result = success_result(handle_config_get(2, &state));
    assert_eq!(result["control.mode"], "debug");
    assert_eq!(result["fault.policy"], "halt");
    assert_eq!(result["log.level"], "trace");
    assert_eq!(result["retain.mode"], "file");
    assert_eq!(result["watchdog.action"], "restart");
    assert_eq!(result["watchdog.enabled"], true);
    assert_eq!(result["watchdog.timeout_ms"], 77);
}

#[test]
fn config_handler_contract_config_get_contains_complete_stable_key_set() {
    let state = state();
    let result = success_result(handle_config_get(2, &state));
    let actual = result
        .as_object()
        .expect("config object")
        .keys()
        .map(String::as_str)
        .collect::<Vec<_>>();
    assert_eq!(
        actual,
        [
            "control.auth_token_length",
            "control.auth_token_set",
            "control.debug_enabled",
            "control.mode",
            "discovery.advertise",
            "discovery.enabled",
            "discovery.interfaces",
            "discovery.service_name",
            "fault.policy",
            "hmi.read_only",
            "log.level",
            "mesh.auth_token_set",
            "mesh.connect",
            "mesh.enabled",
            "mesh.listen",
            "mesh.plugin_versions",
            "mesh.publish",
            "mesh.role",
            "mesh.subscribe",
            "mesh.tls",
            "mesh.zenohd_version",
            "observability.alerts",
            "observability.enabled",
            "observability.history_path",
            "observability.include",
            "observability.max_entries",
            "observability.mode",
            "observability.prometheus_enabled",
            "observability.prometheus_path",
            "observability.sample_interval_ms",
            "opcua.allow_anonymous",
            "opcua.enabled",
            "opcua.endpoint_path",
            "opcua.expose",
            "opcua.listen",
            "opcua.max_nodes",
            "opcua.namespace_uri",
            "opcua.publish_interval_ms",
            "opcua.security_mode",
            "opcua.security_policy",
            "opcua.username_set",
            "realtime.cpu_affinity",
            "realtime.enabled",
            "realtime.lock_memory",
            "realtime.priority",
            "realtime.profile",
            "realtime.require_preempt_rt_kernel",
            "realtime.scheduler",
            "realtime.strict",
            "resource.cycle_interval_ms",
            "retain.mode",
            "retain.save_interval_ms",
            "runtime.execution_backend",
            "runtime.execution_backend_source",
            "runtime_cloud.links.transports",
            "runtime_cloud.profile",
            "runtime_cloud.wan.allow_write",
            "simulation.enabled",
            "simulation.mode",
            "simulation.time_scale",
            "simulation.warning",
            "watchdog.action",
            "watchdog.enabled",
            "watchdog.timeout_ms",
            "web.auth",
            "web.enabled",
            "web.listen",
            "web.tls",
        ]
    );
}

#[test]
fn config_handler_contract_rejected_request_has_no_success_lists_or_secret() {
    let secret = "rejected-control-secret";
    let state = state();
    let response = set(
        &state,
        json!({
            "control.auth_token": secret,
            "unknown.key": true,
        }),
    );
    assert!(!response.ok);
    assert!(response.result.is_none());
    assert!(!response
        .error
        .as_deref()
        .unwrap_or_default()
        .contains(secret));
}
