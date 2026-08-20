use trust_runtime::error::RuntimeError;
use trust_runtime::stdlib::conversions::is_conversion_name;
use trust_runtime::stdlib::{StandardLibrary, StdParams};
use trust_runtime::value::{
    DateTimeValue, DateValue, Duration, LDateTimeValue, LTimeOfDayValue, TimeOfDayValue, Value,
};

fn library() -> StandardLibrary {
    StandardLibrary::new()
}

#[test]
fn conversion_names_are_case_insensitive_and_unknown_names_remain_undefined() {
    let lib = library();

    for name in [
        "int_to_dint",
        "To_LrEaL",
        "real_trunc_int",
        "trunc_dint",
        "usint_to_bcd_byte",
        "word_bcd_to_uint",
    ] {
        assert!(is_conversion_name(name), "{name}");
    }
    assert!(!is_conversion_name("INT_TO_NOT_A_TYPE"));
    assert!(matches!(
        lib.call("INT_TO_NOT_A_TYPE", &[Value::Int(1)]),
        Err(RuntimeError::UndefinedFunction(_))
    ));
    assert_eq!(
        lib.call("int_to_dint", &[Value::Int(7)]),
        Ok(Value::DInt(7))
    );
}

#[test]
fn conversions_require_exactly_one_argument() {
    let lib = library();

    assert!(matches!(
        lib.call("INT_TO_DINT", &[]),
        Err(RuntimeError::InvalidArgumentCount {
            expected: 1,
            got: 0
        })
    ));
    assert!(matches!(
        lib.call("INT_TO_DINT", &[Value::Int(1), Value::Int(2)]),
        Err(RuntimeError::InvalidArgumentCount {
            expected: 1,
            got: 2
        })
    ));
}

#[test]
fn named_source_is_validated_and_normalized_before_destination_conversion() {
    let lib = library();

    assert_eq!(
        lib.call("INT_TO_DINT", &[Value::DInt(7)]),
        Ok(Value::DInt(7)),
        "a widened stored scalar is normalized through the named source"
    );
    assert_eq!(
        lib.call("SINT_TO_BYTE", &[Value::Int(256)]),
        Err(RuntimeError::Overflow),
        "normalization to SINT must occur before the BYTE transfer"
    );
    assert_eq!(
        lib.call("USINT_TO_BCD_BYTE", &[Value::UInt(42)]),
        Err(RuntimeError::TypeMismatch),
        "typed BCD sources require their exact declared type"
    );
}

#[test]
fn real_to_integer_rounds_ties_to_even_for_positive_and_negative_values() {
    let lib = library();

    for (input, expected) in [
        (1.5, Value::Int(2)),
        (2.5, Value::Int(2)),
        (3.5, Value::Int(4)),
        (-1.5, Value::Int(-2)),
        (-2.5, Value::Int(-2)),
        (-3.5, Value::Int(-4)),
    ] {
        assert_eq!(
            lib.call("LREAL_TO_INT", &[Value::LReal(input)]),
            Ok(expected),
            "{input}"
        );
    }
}

#[test]
fn truncation_forms_truncate_toward_zero_and_require_real_input() {
    let lib = library();

    assert_eq!(lib.call("TRUNC", &[Value::Real(3.9)]), Ok(Value::DInt(3)));
    assert_eq!(
        lib.call("TRUNC_INT", &[Value::LReal(-3.9)]),
        Ok(Value::Int(-3))
    );
    assert_eq!(
        lib.call("LREAL_TRUNC_LINT", &[Value::LReal(-9.9)]),
        Ok(Value::LInt(-9))
    );
    assert_eq!(
        lib.call("TRUNC_INT", &[Value::Int(3)]),
        Err(RuntimeError::TypeMismatch)
    );
}

#[test]
fn integer_conversions_check_signed_unsigned_and_width_boundaries() {
    let lib = library();

    assert_eq!(
        lib.call("DINT_TO_SINT", &[Value::DInt(127)]),
        Ok(Value::SInt(127))
    );
    assert_eq!(
        lib.call("DINT_TO_SINT", &[Value::DInt(128)]),
        Err(RuntimeError::Overflow)
    );
    assert_eq!(
        lib.call("INT_TO_UINT", &[Value::Int(-1)]),
        Err(RuntimeError::Overflow)
    );
    assert_eq!(
        lib.call("ULINT_TO_LINT", &[Value::ULInt(i64::MAX as u64 + 1)]),
        Err(RuntimeError::Overflow)
    );
}

