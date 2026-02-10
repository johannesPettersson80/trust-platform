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
    lib.register(
        "ASSERT_NOT_EQUAL",
        &["EXPECTED", "ACTUAL"],
        assert_not_equal,
    );
    lib.register("ASSERT_GREATER", &["VALUE", "BOUND"], assert_greater);
    lib.register("ASSERT_LESS", &["VALUE", "BOUND"], assert_less);
    lib.register(
        "ASSERT_GREATER_OR_EQUAL",
        &["VALUE", "BOUND"],
        assert_greater_or_equal,
    );
    lib.register(
        "ASSERT_LESS_OR_EQUAL",
        &["VALUE", "BOUND"],
        assert_less_or_equal,
    );
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
    assert_compare(args, CmpOp::Eq, "ASSERT_EQUAL", |left, right| {
        format!(
            "ASSERT_EQUAL failed: expected {:?}, actual {:?}",
            left, right
        )
    })
}

fn assert_not_equal(args: &[Value]) -> Result<Value, RuntimeError> {
    assert_compare(args, CmpOp::Ne, "ASSERT_NOT_EQUAL", |left, right| {
        format!(
            "ASSERT_NOT_EQUAL failed: values should differ, left {:?}, right {:?}",
            left, right
        )
    })
}

fn assert_greater(args: &[Value]) -> Result<Value, RuntimeError> {
    assert_compare(args, CmpOp::Gt, "ASSERT_GREATER", |value, bound| {
        format!(
            "ASSERT_GREATER failed: value {:?} is not greater than bound {:?}",
            value, bound
        )
    })
}

fn assert_less(args: &[Value]) -> Result<Value, RuntimeError> {
    assert_compare(args, CmpOp::Lt, "ASSERT_LESS", |value, bound| {
        format!(
            "ASSERT_LESS failed: value {:?} is not less than bound {:?}",
            value, bound
        )
    })
}

fn assert_greater_or_equal(args: &[Value]) -> Result<Value, RuntimeError> {
    assert_compare(
        args,
        CmpOp::Ge,
        "ASSERT_GREATER_OR_EQUAL",
        |value, bound| {
            format!(
                "ASSERT_GREATER_OR_EQUAL failed: value {:?} is not >= bound {:?}",
                value, bound
            )
        },
    )
}

fn assert_less_or_equal(args: &[Value]) -> Result<Value, RuntimeError> {
    assert_compare(args, CmpOp::Le, "ASSERT_LESS_OR_EQUAL", |value, bound| {
        format!(
            "ASSERT_LESS_OR_EQUAL failed: value {:?} is not <= bound {:?}",
            value, bound
        )
    })
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

fn assert_compare(
    args: &[Value],
    op: CmpOp,
    _name: &str,
    message: impl Fn(&Value, &Value) -> String,
) -> Result<Value, RuntimeError> {
    require_arity(args, 2)?;
    let kind = common_kind(args)?;
    let left = coerce_to_common(&args[0], &kind)?;
    let right = coerce_to_common(&args[1], &kind)?;
    if compare_common(&left, &right, &kind, op)? {
        Ok(Value::Null)
    } else {
        Err(RuntimeError::AssertionFailed(
            message(&args[0], &args[1]).into(),
        ))
    }
}
