//! Operator implementations shared by the VM and helper evaluators.

#![allow(missing_docs)]

use crate::error::RuntimeError;
use crate::numeric::{
    numeric_kind, signed_from_i128, to_f64, to_i64, to_u64, unsigned_from_u128, wider_numeric,
    NumericKind,
};
use crate::value::{
    DateTimeProfile, DateTimeValue, DateValue, Duration, LDateTimeValue, LDateValue,
    LTimeOfDayValue, TimeOfDayValue, Value,
};

include!("ops/contracts.rs");
include!("ops/logical_cmp.rs");
include!("ops/time_ops.rs");
include!("ops/numeric_arith.rs");

#[cfg(test)]
mod tests {
    use super::{apply_binary, apply_unary, BinaryOp, UnaryOp};
    use crate::error::RuntimeError;
    use crate::value::{DateTimeProfile, Value};

    #[test]
    fn numeric_ops_preserve_checked_runtime_contract() {
        let profile = DateTimeProfile::default();

        assert_eq!(
            apply_binary(BinaryOp::Add, Value::Int(2), Value::Int(3), &profile),
            Ok(Value::Int(5))
        );
        assert_eq!(
            apply_unary(UnaryOp::Not, Value::Bool(true)),
            Ok(Value::Bool(false))
        );
    }

    #[test]
    fn real_arithmetic_rejects_non_finite_single_width_results() {
        let profile = DateTimeProfile::default();
        let cases = [
            (BinaryOp::Add, f32::MAX, f32::MAX),
            (BinaryOp::Sub, f32::MAX, -f32::MAX),
            (BinaryOp::Mul, f32::MAX, 2.0),
            (BinaryOp::Div, f32::MAX, 0.5),
            (BinaryOp::Pow, f32::MAX, 2.0),
        ];

        for (op, left, right) in cases {
            assert_eq!(
                apply_binary(op, Value::Real(left), Value::Real(right), &profile,),
                Err(RuntimeError::Overflow),
                "operator {op:?} must reject single-width overflow"
            );
        }
    }

    #[test]
    fn non_numeric_comparisons_preserve_runtime_contract() {
        let profile = DateTimeProfile::default();

        assert_eq!(
            apply_binary(
                BinaryOp::Lt,
                Value::String("A".into()),
                Value::String("B".into()),
                &profile,
            ),
            Ok(Value::Bool(true))
        );
    }
}
