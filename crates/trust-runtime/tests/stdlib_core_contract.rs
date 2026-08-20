use trust_runtime::error::RuntimeError;
use trust_runtime::stdlib::StandardLibrary;
use trust_runtime::value::{Duration, Value};

fn library() -> StandardLibrary {
    StandardLibrary::new()
}

fn assertion_message(error: RuntimeError) -> String {
    match error {
        RuntimeError::AssertionFailed(message) => message.to_string(),
        other => panic!("expected assertion failure, got {other:?}"),
    }
}

#[test]
fn abs_preserves_each_integer_family_and_rejects_signed_minima() {
    let lib = library();

    let cases = [
        (Value::SInt(-7), Value::SInt(7)),
        (Value::Int(-7), Value::Int(7)),
        (Value::DInt(-7), Value::DInt(7)),
        (Value::LInt(-7), Value::LInt(7)),
        (Value::USInt(7), Value::USInt(7)),
        (Value::UInt(7), Value::UInt(7)),
        (Value::UDInt(7), Value::UDInt(7)),
        (Value::ULInt(7), Value::ULInt(7)),
    ];
    for (input, expected) in cases {
        assert_eq!(lib.call("ABS", &[input]), Ok(expected));
    }

    for input in [
        Value::SInt(i8::MIN),
        Value::Int(i16::MIN),
        Value::DInt(i32::MIN),
        Value::LInt(i64::MIN),
    ] {
        assert_eq!(lib.call("ABS", &[input]), Err(RuntimeError::Overflow));
    }
}

#[test]
fn real_unary_functions_preserve_width_and_reject_non_finite_results() {
    let lib = library();

    assert!(matches!(
        lib.call("SQRT", &[Value::Real(9.0)]),
        Ok(Value::Real(value)) if value == 3.0
    ));
    assert!(matches!(
        lib.call("SQRT", &[Value::LReal(9.0)]),
        Ok(Value::LReal(value)) if value == 3.0
    ));
    assert_eq!(
        lib.call("SQRT", &[Value::Real(-1.0)]),
        Err(RuntimeError::Overflow)
    );
    assert_eq!(
        lib.call("LN", &[Value::LReal(0.0)]),
        Err(RuntimeError::Overflow)
    );
    assert_eq!(
        lib.call("ASIN", &[Value::Real(2.0)]),
        Err(RuntimeError::Overflow)
    );
}

#[test]
fn trigonometric_functions_cover_zero_and_quadrant_contracts() {
    let lib = library();

    for name in ["SIN", "TAN", "ASIN", "ATAN"] {
        assert!(matches!(
            lib.call(name, &[Value::Real(0.0)]),
            Ok(Value::Real(value)) if value.abs() < f32::EPSILON
        ));
    }
    assert!(matches!(
        lib.call("COS", &[Value::Real(0.0)]),
        Ok(Value::Real(value)) if (value - 1.0).abs() < f32::EPSILON
    ));
    assert!(matches!(
        lib.call("ACOS", &[Value::LReal(1.0)]),
        Ok(Value::LReal(value)) if value.abs() < f64::EPSILON
    ));
}

#[test]
fn atan2_promotes_mixed_real_widths_to_lreal() {
    let lib = library();

    assert!(matches!(
        lib.call("ATAN2", &[Value::Real(1.0), Value::LReal(1.0)]),
        Ok(Value::LReal(value)) if (value - std::f64::consts::FRAC_PI_4).abs() < 1e-12
    ));
    assert!(matches!(
        lib.call("ATAN2", &[Value::LReal(1.0), Value::Real(-1.0)]),
        Ok(Value::LReal(value)) if (value - 3.0 * std::f64::consts::FRAC_PI_4).abs() < 1e-12
    ));
}

#[test]
fn extensible_arithmetic_folds_left_and_enforces_minimum_arity() {
    let lib = library();

    assert_eq!(
        lib.call(
            "ADD",
            &[
                Value::DInt(1),
                Value::DInt(2),
                Value::DInt(3),
                Value::DInt(4)
            ]
        ),
        Ok(Value::DInt(10))
    );
    assert_eq!(
        lib.call("MUL", &[Value::LInt(2), Value::LInt(3), Value::LInt(4)]),
        Ok(Value::LInt(24))
    );
    assert!(matches!(
        lib.call("ADD", &[Value::Int(1)]),
        Err(RuntimeError::InvalidArgumentCount {
            expected: 2,
            got: 1
        })
    ));
    assert!(matches!(
        lib.call("MUL", &[]),
        Err(RuntimeError::InvalidArgumentCount {
            expected: 2,
            got: 0
        })
    ));
}

