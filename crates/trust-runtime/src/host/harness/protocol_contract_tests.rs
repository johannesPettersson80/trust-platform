use super::*;

use std::fmt::Debug;
use std::sync::Arc;

use indexmap::IndexMap;
use serde_json::{json, Number};

use crate::memory::InstanceId;

const MAX_DURATION_MILLIS: i64 = i64::MAX / 1_000_000;

fn counter_program() -> String {
    r#"
PROGRAM Main
VAR
    counter : DINT;
END_VAR
counter := counter + 1;
END_PROGRAM
"#
    .to_string()
}

fn load_counter() -> HarnessAutomation {
    let mut automation = HarnessAutomation::new();
    automation.load_sources(&[counter_program()]).unwrap();
    automation
}

fn assert_invalid_argument<T: Debug>(result: Result<T, HarnessAutomationError>, expected: &str) {
    let error = result.expect_err("expected invalid argument");
    let HarnessAutomationError::InvalidArgument(message) = error else {
        panic!("expected invalid argument, got {error:?}");
    };
    assert!(
        message
            .to_ascii_lowercase()
            .contains(&expected.to_ascii_lowercase()),
        "expected {message:?} to contain {expected:?}"
    );
}

fn assert_roundtrip(value: Value) {
    let encoded = encode_json_value(&value);
    assert_eq!(
        decode_json_value(&encoded).unwrap(),
        value,
        "encoded value was {encoded}"
    );
}

#[test]
fn harness_protocol_contract_automation_starts_unloaded() {
    let automation = HarnessAutomation::new();
    assert!(!automation.is_loaded());
    assert!(!HarnessAutomation::default().is_loaded());
}

#[test]
fn harness_protocol_contract_runtime_operations_fail_not_loaded() {
    let mut automation = HarnessAutomation::new();
    let watch = ["value".to_string()];

    assert_eq!(
        automation.reload_sources(&[counter_program()]),
        Err(HarnessAutomationError::NotLoaded)
    );
    assert_eq!(
        automation.cycle(1, 0, &watch),
        Err(HarnessAutomationError::NotLoaded)
    );
    assert_eq!(
        automation.set_input("value", Value::Bool(true)),
        Err(HarnessAutomationError::NotLoaded)
    );
    assert_eq!(
        automation.get_output("value"),
        Err(HarnessAutomationError::NotLoaded)
    );
    assert_eq!(
        automation.set_access("value", Value::Bool(true)),
        Err(HarnessAutomationError::NotLoaded)
    );
    assert_eq!(
        automation.get_access("value"),
        Err(HarnessAutomationError::NotLoaded)
    );
    assert_eq!(
        automation.bind_direct("value", "%IX0.0"),
        Err(HarnessAutomationError::NotLoaded)
    );
    assert_eq!(
        automation.set_direct_input("%IX0.0", Value::Bool(true)),
        Err(HarnessAutomationError::NotLoaded)
    );
    assert_eq!(
        automation.get_direct_output("%QX0.0"),
        Err(HarnessAutomationError::NotLoaded)
    );
    assert_eq!(
        automation.advance_time(1),
        Err(HarnessAutomationError::NotLoaded)
    );
    assert_eq!(
        automation.restart(RestartMode::Cold),
        Err(HarnessAutomationError::NotLoaded)
    );
    assert_eq!(
        automation.snapshot(&watch),
        Err(HarnessAutomationError::NotLoaded)
    );
    assert_eq!(
        automation.run_until("value", Value::Bool(true), 0, 1, &watch),
        Err(HarnessAutomationError::NotLoaded)
    );
}

#[test]
fn harness_protocol_contract_empty_source_lists_are_rejected_without_loading() {
    let mut automation = HarnessAutomation::new();
    assert_invalid_argument(automation.load_sources(&[]), "must not be empty");
    assert!(!automation.is_loaded());
}

#[test]
fn harness_protocol_contract_load_runs_initial_cycle_and_reports_state() {
    let mut automation = HarnessAutomation::new();
    let summary = automation.load_sources(&[counter_program()]).unwrap();

    assert!(automation.is_loaded());
    assert_eq!(summary.source_count, 1);
    assert_eq!(summary.cycle_count, 1);
    assert_eq!(summary.elapsed_ms, 0);
    assert_eq!(
        automation.get_output("counter").unwrap().value,
        Value::DInt(1)
    );
}

