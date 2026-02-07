//! Assertion helpers for user-facing ST tests.

#![allow(missing_docs)]

use crate::error::RuntimeError;
use crate::stdlib::helpers::{
    coerce_to_common, common_kind, compare_common, require_arity, to_f64, CmpOp,
};
use crate::stdlib::StandardLibrary;
use crate::value::Value;

pub fn register(lib: &mut StandardLibrary) {
    lib.register("ASSERT_TRUE", &["IN"], assert_true);
    lib.register("ASSERT_FALSE", &["IN"], assert_false);
    lib.register("ASSERT_EQUAL", &["EXPECTED", "ACTUAL"], assert_equal);
    lib.register("ASSERT_NEAR", &["EXPECTED", "ACTUAL", "DELTA"], assert_near);
}

fn assert_true(args: &[Value]) -> Result<Value, RuntimeError> {
    require_arity(args, 1)?;
    match &args[0] {
        Value::Bool(true) => Ok(Value::Null),
        Value::Bool(false) => Err(RuntimeError::AssertionFailed(
            "ASSERT_TRUE expected TRUE, got FALSE".into(),
        )),
        _ => Err(RuntimeError::TypeMismatch),
    }
}

fn assert_false(args: &[Value]) -> Result<Value, RuntimeError> {
    require_arity(args, 1)?;
    match &args[0] {
        Value::Bool(false) => Ok(Value::Null),
        Value::Bool(true) => Err(RuntimeError::AssertionFailed(
            "ASSERT_FALSE expected FALSE, got TRUE".into(),
        )),
        _ => Err(RuntimeError::TypeMismatch),
    }
}

fn assert_equal(args: &[Value]) -> Result<Value, RuntimeError> {
    require_arity(args, 2)?;
    let kind = common_kind(args)?;
    let expected = coerce_to_common(&args[0], &kind)?;
    let actual = coerce_to_common(&args[1], &kind)?;
    if compare_common(&expected, &actual, &kind, CmpOp::Eq)? {
        Ok(Value::Null)
    } else {
        Err(RuntimeError::AssertionFailed(
            format!(
                "ASSERT_EQUAL failed: expected {:?}, actual {:?}",
                args[0], args[1]
            )
            .into(),
        ))
    }
}

fn assert_near(args: &[Value]) -> Result<Value, RuntimeError> {
    require_arity(args, 3)?;
    let expected = to_f64(&args[0])?;
    let actual = to_f64(&args[1])?;
    let delta = to_f64(&args[2])?;

    if !expected.is_finite() || !actual.is_finite() || !delta.is_finite() {
        return Err(RuntimeError::Overflow);
    }
    if delta < 0.0 {
        return Err(RuntimeError::AssertionFailed(
            "ASSERT_NEAR failed: DELTA must be non-negative".into(),
        ));
    }

    let diff = (expected - actual).abs();
    if diff <= delta {
        Ok(Value::Null)
    } else {
        Err(RuntimeError::AssertionFailed(
            format!(
                "ASSERT_NEAR failed: expected {expected}, actual {actual}, delta {delta}, diff {diff}"
            )
            .into(),
        ))
    }
}