#[test]
fn integer_text_above_ulint_max_reports_overflow_instead_of_wrapping() {
    let lib = library();

    assert_eq!(
        lib.call(
            "STRING_TO_ULINT",
            &[Value::String("18446744073709551616".into())]
        ),
        Err(RuntimeError::Overflow)
    );
}

#[test]
fn lreal_above_ulint_max_reports_overflow_instead_of_wrapping() {
    let lib = library();
    let first_unrepresentable = (u64::MAX as f64) + 1.0;

    assert_eq!(
        lib.call("LREAL_TO_ULINT", &[Value::LReal(first_unrepresentable)]),
        Err(RuntimeError::Overflow)
    );
}

#[test]
fn bit_string_width_changes_preserve_rightmost_bits_and_zero_extend() {
    let lib = library();

    assert_eq!(
        lib.call("LWORD_TO_BYTE", &[Value::LWord(0x1234_5678_9abc_def0)]),
        Ok(Value::Byte(0xf0))
    );
    assert_eq!(
        lib.call("BYTE_TO_LWORD", &[Value::Byte(0x80)]),
        Ok(Value::LWord(0x80))
    );
    assert_eq!(
        lib.call("TO_WORD", &[Value::DInt(-1)]),
        Ok(Value::Word(0xffff))
    );
}

#[test]
fn bit_string_to_signed_integer_sign_extends_at_destination_width() {
    let lib = library();

    assert_eq!(
        lib.call("BYTE_TO_SINT", &[Value::Byte(0x80)]),
        Ok(Value::SInt(-128))
    );
    assert_eq!(
        lib.call("WORD_TO_SINT", &[Value::Word(0x0180)]),
        Ok(Value::SInt(-128))
    );
    assert_eq!(
        lib.call("DWORD_TO_INT", &[Value::DWord(0x0000_8000)]),
        Ok(Value::Int(i16::MIN))
    );
}

#[test]
fn finite_real_bit_transfers_preserve_exact_payload_bits() {
    let lib = library();
    let real = -0.0_f32;
    let lreal = -1234.5_f64;

    assert_eq!(
        lib.call("REAL_TO_DWORD", &[Value::Real(real)]),
        Ok(Value::DWord(real.to_bits()))
    );
    assert!(matches!(
        lib.call("DWORD_TO_REAL", &[Value::DWord(real.to_bits())]),
        Ok(Value::Real(value)) if value.to_bits() == real.to_bits()
    ));
    assert_eq!(
        lib.call("LREAL_TO_LWORD", &[Value::LReal(lreal)]),
        Ok(Value::LWord(lreal.to_bits()))
    );
    assert!(matches!(
        lib.call("LWORD_TO_LREAL", &[Value::LWord(lreal.to_bits())]),
        Ok(Value::LReal(value)) if value.to_bits() == lreal.to_bits()
    ));
}

#[test]
fn non_finite_real_values_and_bit_payloads_are_rejected() {
    let lib = library();

    for (name, value) in [
        ("REAL_TO_INT", Value::Real(f32::NAN)),
        ("LREAL_TO_DINT", Value::LReal(f64::INFINITY)),
        ("DWORD_TO_REAL", Value::DWord(f32::NEG_INFINITY.to_bits())),
        ("LWORD_TO_LREAL", Value::LWord(f64::NAN.to_bits())),
    ] {
        assert_eq!(
            lib.call(name, &[value]),
            Err(RuntimeError::Overflow),
            "{name}"
        );
    }
}

#[test]
fn integer_text_accepts_whitespace_separators_radix_and_digit_sign() {
    let lib = library();

    assert_eq!(
        lib.call("STRING_TO_DINT", &[Value::String("  16#7_FF  ".into())]),
        Ok(Value::DInt(2047))
    );
    assert_eq!(
        lib.call("WSTRING_TO_INT", &[Value::WString("16#-80".into())]),
        Ok(Value::Int(-128))
    );
    assert_eq!(
        lib.call("STRING_TO_UINT", &[Value::String("+1_024".into())]),
        Ok(Value::UInt(1024))
    );
}

#[test]
fn malformed_or_out_of_range_radix_text_returns_error_without_panicking() {
    let lib = library();

    for text in ["1#0", "37#0", "2#2", "16#", "not-a-number", ""] {
        assert_eq!(
            lib.call("STRING_TO_DINT", &[Value::String(text.into())]),
            Err(RuntimeError::TypeMismatch),
            "{text:?}"
        );
    }
}