#[test]
fn fixed_arithmetic_reports_division_and_arity_errors() {
    let lib = library();

    assert_eq!(
        lib.call("SUB", &[Value::Int(9), Value::Int(4)]),
        Ok(Value::Int(5))
    );
    assert_eq!(
        lib.call("MOD", &[Value::DInt(11), Value::DInt(4)]),
        Ok(Value::DInt(3))
    );
    assert!(lib.call("DIV", &[Value::Int(1), Value::Int(0)]).is_err());
    assert!(matches!(
        lib.call("SUB", &[Value::Int(1)]),
        Err(RuntimeError::InvalidArgumentCount {
            expected: 2,
            got: 1
        })
    ));
}

#[test]
fn exponentiation_preserves_base_width_and_rejects_unrepresentable_results() {
    let lib = library();

    assert!(matches!(
        lib.call("EXPT", &[Value::Real(4.0), Value::Int(2)]),
        Ok(Value::Real(value)) if value == 16.0
    ));
    assert!(matches!(
        lib.call("EXPT", &[Value::LReal(4.0), Value::DInt(2)]),
        Ok(Value::LReal(value)) if value == 16.0
    ));
    assert_eq!(
        lib.call("EXPT", &[Value::Real(10.0), Value::Int(100)]),
        Err(RuntimeError::Overflow)
    );
}

#[test]
fn duration_scale_accepts_both_multiplication_orders_and_left_division() {
    let lib = library();
    let time = Value::Time(Duration::from_millis(1_500));
    let ltime = Value::LTime(Duration::from_nanos(1_500_000_000));

    assert_eq!(
        lib.call("MUL", &[time.clone(), Value::Int(2)]),
        Ok(Value::Time(Duration::from_secs(3)))
    );
    assert_eq!(
        lib.call("MUL", &[Value::Int(2), time.clone()]),
        Ok(Value::Time(Duration::from_secs(3)))
    );
    assert_eq!(
        lib.call("DIV", &[ltime, Value::Int(3)]),
        Ok(Value::LTime(Duration::from_millis(500)))
    );
    assert!(matches!(
        lib.call("DIV", &[Value::Int(3), time]),
        Err(RuntimeError::TypeMismatch)
    ));
}

#[test]
fn time_related_addition_is_binary_and_move_is_identity() {
    let lib = library();
    let one = Value::Time(Duration::from_secs(1));
    let two = Value::Time(Duration::from_secs(2));

    assert_eq!(
        lib.call("ADD", &[one.clone(), two.clone()]),
        Ok(Value::Time(Duration::from_secs(3)))
    );
    assert!(matches!(
        lib.call("ADD", &[one, two, Value::Time(Duration::from_secs(3))]),
        Err(RuntimeError::InvalidArgumentCount {
            expected: 2,
            got: 3
        })
    ));
    assert_eq!(
        lib.call("MOVE", &[Value::WString("Ångström".into())]),
        Ok(Value::WString("Ångström".into()))
    );
}

#[test]
fn shifts_zero_fill_at_or_above_width_and_reject_negative_counts() {
    let lib = library();

    assert_eq!(
        lib.call("SHL", &[Value::Byte(0xff), Value::Int(8)]),
        Ok(Value::Byte(0))
    );
    assert_eq!(
        lib.call("SHR", &[Value::Word(0xffff), Value::Int(99)]),
        Ok(Value::Word(0))
    );
    assert_eq!(
        lib.call("SHL", &[Value::DWord(1), Value::Int(-1)]),
        Err(RuntimeError::TypeMismatch)
    );
}

#[test]
fn rotations_reduce_counts_modulo_every_width_including_lword() {
    let lib = library();
    let input = Value::LWord(0x8000_0000_0000_0001);

    assert_eq!(
        lib.call("ROL", &[input.clone(), Value::Int(0)]),
        Ok(input.clone())
    );
    assert_eq!(
        lib.call("ROR", &[input.clone(), Value::Int(64)]),
        Ok(input.clone())
    );
    assert_eq!(
        lib.call("ROL", &[input.clone(), Value::Int(65)]),
        lib.call("ROL", &[input, Value::Int(1)])
    );
}