#[test]
fn harness_protocol_contract_failed_compile_does_not_replace_live_session() {
    let mut automation = load_counter();
    let error = automation
        .load_sources(&["PROGRAM Broken".to_string()])
        .unwrap_err();
    assert!(matches!(error, HarnessAutomationError::Compile(_)));
    assert!(automation.is_loaded());
    assert_eq!(
        automation.get_output("counter").unwrap().value,
        Value::DInt(1)
    );
}

#[test]
fn harness_protocol_contract_failed_initial_cycle_does_not_replace_live_session() {
    let mut automation = load_counter();
    let error = automation
        .load_sources(&[r#"
PROGRAM Main
VAR
    result : DINT;
END_VAR
result := 1 / 0;
END_PROGRAM
"#
        .to_string()])
        .unwrap_err();
    assert!(matches!(error, HarnessAutomationError::RuntimeCycle { .. }));
    assert!(automation.is_loaded());
    assert_eq!(
        automation.get_output("counter").unwrap().value,
        Value::DInt(1)
    );
}

#[test]
fn harness_protocol_contract_zero_count_cycle_is_passive() {
    let mut automation = load_counter();
    let snapshot = automation
        .cycle(0, MAX_DURATION_MILLIS, &["counter".to_string()])
        .unwrap();

    assert_eq!(snapshot.cycle_count, 1);
    assert_eq!(snapshot.elapsed_ms, 0);
    assert_eq!(snapshot.values["counter"].value, Some(Value::DInt(1)));
}

#[test]
fn harness_protocol_contract_cycle_advances_before_each_cycle() {
    let mut automation = load_counter();
    let snapshot = automation.cycle(3, 7, &["counter".to_string()]).unwrap();

    assert_eq!(snapshot.cycle_count, 4);
    assert_eq!(snapshot.elapsed_ms, 21);
    assert_eq!(snapshot.values["counter"].value, Some(Value::DInt(4)));
}

#[test]
fn harness_protocol_contract_negative_time_inputs_are_rejected_without_mutation() {
    let mut automation = load_counter();
    assert_invalid_argument(automation.cycle(1, -1, &[]), "dt_ms");
    assert_invalid_argument(automation.advance_time(-1), "duration_ms");
    assert_invalid_argument(
        automation.run_until("counter", Value::DInt(2), -1, 1, &[]),
        "dt_ms",
    );

    let state = automation.snapshot(&[]).unwrap();
    assert_eq!(state.cycle_count, 1);
    assert_eq!(state.elapsed_ms, 0);
}

#[test]
fn harness_protocol_contract_maximum_millisecond_duration_is_accepted() {
    let mut automation = load_counter();
    let state = automation.advance_time(MAX_DURATION_MILLIS).unwrap();
    assert_eq!(state.cycle_count, 1);
    assert_eq!(state.elapsed_ms, MAX_DURATION_MILLIS);
}

#[test]
fn harness_protocol_contract_oversize_cycle_duration_is_rejected_without_mutation() {
    let mut automation = load_counter();
    assert_invalid_argument(automation.cycle(1, MAX_DURATION_MILLIS + 1, &[]), "dt_ms");
    let state = automation.snapshot(&[]).unwrap();
    assert_eq!(state.cycle_count, 1);
    assert_eq!(state.elapsed_ms, 0);
}

#[test]
fn harness_protocol_contract_oversize_advance_duration_is_rejected_without_mutation() {
    let mut automation = load_counter();
    assert_invalid_argument(
        automation.advance_time(MAX_DURATION_MILLIS + 1),
        "duration_ms",
    );
    let state = automation.snapshot(&[]).unwrap();
    assert_eq!(state.cycle_count, 1);
    assert_eq!(state.elapsed_ms, 0);
}

#[test]
fn harness_protocol_contract_oversize_run_until_duration_is_rejected_before_match() {
    let mut automation = load_counter();
    assert_invalid_argument(
        automation.run_until("counter", Value::DInt(1), MAX_DURATION_MILLIS + 1, 0, &[]),
        "dt_ms",
    );
    let state = automation.snapshot(&[]).unwrap();
    assert_eq!(state.cycle_count, 1);
    assert_eq!(state.elapsed_ms, 0);
}

