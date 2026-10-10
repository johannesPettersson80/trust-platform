//! Compiler-free scalar native contracts retained through the A4 extraction.

use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::stdlib::{conversions, time, StandardLibrary};
use trust_runtime_core::value::{DateTimeValue, Duration, Value};

#[test]
fn prepared_conversions_preserve_rounding_narrowing_and_source_normalization() {
    for (name, input, expected) in [
        ("LREAL_TO_INT", Value::LReal(2.5), Ok(Value::Int(2))),
        ("LREAL_TO_INT", Value::LReal(3.5), Ok(Value::Int(4))),
        ("LREAL_TO_INT", Value::LReal(-2.5), Ok(Value::Int(-2))),
        ("LREAL_TO_INT", Value::LReal(-3.5), Ok(Value::Int(-4))),
        ("TRUNC", Value::LReal(-2.9), Ok(Value::DInt(-2))),
        ("INT_TO_DINT", Value::DInt(7), Ok(Value::DInt(7))),
        // The explicit source type is checked before converting to a wider result.
        (
            "SINT_TO_DINT",
            Value::DInt(128),
            Err(RuntimeError::Overflow),
        ),
        (
            "DWORD_TO_REAL",
            Value::DWord(0x3f80_0000),
            Ok(Value::Real(1.0)),
        ),
        (
            "INT_TO_STRING",
            Value::Int(42),
            Ok(Value::String("42".into())),
        ),
        (
            "DWORD_TO_TIME",
            Value::DWord(25),
            Ok(Value::Time(Duration::from_millis(25))),
        ),
    ] {
        let spec = conversions::conversion_spec(name).expect("prepared native conversion");
        assert_eq!(
            conversions::call_conversion_spec(spec, &[input]),
            expected,
            "{name}"
        );
    }
}

#[test]
fn conversion_names_keep_aliases_and_generic_rejection_identity() {
    for (short, long) in [
        ("TO_TOD", "to_time_of_day"),
        ("TO_LTOD", "to_ltime_of_day"),
        ("TO_DT", "to_date_and_time"),
        ("TO_LDT", "to_ldate_and_time"),
    ] {
        assert_eq!(
            conversions::conversion_spec(short),
            conversions::conversion_spec(long)
        );
        assert!(conversions::is_conversion_name(long));
    }
    for category in [
        "ANY",
        "ANY_DERIVED",
        "ANY_ELEMENTARY",
        "ANY_MAGNITUDE",
        "ANY_INT",
        "ANY_UNSIGNED",
        "ANY_SIGNED",
        "ANY_REAL",
        "ANY_NUM",
        "ANY_DURATION",
        "ANY_BIT",
        "ANY_CHARS",
        "ANY_STRING",
        "ANY_CHAR",
        "ANY_DATE",
    ] {
        let name = format!("TO_{category}");
        assert_eq!(
            conversions::call_conversion(&name, &[Value::Int(1)]),
            Some(Err(RuntimeError::TypeMismatch)),
            "recognized generic name {name} must keep its conversion error"
        );
    }
    assert_eq!(
        conversions::call_conversion("TO_NOT_A_TYPE", &[Value::Int(1)]),
        None
    );
}

#[test]
fn registry_keeps_user_registration_precedence_over_conversion_fallback() {
    fn custom(_: &[Value]) -> Result<Value, RuntimeError> {
        Ok(Value::DInt(99))
    }
    let mut lib = StandardLibrary::new();
    assert_eq!(
        lib.call("int_to_dint", &[Value::Int(7)]),
        Ok(Value::DInt(7))
    );
    lib.register("INT_TO_DINT", &["IN"], custom);
    assert_eq!(
        lib.call("int_to_dint", &[Value::Int(7)]),
        Ok(Value::DInt(99))
    );
    // The VM's explicit prepared-conversion path remains independent of overrides.
    assert_eq!(
        conversions::call_conversion("INT_TO_DINT", &[Value::Int(7)]),
        Some(Ok(Value::DInt(7)))
    );
    assert_eq!(
        lib.call("missing", &[]),
        Err(RuntimeError::UndefinedFunction("missing".into()))
    );
}

#[test]
fn portable_clock_dispatch_only_requests_wall_time_for_current_dt() {
    let elapsed = Duration::from_nanos(-123);
    assert_eq!(
        time::runtime_clock_value("TIME", elapsed, || panic!("wall clock not needed")),
        Ok(Value::Time(elapsed))
    );
    assert_eq!(
        time::runtime_clock_value("unknown", elapsed, || panic!("wall clock not needed")),
        Err(RuntimeError::UndefinedFunction("unknown".into()))
    );
    assert_eq!(
        time::runtime_clock_value("CURRENT_DT", elapsed, || Ok(DateTimeValue::new(42))),
        Ok(Value::Dt(DateTimeValue::new(42)))
    );
    assert_eq!(
        time::runtime_clock_value("CURRENT_DT", elapsed, || Err(RuntimeError::Overflow)),
        Err(RuntimeError::Overflow)
    );
    assert_eq!(
        time::current_dt_elapsed(core::time::Duration::from_nanos(1_999_999)),
        Ok(DateTimeValue::new(1))
    );
    assert_eq!(
        time::current_dt_from_epoch_elapsed(None),
        Err(RuntimeError::Overflow)
    );
    assert_eq!(
        time::current_dt_elapsed(core::time::Duration::from_millis(i64::MAX as u64)),
        Ok(DateTimeValue::new(i64::MAX))
    );
    assert_eq!(
        time::current_dt_elapsed(core::time::Duration::from_millis(i64::MAX as u64 + 1)),
        Err(RuntimeError::Overflow)
    );
}

#[test]
fn portable_registry_retains_standard_functions_and_diagnostic_value_formatting() {
    let lib = StandardLibrary::new();
    assert_eq!(lib.call("SQRT", &[Value::Real(9.0)]), Ok(Value::Real(3.0)));
    assert_eq!(
        lib.call("LEN", &[Value::String("hello".into())]),
        Ok(Value::Int(5))
    );
    assert_eq!(
        trust_runtime_core::value::format_user_value(&Value::String("pump$'a".into())),
        "'pump$$$'a'"
    );
}
