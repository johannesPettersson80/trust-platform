//! The shared part of `config.get` and `config.set` (specification 11, the runtime configuration
//! contract). Every expected value is written out by hand from the contract: the keys and their
//! spellings are those a computer's `config.get` reports, and the refusals are the contract's.

use alloc::string::{String, ToString};
use alloc::vec;

use serde_json::{json, Map, Value};
use smol_str::SmolStr;
use trust_runtime_core::deployment::{ControlMode, HangAction, LogLevel};
use trust_runtime_core::value::Duration;
use trust_runtime_core::watchdog::{FaultPolicy, RetainMode, WatchdogAction, WatchdogPolicy};

use super::{project, stage_request, RuntimeSettings, StagedSettings, TokenChange};
use crate::resource::commands::SchedulerPolicies;
use crate::status::StatusSettings;

fn settings() -> RuntimeSettings {
    RuntimeSettings {
        log_level: LogLevel::Warn,
        cycle_interval: Duration::from_millis(10),
        watchdog: WatchdogPolicy {
            enabled: true,
            timeout: Duration::from_millis(250),
            action: WatchdogAction::SafeHalt,
        },
        hang_action: HangAction::Abort,
        fault_policy: FaultPolicy::Restart,
        retain_mode: RetainMode::File,
        retain_save_interval: Some(Duration::from_millis(5_000)),
        control_mode: ControlMode::Debug,
        debug_enabled: true,
    }
}

fn status() -> StatusSettings {
    StatusSettings {
        execution_backend: "native",
        execution_backend_source: "default",
        simulation_enabled: false,
        simulation_time_scale: 1,
        simulation_mode: SmolStr::new_static("production"),
        simulation_warning: SmolStr::default(),
    }
}

fn object(value: Value) -> Map<String, Value> {
    match value {
        Value::Object(map) => map,
        other => panic!("not an object: {other}"),
    }
}

/// Stages `params` on [`settings`] with a host that has no keys of its own.
fn stage(params: Value, token_required: bool) -> Result<StagedSettings, String> {
    let mut staged = StagedSettings::new(settings());
    stage_request(&object(params), &mut staged, token_required, |_, _, _| None)?;
    Ok(staged)
}

fn refusal(params: Value) -> String {
    match stage(params, false) {
        Ok(staged) => panic!("must be refused: {staged:?}"),
        Err(error) => error,
    }
}

/// Every key in the computer's spelling, each value in the configuration's: 10 ms is 10, 250 ms
/// 250, `safe_halt` with its underscore, the token's presence and length but never the token.
/// Catches a key renamed, a value written as a debug name (`SafeHalt`) and a token written out.
#[test]
fn the_projection_writes_every_shared_key_in_the_configuration_spelling() {
    let mut out = Map::new();
    project(&settings(), Some(12), &status(), &mut out);
    assert_eq!(
        Value::Object(out),
        json!({
            "log.level": "warn",
            "watchdog.enabled": true,
            "watchdog.timeout_ms": 250,
            "watchdog.action": "safe_halt",
            "watchdog.hang_action": "abort",
            "resource.cycle_interval_ms": 10,
            "runtime.execution_backend": "native",
            "runtime.execution_backend_source": "default",
            "fault.policy": "restart",
            "retain.mode": "file",
            "retain.save_interval_ms": 5000,
            "control.auth_token_set": true,
            "control.auth_token_length": 12,
            "control.debug_enabled": true,
            "control.mode": "debug",
        })
    );
    let mut out = Map::new();
    let without = RuntimeSettings {
        retain_save_interval: None,
        ..settings()
    };
    project(&without, None, &status(), &mut out);
    assert_eq!(out["control.auth_token_set"], json!(false));
    assert_eq!(out["control.auth_token_length"], Value::Null);
    assert_eq!(out["retain.save_interval_ms"], Value::Null);
}