#[test]
fn real_text_accepts_finite_exponents_and_rejects_non_finite_results() {
    let lib = library();

    assert_eq!(
        lib.call("STRING_TO_LREAL", &[Value::String(" 1_25e-2 ".into())]),
        Ok(Value::LReal(1.25))
    );
    for text in ["NaN", "inf", "-inf", "1e9999"] {
        assert_eq!(
            lib.call("STRING_TO_LREAL", &[Value::String(text.into())]),
            Err(RuntimeError::Overflow),
            "{text}"
        );
    }
}

#[test]
fn numeric_text_output_is_decimal_and_integral_real_keeps_fraction_marker() {
    let lib = library();

    assert_eq!(
        lib.call("LWORD_TO_STRING", &[Value::LWord(0xff)]),
        Ok(Value::String("255".into()))
    );
    assert_eq!(
        lib.call("REAL_TO_STRING", &[Value::Real(2.0)]),
        Ok(Value::String("2.0".into()))
    );
    assert_eq!(
        lib.call("LREAL_TO_WSTRING", &[Value::LReal(-0.0)]),
        Ok(Value::WString("-0.0".into()))
    );
}

#[test]
fn character_conversions_require_one_scalar_and_check_target_width() {
    let lib = library();

    assert_eq!(
        lib.call("STRING_TO_CHAR", &[Value::String("Ä".into())]),
        Ok(Value::Char(0xc4))
    );
    assert_eq!(
        lib.call("WSTRING_TO_WCHAR", &[Value::WString("Ω".into())]),
        Ok(Value::WChar('Ω' as u16))
    );
    assert_eq!(
        lib.call("STRING_TO_CHAR", &[Value::String("".into())]),
        Err(RuntimeError::TypeMismatch)
    );
    assert_eq!(
        lib.call("STRING_TO_CHAR", &[Value::String("AB".into())]),
        Err(RuntimeError::TypeMismatch)
    );
    assert_eq!(
        lib.call("WCHAR_TO_CHAR", &[Value::WChar('Ω' as u16)]),
        Err(RuntimeError::Overflow)
    );
}

#[test]
fn time_dword_extension_uses_checked_whole_milliseconds() {
    let lib = library();

    assert_eq!(
        lib.call(
            "TIME_TO_DWORD",
            &[Value::Time(Duration::from_nanos(1_999_999))]
        ),
        Ok(Value::DWord(1))
    );
    assert_eq!(
        lib.call("TIME_TO_DWORD", &[Value::Time(Duration::from_millis(-1))]),
        Err(RuntimeError::Overflow)
    );
    assert_eq!(
        lib.call(
            "TIME_TO_DWORD",
            &[Value::Time(Duration::from_millis(i64::from(u32::MAX) + 1))]
        ),
        Err(RuntimeError::Overflow)
    );
    assert_eq!(
        lib.call("DWORD_TO_TIME", &[Value::DWord(u32::MAX)]),
        Ok(Value::Time(Duration::from_millis(i64::from(u32::MAX))))
    );
}

#[test]
fn datetime_extraction_uses_euclidean_days_before_epoch() {
    let lib = library();
    let instant = Value::Dt(DateTimeValue::new(-1));

    assert_eq!(
        lib.call("DT_TO_DATE", std::slice::from_ref(&instant)),
        Ok(Value::Date(DateValue::new(-86_400_000)))
    );
    assert_eq!(
        lib.call("DT_TO_TOD", std::slice::from_ref(&instant)),
        Ok(Value::Tod(TimeOfDayValue::new(86_399_999)))
    );
    assert_eq!(
        lib.call("DT_TO_LTOD", std::slice::from_ref(&instant)),
        Ok(Value::LTod(LTimeOfDayValue::new(86_399_999_000_000)))
    );
}

#[test]
fn long_datetime_to_short_floors_negative_sub_millisecond_instant() {
    let lib = library();
    let instant = Value::Ldt(LDateTimeValue::new(-1));

    assert_eq!(
        lib.call("LDT_TO_DT", std::slice::from_ref(&instant)),
        Ok(Value::Dt(DateTimeValue::new(-1)))
    );
    assert_eq!(
        lib.call("LDT_TO_TOD", std::slice::from_ref(&instant)),
        Ok(Value::Tod(TimeOfDayValue::new(86_399_999)))
    );
}

#[test]
fn time_and_long_time_width_conversion_preserves_signed_duration() {
    let lib = library();
    let negative = Duration::from_nanos(-123);

    assert_eq!(
        lib.call("TIME_TO_LTIME", &[Value::Time(negative)]),
        Ok(Value::LTime(negative))
    );
    assert_eq!(
        lib.call("LTIME_TO_TIME", &[Value::LTime(negative)]),
        Ok(Value::Time(negative))
    );
}

