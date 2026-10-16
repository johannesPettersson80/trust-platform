//! Legacy 1.1/2.0 physical temporal leaves at the 4-byte boundary. A located short temporal value
//! is stored as a signed 32-bit integer: TIME in nanoseconds, DATE, TOD and DT in profile ticks
//! (value/overlap/scalar.rs, the i32::try_from check). A write outside that range is refused as
//! Overflow, never truncated or wrapped, and it changes neither the value nor the backing bytes
//! (spec 12 §4.1; spec 34 §4.3.1 acceptance: atomic refusal at legacy physical boundaries).
#![cfg(feature = "full")]
use trust_runtime::{
    bytecode::BytecodeVersion,
    error::RuntimeError,
    harness::{parse_debug_expression_with_abi, parse_debug_lvalue_with_abi, CompileSession},
    value::{DateTimeValue, DateValue, Duration, StorageAbi, TimeOfDayValue, Value},
    Runtime,
};

/// `slot` shares its four bytes with the start of `raw`, as in the existing legacy overlay tests.
/// The first source is the 1.1 artifact, which has no relative layout; the second supplies it.
fn sources(declared: &str) -> (String, String) {
    (
        format!(
            "TYPE OverlayType : STRUCT slot : {declared}; raw : LINT; END_STRUCT; END_TYPE \
             VAR_GLOBAL RETAIN overlay : OverlayType; END_VAR PROGRAM Main END_PROGRAM"
        ),
        format!(
            "TYPE OverlayType : STRUCT OVERLAP slot AT %B0 : {declared}; raw AT %B0 : LINT; \
             END_STRUCT; END_TYPE VAR_GLOBAL RETAIN overlay : OverlayType; END_VAR \
             PROGRAM Main END_PROGRAM"
        ),
    )
}

fn legacy_overlay(declared: &str) -> Runtime {
    let (plain, context) = sources(declared);
    let original = CompileSession::from_source(&plain)
        .build_bytecode_module_for_version(BytecodeVersion::LEGACY)
        .unwrap();
    CompileSession::from_source(&context)
        .build_runtime_from_bytecode(&original, None)
        .unwrap()
}

fn evaluate(runtime: &Runtime, text: &str) -> Value {
    let mut registry = runtime.registry().clone();
    let expression = parse_debug_expression_with_abi(
        text,
        &mut registry,
        runtime.profile(),
        &[],
        runtime.prepared().storage_abi(),
    )
    .unwrap();
    runtime
        .evaluate_debug_expression(&expression, &registry, None)
        .unwrap()
}

/// Each edge is tested: the signed 32-bit maximum and minimum are stored and read back exactly,
/// and the value one past either end is refused. A refused write leaves the overlay unchanged.
fn check_physical_boundaries(declared: &str, make: fn(i64) -> Value) {
    let mut runtime = legacy_overlay(declared);
    let mut registry = runtime.registry().clone();
    let target = parse_debug_lvalue_with_abi(
        "overlay.slot",
        &mut registry,
        runtime.profile(),
        &[],
        StorageAbi::LegacyTemporal32,
    )
    .unwrap();
    for (value, accepted) in [
        (i64::from(i32::MAX), true),
        (i64::from(i32::MIN), true),
        (i64::from(i32::MAX) + 1, false),
        (i64::from(i32::MIN) - 1, false),
    ] {
        let before = runtime.storage().get_global("overlay").cloned();
        let result = runtime.write_lvalue(&target, make(value), None);
        if accepted {
            assert!(result.is_ok(), "{declared} {value}");
            assert_eq!(
                evaluate(&runtime, "overlay.slot"),
                make(value),
                "{declared} {value}"
            );
        } else {
            assert!(
                matches!(result, Err(RuntimeError::Overflow)),
                "{declared} {value}"
            );
            assert_eq!(
                runtime.storage().get_global("overlay").cloned(),
                before,
                "{declared} {value}"
            );
        }
    }
}

#[test]
fn legacy_time_slot_accepts_the_signed_32_bit_nanosecond_range_only() {
    check_physical_boundaries("TIME", |nanos| Value::Time(Duration::from_nanos(nanos)));
}

#[test]
fn legacy_date_tod_and_dt_slots_accept_the_signed_32_bit_tick_range_only() {
    check_physical_boundaries("DATE", |ticks| Value::Date(DateValue::new(ticks)));
    check_physical_boundaries("TOD", |ticks| Value::Tod(TimeOfDayValue::new(ticks)));
    check_physical_boundaries("DT", |ticks| Value::Dt(DateTimeValue::new(ticks)));
}
