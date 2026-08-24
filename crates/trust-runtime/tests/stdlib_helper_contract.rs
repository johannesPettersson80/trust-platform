use trust_runtime::error::RuntimeError;
use trust_runtime::stdlib::helpers::{
    bit_value, bit_value_to_result, coerce_to_common, common_kind, compare_common, mask_for,
    require_arity, require_min, round_ties_to_even, scale_time, CmpOp, CommonKind, TimeKind,
};
use trust_runtime::stdlib::time::{is_runtime_clock_name, runtime_clock_value};
use trust_runtime::stdlib::{StandardLibrary, StdParams};
use trust_runtime::value::{Duration, Value};

fn first(args: &[Value]) -> Result<Value, RuntimeError> {
    Ok(args.first().cloned().unwrap_or(Value::Null))
}

fn last(args: &[Value]) -> Result<Value, RuntimeError> {
    Ok(args.last().cloned().unwrap_or(Value::Null))
}

#[test]
fn arity_helpers_report_required_and_observed_counts() {
    assert_eq!(require_arity(&[Value::Null], 1), Ok(()));
    assert_eq!(require_min(&[Value::Null, Value::Null], 2), Ok(()));
    assert_eq!(
        require_arity(&[], 1),
        Err(RuntimeError::InvalidArgumentCount {
            expected: 1,
            got: 0
        })
    );
    assert_eq!(
        require_min(&[Value::Null], 2),
        Err(RuntimeError::InvalidArgumentCount {
            expected: 2,
            got: 1
        })
    );
}

#[test]
fn common_numeric_kind_widens_compatible_values_and_rejects_mixed_sign() {
    assert!(matches!(
        common_kind(&[Value::Int(1), Value::DInt(2)]),
        Ok(CommonKind::Numeric(_))
    ));
    let kind = common_kind(&[Value::Real(1.0), Value::LReal(2.0)])
        .expect("REAL and LREAL have a common kind");
    assert_eq!(
        coerce_to_common(&Value::Real(1.0), &kind),
        Ok(Value::LReal(1.0))
    );
    assert_eq!(
        common_kind(&[Value::UInt(1), Value::SInt(1)]),
        Err(RuntimeError::TypeMismatch)
    );
}

#[test]
fn common_bit_kind_uses_widest_width_and_zero_extends() {
    let kind = common_kind(&[Value::Byte(0xff), Value::DWord(0x100)]).expect("common bit kind");

    assert_eq!(kind, CommonKind::Bit(32));
    assert_eq!(
        coerce_to_common(&Value::Byte(0xff), &kind),
        Ok(Value::DWord(0xff))
    );
}

#[test]
fn char_values_join_their_matching_string_family() {
    let narrow =
        common_kind(&[Value::String("A".into()), Value::Char(b'B')]).expect("narrow string kind");
    assert_eq!(narrow, CommonKind::String { wide: false });
    assert_eq!(
        coerce_to_common(&Value::Char(b'B'), &narrow),
        Ok(Value::String("B".into()))
    );

    let wide = common_kind(&[Value::WString("A".into()), Value::WChar('Ω' as u16)])
        .expect("wide string kind");
    assert_eq!(wide, CommonKind::String { wide: true });
    assert_eq!(
        coerce_to_common(&Value::WChar('Ω' as u16), &wide),
        Ok(Value::WString("Ω".into()))
    );

    assert_eq!(
        common_kind(&[Value::String("A".into()), Value::WString("B".into())]),
        Err(RuntimeError::TypeMismatch)
    );
}

#[test]
fn time_common_kind_requires_the_identical_runtime_family() {
    let kind = common_kind(&[
        Value::Time(Duration::from_secs(1)),
        Value::Time(Duration::from_secs(2)),
    ])
    .expect("TIME values share a family");
    assert_eq!(kind, CommonKind::Time(TimeKind::Time));
    assert_eq!(
        common_kind(&[Value::Time(Duration::ZERO), Value::LTime(Duration::ZERO)]),
        Err(RuntimeError::TypeMismatch)
    );
}

#[test]
fn common_numeric_coercion_rejects_out_of_range_value() {
    let kind = common_kind(&[Value::SInt(1), Value::SInt(2)]).expect("SINT kind");

    assert_eq!(
        coerce_to_common(&Value::DInt(128), &kind),
        Err(RuntimeError::Overflow)
    );
}

#[test]
fn common_comparison_supports_all_relations_after_coercion() {
    let kind = common_kind(&[Value::Int(2), Value::DInt(3)]).expect("numeric kind");

    for (operation, expected) in [
        (CmpOp::Lt, true),
        (CmpOp::Le, true),
        (CmpOp::Gt, false),
        (CmpOp::Ge, false),
        (CmpOp::Eq, false),
        (CmpOp::Ne, true),
    ] {
        assert_eq!(
            compare_common(&Value::Int(2), &Value::DInt(3), &kind, operation),
            Ok(expected)
        );
    }
}

#[test]
fn floating_common_comparison_retains_ieee_nan_relations() {
    let nan = Value::LReal(f64::NAN);
    let kind = common_kind(std::slice::from_ref(&nan)).expect("LREAL common kind");

    assert_eq!(compare_common(&nan, &nan, &kind, CmpOp::Eq), Ok(false));
    assert_eq!(compare_common(&nan, &nan, &kind, CmpOp::Ne), Ok(true));
}