#[test]
fn harness_protocol_contract_snapshot_is_passive_sorted_and_entry_local() {
    let mut automation = load_counter();
    let snapshot = automation
        .snapshot(&[
            "missing".to_string(),
            "counter".to_string(),
            "counter".to_string(),
        ])
        .unwrap();

    assert_eq!(snapshot.cycle_count, 1);
    assert_eq!(snapshot.elapsed_ms, 0);
    assert_eq!(
        snapshot
            .values
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        vec!["counter", "missing"]
    );
    assert_eq!(snapshot.values["counter"].value, Some(Value::DInt(1)));
    assert_eq!(
        snapshot.values["missing"].error.as_ref().unwrap().code(),
        "unresolved_name"
    );
}

#[test]
fn harness_protocol_contract_run_until_prechecks_current_value() {
    let mut automation = load_counter();
    let summary = automation
        .run_until("counter", Value::DInt(1), 10, 5, &["counter".to_string()])
        .unwrap();

    assert_eq!(summary.name, "counter");
    assert_eq!(summary.cycles_ran, 0);
    assert_eq!(summary.cycle_count, 1);
    assert_eq!(summary.elapsed_ms, 0);
    assert_eq!(summary.matched_value, Value::DInt(1));
}

#[test]
fn harness_protocol_contract_run_until_commits_exact_success_budget() {
    let mut automation = load_counter();
    let summary = automation
        .run_until("counter", Value::DInt(3), 5, 2, &[])
        .unwrap();

    assert_eq!(summary.cycles_ran, 2);
    assert_eq!(summary.cycle_count, 3);
    assert_eq!(summary.elapsed_ms, 10);
    assert_eq!(summary.matched_value, Value::DInt(3));
}

#[test]
fn harness_protocol_contract_run_until_timeout_commits_completed_work() {
    let mut automation = load_counter();
    let error = automation
        .run_until("counter", Value::DInt(99), 5, 2, &[])
        .unwrap_err();
    assert_eq!(
        error,
        HarnessAutomationError::RunUntilTimeout {
            name: "counter".to_string(),
            max_cycles: 2,
            expected: Value::DInt(99),
        }
    );

    let snapshot = automation.snapshot(&["counter".to_string()]).unwrap();
    assert_eq!(snapshot.cycle_count, 3);
    assert_eq!(snapshot.elapsed_ms, 10);
    assert_eq!(snapshot.values["counter"].value, Some(Value::DInt(3)));
}

#[test]
fn harness_protocol_contract_error_display_preserves_stable_context() {
    assert_eq!(
        HarnessAutomationError::NotLoaded.to_string(),
        "Harness is not loaded. Call load first."
    );
    assert_eq!(
        HarnessAutomationError::RuntimeCycle {
            message: "cycle failed".to_string(),
            errors: vec!["first".to_string(), "second".to_string()],
        }
        .to_string(),
        "cycle failed: first; second"
    );
    assert_eq!(
        HarnessAutomationError::RunUntilTimeout {
            name: "ready".to_string(),
            max_cycles: 7,
            expected: Value::Bool(true),
        }
        .to_string(),
        "run_until exceeded 7 cycles before 'ready' matched the expected value"
    );
}

#[test]
fn harness_protocol_contract_runtime_error_helpers_preserve_order_and_class() {
    assert_eq!(
        render_runtime_errors(&[RuntimeError::Overflow, RuntimeError::TypeMismatch]),
        vec!["arithmetic overflow", "type mismatch"]
    );
    assert_eq!(
        runtime_to_error(RuntimeError::Overflow),
        HarnessAutomationError::Runtime("arithmetic overflow".to_string())
    );
}

#[test]
fn harness_protocol_contract_untyped_primitives_decode_deterministically() {
    assert_eq!(decode_json_value(&json!(true)).unwrap(), Value::Bool(true));
    assert_eq!(
        decode_json_value(&json!("text")).unwrap(),
        Value::String("text".into())
    );
    assert_eq!(decode_json_value(&JsonValue::Null).unwrap(), Value::Null);
    assert_eq!(decode_json_value(&json!(1.5)).unwrap(), Value::LReal(1.5));
}

#[test]
fn harness_protocol_contract_untyped_signed_numbers_use_smallest_container() {
    for (json_value, expected) in [
        (json!(-128), Value::SInt(-128)),
        (json!(-129), Value::Int(-129)),
        (json!(-32769), Value::DInt(-32769)),
        (json!(-2147483649_i64), Value::LInt(-2147483649)),
        (json!(127), Value::SInt(127)),
        (json!(128), Value::Int(128)),
        (json!(32768), Value::DInt(32768)),
        (json!(2147483648_i64), Value::LInt(2147483648)),
    ] {
        assert_eq!(decode_json_value(&json_value).unwrap(), expected);
    }
}

