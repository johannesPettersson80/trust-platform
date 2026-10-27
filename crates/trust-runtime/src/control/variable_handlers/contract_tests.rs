//! The host's own `eval`, which needs the HIR expression evaluator. `set`, `var.force`,
//! `var.unforce` and `var.forced` are answered by the shared request pipeline
//! (`trust_runtime_services::live_values::var_requests`); their host wiring is tested by
//! `control/tests/live_values_pipeline.rs`.
//!
//! The tests need the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

use serde_json::{json, Value as JsonValue};

use super::*;

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
    crate::control::tests::control_test_state(SOURCE)
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