#[test]
fn bitwise_variadics_widen_zero_extend_and_mask() {
    let lib = library();

    assert_eq!(
        lib.call(
            "OR",
            &[
                Value::Byte(0x0f),
                Value::Word(0x0f00),
                Value::DWord(0x10000)
            ]
        ),
        Ok(Value::DWord(0x10f0f))
    );
    assert_eq!(
        lib.call("AND", &[Value::Byte(0xff), Value::Word(0x01ff)]),
        Ok(Value::Word(0x00ff))
    );
    assert_eq!(
        lib.call("XOR", &[Value::Bool(true), Value::Bool(true)]),
        Ok(Value::Bool(false))
    );
}

#[test]
fn bitwise_not_flips_only_the_declared_width() {
    let lib = library();

    assert_eq!(
        lib.call("NOT", &[Value::Bool(true)]),
        Ok(Value::Bool(false))
    );
    assert_eq!(lib.call("NOT", &[Value::Byte(0x0f)]), Ok(Value::Byte(0xf0)));
    assert_eq!(
        lib.call("NOT", &[Value::Word(0x00ff)]),
        Ok(Value::Word(0xff00))
    );
}

#[test]
fn sel_uses_bool_gate_and_common_numeric_type() {
    let lib = library();

    assert_eq!(
        lib.call("SEL", &[Value::Bool(false), Value::Int(7), Value::DInt(9)]),
        Ok(Value::DInt(7))
    );
    assert_eq!(
        lib.call("SEL", &[Value::Bool(true), Value::Int(7), Value::DInt(9)]),
        Ok(Value::DInt(9))
    );
    assert_eq!(
        lib.call("SEL", &[Value::Int(1), Value::Int(7), Value::Int(9)]),
        Err(RuntimeError::TypeMismatch)
    );
}

#[test]
fn min_max_and_limit_compare_all_values_in_one_common_type() {
    let lib = library();

    assert_eq!(
        lib.call("MIN", &[Value::Int(5), Value::DInt(-2), Value::DInt(7)]),
        Ok(Value::DInt(-2))
    );
    assert_eq!(
        lib.call(
            "MAX",
            &[Value::Real(5.0), Value::LReal(8.5), Value::Real(7.0)]
        ),
        Ok(Value::LReal(8.5))
    );
    assert_eq!(
        lib.call("LIMIT", &[Value::Int(0), Value::DInt(12), Value::Int(10)]),
        Ok(Value::DInt(10))
    );
}

#[test]
fn mux_is_zero_based_and_reports_both_out_of_range_directions() {
    let lib = library();
    let inputs = [Value::Int(10), Value::Int(20), Value::Int(30)];

    assert_eq!(
        lib.call(
            "MUX",
            &[
                Value::Int(0),
                inputs[0].clone(),
                inputs[1].clone(),
                inputs[2].clone()
            ]
        ),
        Ok(Value::Int(10))
    );
    assert!(matches!(
        lib.call(
            "MUX",
            &[Value::Int(-1), inputs[0].clone(), inputs[1].clone()]
        ),
        Err(RuntimeError::IndexOutOfBounds {
            index: -1,
            lower: 0,
            upper: 1
        })
    ));
    assert!(matches!(
        lib.call(
            "MUX",
            &[Value::Int(2), inputs[0].clone(), inputs[1].clone()]
        ),
        Err(RuntimeError::IndexOutOfBounds {
            index: 2,
            lower: 0,
            upper: 1
        })
    ));
}

#[test]
fn mux_validates_unselected_inputs_before_returning_selection() {
    let lib = library();

    assert_eq!(
        lib.call(
            "MUX",
            &[
                Value::Int(0),
                Value::Int(7),
                Value::String("incompatible".into())
            ]
        ),
        Err(RuntimeError::TypeMismatch)
    );
}

#[test]
fn comparison_chains_compare_adjacent_pairs() {
    let lib = library();

    assert_eq!(
        lib.call("GT", &[Value::Int(5), Value::Int(4), Value::Int(3)]),
        Ok(Value::Bool(true))
    );
    assert_eq!(
        lib.call("GT", &[Value::Int(5), Value::Int(3), Value::Int(4)]),
        Ok(Value::Bool(false))
    );
    assert_eq!(
        lib.call("EQ", &[Value::Int(5), Value::DInt(5), Value::LInt(5)]),
        Ok(Value::Bool(true))
    );
    assert_eq!(
        lib.call("LE", &[Value::Int(3), Value::Int(3), Value::Int(4)]),
        Ok(Value::Bool(true))
    );
}