#[test]
fn harness_protocol_contract_untyped_large_unsigned_number_uses_ulint() {
    let number = Number::from((i64::MAX as u64) + 1);
    assert_eq!(
        decode_json_value(&JsonValue::Number(number)).unwrap(),
        Value::ULInt((i64::MAX as u64) + 1)
    );
}

#[test]
fn harness_protocol_contract_typed_tags_are_ascii_case_insensitive() {
    assert_eq!(
        decode_json_value(&json!({"type": "bOoL", "value": true})).unwrap(),
        Value::Bool(true)
    );
    assert_eq!(
        decode_json_value(&json!({"type": "wOrD", "value": 42})).unwrap(),
        Value::Word(42)
    );
}

#[test]
fn harness_protocol_contract_typed_object_requires_string_type() {
    assert_invalid_argument(decode_json_value(&json!({})), "string 'type'");
    assert_invalid_argument(
        decode_json_value(&json!({"type": 7, "value": true})),
        "string 'type'",
    );
}

#[test]
fn harness_protocol_contract_bool_requires_boolean_payload() {
    assert_eq!(
        decode_json_value(&json!({"type": "BOOL", "value": false})).unwrap(),
        Value::Bool(false)
    );
    assert_invalid_argument(
        decode_json_value(&json!({"type": "BOOL", "value": 0})),
        "boolean 'value'",
    );
}

#[test]
fn harness_protocol_contract_signed_integer_widths_are_checked() {
    for (kind, minimum, maximum) in [
        ("SINT", i8::MIN as i64, i8::MAX as i64),
        ("INT", i16::MIN as i64, i16::MAX as i64),
        ("DINT", i32::MIN as i64, i32::MAX as i64),
        ("LINT", i64::MIN, i64::MAX),
    ] {
        assert!(decode_json_value(&json!({"type": kind, "value": minimum})).is_ok());
        assert!(decode_json_value(&json!({"type": kind, "value": maximum})).is_ok());
    }
    assert_invalid_argument(
        decode_json_value(&json!({"type": "SINT", "value": 128})),
        "out of range",
    );
    assert_invalid_argument(
        decode_json_value(&json!({"type": "INT", "value": -32769})),
        "out of range",
    );
    assert_invalid_argument(
        decode_json_value(&json!({"type": "DINT", "value": 2147483648_i64})),
        "out of range",
    );
}

#[test]
fn harness_protocol_contract_unsigned_integer_and_bit_widths_are_checked() {
    for (kind, maximum) in [
        ("USINT", u8::MAX as u64),
        ("UINT", u16::MAX as u64),
        ("UDINT", u32::MAX as u64),
        ("ULINT", u64::MAX),
        ("BYTE", u8::MAX as u64),
        ("WORD", u16::MAX as u64),
        ("DWORD", u32::MAX as u64),
        ("LWORD", u64::MAX),
    ] {
        assert!(decode_json_value(&json!({"type": kind, "value": maximum})).is_ok());
    }
    assert_invalid_argument(
        decode_json_value(&json!({"type": "USINT", "value": 256})),
        "out of range",
    );
    assert_invalid_argument(
        decode_json_value(&json!({"type": "WORD", "value": 65536})),
        "out of range",
    );
    assert_invalid_argument(
        decode_json_value(&json!({"type": "DWORD", "value": 4294967296_u64})),
        "out of range",
    );
    assert_invalid_argument(
        decode_json_value(&json!({"type": "UINT", "value": -1})),
        "unsigned integer",
    );
}

#[test]
fn harness_protocol_contract_float_domains_are_checked() {
    assert_eq!(
        decode_json_value(&json!({"type": "REAL", "value": 1.25})).unwrap(),
        Value::Real(1.25)
    );
    assert_eq!(
        decode_json_value(&json!({"type": "LREAL", "value": -2.5})).unwrap(),
        Value::LReal(-2.5)
    );
    assert_invalid_argument(
        decode_json_value(&json!({"type": "REAL", "value": f64::MAX})),
        "out of range",
    );
    assert_invalid_argument(
        decode_json_value(&json!({"type": "REAL", "value": "1.0"})),
        "numeric 'value'",
    );
}