#[test]
fn bit_value_round_trips_all_supported_declared_widths() {
    for (value, bits, width) in [
        (Value::Bool(true), 1, 1),
        (Value::Byte(0x81), 0x81, 8),
        (Value::Word(0x8001), 0x8001, 16),
        (Value::DWord(0x8000_0001), 0x8000_0001, 32),
        (
            Value::LWord(0x8000_0000_0000_0001),
            0x8000_0000_0000_0001,
            64,
        ),
    ] {
        assert_eq!(bit_value(&value), Ok((bits, width)));
        assert_eq!(bit_value_to_result(bits, width), value);
    }
    assert_eq!(bit_value(&Value::Int(1)), Err(RuntimeError::TypeMismatch));
}

#[test]
fn bit_masks_cover_exact_low_width_and_saturate_at_64() {
    assert_eq!(mask_for(0), 0);
    assert_eq!(mask_for(1), 0x1);
    assert_eq!(mask_for(8), 0xff);
    assert_eq!(mask_for(16), 0xffff);
    assert_eq!(mask_for(32), 0xffff_ffff);
    assert_eq!(mask_for(64), u64::MAX);
    assert_eq!(mask_for(65), u64::MAX);
}

#[test]
fn ties_to_even_rounding_handles_both_signs() {
    let cases: [(f64, f64); 8] = [
        (0.5, 0.0),
        (1.5, 2.0),
        (2.5, 2.0),
        (3.5, 4.0),
        (-0.5, -0.0),
        (-1.5, -2.0),
        (-2.5, -2.0),
        (-3.5, -4.0),
    ];
    for (input, expected) in cases {
        assert_eq!(round_ties_to_even(input).to_bits(), expected.to_bits());
    }
}

#[test]
fn duration_scaling_rounds_nanoseconds_ties_to_even() {
    assert_eq!(
        scale_time(Duration::from_nanos(1), &Value::LReal(2.5), true),
        Ok(Duration::from_nanos(2))
    );
    assert_eq!(
        scale_time(Duration::from_nanos(3), &Value::LReal(0.5), true),
        Ok(Duration::from_nanos(2))
    );
    assert_eq!(
        scale_time(Duration::from_nanos(5), &Value::Int(2), false),
        Ok(Duration::from_nanos(2))
    );
}

#[test]
fn duration_scaling_rejects_zero_non_finite_and_overflow() {
    assert_eq!(
        scale_time(Duration::from_secs(1), &Value::Int(0), false),
        Err(RuntimeError::DivisionByZero)
    );
    assert_eq!(
        scale_time(Duration::from_secs(1), &Value::LReal(f64::NAN), true),
        Err(RuntimeError::Overflow)
    );
    assert_eq!(
        scale_time(Duration::from_nanos(i64::MAX), &Value::LReal(2.0), true),
        Err(RuntimeError::Overflow)
    );
}

#[test]
fn custom_standard_library_registration_normalizes_names_and_metadata() {
    let mut lib = StandardLibrary::default();
    lib.register("echo", &["input"], first);
    lib.register_variadic_with_fixed("choose_last", &["selector"], "item", 0, 2, last);

    assert_eq!(lib.call("EcHo", &[Value::Int(7)]), Ok(Value::Int(7)));
    let function = lib
        .get("CHOOSE_LAST")
        .expect("registered variadic function");
    match &function.params {
        StdParams::Variadic {
            fixed,
            prefix,
            start,
            min,
        } => {
            assert_eq!(fixed[0].as_str(), "SELECTOR");
            assert_eq!(prefix.as_str(), "ITEM");
            assert_eq!((*start, *min), (0, 2));
        }
        other => panic!("expected variadic metadata, got {other:?}"),
    }
}

#[test]
fn cloned_standard_libraries_have_independent_registries() {
    let mut original = StandardLibrary::default();
    original.register("FIRST", &["IN"], first);
    let mut cloned = original.clone();
    cloned.register("LAST", &["IN"], last);

    assert!(original.get("FIRST").is_some());
    assert!(original.get("LAST").is_none());
    assert!(cloned.get("FIRST").is_some());
    assert!(cloned.get("LAST").is_some());
}

#[test]
fn runtime_clock_dispatch_preserves_logical_time_and_rejects_unknown_names() {
    let elapsed = Duration::from_nanos(-123);

    assert!(is_runtime_clock_name("TIME"));
    assert!(is_runtime_clock_name("CURRENT_DT"));
    assert!(!is_runtime_clock_name("time"));
    assert_eq!(
        runtime_clock_value("TIME", elapsed),
        Ok(Value::Time(elapsed))
    );
    assert!(matches!(
        runtime_clock_value("NOT_A_CLOCK", elapsed),
        Err(RuntimeError::UndefinedFunction(_))
    ));
}

#[test]
fn current_dt_returns_a_nonnegative_unix_millisecond_value() {
    let value = runtime_clock_value("CURRENT_DT", Duration::from_secs(-1))
        .expect("current host time is representable");

    assert!(matches!(value, Value::Dt(value) if value.ticks() >= 0));
}