/// A request stages each key it names, trimmed and in any case, and lists it once; the keys whose
/// service starts again to take them (`control.mode`, `retain.mode`, `watchdog.hang_action`) are
/// also listed as needing a restart. Its scheduler copies are sent only when it set one of them.
/// Catches a level kept as written (` WARNING `), a key listed twice and a policy sent for a
/// request that changed none.
#[test]
fn a_request_stages_its_keys_and_names_those_that_need_a_restart() {
    let staged = stage(
        json!({
            "control.mode": "Production",
            "log.level": " WARNING ",
            "retain.mode": "none",
            "watchdog.hang_action": "fault",
            "watchdog.timeout_ms": 500,
        }),
        false,
    )
    .expect("valid keys are staged");
    assert_eq!(staged.settings.control_mode, ControlMode::Production);
    assert_eq!(staged.settings.log_level, LogLevel::Warn);
    assert_eq!(staged.settings.retain_mode, RetainMode::None);
    assert_eq!(staged.settings.hang_action, HangAction::Fault);
    assert_eq!(staged.settings.watchdog.timeout, Duration::from_millis(500));
    assert_eq!(
        staged.updated,
        vec![
            "control.mode",
            "log.level",
            "retain.mode",
            "watchdog.hang_action",
            "watchdog.timeout_ms"
        ]
    );
    assert_eq!(
        staged.restart_required,
        vec!["control.mode", "retain.mode", "watchdog.hang_action"]
    );
    assert_eq!(
        staged.changed_policies(),
        Some(SchedulerPolicies {
            watchdog: WatchdogPolicy {
                enabled: true,
                timeout: Duration::from_millis(500),
                action: WatchdogAction::SafeHalt,
            },
            fault_policy: FaultPolicy::Restart,
            retain_save_interval: Some(Duration::from_millis(5_000)),
        })
    );
    let staged = stage(json!({"log.level": "debug"}), false).expect("a level is staged");
    assert_eq!(staged.changed_policies(), None);
}

/// The control token: a trimmed string sets it, null clears it where the endpoint takes requests
/// without one and is refused where it requires one, an empty or non-string value is refused. It
/// is staged first, whatever the order of the keys. Catches a token cleared on an endpoint that
/// requires it and a token error hidden behind a later key's.
#[test]
fn the_control_token_is_set_cleared_or_refused_by_the_contract() {
    let staged = stage(json!({"control.auth_token": " s3cret "}), false).expect("a token is set");
    assert_eq!(
        staged.token,
        TokenChange::Set(SmolStr::new_static("s3cret"))
    );
    assert_eq!(staged.updated, vec!["control.auth_token"]);
    let staged = stage(json!({"control.auth_token": null}), false).expect("a token is cleared");
    assert_eq!(staged.token, TokenChange::Clear);
    assert_eq!(
        stage(json!({"control.auth_token": null}), true).expect_err("required"),
        "auth token required for tcp endpoints"
    );
    assert_eq!(
        refusal(json!({"control.auth_token": "  "})),
        "invalid config value for 'control.auth_token': must not be empty"
    );
    assert_eq!(
        refusal(json!({"a.unknown": 1, "control.auth_token": 7})),
        "invalid config value for 'control.auth_token': expected string or null"
    );
}

/// Each refusal names its key and changes nothing: a level the configuration does not take, a
/// hang action, a timeout of 0, a Boolean of the wrong type, a mode, a startup-only key and a key
/// that neither every runtime nor the host has. Catches a level accepted as any text, as the
/// computer accepted it, and an unknown key ignored.
#[test]
fn refusals_name_the_key_and_the_rule() {
    let cases: [(Value, &str); 7] = [
        (
            json!({"log.level": "loud"}),
            "invalid config value for 'log.level': must be error, warn, warning, info, debug or trace",
        ),
        (
            json!({"watchdog.hang_action": "panic"}),
            "invalid config value for 'watchdog.hang_action': invalid config 'invalid watchdog hang action 'panic''",
        ),
        (
            json!({"watchdog.timeout_ms": 0}),
            "invalid config value for 'watchdog.timeout_ms': must be >= 1",
        ),
        (
            json!({"control.debug_enabled": "yes"}),
            "invalid config value for 'control.debug_enabled': expected boolean",
        ),
        (
            json!({"control.mode": "service"}),
            "invalid config value for 'control.mode': expected 'production' or 'debug'",
        ),
        (
            json!({"runtime.execution_backend": "native"}),
            "runtime.execution_backend is startup-only; change it via runtime.toml/service posture and restart",
        ),
        (
            json!({"web.enabled": true}),
            "unknown config key 'web.enabled'",
        ),
    ];
    for (params, expected) in cases {
        assert_eq!(refusal(params.clone()), expected, "{params}");
    }
}

/// A host stages the keys of its own services; a key it stages is listed as the shared keys are.
/// Catches a host key refused as unknown.
#[test]
fn a_host_stages_the_keys_of_its_own_services() {
    let mut staged = StagedSettings::new(settings());
    let mut web = false;
    stage_request(
        &object(json!({"web.enabled": true})),
        &mut staged,
        false,
        |key, value, staged| {
            (key == "web.enabled").then(|| {
                web = value.as_bool().ok_or_else(|| "not a Boolean".to_string())?;
                staged.updated.push("web.enabled");
                staged.restart_required.push("web.enabled");
                Ok(())
            })
        },
    )
    .expect("the host's key is staged");
    assert!(web);
    assert_eq!(staged.updated, vec!["web.enabled"]);
    assert_eq!(staged.restart_required, vec!["web.enabled"]);
}
