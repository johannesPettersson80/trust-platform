use serde_json::{json, Value as JsonValue};

use super::*;
use crate::debug::{ForcedVarTarget, PendingVarTarget};
use crate::memory::InstanceId;
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

fn assert_unsupported_value(response: ControlResponse) {
    assert!(!response.ok, "unsupported scalar must reject");
    assert!(
        response
            .error
            .as_deref()
            .is_some_and(|error| error.contains("unsupported value")),
        "missing unsupported-value diagnostic: {:?}",
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
fn set_requires_params() {
    let response = handle_set(6, None, &state());
    assert!(!response.ok);
    assert_eq!(response.error.as_deref(), Some("missing params"));
}

macro_rules! set_invalid_params_case {
    ($name:ident, $params:expr) => {
        #[test]
        fn $name() {
            assert_invalid_params(handle_set(200, Some($params), &state()));
        }
    };
}

set_invalid_params_case!(set_rejects_null_params, JsonValue::Null);
set_invalid_params_case!(set_rejects_array_params, json!([]));
set_invalid_params_case!(set_rejects_string_params, json!("global:x"));
set_invalid_params_case!(set_rejects_missing_target, json!({"value": "1"}));
set_invalid_params_case!(set_rejects_missing_value, json!({"target": "global:x"}));
set_invalid_params_case!(
    set_rejects_unknown_field,
    json!({"target": "global:x", "value": "1", "immediate": true})
);
set_invalid_params_case!(
    set_rejects_null_target,
    json!({"target": null, "value": "1"})
);
set_invalid_params_case!(
    set_rejects_numeric_target,
    json!({"target": 1, "value": "1"})
);
set_invalid_params_case!(
    set_rejects_null_value,
    json!({"target": "global:x", "value": null})
);
set_invalid_params_case!(
    set_rejects_boolean_value,
    json!({"target": "global:x", "value": true})
);

#[test]
fn set_trims_and_queues_global_target() {
    let state = state();
    assert_eq!(
        result(handle_set(
            7,
            set_params("global: \tglobal_value\n", "7"),
            &state,
        )),
        json!({"status": "queued"})
    );

    let writes = state.debug.drain_var_writes();
    assert_eq!(writes.len(), 1);
    assert_eq!(writes[0].value, Value::LInt(7));
    match &writes[0].target {
        PendingVarTarget::Global(name) => assert_eq!(name, "global_value"),
        target => panic!("unexpected queued target: {target:?}"),
    }
}

#[test]
fn set_trims_and_queues_retain_target() {
    let state = state();
    assert_eq!(
        result(handle_set(
            8,
            set_params("retain: retained_value ", "FALSE"),
            &state,
        )),
        json!({"status": "queued"})
    );

    let writes = state.debug.drain_var_writes();
    assert_eq!(writes.len(), 1);
    assert_eq!(writes[0].value, Value::Bool(false));
    match &writes[0].target {
        PendingVarTarget::Retain(name) => assert_eq!(name, "retained_value"),
        target => panic!("unexpected queued target: {target:?}"),
    }
}

#[test]
fn set_scalar_grammar_accepts_boolean_case_and_whitespace() {
    let state = state();
    for (target, text) in [
        ("global:a", "TRUE"),
        ("global:b", "false"),
        ("global:c", " \tTrUe\n"),
    ] {
        let response = handle_set(9, set_params(target, text), &state);
        assert!(response.ok, "{text:?} rejected: {:?}", response.error);
    }

    let values = state
        .debug
        .drain_var_writes()
        .into_iter()
        .map(|write| write.value)
        .collect::<Vec<_>>();
    assert_eq!(
        values,
        [Value::Bool(true), Value::Bool(false), Value::Bool(true)]
    );
}

#[test]
fn set_scalar_grammar_accepts_signed_i64_boundaries() {
    let state = state();
    for (target, text) in [
        ("global:a", "0"),
        ("global:b", "+0007"),
        ("global:c", "-0007"),
        ("global:d", "9223372036854775807"),
        ("global:e", "-9223372036854775808"),
    ] {
        let response = handle_set(10, set_params(target, text), &state);
        assert!(response.ok, "{text:?} rejected: {:?}", response.error);
    }

    let values = state
        .debug
        .drain_var_writes()
        .into_iter()
        .map(|write| write.value)
        .collect::<Vec<_>>();
    assert_eq!(
        values,
        [
            Value::LInt(0),
            Value::LInt(7),
            Value::LInt(-7),
            Value::LInt(i64::MAX),
            Value::LInt(i64::MIN),
        ]
    );
}

macro_rules! set_invalid_scalar_case {
    ($name:ident, $text:expr) => {
        #[test]
        fn $name() {
            let state = state();
            assert_unsupported_value(handle_set(300, set_params("global:target", $text), &state));
            assert!(state.debug.drain_var_writes().is_empty());
        }
    };
}

set_invalid_scalar_case!(set_rejects_empty_value, "");
set_invalid_scalar_case!(set_rejects_whitespace_value, " \t\n");
set_invalid_scalar_case!(set_rejects_integer_separator, "1_000");
set_invalid_scalar_case!(set_rejects_hex_prefix, "16#FF");
set_invalid_scalar_case!(set_rejects_decimal_point, "1.0");
set_invalid_scalar_case!(set_rejects_exponent, "1e3");
set_invalid_scalar_case!(set_rejects_quoted_text, "\"TRUE\"");
set_invalid_scalar_case!(set_rejects_positive_i64_overflow, "9223372036854775808");
set_invalid_scalar_case!(set_rejects_negative_i64_overflow, "-9223372036854775809");

#[test]
fn set_rejects_empty_global_name_without_queueing() {
    let state = state();
    let response = handle_set(11, set_params("global: \t", "1"), &state);
    assert!(!response.ok);
    assert_eq!(response.error.as_deref(), Some("missing global name"));
    assert!(state.debug.drain_var_writes().is_empty());
}

#[test]
fn set_rejects_empty_retain_name_without_queueing() {
    let state = state();
    let response = handle_set(12, set_params("retain:\n", "1"), &state);
    assert!(!response.ok);
    assert_eq!(response.error.as_deref(), Some("missing retain name"));
    assert!(state.debug.drain_var_writes().is_empty());
}

#[test]
fn set_rejects_unsupported_target_without_queueing() {
    let state = state();
    for target in [
        "instance:1:value",
        "local:value",
        "GLOBAL:value",
        " global:value",
        "value",
    ] {
        let response = handle_set(13, set_params(target, "1"), &state);
        assert!(!response.ok, "{target:?} unexpectedly accepted");
        assert_eq!(response.error.as_deref(), Some("unsupported target"));
    }
    assert!(state.debug.drain_var_writes().is_empty());
}

#[test]
fn set_replaces_the_same_pending_target_in_place() {
    let state = state();
    assert!(handle_set(14, set_params("global:a", "1"), &state).ok);
    assert!(handle_set(15, set_params("retain:b", "2"), &state).ok);
    assert!(handle_set(16, set_params("global:a", "3"), &state).ok);

    let writes = state.debug.drain_var_writes();
    assert_eq!(writes.len(), 2);
    assert_eq!(writes[0].value, Value::LInt(3));
    assert_eq!(writes[1].value, Value::LInt(2));
    assert!(matches!(
        &writes[0].target,
        PendingVarTarget::Global(name) if name == "a"
    ));
    assert!(matches!(
        &writes[1].target,
        PendingVarTarget::Retain(name) if name == "b"
    ));
}

#[test]
fn rejected_set_preserves_existing_pending_write() {
    let state = state();
    state.debug.enqueue_global_write("a", Value::LInt(7));

    assert_unsupported_value(handle_set(
        17,
        set_params("global:a", "not-a-value"),
        &state,
    ));
    let writes = state.debug.drain_var_writes();
    assert_eq!(writes.len(), 1);
    assert_eq!(writes[0].value, Value::LInt(7));
}

#[derive(Debug, Clone, Copy)]
enum ExpectedTarget<'a> {
    Global(&'a str),
    Retain(&'a str),
    Instance(u32, &'a str),
}

fn assert_parsed_target(text: &str, expected: ExpectedTarget<'_>) {
    let target = parse_var_target(text).unwrap_or_else(|error| panic!("{text:?}: {error}"));
    match (target, expected) {
        (VarTarget::Global(actual), ExpectedTarget::Global(expected)) => {
            assert_eq!(actual, expected);
        }
        (VarTarget::Retain(actual), ExpectedTarget::Retain(expected)) => {
            assert_eq!(actual, expected);
        }
        (
            VarTarget::Instance(actual_id, actual),
            ExpectedTarget::Instance(expected_id, expected),
        ) => {
            assert_eq!(actual_id, expected_id);
            assert_eq!(actual, expected);
        }
        (_, expected) => panic!("target kind did not match {expected:?}"),
    }
}

#[test]
fn var_target_parser_accepts_and_trims_global_and_retain_names() {
    assert_parsed_target(
        "global: \tglobal_value\n",
        ExpectedTarget::Global("global_value"),
    );
    assert_parsed_target(
        "retain: retained_value ",
        ExpectedTarget::Retain("retained_value"),
    );
}

#[test]
fn var_target_parser_accepts_instance_id_boundaries() {
    assert_parsed_target("instance:0:value", ExpectedTarget::Instance(0, "value"));
    assert_parsed_target(
        "instance:4294967295:value",
        ExpectedTarget::Instance(u32::MAX, "value"),
    );
}

#[test]
fn var_target_parser_normalizes_instance_id_and_trims_name() {
    assert_parsed_target(
        "instance:000007: \tvalue\n",
        ExpectedTarget::Instance(7, "value"),
    );
}

#[test]
fn var_target_parser_preserves_colons_after_instance_name_boundary() {
    assert_parsed_target(
        "instance:7:member:field",
        ExpectedTarget::Instance(7, "member:field"),
    );
}

macro_rules! invalid_var_target_case {
    ($name:ident, $target:expr, $error:expr) => {
        #[test]
        fn $name() {
            match parse_var_target($target) {
                Ok(_) => panic!("expected target parse to fail"),
                Err(error) => assert_eq!(error, $error),
            }
        }
    };
}

invalid_var_target_case!(
    var_target_rejects_empty_text,
    "",
    "unsupported target (use global:<name> or retain:<name>)"
);
invalid_var_target_case!(
    var_target_rejects_leading_whitespace,
    " global:value",
    "unsupported target (use global:<name> or retain:<name>)"
);
invalid_var_target_case!(
    var_target_rejects_uppercase_global_prefix,
    "GLOBAL:value",
    "unsupported target (use global:<name> or retain:<name>)"
);
invalid_var_target_case!(
    var_target_rejects_uppercase_retain_prefix,
    "RETAIN:value",
    "unsupported target (use global:<name> or retain:<name>)"
);
invalid_var_target_case!(
    var_target_rejects_uppercase_instance_prefix,
    "INSTANCE:1:value",
    "unsupported target (use global:<name> or retain:<name>)"
);
invalid_var_target_case!(
    var_target_rejects_missing_global_name,
    "global:",
    "missing global name"
);
invalid_var_target_case!(
    var_target_rejects_blank_global_name,
    "global: \t",
    "missing global name"
);
invalid_var_target_case!(
    var_target_rejects_missing_retain_name,
    "retain:",
    "missing retain name"
);
invalid_var_target_case!(
    var_target_rejects_blank_retain_name,
    "retain:\n",
    "missing retain name"
);
invalid_var_target_case!(
    var_target_rejects_missing_instance_id,
    "instance::value",
    "invalid instance id"
);
invalid_var_target_case!(
    var_target_rejects_negative_instance_id,
    "instance:-1:value",
    "invalid instance id"
);
invalid_var_target_case!(
    var_target_rejects_positive_sign_instance_id,
    "instance:+1:value",
    "invalid instance id"
);
invalid_var_target_case!(
    var_target_rejects_nondigit_instance_id,
    "instance:one:value",
    "invalid instance id"
);
invalid_var_target_case!(
    var_target_rejects_overflowing_instance_id,
    "instance:4294967296:value",
    "invalid instance id"
);
invalid_var_target_case!(
    var_target_rejects_missing_instance_name,
    "instance:1",
    "missing instance name"
);
invalid_var_target_case!(
    var_target_rejects_empty_instance_name,
    "instance:1:",
    "missing instance name"
);
invalid_var_target_case!(
    var_target_rejects_blank_instance_name,
    "instance:1: \t",
    "missing instance name"
);

#[test]
fn var_force_requires_params() {
    let response = handle_var_force(18, None, &state());
    assert!(!response.ok);
    assert_eq!(response.error.as_deref(), Some("missing params"));
}

macro_rules! force_invalid_params_case {
    ($name:ident, $params:expr) => {
        #[test]
        fn $name() {
            assert_invalid_params(handle_var_force(400, Some($params), &state()));
        }
    };
}

force_invalid_params_case!(var_force_rejects_null_params, JsonValue::Null);
force_invalid_params_case!(var_force_rejects_array_params, json!([]));
force_invalid_params_case!(var_force_rejects_string_params, json!("global:x"));
force_invalid_params_case!(var_force_rejects_missing_target, json!({"value": "1"}));
force_invalid_params_case!(
    var_force_rejects_missing_value,
    json!({"target": "global:x"})
);
force_invalid_params_case!(
    var_force_rejects_unknown_field,
    json!({"target": "global:x", "value": "1", "sticky": true})
);
force_invalid_params_case!(
    var_force_rejects_null_target,
    json!({"target": null, "value": "1"})
);
force_invalid_params_case!(
    var_force_rejects_object_target,
    json!({"target": {}, "value": "1"})
);
force_invalid_params_case!(
    var_force_rejects_null_value,
    json!({"target": "global:x", "value": null})
);
force_invalid_params_case!(
    var_force_rejects_numeric_value,
    json!({"target": "global:x", "value": 1})
);

#[test]
fn var_force_accepts_each_target_kind_in_arrival_order() {
    let state = state();
    for (target, value) in [
        ("global:a", "TRUE"),
        ("retain:b", "2"),
        ("instance:7:c", "-3"),
    ] {
        assert_eq!(
            result(handle_var_force(19, set_params(target, value), &state)),
            json!({"status": "forced"})
        );
    }

    let forced = state.debug.forced_snapshot().vars;
    assert_eq!(forced.len(), 3);
    assert!(matches!(
        &forced[0].target,
        ForcedVarTarget::Global(name) if name == "a"
    ));
    assert_eq!(forced[0].value, Value::Bool(true));
    assert!(matches!(
        &forced[1].target,
        ForcedVarTarget::Retain(name) if name == "b"
    ));
    assert_eq!(forced[1].value, Value::LInt(2));
    assert!(matches!(
        &forced[2].target,
        ForcedVarTarget::Instance(InstanceId(7), name) if name == "c"
    ));
    assert_eq!(forced[2].value, Value::LInt(-3));
}

#[test]
fn var_force_replaces_same_target_without_duplication() {
    let state = state();
    assert!(handle_var_force(20, set_params("global:a", "1"), &state).ok);
    assert!(handle_var_force(21, set_params("retain:b", "2"), &state).ok);
    assert!(handle_var_force(22, set_params("global:a", "3"), &state).ok);

    let forced = state.debug.forced_snapshot().vars;
    assert_eq!(forced.len(), 2);
    assert_eq!(forced[0].value, Value::LInt(3));
    assert_eq!(forced[1].value, Value::LInt(2));
}

#[test]
fn rejected_var_force_preserves_existing_force() {
    let state = state();
    state.debug.force_global("a", Value::LInt(7));

    assert_unsupported_value(handle_var_force(
        23,
        set_params("global:a", "not-a-value"),
        &state,
    ));
    let forced = state.debug.forced_snapshot().vars;
    assert_eq!(forced.len(), 1);
    assert_eq!(forced[0].value, Value::LInt(7));
}

#[test]
fn invalid_target_never_creates_a_force() {
    let state = state();
    for target in ["global:", "retain: ", "instance:no:c", "instance:1:"] {
        let response = handle_var_force(24, set_params(target, "1"), &state);
        assert!(!response.ok, "{target:?} unexpectedly accepted");
    }
    assert!(state.debug.forced_snapshot().vars.is_empty());
}

#[test]
fn var_unforce_requires_params() {
    let response = handle_var_unforce(25, None, &state());
    assert!(!response.ok);
    assert_eq!(response.error.as_deref(), Some("missing params"));
}

macro_rules! unforce_invalid_params_case {
    ($name:ident, $params:expr) => {
        #[test]
        fn $name() {
            assert_invalid_params(handle_var_unforce(500, Some($params), &state()));
        }
    };
}

unforce_invalid_params_case!(var_unforce_rejects_null_params, JsonValue::Null);
unforce_invalid_params_case!(var_unforce_rejects_array_params, json!([]));
unforce_invalid_params_case!(var_unforce_rejects_string_params, json!("global:x"));
unforce_invalid_params_case!(var_unforce_rejects_missing_target, json!({}));
unforce_invalid_params_case!(
    var_unforce_rejects_unknown_field,
    json!({"target": "global:x", "all": true})
);
unforce_invalid_params_case!(var_unforce_rejects_null_target, json!({"target": null}));
unforce_invalid_params_case!(var_unforce_rejects_numeric_target, json!({"target": 1}));

#[test]
fn var_unforce_removes_only_the_exact_global_target() {
    let state = state();
    state.debug.force_global("a", Value::LInt(1));
    state.debug.force_retain("a", Value::LInt(2));

    assert_eq!(
        result(handle_var_unforce(26, target_params("global: a "), &state)),
        json!({"status": "released"})
    );
    let forced = state.debug.forced_snapshot().vars;
    assert_eq!(forced.len(), 1);
    assert!(matches!(
        &forced[0].target,
        ForcedVarTarget::Retain(name) if name == "a"
    ));
}

#[test]
fn var_unforce_removes_only_the_exact_retain_target() {
    let state = state();
    state.debug.force_retain("a", Value::LInt(1));
    state.debug.force_retain("b", Value::LInt(2));

    assert!(handle_var_unforce(27, target_params("retain:a"), &state).ok);
    let forced = state.debug.forced_snapshot().vars;
    assert_eq!(forced.len(), 1);
    assert!(matches!(
        &forced[0].target,
        ForcedVarTarget::Retain(name) if name == "b"
    ));
}

#[test]
fn var_unforce_removes_only_the_exact_instance_target() {
    let state = state();
    state
        .debug
        .force_instance(InstanceId(7), "a", Value::LInt(1));
    state
        .debug
        .force_instance(InstanceId(8), "a", Value::LInt(2));

    assert!(handle_var_unforce(28, target_params("instance:7:a"), &state).ok);
    let forced = state.debug.forced_snapshot().vars;
    assert_eq!(forced.len(), 1);
    assert!(matches!(
        &forced[0].target,
        ForcedVarTarget::Instance(InstanceId(8), name) if name == "a"
    ));
}

#[test]
fn var_unforce_is_idempotent_for_an_absent_force() {
    let state = state();
    assert_eq!(
        result(handle_var_unforce(
            29,
            target_params("global:absent"),
            &state
        )),
        json!({"status": "released"})
    );
    assert!(state.debug.forced_snapshot().vars.is_empty());
}

#[test]
fn rejected_var_unforce_preserves_existing_forces() {
    let state = state();
    state.debug.force_global("a", Value::LInt(1));

    let response = handle_var_unforce(30, target_params("instance:no:a"), &state);
    assert!(!response.ok);
    let forced = state.debug.forced_snapshot().vars;
    assert_eq!(forced.len(), 1);
    assert_eq!(forced[0].value, Value::LInt(1));
}

#[test]
fn var_forced_empty_projection_has_stable_shape() {
    assert_eq!(result(handle_var_forced(31, &state())), json!({"vars": []}));
}

#[test]
fn var_forced_projection_preserves_order_and_canonicalizes_targets() {
    let state = state();
    state.debug.force_global("a", Value::Bool(true));
    state.debug.force_retain("b", Value::LInt(-2));
    state
        .debug
        .force_instance(InstanceId(7), "c:d", Value::LInt(3));

    assert_eq!(
        result(handle_var_forced(32, &state)),
        json!({
            "vars": [
                {"target": "global:a", "value": "TRUE"},
                {"target": "retain:b", "value": "-2"},
                {"target": "instance:7:c:d", "value": "3"}
            ]
        })
    );
}

#[test]
fn var_forced_projection_keeps_replacement_position() {
    let state = state();
    state.debug.force_global("a", Value::LInt(1));
    state.debug.force_retain("b", Value::LInt(2));
    state.debug.force_global("a", Value::LInt(3));

    assert_eq!(
        result(handle_var_forced(33, &state)),
        json!({
            "vars": [
                {"target": "global:a", "value": "3"},
                {"target": "retain:b", "value": "2"}
            ]
        })
    );
}
