//! ASSERT_NEAR tolerance (spec 07 section 13).

use trust_runtime::error::RuntimeError;
use trust_runtime::stdlib::StandardLibrary;
use trust_runtime::value::Value;

fn assert_near(expected: f64, actual: f64, delta: f64) -> Result<Value, RuntimeError> {
    StandardLibrary::new().call(
        "ASSERT_NEAR",
        &[
            Value::LReal(expected),
            Value::LReal(actual),
            Value::LReal(delta),
        ],
    )
}

#[test]
fn assert_near_accepts_the_delta_boundary_and_rejects_beyond_it() {
    assert_eq!(assert_near(1.0, 1.25, 0.25), Ok(Value::Null));
    assert!(matches!(
        assert_near(1.0, 1.5, 0.25),
        Err(RuntimeError::AssertionFailed(_))
    ));
}

#[test]
fn assert_near_absorbs_only_the_documented_rounding_tolerance() {
    // Spec 07 section 13: beyond DELTA the tolerance is four binary64 epsilons
    // times the largest of |EXPECTED|, |ACTUAL|, DELTA and 1.0. With EXPECTED
    // 1.0 and DELTA 0.25 the largest term is |ACTUAL| = 1.25 + k * EPSILON, so
    // the margin is 4 * 1.25 * EPSILON = 5 * EPSILON.
    let inside = 1.25 + 4.0 * f64::EPSILON;
    let outside = 1.25 + 6.0 * f64::EPSILON;

    assert_eq!(assert_near(1.0, inside, 0.25), Ok(Value::Null));
    assert!(matches!(
        assert_near(1.0, outside, 0.25),
        Err(RuntimeError::AssertionFailed(_))
    ));
}

#[test]
fn assert_near_compares_decimal_operands_in_binary64() {
    // 1.1 is not exact in binary64, so |1.0 - 1.1| exceeds 0.1 by a rounding-sized amount.
    assert_eq!(assert_near(1.0, 1.1, 0.1), Ok(Value::Null));
}

#[test]
fn assert_near_rejects_negative_delta_and_non_finite_inputs() {
    assert!(matches!(
        assert_near(1.0, 1.0, -0.1),
        Err(RuntimeError::AssertionFailed(_))
    ));
    assert_eq!(assert_near(f64::NAN, 1.0, 0.1), Err(RuntimeError::Overflow));
    assert_eq!(
        assert_near(1.0, f64::INFINITY, 0.1),
        Err(RuntimeError::Overflow)
    );
}
