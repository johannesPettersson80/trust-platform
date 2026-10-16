//! Duration scaling and date/time concatenation (spec 07 section 9).

use trust_runtime::error::RuntimeError;
use trust_runtime::harness::TestHarness;
use trust_runtime::stdlib::StandardLibrary;
use trust_runtime::value::{
    DateTimeValue, Duration, LDateTimeValue, LTimeOfDayValue, TimeOfDayValue, Value,
};

#[test]
fn named_duration_helpers_truncate_like_the_operators() {
    let lib = StandardLibrary::new();

    // 3 ns * 0.5 = 1.5 ns, truncated toward zero.
    assert_eq!(
        lib.call(
            "MUL_TIME",
            &[Value::Time(Duration::from_nanos(3)), Value::Real(0.5)]
        ),
        Ok(Value::Time(Duration::from_nanos(1)))
    );
    // 7 ns / 2 with an integer factor: exact integer division.
    assert_eq!(
        lib.call(
            "DIV_TIME",
            &[Value::Time(Duration::from_nanos(7)), Value::Int(2)]
        ),
        Ok(Value::Time(Duration::from_nanos(3)))
    );
    // 7 ns * -0.5 = -3.5 ns, truncated toward zero.
    assert_eq!(
        lib.call(
            "MUL_TIME",
            &[Value::Time(Duration::from_nanos(7)), Value::LReal(-0.5)]
        ),
        Ok(Value::Time(Duration::from_nanos(-3)))
    );
    assert_eq!(
        lib.call(
            "DIV_LTIME",
            &[Value::LTime(Duration::from_nanos(7)), Value::Int(2)]
        ),
        Ok(Value::LTime(Duration::from_nanos(3)))
    );
}

#[test]
fn integer_factors_scale_nanoseconds_exactly_above_two_pow_53() {
    let lib = StandardLibrary::new();
    // 2^53 + 1 nanoseconds: an f64 round trip would drop the last nanosecond.
    let nanos = 9_007_199_254_740_993_i64;

    assert_eq!(
        lib.call(
            "MUL_TIME",
            &[Value::Time(Duration::from_nanos(nanos)), Value::Int(1)]
        ),
        Ok(Value::Time(Duration::from_nanos(nanos)))
    );
    assert_eq!(
        lib.call(
            "MUL_LTIME",
            &[Value::LTime(Duration::from_nanos(nanos)), Value::Int(1)]
        ),
        Ok(Value::LTime(Duration::from_nanos(nanos)))
    );
    assert_eq!(
        lib.call(
            "DIV_LTIME",
            &[Value::LTime(Duration::from_nanos(nanos)), Value::ULInt(1)]
        ),
        Ok(Value::LTime(Duration::from_nanos(nanos)))
    );
}

#[test]
fn non_finite_and_zero_factors_fault_before_scaling() {
    let lib = StandardLibrary::new();

    assert_eq!(
        lib.call(
            "DIV_TIME",
            &[
                Value::Time(Duration::from_nanos(5)),
                Value::LReal(f64::INFINITY)
            ]
        ),
        Err(RuntimeError::Overflow)
    );
    assert_eq!(
        lib.call(
            "DIV_TIME",
            &[Value::Time(Duration::from_nanos(5)), Value::LReal(f64::NAN)]
        ),
        Err(RuntimeError::Overflow)
    );
    assert_eq!(
        lib.call(
            "MUL_TIME",
            &[
                Value::Time(Duration::from_secs(1)),
                Value::LReal(f64::NEG_INFINITY)
            ]
        ),
        Err(RuntimeError::Overflow)
    );
    assert_eq!(
        lib.call(
            "DIV_TIME",
            &[Value::Time(Duration::from_nanos(5)), Value::Real(0.0)]
        ),
        Err(RuntimeError::DivisionByZero)
    );
    assert_eq!(
        lib.call(
            "MUL_TIME",
            &[Value::Time(Duration::from_nanos(5)), Value::Bool(true)]
        ),
        Err(RuntimeError::TypeMismatch)
    );
}