#[test]
fn comparison_widens_bit_strings_and_rejects_incompatible_families() {
    let lib = library();

    assert_eq!(
        lib.call("EQ", &[Value::Byte(0xff), Value::Word(0x00ff)]),
        Ok(Value::Bool(true))
    );
    assert_eq!(
        lib.call(
            "LT",
            &[Value::String("alpha".into()), Value::String("beta".into())]
        ),
        Ok(Value::Bool(true))
    );
    assert_eq!(
        lib.call("NE", &[Value::Int(1), Value::String("1".into())]),
        Err(RuntimeError::TypeMismatch)
    );
}

#[test]
fn non_extensible_ne_requires_exactly_two_inputs() {
    let lib = library();

    assert_eq!(
        lib.call("NE", &[Value::Int(1), Value::Int(2)]),
        Ok(Value::Bool(true))
    );
    assert!(matches!(
        lib.call("NE", &[Value::Int(1), Value::Int(2), Value::Int(3)]),
        Err(RuntimeError::InvalidArgumentCount {
            expected: 2,
            got: 3
        })
    ));
}

#[test]
fn string_functions_preserve_narrow_and_wide_families() {
    let lib = library();

    assert_eq!(
        lib.call("LEFT", &[Value::String("ÄBC".into()), Value::Int(2)]),
        Ok(Value::String("ÄB".into()))
    );
    assert_eq!(
        lib.call("RIGHT", &[Value::WString("ÄBC".into()), Value::Int(2)]),
        Ok(Value::WString("BC".into()))
    );
    assert_eq!(
        lib.call(
            "CONCAT",
            &[Value::String("A".into()), Value::WString("B".into())]
        ),
        Err(RuntimeError::TypeMismatch)
    );
}

#[test]
fn substring_lengths_clamp_at_zero_and_end() {
    let lib = library();

    assert_eq!(
        lib.call("LEFT", &[Value::String("ABC".into()), Value::Int(-1)]),
        Ok(Value::String("".into()))
    );
    assert_eq!(
        lib.call("RIGHT", &[Value::String("ABC".into()), Value::Int(99)]),
        Ok(Value::String("ABC".into()))
    );
    assert_eq!(
        lib.call(
            "MID",
            &[Value::String("ABC".into()), Value::Int(2), Value::Int(99)]
        ),
        Ok(Value::String("".into()))
    );
}

#[test]
fn insert_uses_element_boundary_and_clamps_outside_range() {
    let lib = library();

    assert_eq!(
        lib.call(
            "INSERT",
            &[
                Value::String("AC".into()),
                Value::String("B".into()),
                Value::Int(1)
            ]
        ),
        Ok(Value::String("ABC".into()))
    );
    assert_eq!(
        lib.call(
            "INSERT",
            &[
                Value::String("AC".into()),
                Value::String("B".into()),
                Value::Int(0)
            ]
        ),
        Ok(Value::String("BAC".into()))
    );
    assert_eq!(
        lib.call(
            "INSERT",
            &[
                Value::String("AC".into()),
                Value::String("B".into()),
                Value::Int(99)
            ]
        ),
        Ok(Value::String("ACB".into()))
    );
}

#[test]
fn delete_and_replace_obey_one_based_clamped_boundaries() {
    let lib = library();

    assert_eq!(
        lib.call(
            "DELETE",
            &[Value::String("ABCDE".into()), Value::Int(2), Value::Int(2)]
        ),
        Ok(Value::String("ADE".into()))
    );
    assert_eq!(
        lib.call(
            "DELETE",
            &[Value::String("ABCDE".into()), Value::Int(0), Value::Int(2)]
        ),
        Ok(Value::String("ABCDE".into()))
    );
    assert_eq!(
        lib.call(
            "REPLACE",
            &[
                Value::String("ABCDE".into()),
                Value::String("X".into()),
                Value::Int(0),
                Value::Int(2)
            ]
        ),
        Ok(Value::String("AXBCDE".into()))
    );
    assert_eq!(
        lib.call(
            "REPLACE",
            &[
                Value::String("ABCDE".into()),
                Value::String("X".into()),
                Value::Int(2),
                Value::Int(99)
            ]
        ),
        Ok(Value::String("ABCDE".into()))
    );
}

#[test]
fn find_reports_first_unicode_element_position_or_zero() {
    let lib = library();

    assert_eq!(
        lib.call(
            "FIND",
            &[Value::String("ÄBCBC".into()), Value::String("BC".into())]
        ),
        Ok(Value::Int(2))
    );
    assert_eq!(
        lib.call(
            "FIND",
            &[
                Value::WString("ÄBC".into()),
                Value::WString("missing".into())
            ]
        ),
        Ok(Value::Int(0))
    );
}