#[test]
fn harness_protocol_contract_temporal_values_roundtrip_at_signed_boundaries() {
    for value in [
        Value::Time(Duration::from_nanos(i64::MIN)),
        Value::LTime(Duration::from_nanos(i64::MAX)),
        Value::Date(DateValue::new(i64::MIN)),
        Value::LDate(LDateValue::new(i64::MAX)),
        Value::Tod(TimeOfDayValue::new(-1)),
        Value::LTod(LTimeOfDayValue::new(1)),
        Value::Dt(DateTimeValue::new(-2)),
        Value::Ldt(LDateTimeValue::new(2)),
    ] {
        assert_roundtrip(value);
    }
}

#[test]
fn harness_protocol_contract_temporal_values_require_integer_payloads() {
    for (kind, field) in [
        ("TIME", "nanos"),
        ("LTIME", "nanos"),
        ("DATE", "ticks"),
        ("LDATE", "nanos"),
        ("TOD", "ticks"),
        ("LTOD", "nanos"),
        ("DT", "ticks"),
        ("LDT", "nanos"),
    ] {
        assert_invalid_argument(
            decode_json_value(&json!({"type": kind, (field): 1.5})),
            "integer",
        );
    }
}

#[test]
fn harness_protocol_contract_string_values_require_string_payloads() {
    assert_roundtrip(Value::String("narrow".into()));
    assert_roundtrip(Value::WString("wide å".to_string()));
    assert_invalid_argument(
        decode_json_value(&json!({"type": "STRING", "value": 7})),
        "string 'value'",
    );
    assert_invalid_argument(
        decode_json_value(&json!({"type": "WSTRING"})),
        "string 'value'",
    );
}

#[test]
fn harness_protocol_contract_char_requires_one_ascii_scalar() {
    assert_roundtrip(Value::Char(b'Z'));
    for value in ["", "AB", "å"] {
        assert_invalid_argument(
            decode_json_value(&json!({"type": "CHAR", "value": value})),
            "one-character",
        );
    }
}

#[test]
fn harness_protocol_contract_wchar_requires_one_bmp_scalar() {
    assert_roundtrip(Value::WChar('å' as u16));
    for value in ["", "AB", "😀"] {
        assert_invalid_argument(
            decode_json_value(&json!({"type": "WCHAR", "value": value})),
            if value == "😀" {
                "out of range"
            } else {
                "one-character"
            },
        );
    }
}

#[test]
fn harness_protocol_contract_scalar_encodings_are_canonical_and_roundtrip() {
    for value in [
        Value::Bool(true),
        Value::SInt(i8::MIN),
        Value::Int(i16::MIN),
        Value::DInt(i32::MIN),
        Value::LInt(i64::MIN),
        Value::USInt(u8::MAX),
        Value::UInt(u16::MAX),
        Value::UDInt(u32::MAX),
        Value::ULInt(u64::MAX),
        Value::Real(1.5),
        Value::LReal(-2.25),
        Value::Byte(u8::MAX),
        Value::Word(u16::MAX),
        Value::DWord(u32::MAX),
        Value::LWord(u64::MAX),
        Value::Null,
    ] {
        assert_roundtrip(value);
    }
}

#[test]
fn harness_protocol_contract_untyped_array_is_zero_based_and_recursive() {
    let value = decode_json_value(&json!([true, 7, "text"])).unwrap();
    let Value::Array(array) = value else {
        panic!("expected array");
    };
    assert_eq!(array.dimensions(), &[(0, 2)]);
    assert_eq!(
        array.elements(),
        &[
            Value::Bool(true),
            Value::SInt(7),
            Value::String("text".into())
        ]
    );
}

#[test]
fn harness_protocol_contract_empty_untyped_array_is_rejected() {
    assert_invalid_argument(decode_json_value(&json!([])), "invalid json array");
}

#[test]
fn harness_protocol_contract_typed_array_roundtrips_dimensions_and_elements() {
    let value = Value::Array(Box::new(
        ArrayValue::from_untyped_parts(
            vec![Value::Int(1), Value::Int(2), Value::Int(3), Value::Int(4)],
            vec![(-1, 0), (3, 4)],
        )
        .unwrap(),
    ));
    assert_roundtrip(value);
}

#[test]
fn harness_protocol_contract_typed_array_requires_dimension_pairs() {
    for value in [
        json!({"type": "ARRAY", "elements": []}),
        json!({"type": "ARRAY", "dimensions": "bad", "elements": []}),
        json!({"type": "ARRAY", "dimensions": [0], "elements": []}),
        json!({"type": "ARRAY", "dimensions": [[0]], "elements": []}),
        json!({"type": "ARRAY", "dimensions": [["0", 1]], "elements": [1, 2]}),
        json!({"type": "ARRAY", "dimensions": [[0, "1"]], "elements": [1, 2]}),
    ] {
        assert!(matches!(
            decode_json_value(&value),
            Err(HarnessAutomationError::InvalidArgument(_))
        ));
    }
}