#[test]
fn bcd_round_trip_covers_each_storage_width() {
    let lib = library();

    for (encode, decode, input, packed, output) in [
        (
            "USINT_TO_BCD_BYTE",
            "BYTE_BCD_TO_USINT",
            Value::USInt(99),
            Value::Byte(0x99),
            Value::USInt(99),
        ),
        (
            "UINT_TO_BCD_WORD",
            "WORD_BCD_TO_UINT",
            Value::UInt(9999),
            Value::Word(0x9999),
            Value::UInt(9999),
        ),
        (
            "UDINT_TO_BCD_DWORD",
            "DWORD_BCD_TO_UDINT",
            Value::UDInt(12_345_678),
            Value::DWord(0x1234_5678),
            Value::UDInt(12_345_678),
        ),
        (
            "ULINT_TO_BCD_LWORD",
            "LWORD_BCD_TO_ULINT",
            Value::ULInt(1_234_567_890_123_456),
            Value::LWord(0x1234_5678_9012_3456),
            Value::ULInt(1_234_567_890_123_456),
        ),
    ] {
        assert_eq!(lib.call(encode, &[input]), Ok(packed.clone()), "{encode}");
        assert_eq!(lib.call(decode, &[packed]), Ok(output), "{decode}");
    }
}

#[test]
fn bcd_encoding_checks_digit_capacity_and_decoding_rejects_bad_nibbles() {
    let lib = library();

    assert_eq!(
        lib.call("USINT_TO_BCD_BYTE", &[Value::USInt(100)]),
        Err(RuntimeError::Overflow)
    );
    assert_eq!(
        lib.call("BYTE_BCD_TO_UINT", &[Value::Byte(0xfa)]),
        Err(RuntimeError::TypeMismatch)
    );
}

#[test]
fn is_valid_accepts_only_real_families_and_reports_finiteness() {
    let lib = library();

    assert_eq!(
        lib.call("IS_VALID", &[Value::Real(1.0)]),
        Ok(Value::Bool(true))
    );
    assert_eq!(
        lib.call("IS_VALID", &[Value::Real(f32::NAN)]),
        Ok(Value::Bool(false))
    );
    assert_eq!(
        lib.call("IS_VALID", &[Value::LReal(f64::NEG_INFINITY)]),
        Ok(Value::Bool(false))
    );
    assert_eq!(
        lib.call("IS_VALID", &[Value::DInt(1)]),
        Err(RuntimeError::TypeMismatch)
    );
}

#[test]
fn is_valid_bcd_checks_every_nibble_for_each_declared_width() {
    let lib = library();

    for value in [
        Value::Byte(0x99),
        Value::Word(0x9999),
        Value::DWord(0x9999_9999),
        Value::LWord(0x9999_9999_9999_9999),
    ] {
        assert_eq!(lib.call("IS_VALID_BCD", &[value]), Ok(Value::Bool(true)));
    }
    for value in [
        Value::Byte(0xa0),
        Value::Word(0xa000),
        Value::DWord(0xa000_0000),
        Value::LWord(0xa000_0000_0000_0000),
    ] {
        assert_eq!(lib.call("IS_VALID_BCD", &[value]), Ok(Value::Bool(false)));
    }
}

#[test]
fn standard_library_lookup_and_parameter_metadata_are_case_insensitive() {
    let lib = library();

    let fixed = lib.get("aSsErT_NeAr").expect("fixed function metadata");
    match &fixed.params {
        StdParams::Fixed(parameters) => {
            assert_eq!(
                parameters
                    .iter()
                    .map(|name| name.as_str())
                    .collect::<Vec<_>>(),
                ["EXPECTED", "ACTUAL", "DELTA"]
            );
        }
        other => panic!("expected fixed metadata, got {other:?}"),
    }

    let variadic = lib.get("mUx").expect("variadic function metadata");
    match &variadic.params {
        StdParams::Variadic {
            fixed,
            prefix,
            start,
            min,
        } => {
            assert_eq!(
                fixed.iter().map(|name| name.as_str()).collect::<Vec<_>>(),
                ["K"]
            );
            assert_eq!(prefix.as_str(), "IN");
            assert_eq!((*start, *min), (0, 2));
        }
        other => panic!("expected variadic metadata, got {other:?}"),
    }
}
