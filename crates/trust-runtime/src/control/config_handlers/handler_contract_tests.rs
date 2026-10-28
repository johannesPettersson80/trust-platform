//! The tests need the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

use std::sync::atomic::Ordering;

use serde_json::{json, Map, Value};
use smol_str::SmolStr;

use super::*;
use crate::control::tests::control_test_state;
use crate::settings::RuntimeSettings;

const SOURCE: &str = r#"
PROGRAM Main
VAR
    run : BOOL := TRUE;
END_VAR
END_PROGRAM
"#;

fn state() -> ControlState {
    control_test_state(SOURCE)
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

/// A state whose runtime runs a simulation at `scale`, with the speed attached as the resource loop shares it.
fn simulated_state(scale: u32) -> (ControlState, crate::scheduler::SimulationSpeed) {
    let mut state = state();
    let speed = crate::scheduler::SimulationSpeed::new(scale);
    state.resource = state.resource.clone().with_simulation_speed(speed.clone());
    {
        let mut settings = state.settings.lock().expect("settings");
        settings.simulation.enabled = true;
        settings.simulation.time_scale = scale;
    }
    (state, speed)
}

#[test]
fn config_set_simulation_time_scale_is_refused_without_a_running_simulation() {
    let state = state();
    let error = rejected_error(set(&state, json!({"simulation.time_scale": 8})));
    assert!(error.contains("simulation.time_scale"), "{error}");
    assert!(error.contains("not running a simulation"), "{error}");
    assert_eq!(settings_snapshot(&state).simulation.time_scale, 1);
}

#[test]
fn config_set_simulation_time_scale_applies_live_without_a_restart_claim() {
    let (state, speed) = simulated_state(1);
    for scale in [1_u32, 100, 10] {
        let response = success_result(set(&state, json!({"simulation.time_scale": scale})));
        assert_eq!(response["updated"], json!(["simulation.time_scale"]));
        assert_eq!(response["restart_required"], json!([]));
        assert_eq!(
            speed.get(),
            scale,
            "the running resource paces at the new speed"
        );
        assert_eq!(settings_snapshot(&state).simulation.time_scale, scale);
    }
    let config = success_result(handle_config_get(2, &state));
    assert_eq!(config["simulation.time_scale"], json!(10));
}

#[test]
fn config_set_simulation_time_scale_refuses_values_outside_the_range_and_fractions() {
    let (state, speed) = simulated_state(4);
    for (value, expected) in [
        (json!(0), ">= 1 and <= 100"),
        (json!(101), ">= 1 and <= 100"),
        (json!(-3), ">= 1 and <= 100"),
        (json!(2.5), "whole number"),
        (json!("8"), "whole number"),
    ] {
        let error = rejected_error(set(&state, json!({"simulation.time_scale": value})));
        assert!(error.contains("simulation.time_scale"), "{error}");
        assert!(error.contains(expected), "{error}");
    }
    assert_eq!(
        speed.get(),
        4,
        "a refused speed does not change the running simulation"
    );
    assert_eq!(settings_snapshot(&state).simulation.time_scale, 4);
}

#[test]
fn config_set_refused_simulation_time_scale_rejects_the_whole_request() {
    let (state, speed) = simulated_state(4);
    let error = rejected_error(set(
        &state,
        json!({"simulation.time_scale": 0, "log.level": "debug"}),
    ));
    assert!(error.contains("simulation.time_scale"), "{error}");
    assert_ne!(settings_snapshot(&state).log_level.as_str(), "debug");
    assert_eq!(speed.get(), 4);
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
fn config_handler_contract_refuses_durations_beyond_the_signed_nanosecond_range() {
    let state = state();
    let before = settings_snapshot(&state);
    let watchdog = rejected_error(set(&state, json!({ "watchdog.timeout_ms": i64::MAX })));
    assert_eq!(
        watchdog,
        "invalid config value for 'watchdog.timeout_ms': must be <= 9223372036854"
    );
    let save_interval = rejected_error(set(&state, json!({ "retain.save_interval_ms": i64::MAX })));
    assert_eq!(
        save_interval,
        "invalid config value for 'retain.save_interval_ms': must be <= 9223372036854"
    );
    let unsigned = rejected_error(set(&state, json!({ "watchdog.timeout_ms": u64::MAX })));
    assert_eq!(
        unsigned,
        "invalid config value for 'watchdog.timeout_ms': expected integer >= 1"
    );
    let after = settings_snapshot(&state);
    assert_eq!(after.watchdog, before.watchdog);
    assert_eq!(after.retain_save_interval, before.retain_save_interval);
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
    assert_eq!(result["runtime.execution_backend"], "native");
    assert_eq!(result["runtime.execution_backend_source"], "default");
}

// Specification 11, `config.get` / `config.set`: `discovery.host_group` is readable and writable,
// with `runtime.toml`'s trimming and non-empty rule, and it needs a restart. It would catch a
// missing key, an untrimmed value, an accepted empty value, a missing `restart_required` entry,
// or a null that does not clear the setting.
#[test]
fn discovery_host_group_readable_and_writable() {
    let state = state();
    assert_eq!(
        success_result(handle_config_get(2, &state))["discovery.host_group"],
        Value::Null
    );
    let result = success_result(set(&state, json!({"discovery.host_group": "  line-a  "})));
    assert_eq!(
        result,
        json!({
            "updated": ["discovery.host_group"],
            "restart_required": ["discovery.host_group"],
        })
    );
    assert_eq!(
        settings_snapshot(&state).discovery.host_group.as_deref(),
        Some("line-a")
    );
    assert_eq!(
        success_result(handle_config_get(2, &state))["discovery.host_group"],
        json!("line-a")
    );

    let error = rejected_error(set(&state, json!({"discovery.host_group": "   "})));
    assert_eq!(
        error,
        "invalid config value for 'discovery.host_group': must not be empty"
    );
    assert_eq!(
        settings_snapshot(&state).discovery.host_group.as_deref(),
        Some("line-a")
    );

    let cleared = success_result(set(&state, json!({"discovery.host_group": null})));
    assert_eq!(
        cleared,
        json!({
            "updated": ["discovery.host_group"],
            "restart_required": ["discovery.host_group"],
        })
    );
    assert_eq!(settings_snapshot(&state).discovery.host_group, None);
    assert_eq!(
        success_result(handle_config_get(2, &state))["discovery.host_group"],
        Value::Null
    );
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
            "discovery.host_group",
            "discovery.interfaces",
            "discovery.service_name",
            "fault.policy",
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
            "watchdog.hang_action",
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

/// A scratch project folder for the endpoint-file tests, removed when the test ends.
struct ScratchProject(std::path::PathBuf);

impl ScratchProject {
    fn new(label: &str) -> Self {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_nanos())
            .unwrap_or_default();
        let path = std::env::temp_dir().join(format!(
            "trust-config-set-{label}-{}-{nanos}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).expect("create scratch project");
        Self(path)
    }

    fn endpoint_file(&self) -> std::path::PathBuf {
        self.0
            .join(crate::control::endpoint::ENDPOINT_DIR)
            .join(crate::control::endpoint::ENDPOINT_FILE)
    }

    /// Publishes this process's endpoint file with a generated token, kept where this platform
    /// keeps it (spec 11, section 6.8.0a).
    fn publish_generated_token(&self) -> crate::control::endpoint::PublishedEndpoint {
        let endpoint =
            crate::control::ControlEndpoint::parse("tcp://127.0.0.1:5555").expect("endpoint");
        crate::control::endpoint::PublishedEndpoint::publish(
            &self.0,
            &endpoint,
            Some("generated-token"),
        )
        .expect("publish endpoint file")
    }

    /// The token that the project's endpoint file offers a client. The project has no
    /// runtime.toml, so no configured token can stand in for it.
    fn offered_token(&self) -> Option<String> {
        crate::control::endpoint::resolve_control_target(Some(&self.0), None, None)
            .expect("resolve the published endpoint")
            .auth_token
    }
}

impl Drop for ScratchProject {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn config_set_of_the_auth_token_withdraws_it_from_the_endpoint_file() {
    let project = ScratchProject::new("withdraw");
    let published = project.publish_generated_token();
    assert_eq!(project.offered_token().as_deref(), Some("generated-token"));
    let before: Value = serde_json::from_str(
        &std::fs::read_to_string(project.endpoint_file()).expect("read endpoint file"),
    )
    .expect("parse endpoint file");
    let mut state = state();
    state.project_root = Some(project.0.clone());

    success_result(set(
        &state,
        json!({"control.auth_token": "replacement-token"}),
    ));

    assert_eq!(project.offered_token(), None);
    let after: Value = serde_json::from_str(
        &std::fs::read_to_string(project.endpoint_file()).expect("read endpoint file"),
    )
    .expect("parse endpoint file");
    assert_eq!(after["endpoint"], before["endpoint"]);
    assert_eq!(after["pid"], before["pid"]);
    assert!(after.get("auth_token").is_none());
    drop(published);
}

#[test]
fn config_set_of_the_auth_token_without_an_endpoint_file_creates_none() {
    let project = ScratchProject::new("no-file");
    let mut state = state();
    state.project_root = Some(project.0.clone());

    success_result(set(
        &state,
        json!({"control.auth_token": "replacement-token"}),
    ));

    assert!(!project
        .0
        .join(crate::control::endpoint::ENDPOINT_DIR)
        .exists());
}

#[test]
fn a_refused_config_set_leaves_the_endpoint_file_token_in_place() {
    use crate::scheduler::{ResourceCommand, ResourceControl, StdClock};

    let project = ScratchProject::new("refused");
    let published = project.publish_generated_token();
    let mut state = state();
    state.project_root = Some(project.0.clone());
    // Nothing drains this stub queue, so filling it makes the scheduler refuse the request.
    let (control, _queued) = ResourceControl::stub(StdClock::new());
    state.resource = control;
    for _ in 0..crate::scheduler::RESOURCE_COMMAND_CAPACITY {
        let (tx, _rx) = std::sync::mpsc::channel();
        state
            .resource
            .send_command(ResourceCommand::Snapshot { respond_to: tx })
            .expect("the queue takes commands up to its bound");
    }
    let watchdog = settings_snapshot(&state).watchdog.enabled;

    let response = set(
        &state,
        json!({
            "control.auth_token": "replacement-token",
            "watchdog.enabled": !watchdog,
        }),
    );

    assert!(!response.ok, "a full scheduler queue refuses the request");
    assert_eq!(response.error_code.as_deref(), Some("resource_busy"));
    assert_eq!(project.offered_token().as_deref(), Some("generated-token"));
    drop(published);
}

#[test]
fn config_set_of_the_auth_token_ignores_an_endpoint_file_it_does_not_own() {
    let project = ScratchProject::new("not-owned");
    std::fs::create_dir_all(project.0.join(crate::control::endpoint::ENDPOINT_DIR))
        .expect("endpoint folder");
    std::fs::write(project.endpoint_file(), "not json").expect("write foreign endpoint file");
    let mut state = state();
    state.project_root = Some(project.0.clone());

    success_result(set(
        &state,
        json!({"control.auth_token": "replacement-token"}),
    ));

    assert_eq!(
        std::fs::read_to_string(project.endpoint_file()).expect("read endpoint file"),
        "not json"
    );
}