#[test]
fn harness_protocol_contract_typed_array_requires_elements_and_exact_shape() {
    assert_invalid_argument(
        decode_json_value(&json!({"type": "ARRAY", "dimensions": [[0, 1]]})),
        "elements",
    );
    assert_invalid_argument(
        decode_json_value(&json!({"type": "ARRAY", "dimensions": [[0, 1]], "elements": [1]})),
        "element count",
    );
    assert_invalid_argument(
        decode_json_value(&json!({"type": "ARRAY", "dimensions": [[2, 1]], "elements": []})),
        "invalid array",
    );
}

#[test]
fn harness_protocol_contract_struct_roundtrips_identity_and_fields() {
    let mut fields = IndexMap::new();
    fields.insert("enabled".into(), Value::Bool(true));
    fields.insert("count".into(), Value::Int(7));
    let value = Value::Struct(Arc::new(StructValue::from_untyped_parts(
        "State".into(),
        fields,
    )));
    assert_roundtrip(value);
}

#[test]
fn harness_protocol_contract_struct_requires_name_object_and_valid_fields() {
    assert_invalid_argument(
        decode_json_value(&json!({"type": "STRUCT", "fields": {}})),
        "type_name",
    );
    assert_invalid_argument(
        decode_json_value(&json!({"type": "STRUCT", "type_name": "State"})),
        "fields",
    );
    assert_invalid_argument(
        decode_json_value(
            &json!({"type": "STRUCT", "type_name": "State", "fields": {"x": {"type": "NOPE"}}}),
        ),
        "unsupported typed value",
    );
}

#[test]
fn harness_protocol_contract_enum_roundtrips_all_identity_fields() {
    let value = Value::Enum(Box::new(EnumValue::from_canonical_parts(
        "Mode".into(),
        "Auto".into(),
        7,
    )));
    let encoded = encode_json_value(&value);
    assert_eq!(
        encoded,
        json!({
            "type": "ENUM",
            "type_name": "Mode",
            "variant": "Auto",
            "numeric": 7
        })
    );
    let Value::Enum(decoded) = decode_json_value(&encoded).unwrap() else {
        panic!("expected enum");
    };
    assert_eq!(decoded.type_name(), "Mode");
    assert_eq!(decoded.variant_name(), "Auto");
    assert_eq!(decoded.numeric_value(), 7);
}

#[test]
fn harness_protocol_contract_enum_requires_all_typed_fields() {
    for (value, expected) in [
        (
            json!({"type": "ENUM", "variant": "Auto", "numeric": 1}),
            "type_name",
        ),
        (
            json!({"type": "ENUM", "type_name": "Mode", "numeric": 1}),
            "variant",
        ),
        (
            json!({"type": "ENUM", "type_name": "Mode", "variant": "Auto"}),
            "numeric",
        ),
    ] {
        assert_invalid_argument(decode_json_value(&value), expected);
    }
}

#[test]
fn harness_protocol_contract_null_ignores_no_payload_and_is_canonical() {
    assert_eq!(
        decode_json_value(&json!({"type": "NULL"})).unwrap(),
        Value::Null
    );
    assert_eq!(encode_json_value(&Value::Null), json!({"type": "NULL"}));
}

#[test]
fn harness_protocol_contract_reference_and_instance_encodings_are_observable_only() {
    assert_eq!(
        encode_json_value(&Value::Reference(None)),
        json!({"type": "REFERENCE", "value": null})
    );
    assert_eq!(
        encode_json_value(&Value::Instance(InstanceId(42))),
        json!({"type": "INSTANCE", "value": 42})
    );
    assert_invalid_argument(
        decode_json_value(&json!({"type": "REFERENCE", "value": null})),
        "unsupported typed value",
    );
    assert_invalid_argument(
        decode_json_value(&json!({"type": "INSTANCE", "value": 42})),
        "unsupported typed value",
    );
}

#[test]
fn harness_protocol_contract_unknown_typed_value_kind_is_rejected() {
    assert_invalid_argument(
        decode_json_value(&json!({"type": "POINTER", "value": 1})),
        "unsupported typed value",
    );
}
