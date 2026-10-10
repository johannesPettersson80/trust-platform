//! Assertion helpers for user-facing ST tests.

use crate::error::RuntimeError;
use crate::stdlib::helpers::{
    coerce_to_common, common_kind, compare_common, require_arity, to_f64, CmpOp,
};
#[cfg(feature = "hir")]
use crate::stdlib::StandardLibrary;
use crate::value::{format_user_value, Value};
use alloc::format;

/// Register the shared assertions functions in a hosted registry.
#[cfg(feature = "hir")]
pub fn register(lib: &mut StandardLibrary) {
    lib.register_descriptors(FUNCTIONS);
}

pub(super) static FUNCTIONS: &[(&str, super::StdFunctionRef<'static>)] = &[
    super::registration::descriptor!("ASSERT_EQUAL", EXPECTED_ACTUAL, assert_equal),
    super::registration::descriptor!("ASSERT_FALSE", IN, assert_false),
    super::registration::descriptor!("ASSERT_GREATER", VALUE_BOUND, assert_greater),
    super::registration::descriptor!(
        "ASSERT_GREATER_OR_EQUAL",
        VALUE_BOUND,
        assert_greater_or_equal
    ),
    super::registration::descriptor!("ASSERT_LESS", VALUE_BOUND, assert_less),
    super::registration::descriptor!("ASSERT_LESS_OR_EQUAL", VALUE_BOUND, assert_less_or_equal),
    super::registration::descriptor!("ASSERT_NEAR", EXPECTED_ACTUAL_DELTA, assert_near),
    super::registration::descriptor!("ASSERT_NOT_EQUAL", EXPECTED_ACTUAL, assert_not_equal),
    super::registration::descriptor!("ASSERT_TRUE", IN, assert_true),
];

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
    assert_compare(args, CmpOp::Eq)
}

fn assert_not_equal(args: &[Value]) -> Result<Value, RuntimeError> {
    assert_compare(args, CmpOp::Ne)
}

fn assert_greater(args: &[Value]) -> Result<Value, RuntimeError> {
    assert_compare(args, CmpOp::Gt)
}

fn assert_less(args: &[Value]) -> Result<Value, RuntimeError> {
    assert_compare(args, CmpOp::Lt)
}

fn assert_greater_or_equal(args: &[Value]) -> Result<Value, RuntimeError> {
    assert_compare(args, CmpOp::Ge)
}

fn assert_less_or_equal(args: &[Value]) -> Result<Value, RuntimeError> {
    assert_compare(args, CmpOp::Le)
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
    let rounding_tolerance =
        f64::EPSILON * expected.abs().max(actual.abs()).max(delta).max(1.0) * 4.0;
    if diff <= delta || diff - delta <= rounding_tolerance {
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

// One shared control-flow body; wrappers retain all public function identities.
#[inline(never)]
fn assert_compare(args: &[Value], op: CmpOp) -> Result<Value, RuntimeError> {
    require_arity(args, 2)?;
    let kind = common_kind(args)?;
    let left = coerce_to_common(&args[0], &kind)?;
    let right = coerce_to_common(&args[1], &kind)?;
    if compare_common(&left, &right, &kind, op)? {
        Ok(Value::Null)
    } else {
        Err(comparison_failure(op, &args[0], &args[1]))
    }
}

#[cold]
#[inline(never)]
fn comparison_failure(op: CmpOp, left: &Value, right: &Value) -> RuntimeError {
    let left = format_user_value(left);
    let right = format_user_value(right);
    let message = match op {
        CmpOp::Eq => format!("ASSERT_EQUAL failed: expected {left}, actual {right}"),
        CmpOp::Ne => {
            format!("ASSERT_NOT_EQUAL failed: values should differ, left {left}, right {right}")
        }
        CmpOp::Gt => {
            format!("ASSERT_GREATER failed: value {left} is not greater than bound {right}")
        }
        CmpOp::Lt => format!("ASSERT_LESS failed: value {left} is not less than bound {right}"),
        CmpOp::Ge => {
            format!("ASSERT_GREATER_OR_EQUAL failed: value {left} is not >= bound {right}")
        }
        CmpOp::Le => format!("ASSERT_LESS_OR_EQUAL failed: value {left} is not <= bound {right}"),
    };
    RuntimeError::AssertionFailed(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_comparison_keeps_all_predicates_and_original_operand_messages() {
        for (function, op, left, right, message) in [
            (
                assert_equal as super::super::StdFunc,
                CmpOp::Eq,
                1,
                2,
                "ASSERT_EQUAL failed: expected 1, actual 2",
            ),
            (
                assert_not_equal,
                CmpOp::Ne,
                1,
                1,
                "ASSERT_NOT_EQUAL failed: values should differ, left 1, right 1",
            ),
            (
                assert_greater,
                CmpOp::Gt,
                1,
                2,
                "ASSERT_GREATER failed: value 1 is not greater than bound 2",
            ),
            (
                assert_less,
                CmpOp::Lt,
                2,
                1,
                "ASSERT_LESS failed: value 2 is not less than bound 1",
            ),
            (
                assert_greater_or_equal,
                CmpOp::Ge,
                1,
                2,
                "ASSERT_GREATER_OR_EQUAL failed: value 1 is not >= bound 2",
            ),
            (
                assert_less_or_equal,
                CmpOp::Le,
                2,
                1,
                "ASSERT_LESS_OR_EQUAL failed: value 2 is not <= bound 1",
            ),
        ] {
            assert_eq!(
                function(&[Value::Int(left), Value::DInt(right)]),
                Err(RuntimeError::AssertionFailed(message.into()))
            );
            let pass = match op {
                CmpOp::Eq | CmpOp::Ge | CmpOp::Le => [1, 1],
                CmpOp::Ne | CmpOp::Lt => [1, 2],
                CmpOp::Gt => [2, 1],
            };
            assert_eq!(
                function(&[Value::Int(pass[0]), Value::Int(pass[1])]),
                Ok(Value::Null)
            );
            assert!(matches!(
                function(&[]),
                Err(RuntimeError::InvalidArgumentCount {
                    expected: 2,
                    got: 0
                })
            ));
            assert_eq!(
                function(&[Value::Null, Value::Int(1)]),
                Err(RuntimeError::TypeMismatch)
            );
        }
        assert_eq!(
            assert_equal(&[Value::Real(1.0), Value::LReal(2.0)]),
            Err(RuntimeError::AssertionFailed(
                "ASSERT_EQUAL failed: expected 1.0, actual 2.0".into()
            ))
        );
    }
}