#[test]
fn duration_operators_match_the_named_helpers_in_a_compiled_program() {
    let source = r#"
        PROGRAM Example
        VAR
            t_mul : TIME := T#3ns;
            r_mul : TIME;
            t_div : TIME := T#7ns;
            r_div : TIME;
        END_VAR
        r_mul := t_mul * 0.5;
        r_div := t_div / 2;
        END_PROGRAM
    "#;

    let mut harness = TestHarness::from_source(source).unwrap();
    harness.cycle();
    harness.assert_eq("r_mul", Value::Time(Duration::from_nanos(1)));
    harness.assert_eq("r_div", Value::Time(Duration::from_nanos(3)));
}

#[test]
fn concat_time_of_day_rejects_each_component_outside_its_range() {
    let lib = StandardLibrary::new();
    let cases = [
        [Value::Int(24), Value::Int(0), Value::Int(0), Value::Int(0)],
        [Value::Int(0), Value::Int(75), Value::Int(0), Value::Int(0)],
        [Value::Int(0), Value::Int(0), Value::Int(60), Value::Int(0)],
        [
            Value::Int(0),
            Value::Int(0),
            Value::Int(0),
            Value::Int(1000),
        ],
        // A millisecond overflow must not carry into the seconds field.
        [
            Value::Int(23),
            Value::Int(59),
            Value::Int(59),
            Value::Int(1000),
        ],
    ];

    for args in cases {
        assert_eq!(
            lib.call("CONCAT_TOD", &args),
            Err(RuntimeError::Overflow),
            "{args:?}"
        );
        assert_eq!(
            lib.call("CONCAT_LTOD", &args),
            Err(RuntimeError::Overflow),
            "{args:?}"
        );
    }
}

#[test]
fn concat_time_of_day_accepts_the_range_edges() {
    let lib = StandardLibrary::new();

    assert_eq!(
        lib.call(
            "CONCAT_TOD",
            &[
                Value::Int(23),
                Value::Int(59),
                Value::Int(59),
                Value::Int(999)
            ]
        ),
        Ok(Value::Tod(TimeOfDayValue::new(86_399_999)))
    );
    assert_eq!(
        lib.call(
            "CONCAT_LTOD",
            &[
                Value::Int(23),
                Value::Int(59),
                Value::Int(59),
                Value::Int(999)
            ]
        ),
        Ok(Value::LTod(LTimeOfDayValue::new(86_399_999_000_000)))
    );
    assert_eq!(
        lib.call(
            "CONCAT_TOD",
            &[Value::Int(0), Value::Int(0), Value::Int(0), Value::Int(0)]
        ),
        Ok(Value::Tod(TimeOfDayValue::new(0)))
    );
}

#[test]
fn concat_date_time_checks_calendar_and_clock_components() {
    let lib = StandardLibrary::new();

    assert_eq!(
        lib.call(
            "CONCAT_DT",
            &[
                Value::Int(2023),
                Value::Int(1),
                Value::Int(1),
                Value::Int(24),
                Value::Int(0),
                Value::Int(0),
                Value::Int(0)
            ]
        ),
        Err(RuntimeError::Overflow)
    );
    assert_eq!(
        lib.call(
            "CONCAT_LDT",
            &[
                Value::Int(2023),
                Value::Int(1),
                Value::Int(1),
                Value::Int(0),
                Value::Int(0),
                Value::Int(0),
                Value::Int(1000)
            ]
        ),
        Err(RuntimeError::Overflow)
    );
    assert_eq!(
        lib.call(
            "CONCAT_DATE",
            &[Value::Int(2023), Value::Int(2), Value::Int(29)]
        ),
        Err(RuntimeError::Overflow)
    );
    assert_eq!(
        lib.call(
            "CONCAT_DT",
            &[
                Value::Int(2023),
                Value::Int(1),
                Value::Int(1),
                Value::Int(23),
                Value::Int(59),
                Value::Int(59),
                Value::Int(999)
            ]
        ),
        Ok(Value::Dt(DateTimeValue::new(1_672_617_599_999)))
    );
    assert_eq!(
        lib.call(
            "CONCAT_LDT",
            &[
                Value::Int(2023),
                Value::Int(1),
                Value::Int(1),
                Value::Int(23),
                Value::Int(59),
                Value::Int(59),
                Value::Int(999)
            ]
        ),
        Ok(Value::Ldt(LDateTimeValue::new(1_672_617_599_999_000_000)))
    );
}