#[test]
fn internal_string_limit_counts_elements_and_rejects_negative_capacity() {
    let lib = library();

    assert_eq!(
        lib.call(
            "__TRUST_LIMIT_STRING",
            &[Value::String("ÄBC".into()), Value::Int(2)]
        ),
        Ok(Value::String("ÄB".into()))
    );
    assert_eq!(
        lib.call(
            "__TRUST_LIMIT_STRING",
            &[Value::WString("ÄBC".into()), Value::Int(2)]
        ),
        Ok(Value::WString("ÄB".into()))
    );
    assert_eq!(
        lib.call(
            "__TRUST_LIMIT_STRING",
            &[Value::String("ABC".into()), Value::Int(-1)]
        ),
        Err(RuntimeError::Overflow)
    );
}

#[test]
fn bool_assertions_return_null_or_stable_failures() {
    let lib = library();

    assert_eq!(
        lib.call("ASSERT_TRUE", &[Value::Bool(true)]),
        Ok(Value::Null)
    );
    assert_eq!(
        lib.call("ASSERT_FALSE", &[Value::Bool(false)]),
        Ok(Value::Null)
    );
    assert_eq!(
        assertion_message(
            lib.call("ASSERT_TRUE", &[Value::Bool(false)])
                .expect_err("false must fail ASSERT_TRUE")
        ),
        "ASSERT_TRUE expected TRUE, got FALSE"
    );
    assert_eq!(
        assertion_message(
            lib.call("ASSERT_FALSE", &[Value::Bool(true)])
                .expect_err("true must fail ASSERT_FALSE")
        ),
        "ASSERT_FALSE expected FALSE, got TRUE"
    );
}

#[test]
fn relational_assertions_support_lossless_numeric_widening() {
    let lib = library();

    for (name, args) in [
        ("ASSERT_EQUAL", [Value::Int(5), Value::DInt(5)]),
        ("ASSERT_NOT_EQUAL", [Value::Int(5), Value::DInt(6)]),
        ("ASSERT_GREATER", [Value::Int(6), Value::DInt(5)]),
        ("ASSERT_LESS", [Value::Int(4), Value::DInt(5)]),
        ("ASSERT_GREATER_OR_EQUAL", [Value::Int(5), Value::DInt(5)]),
        ("ASSERT_LESS_OR_EQUAL", [Value::Int(5), Value::DInt(5)]),
    ] {
        assert_eq!(lib.call(name, &args), Ok(Value::Null), "{name}");
    }
}

#[test]
fn relational_assertion_failures_use_user_value_text() {
    let lib = library();

    assert_eq!(
        assertion_message(
            lib.call("ASSERT_EQUAL", &[Value::Real(1.0), Value::LReal(2.0)])
                .expect_err("different values must fail")
        ),
        "ASSERT_EQUAL failed: expected 1.0, actual 2.0"
    );
    assert_eq!(
        assertion_message(
            lib.call("ASSERT_NOT_EQUAL", &[Value::Bool(true), Value::Bool(true)])
                .expect_err("equal values must fail")
        ),
        "ASSERT_NOT_EQUAL failed: values should differ, left TRUE, right TRUE"
    );
}

#[test]
fn assert_near_accepts_boundary_and_rejects_negative_delta() {
    let lib = library();

    assert_eq!(
        lib.call(
            "ASSERT_NEAR",
            &[Value::LReal(1.0), Value::LReal(1.1), Value::LReal(0.1)]
        ),
        Ok(Value::Null)
    );
    assert_eq!(
        assertion_message(
            lib.call(
                "ASSERT_NEAR",
                &[Value::Real(1.0), Value::Real(1.0), Value::Real(-0.1)]
            )
            .expect_err("negative tolerance must fail")
        ),
        "ASSERT_NEAR failed: DELTA must be non-negative"
    );
}

#[test]
fn assertions_reject_wrong_arity_types_and_non_finite_near_values() {
    let lib = library();

    assert_eq!(
        lib.call("ASSERT_TRUE", &[Value::Int(1)]),
        Err(RuntimeError::TypeMismatch)
    );
    assert!(matches!(
        lib.call("ASSERT_EQUAL", &[Value::Int(1)]),
        Err(RuntimeError::InvalidArgumentCount {
            expected: 2,
            got: 1
        })
    ));
    assert_eq!(
        lib.call(
            "ASSERT_NEAR",
            &[Value::LReal(f64::NAN), Value::LReal(1.0), Value::LReal(0.1)]
        ),
        Err(RuntimeError::Overflow)
    );
}
