//! The host's own `eval`, which needs the HIR expression evaluator, and the host's Live Values
//! wiring over a real runtime: the retained management labels that `set`, `var.force`,
//! `var.unforce` and `var.forced` reach through `DebugControl`. The operations' parameters,
//! refusals, error texts and JSON are the shared `var_requests` contract tests.

use serde_json::{json, Value as JsonValue};

use super::*;
use crate::value::Value;

const SOURCE: &str = r#"
VAR_GLOBAL
    global_value : DINT := 7;
END_VAR

VAR_GLOBAL RETAIN
    retained_value : DINT := 9;
END_VAR

PROGRAM Main
END_PROGRAM
"#;

fn state() -> ControlState {
    crate::control::tests::hmi_test_state(SOURCE)
}

fn result(response: ControlResponse) -> JsonValue {
    assert!(response.ok, "request failed: {:?}", response.error);
    response.result.expect("response result")
}

fn assert_invalid_params(response: ControlResponse) {
    assert!(!response.ok, "invalid parameters must reject");
    assert!(
        response
            .error
            .as_deref()
            .is_some_and(|error| error.contains("invalid params")),
        "missing invalid-params diagnostic: {:?}",
        response.error
    );
}

fn set_params(target: &str, value: &str) -> Option<JsonValue> {
    Some(json!({"target": target, "value": value}))
}

fn target_params(target: &str) -> Option<JsonValue> {
    Some(json!({"target": target}))
}

#[test]
fn eval_requires_params() {
    let response = handle_eval(1, None, &state());
    assert!(!response.ok);
    assert_eq!(response.error.as_deref(), Some("missing params"));
}

macro_rules! eval_invalid_params_case {
    ($name:ident, $params:expr) => {
        #[test]
        fn $name() {
            assert_invalid_params(handle_eval(100, Some($params), &state()));
        }
    };
}

eval_invalid_params_case!(eval_rejects_null_params, JsonValue::Null);
eval_invalid_params_case!(eval_rejects_array_params, json!([]));
eval_invalid_params_case!(eval_rejects_string_params, json!("global_value"));
eval_invalid_params_case!(eval_rejects_missing_expression, json!({}));
eval_invalid_params_case!(
    eval_rejects_unknown_field,
    json!({"expr": "global_value", "frame": 1})
);
eval_invalid_params_case!(eval_rejects_null_expression, json!({"expr": null}));
eval_invalid_params_case!(eval_rejects_numeric_expression, json!({"expr": 7}));
eval_invalid_params_case!(eval_rejects_object_expression, json!({"expr": {}}));

#[test]
fn eval_trims_and_reads_a_global() {
    assert_eq!(
        result(handle_eval(
            2,
            Some(json!({"expr": " \tglobal_value\n"})),
            &state(),
        )),
        json!({"value": "DInt(7)"})
    );
}

#[test]
fn eval_reads_retained_storage_after_global_lookup() {
    assert_eq!(
        result(handle_eval(
            3,
            Some(json!({"expr": "retained_value"})),
            &state(),
        )),
        json!({"value": "DInt(9)"})
    );
}

#[test]
fn eval_rejects_empty_expression() {
    let response = handle_eval(4, Some(json!({"expr": " \t\n"})), &state());
    assert!(!response.ok);
    assert_eq!(response.error.as_deref(), Some("unknown identifier"));
}

#[test]
fn eval_rejects_unknown_identifier() {
    let response = handle_eval(
        5,
        Some(json!({"expr": "definitely_not_declared"})),
        &state(),
    );
    assert!(!response.ok);
    assert_eq!(response.error.as_deref(), Some("unknown identifier"));
}

#[test]
fn declared_retain_targets_keep_control_status_labels_and_apply_to_real_storage() {
    let source = "VAR_GLOBAL RETAIN kept : LINT := 1; END_VAR PROGRAM Main VAR RETAIN member : LINT := 2; END_VAR END_PROGRAM";
    let state = crate::control::tests::hmi_test_state(source);
    let mut runtime = crate::harness::CompileSession::from_source(source)
        .build_runtime()
        .unwrap();
    runtime.set_debug_control(state.debug.clone());
    assert_eq!(
        result(handle_set(901, set_params("retain:kept", "7"), &state)),
        json!({"status":"queued"})
    );
    assert_eq!(
        result(handle_set(
            902,
            set_params("retain:@program/Main/member", "8"),
            &state
        )),
        json!({"status":"queued"})
    );
    runtime.execute_cycle().unwrap();
    state.debug.refresh_snapshot_from_storage_with_abi(
        runtime.storage(),
        runtime.current_time(),
        runtime.prepared().storage_abi(),
    );
    assert_eq!(
        result(handle_eval(
            903,
            Some(json!({"expr":"@program/Main/member"})),
            &state
        )),
        json!({"value":"LInt(8)"})
    );
    assert_eq!(runtime.storage().get_global("kept"), Some(&Value::LInt(7)));
    assert!(runtime.storage().retain().is_empty());
    assert_eq!(
        result(handle_var_force(
            904,
            set_params("retain:kept", "9"),
            &state
        )),
        json!({"status":"forced"})
    );
    assert_eq!(
        result(handle_var_forced(905, &state))["vars"][0]["target"],
        "retain:kept"
    );
    runtime.execute_cycle().unwrap();
    assert_eq!(runtime.storage().get_global("kept"), Some(&Value::LInt(9)));
    assert_eq!(
        result(handle_var_unforce(
            906,
            target_params("retain:kept"),
            &state
        )),
        json!({"status":"released"})
    );
    runtime.write_global("kept", Value::LInt(10)).unwrap();
    runtime.execute_cycle().unwrap();
    assert_eq!(runtime.storage().get_global("kept"), Some(&Value::LInt(10)));
    assert!(!runtime.faulted());
}
