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
    use crate::value::{
        DateTimeError, DateTimeProfile, DateTimeValue, DateValue, Duration, LDateTimeValue,
        LDateValue, LTimeOfDayValue, TimeOfDayValue, Value,
    };

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
    fn same_type_operands_keep_checked_results_errors_and_float_comparisons() {
        use BinaryOp::*;
        use RuntimeError::{DivisionByZero, ModuloByZero, Overflow, TypeMismatch};
        let profile = DateTimeProfile::default();
        let nan = f64::NAN;
        let cases = [
            (Add, Value::Int(32767), Value::Int(1), Err(Overflow)),
            (Sub, Value::Int(-32768), Value::Int(1), Err(Overflow)),
            (Mul, Value::Int(200), Value::Int(200), Err(Overflow)),
            (Div, Value::Int(-32768), Value::Int(-1), Err(Overflow)),
            (Div, Value::Int(7), Value::Int(0), Err(DivisionByZero)),
            (Mod, Value::Int(7), Value::Int(0), Err(ModuloByZero)),
            (Mod, Value::Int(-32768), Value::Int(-1), Ok(Value::Int(0))),
            (Div, Value::Int(-7), Value::Int(2), Ok(Value::Int(-3))),
            (Mod, Value::Int(-7), Value::Int(2), Ok(Value::Int(-1))),
            (Mod, Value::Int(7), Value::Int(-2), Ok(Value::Int(1))),
            (Sub, Value::Int(5), Value::Int(9), Ok(Value::Int(-4))),
            (Pow, Value::Int(2), Value::Int(10), Ok(Value::Int(1024))),
            (And, Value::Int(1), Value::Int(1), Err(TypeMismatch)),
            (Lt, Value::Int(-1), Value::Int(0), Ok(Value::Bool(true))),
            (Ge, Value::Int(3), Value::Int(3), Ok(Value::Bool(true))),
            (Ne, Value::Int(3), Value::Int(4), Ok(Value::Bool(true))),
            (Add, Value::DInt(i32::MAX), Value::DInt(1), Err(Overflow)),
            (Div, Value::DInt(i32::MIN), Value::DInt(-1), Err(Overflow)),
            (
                Mod,
                Value::DInt(i32::MIN),
                Value::DInt(-1),
                Ok(Value::DInt(0)),
            ),
            (Mul, Value::DInt(46341), Value::DInt(46341), Err(Overflow)),
            (
                Mul,
                Value::DInt(-46340),
                Value::DInt(46340),
                Ok(Value::DInt(-2_147_395_600)),
            ),
            (Eq, Value::DInt(5), Value::DInt(5), Ok(Value::Bool(true))),
            (Add, Value::LInt(i64::MAX), Value::LInt(1), Err(Overflow)),
            (Div, Value::LInt(i64::MIN), Value::LInt(-1), Err(Overflow)),
            (
                Mod,
                Value::LInt(i64::MIN),
                Value::LInt(-1),
                Ok(Value::LInt(0)),
            ),
            (
                Gt,
                Value::LInt(i64::MIN),
                Value::LInt(i64::MAX),
                Ok(Value::Bool(false)),
            ),
            (
                Div,
                Value::LReal(1.0),
                Value::LReal(0.0),
                Err(DivisionByZero),
            ),
            (
                Div,
                Value::LReal(1.0),
                Value::LReal(-0.0),
                Err(DivisionByZero),
            ),
            (
                Mul,
                Value::LReal(f64::MAX),
                Value::LReal(2.0),
                Err(Overflow),
            ),
            (Add, Value::LReal(nan), Value::LReal(1.0), Err(Overflow)),
            (Div, Value::LReal(0.0), Value::LReal(nan), Err(Overflow)),
            (
                Add,
                Value::LReal(0.1),
                Value::LReal(0.2),
                Ok(Value::LReal(0.1 + 0.2)),
            ),
            (
                Div,
                Value::LReal(-1.0),
                Value::LReal(4.0),
                Ok(Value::LReal(-0.25)),
            ),
            (
                Pow,
                Value::LReal(2.0),
                Value::LReal(0.5),
                Ok(Value::LReal(2f64.powf(0.5))),
            ),
            (Mod, Value::LReal(5.0), Value::LReal(2.0), Err(TypeMismatch)),
            (
                Eq,
                Value::LReal(nan),
                Value::LReal(nan),
                Ok(Value::Bool(false)),
            ),
            (
                Ne,
                Value::LReal(nan),
                Value::LReal(nan),
                Ok(Value::Bool(true)),
            ),
            (
                Lt,
                Value::LReal(nan),
                Value::LReal(1.0),
                Ok(Value::Bool(false)),
            ),
            (
                Ge,
                Value::LReal(nan),
                Value::LReal(nan),
                Ok(Value::Bool(false)),
            ),
            (
                Eq,
                Value::LReal(0.0),
                Value::LReal(-0.0),
                Ok(Value::Bool(true)),
            ),
            (Mul, Value::Real(3.0e38), Value::Real(10.0), Err(Overflow)),
            (
                Add,
                Value::Real(0.1),
                Value::Real(0.2),
                Ok(Value::Real((f64::from(0.1f32) + f64::from(0.2f32)) as f32)),
            ),
            (Div, Value::Real(1.0), Value::Real(0.0), Err(DivisionByZero)),
            (Mod, Value::Real(5.0), Value::Real(2.0), Err(TypeMismatch)),
            (
                Lt,
                Value::Real(1.0),
                Value::Real(2.0),
                Ok(Value::Bool(true)),
            ),
            (
                Eq,
                Value::Real(f32::NAN),
                Value::Real(f32::NAN),
                Ok(Value::Bool(false)),
            ),
            (
                And,
                Value::Bool(true),
                Value::Bool(false),
                Ok(Value::Bool(false)),
            ),
            (
                AndThen,
                Value::Bool(true),
                Value::Bool(true),
                Ok(Value::Bool(true)),
            ),
            (
                Or,
                Value::Bool(false),
                Value::Bool(false),
                Ok(Value::Bool(false)),
            ),
            (
                OrElse,
                Value::Bool(false),
                Value::Bool(true),
                Ok(Value::Bool(true)),
            ),
            (
                Xor,
                Value::Bool(true),
                Value::Bool(true),
                Ok(Value::Bool(false)),
            ),
            (
                Eq,
                Value::Bool(true),
                Value::Bool(true),
                Ok(Value::Bool(true)),
            ),
            (
                Ne,
                Value::Bool(true),
                Value::Bool(false),
                Ok(Value::Bool(true)),
            ),
            (
                Lt,
                Value::Bool(false),
                Value::Bool(true),
                Ok(Value::Bool(true)),
            ),
            (Add, Value::Bool(true), Value::Bool(true), Err(TypeMismatch)),
        ];

        for (op, left, right, expected) in cases {
            let case = format!("{op:?} {left:?} {right:?}");
            assert_eq!(apply_binary(op, left, right, &profile), expected, "{case}");
        }
    }

    #[test]
    fn mixed_numeric_operands_require_accuracy_preserving_common_type() {
        let profile = DateTimeProfile::default();

        assert_eq!(
            apply_binary(BinaryOp::Add, Value::UInt(2), Value::UInt(1), &profile,),
            Ok(Value::UInt(3))
        );
        assert_eq!(
            apply_binary(BinaryOp::Add, Value::UInt(2), Value::SInt(1), &profile,),
            Err(RuntimeError::TypeMismatch)
        );
        assert_eq!(
            apply_binary(
                BinaryOp::Add,
                Value::ULInt(u64::MAX),
                Value::Real(1.0),
                &profile,
            ),
            Err(RuntimeError::TypeMismatch)
        );
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

    #[test]
    fn duration_arithmetic_preserves_width_and_rejects_overflow() {
        let profile = DateTimeProfile::default();

        assert_eq!(
            apply_binary(
                BinaryOp::Add,
                Value::Time(Duration::from_nanos(5)),
                Value::Time(Duration::from_nanos(-2)),
                &profile,
            ),
            Ok(Value::Time(Duration::from_nanos(3)))
        );
        assert_eq!(
            apply_binary(
                BinaryOp::Sub,
                Value::LTime(Duration::from_nanos(5)),
                Value::LTime(Duration::from_nanos(7)),
                &profile,
            ),
            Ok(Value::LTime(Duration::from_nanos(-2)))
        );
        assert_eq!(
            apply_binary(
                BinaryOp::Add,
                Value::Time(Duration::from_nanos(i64::MAX)),
                Value::Time(Duration::from_nanos(1)),
                &profile,
            ),
            Err(RuntimeError::Overflow)
        );
        assert_eq!(
            apply_binary(
                BinaryOp::Add,
                Value::Time(Duration::from_nanos(1)),
                Value::LTime(Duration::from_nanos(1)),
                &profile,
            ),
            Err(RuntimeError::TypeMismatch)
        );
    }

    #[test]
    fn short_date_time_arithmetic_uses_profile_ticks_without_wrapping() {
        let profile = DateTimeProfile::default();
        let two_and_a_half_ticks = Duration::from_micros(2_500);

        assert_eq!(
            apply_binary(
                BinaryOp::Add,
                Value::Tod(TimeOfDayValue::new(10)),
                Value::Time(two_and_a_half_ticks),
                &profile,
            ),
            Ok(Value::Tod(TimeOfDayValue::new(12)))
        );
        assert_eq!(
            apply_binary(
                BinaryOp::Sub,
                Value::Tod(TimeOfDayValue::new(10)),
                Value::Time(two_and_a_half_ticks),
                &profile,
            ),
            Ok(Value::Tod(TimeOfDayValue::new(8)))
        );
        assert_eq!(
            apply_binary(
                BinaryOp::Add,
                Value::Time(two_and_a_half_ticks),
                Value::Tod(TimeOfDayValue::new(10)),
                &profile,
            ),
            Ok(Value::Tod(TimeOfDayValue::new(12)))
        );
        assert_eq!(
            apply_binary(
                BinaryOp::Add,
                Value::Tod(TimeOfDayValue::new(10)),
                Value::Time(Duration::from_micros(-2_500)),
                &profile,
            ),
            Ok(Value::Tod(TimeOfDayValue::new(8)))
        );
        assert_eq!(
            apply_binary(
                BinaryOp::Add,
                Value::Time(two_and_a_half_ticks),
                Value::Dt(DateTimeValue::new(20)),
                &profile,
            ),
            Ok(Value::Dt(DateTimeValue::new(22)))
        );
        assert_eq!(
            apply_binary(
                BinaryOp::Sub,
                Value::Dt(DateTimeValue::new(20)),
                Value::Time(two_and_a_half_ticks),
                &profile,
            ),
            Ok(Value::Dt(DateTimeValue::new(18)))
        );
        assert_eq!(
            apply_binary(
                BinaryOp::Sub,
                Value::Date(DateValue::new(20)),
                Value::Date(DateValue::new(7)),
                &profile,
            ),
            Ok(Value::Time(Duration::from_millis(13)))
        );
        assert_eq!(
            apply_binary(
                BinaryOp::Sub,
                Value::Date(DateValue::new(7)),
                Value::Date(DateValue::new(20)),
                &profile,
            ),
            Ok(Value::Time(Duration::from_millis(-13)))
        );
        assert_eq!(
            apply_binary(
                BinaryOp::Sub,
                Value::Tod(TimeOfDayValue::new(20)),
                Value::Tod(TimeOfDayValue::new(7)),
                &profile,
            ),
            Ok(Value::Time(Duration::from_millis(13)))
        );
        assert_eq!(
            apply_binary(
                BinaryOp::Sub,
                Value::Dt(DateTimeValue::new(20)),
                Value::Dt(DateTimeValue::new(7)),
                &profile,
            ),
            Ok(Value::Time(Duration::from_millis(13)))
        );
        assert_eq!(
            apply_binary(
                BinaryOp::Add,
                Value::Tod(TimeOfDayValue::new(i64::MAX)),
                Value::Time(Duration::from_millis(1)),
                &profile,
            ),
            Err(RuntimeError::DateTimeRange(DateTimeError::OutOfRange))
        );

        let zero_resolution = DateTimeProfile {
            resolution: Duration::ZERO,
            ..profile
        };
        assert_eq!(
            apply_binary(
                BinaryOp::Add,
                Value::Dt(DateTimeValue::new(0)),
                Value::Time(Duration::from_millis(1)),
                &zero_resolution,
            ),
            Err(RuntimeError::Overflow)
        );
    }

    #[test]
    fn long_date_time_arithmetic_preserves_nanoseconds_and_checked_range() {
        let profile = DateTimeProfile::default();

        assert_eq!(
            apply_binary(
                BinaryOp::Add,
                Value::LTod(LTimeOfDayValue::new(10)),
                Value::LTime(Duration::from_nanos(3)),
                &profile,
            ),
            Ok(Value::LTod(LTimeOfDayValue::new(13)))
        );
        assert_eq!(
            apply_binary(
                BinaryOp::Add,
                Value::LTime(Duration::from_nanos(3)),
                Value::LTod(LTimeOfDayValue::new(10)),
                &profile,
            ),
            Ok(Value::LTod(LTimeOfDayValue::new(13)))
        );
        assert_eq!(
            apply_binary(
                BinaryOp::Sub,
                Value::LTod(LTimeOfDayValue::new(10)),
                Value::LTime(Duration::from_nanos(3)),
                &profile,
            ),
            Ok(Value::LTod(LTimeOfDayValue::new(7)))
        );
        assert_eq!(
            apply_binary(
                BinaryOp::Sub,
                Value::Ldt(LDateTimeValue::new(10)),
                Value::LTime(Duration::from_nanos(3)),
                &profile,
            ),
            Ok(Value::Ldt(LDateTimeValue::new(7)))
        );
        assert_eq!(
            apply_binary(
                BinaryOp::Add,
                Value::Ldt(LDateTimeValue::new(10)),
                Value::LTime(Duration::from_nanos(3)),
                &profile,
            ),
            Ok(Value::Ldt(LDateTimeValue::new(13)))
        );
        for (left, right) in [
            (
                Value::LDate(LDateValue::new(20)),
                Value::LDate(LDateValue::new(7)),
            ),
            (
                Value::LTod(LTimeOfDayValue::new(20)),
                Value::LTod(LTimeOfDayValue::new(7)),
            ),
            (
                Value::Ldt(LDateTimeValue::new(20)),
                Value::Ldt(LDateTimeValue::new(7)),
            ),
        ] {
            assert_eq!(
                apply_binary(BinaryOp::Sub, left, right, &profile),
                Ok(Value::LTime(Duration::from_nanos(13)))
            );
        }
        assert_eq!(
            apply_binary(
                BinaryOp::Add,
                Value::LTime(Duration::from_nanos(1)),
                Value::Ldt(LDateTimeValue::new(i64::MAX)),
                &profile,
            ),
            Err(RuntimeError::Overflow)
        );
    }

    #[test]
    fn time_scaling_truncates_toward_zero_and_rejects_invalid_factors() {
        let profile = DateTimeProfile::default();

        for factor in [
            Value::SInt(2),
            Value::Int(2),
            Value::DInt(2),
            Value::LInt(2),
            Value::USInt(2),
            Value::UInt(2),
            Value::UDInt(2),
            Value::ULInt(2),
            Value::Real(2.0),
            Value::LReal(2.0),
        ] {
            assert_eq!(
                apply_binary(
                    BinaryOp::Mul,
                    Value::Time(Duration::from_nanos(3)),
                    factor,
                    &profile,
                ),
                Ok(Value::Time(Duration::from_nanos(6)))
            );
        }
        assert_eq!(
            apply_binary(
                BinaryOp::Mul,
                Value::Time(Duration::from_nanos(7)),
                Value::DInt(3),
                &profile,
            ),
            Ok(Value::Time(Duration::from_nanos(21)))
        );
        assert_eq!(
            apply_binary(
                BinaryOp::Mul,
                Value::DInt(3),
                Value::LTime(Duration::from_nanos(7)),
                &profile,
            ),
            Ok(Value::LTime(Duration::from_nanos(21)))
        );
        assert_eq!(
            apply_binary(
                BinaryOp::Div,
                Value::Time(Duration::from_nanos(-7)),
                Value::DInt(2),
                &profile,
            ),
            Ok(Value::Time(Duration::from_nanos(-3)))
        );
        assert_eq!(
            apply_binary(
                BinaryOp::Mul,
                Value::Time(Duration::from_nanos(5)),
                Value::Real(0.5),
                &profile,
            ),
            Ok(Value::Time(Duration::from_nanos(2)))
        );
        assert_eq!(
            apply_binary(
                BinaryOp::Div,
                Value::LTime(Duration::from_nanos(-5)),
                Value::LReal(2.0),
                &profile,
            ),
            Ok(Value::LTime(Duration::from_nanos(-2)))
        );
        for zero in [Value::DInt(0), Value::LReal(0.0)] {
            assert_eq!(
                apply_binary(
                    BinaryOp::Div,
                    Value::Time(Duration::from_nanos(1)),
                    zero,
                    &profile,
                ),
                Err(RuntimeError::DivisionByZero)
            );
        }
        assert_eq!(
            apply_binary(
                BinaryOp::Mul,
                Value::Time(Duration::from_nanos(1)),
                Value::Bool(true),
                &profile,
            ),
            Err(RuntimeError::TypeMismatch)
        );
        for factor in [Value::LReal(f64::NAN), Value::LReal(f64::INFINITY)] {
            assert_eq!(
                apply_binary(
                    BinaryOp::Mul,
                    Value::Time(Duration::from_nanos(1)),
                    factor,
                    &profile,
                ),
                Err(RuntimeError::Overflow)
            );
        }
        assert_eq!(
            apply_binary(
                BinaryOp::Mul,
                Value::Time(Duration::from_nanos(i64::MAX)),
                Value::DInt(2),
                &profile,
            ),
            Err(RuntimeError::Overflow)
        );
    }

    #[test]
    fn date_time_ordering_requires_matching_runtime_families() {
        let profile = DateTimeProfile::default();
        let ordered_pairs = [
            (
                Value::Time(Duration::from_nanos(1)),
                Value::Time(Duration::from_nanos(2)),
            ),
            (
                Value::LTime(Duration::from_nanos(1)),
                Value::LTime(Duration::from_nanos(2)),
            ),
            (
                Value::Date(DateValue::new(1)),
                Value::Date(DateValue::new(2)),
            ),
            (
                Value::LDate(LDateValue::new(1)),
                Value::LDate(LDateValue::new(2)),
            ),
            (
                Value::Tod(TimeOfDayValue::new(1)),
                Value::Tod(TimeOfDayValue::new(2)),
            ),
            (
                Value::LTod(LTimeOfDayValue::new(1)),
                Value::LTod(LTimeOfDayValue::new(2)),
            ),
            (
                Value::Dt(DateTimeValue::new(1)),
                Value::Dt(DateTimeValue::new(2)),
            ),
            (
                Value::Ldt(LDateTimeValue::new(1)),
                Value::Ldt(LDateTimeValue::new(2)),
            ),
        ];

        for (left, right) in ordered_pairs {
            assert_eq!(
                apply_binary(BinaryOp::Lt, left.clone(), right.clone(), &profile),
                Ok(Value::Bool(true))
            );
            assert_eq!(
                apply_binary(BinaryOp::Ge, right, left, &profile),
                Ok(Value::Bool(true))
            );
        }
        assert_eq!(
            apply_binary(
                BinaryOp::Le,
                Value::Dt(DateTimeValue::new(2)),
                Value::Dt(DateTimeValue::new(2)),
                &profile,
            ),
            Ok(Value::Bool(true))
        );
        assert_eq!(
            apply_binary(
                BinaryOp::Gt,
                Value::Ldt(LDateTimeValue::new(2)),
                Value::Ldt(LDateTimeValue::new(1)),
                &profile,
            ),
            Ok(Value::Bool(true))
        );
        assert_eq!(
            apply_binary(
                BinaryOp::Lt,
                Value::Date(DateValue::new(1)),
                Value::Dt(DateTimeValue::new(2)),
                &profile,
            ),
            Err(RuntimeError::TypeMismatch)
        );
    }

    #[test]
    fn unary_contracts_preserve_tags_and_reject_invalid_operands() {
        assert_eq!(
            apply_unary(UnaryOp::Neg, Value::DInt(7)),
            Ok(Value::DInt(-7))
        );
        assert_eq!(
            apply_unary(UnaryOp::Neg, Value::DInt(i32::MIN)),
            Err(RuntimeError::Overflow)
        );
        assert_eq!(
            apply_unary(UnaryOp::Pos, Value::String("unchanged".into())),
            Ok(Value::String("unchanged".into()))
        );
        assert_eq!(
            apply_unary(UnaryOp::Not, Value::Word(0x00ff)),
            Ok(Value::Word(0xff00))
        );
        assert_eq!(
            apply_unary(UnaryOp::Not, Value::Int(1)),
            Err(RuntimeError::TypeMismatch)
        );
    }
}
