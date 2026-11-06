//! Compiler-free scalar native contracts retained through the A4 extraction.

use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::stdlib::conversions::ConversionProfile;
use trust_runtime_core::stdlib::{conversions, time, StandardLibrary};
use trust_runtime_core::value::{
    DateTimeValue, Duration, EnumValue, LTimeOfDayValue, TimeOfDayValue, Value,
};

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

#[test]
fn vendor_extension_conversions_cover_enum_bool_time_and_tod() {
    // IEC_DEVIATIONS IEC_DEVIATIONS 2026-10-28: CODESYS-parity conversion entries shared with
    // the compiler accept table.
    let area = Value::Enum(Box::new(EnumValue::from_canonical_parts(
        "eArea".into(),
        "areaB".into(),
        1,
    )));
    for (name, input, expected) in [
        ("TO_STRING", area, Ok(Value::String("areaB".into()))),
        (
            "TO_STRING",
            Value::Bool(true),
            Ok(Value::String("TRUE".into())),
        ),
        (
            "TO_STRING",
            Value::Time(Duration::from_millis(1250)),
            Ok(Value::String("T#1250ms".into())),
        ),
        (
            "TIME_TO_DINT",
            Value::Time(Duration::from_millis(1500)),
            Ok(Value::DInt(1500)),
        ),
        (
            "TIME_TO_UDINT",
            Value::Time(Duration::from_millis(1500)),
            Ok(Value::UDInt(1500)),
        ),
        // The count keeps the destination's low-order bits (specification 07 2.4): 300 ms in a
        // SINT is 300 - 256 = 44, where a checked conversion would overflow.
        (
            "TIME_TO_SINT",
            Value::Time(Duration::from_millis(300)),
            Ok(Value::SInt(44)),
        ),
        (
            "LTIME_TO_LINT",
            Value::LTime(Duration::from_nanos(9)),
            Ok(Value::LInt(9)),
        ),
        (
            "TOD_TO_DWORD",
            Value::Tod(TimeOfDayValue::new(3_600_000)),
            Ok(Value::DWord(3_600_000)),
        ),
        (
            "LTOD_TO_DWORD",
            Value::LTod(LTimeOfDayValue::new(3_600_000_000_000)),
            Ok(Value::DWord(3_600_000)),
        ),
        (
            "TO_STRING",
            Value::String("identity".into()),
            Ok(Value::String("identity".into())),
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
fn round_2_codesys_families_produce_the_measured_vendor_values() {
    // IEC_DEVIATIONS 2026-11-06 (DEV-045, round 2 of DEV-036): every value
    // below is measured on a live e!COCKPIT runtime on a WAGO PFC200
    // (2026-11-06) and recorded in specification 07 2.10. The wrong
    // behaviours this catches: rounding where the vendor truncates
    // (REAL_TO_TIME(2.5) is T#2ms, not the DEV-024 T#3ms), wrapping a
    // negative count where the vendor saturates (T#0ms), faulting a string
    // where the vendor parses (no number 0, trailing ignored), and
    // overflowing an out-of-range string where the vendor keeps the
    // low-order bits (300 -> 44).
    for (name, input, expected) in [
        (
            "REAL_TO_TIME",
            Value::Real(2.5),
            Ok(Value::Time(Duration::from_millis(2))),
        ),
        (
            "REAL_TO_TIME",
            Value::Real(-1.0),
            Ok(Value::Time(Duration::from_millis(0))),
        ),
        (
            "LREAL_TO_TIME",
            Value::LReal(2.5),
            Ok(Value::Time(Duration::from_millis(2))),
        ),
        // The ms-count rule of the integer family (spec 07 2.4).
        (
            "INT_TO_TIME",
            Value::Int(1234),
            Ok(Value::Time(Duration::from_millis(1234))),
        ),
        // A negative integer count follows the measured REAL saturation.
        (
            "INT_TO_TIME",
            Value::Int(-5),
            Ok(Value::Time(Duration::from_millis(0))),
        ),
        (
            "DWORD_TO_TOD",
            Value::DWord(86_400_000),
            Ok(Value::Tod(TimeOfDayValue::new(0))),
        ),
        (
            "DWORD_TO_TOD",
            Value::DWord(u32::MAX),
            Ok(Value::Tod(TimeOfDayValue::new(61_367_295))),
        ),
        ("TO_BYTE", Value::String("abc".into()), Ok(Value::Byte(0))),
        ("TO_BYTE", Value::String("".into()), Ok(Value::Byte(0))),
        ("TO_BYTE", Value::String("300".into()), Ok(Value::Byte(44))),
        (
            "TO_BYTE",
            Value::String("123abc".into()),
            Ok(Value::Byte(123)),
        ),
        // One parse rule per profile: the typed spelling joins the
        // overloaded one under CODESYS.
        (
            "STRING_TO_DINT",
            Value::String("123abc".into()),
            Ok(Value::DInt(123)),
        ),
        (
            "STRING_TO_DINT",
            Value::String("  -42".into()),
            Ok(Value::DInt(-42)),
        ),
        ("TO_SINT", Value::String("2#101".into()), Ok(Value::SInt(5))),
        ("TO_WORD", Value::String("8#17".into()), Ok(Value::Word(15))),
        (
            "TO_DWORD",
            Value::String("16#FF".into()),
            Ok(Value::DWord(255)),
        ),
        // A non-finite REAL keeps the overflow fault (spec 07 2.10).
        (
            "REAL_TO_TIME",
            Value::Real(f32::NAN),
            Err(RuntimeError::Overflow),
        ),
        (
            "REAL_TO_TIME",
            Value::Real(f32::INFINITY),
            Err(RuntimeError::Overflow),
        ),
    ] {
        let spec = conversions::conversion_spec(name).expect("prepared native conversion");
        assert_eq!(
            conversions::call_conversion_spec_with(spec, &[input], ConversionProfile::Codesys),
            expected,
            "{name}"
        );
    }
}

#[test]
fn round_2_keeps_the_strict_iec_parse_and_the_in_day_tod_arithmetic() {
    // The IEC representative parse stays strict outside the profile:
    // trailing characters fault (specification 07 2.7).
    let spec = conversions::conversion_spec("STRING_TO_DINT").expect("prepared native conversion");
    assert_eq!(
        conversions::call_conversion_spec(spec, &[Value::String("123abc".into())]),
        Err(RuntimeError::TypeMismatch),
    );
    // The in-day arithmetic of the vendor midnight idiom works end to end:
    // TOD#18:00:00 minus T#2h as DWORDs reads back as TOD#16:00:00. The
    // idiom does not wrap at midnight (2^32 ms is not a whole day), which
    // the measured out-of-day values in the sibling test pin.
    let to_dword =
        conversions::conversion_spec("TOD_TO_DWORD").expect("prepared native conversion");
    let time_to_dword =
        conversions::conversion_spec("TIME_TO_DWORD").expect("prepared native conversion");
    let dword_to_tod =
        conversions::conversion_spec("DWORD_TO_TOD").expect("prepared native conversion");
    let day =
        conversions::call_conversion_spec(to_dword, &[Value::Tod(TimeOfDayValue::new(64_800_000))])
            .unwrap();
    let hours = conversions::call_conversion_spec(
        time_to_dword,
        &[Value::Time(Duration::from_millis(7_200_000))],
    )
    .unwrap();
    let difference = match (day, hours) {
        (Value::DWord(day), Value::DWord(hours)) => day - hours,
        _ => unreachable!("both helpers yield DWORD"),
    };
    assert_eq!(
        conversions::call_conversion_spec(dword_to_tod, &[Value::DWord(difference)],),
        Ok(Value::Tod(TimeOfDayValue::new(57_600_000))),
    );
}
