//! Shared numeric helpers.

#![allow(missing_docs)]

use crate::error::RuntimeError;
use crate::value::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumericKind {
    SInt,
    Int,
    DInt,
    LInt,
    USInt,
    UInt,
    UDInt,
    ULInt,
    Real,
    LReal,
}

pub fn numeric_kind(value: &Value) -> Option<NumericKind> {
    match value {
        Value::SInt(_) => Some(NumericKind::SInt),
        Value::Int(_) => Some(NumericKind::Int),
        Value::DInt(_) => Some(NumericKind::DInt),
        Value::LInt(_) => Some(NumericKind::LInt),
        Value::USInt(_) => Some(NumericKind::USInt),
        Value::UInt(_) => Some(NumericKind::UInt),
        Value::UDInt(_) => Some(NumericKind::UDInt),
        Value::ULInt(_) => Some(NumericKind::ULInt),
        Value::Real(_) => Some(NumericKind::Real),
        Value::LReal(_) => Some(NumericKind::LReal),
        _ => None,
    }
}

pub fn wider_numeric(left: NumericKind, right: NumericKind) -> Option<NumericKind> {
    if left == right {
        return Some(left);
    }
    if is_accuracy_preserving_widening(left, right) {
        return Some(left);
    }
    if is_accuracy_preserving_widening(right, left) {
        return Some(right);
    }
    None
}

fn is_accuracy_preserving_widening(target: NumericKind, source: NumericKind) -> bool {
    matches!(
        (target, source),
        (NumericKind::Int, NumericKind::SInt)
            | (NumericKind::DInt, NumericKind::SInt | NumericKind::Int)
            | (
                NumericKind::LInt,
                NumericKind::SInt | NumericKind::Int | NumericKind::DInt
            )
            | (NumericKind::UInt, NumericKind::USInt)
            | (NumericKind::UDInt, NumericKind::USInt | NumericKind::UInt)
            | (
                NumericKind::ULInt,
                NumericKind::USInt | NumericKind::UInt | NumericKind::UDInt
            )
            | (NumericKind::Real, NumericKind::SInt | NumericKind::Int)
            | (
                NumericKind::LReal,
                NumericKind::SInt | NumericKind::Int | NumericKind::DInt | NumericKind::Real
            )
    )
}

pub fn to_i64(value: &Value) -> Result<i64, RuntimeError> {
    match value {
        Value::SInt(v) => Ok(*v as i64),
        Value::Int(v) => Ok(*v as i64),
        Value::DInt(v) => Ok(*v as i64),
        Value::LInt(v) => Ok(*v),
        Value::USInt(v) => Ok(*v as i64),
        Value::UInt(v) => Ok(*v as i64),
        Value::UDInt(v) => Ok(*v as i64),
        Value::ULInt(v) => i64::try_from(*v).map_err(|_| RuntimeError::Overflow),
        _ => Err(RuntimeError::TypeMismatch),
    }
}

pub fn to_u64(value: &Value) -> Result<u64, RuntimeError> {
    match value {
        Value::USInt(v) => Ok(*v as u64),
        Value::UInt(v) => Ok(*v as u64),
        Value::UDInt(v) => Ok(*v as u64),
        Value::ULInt(v) => Ok(*v),
        Value::SInt(v) => {
            if *v < 0 {
                Err(RuntimeError::TypeMismatch)
            } else {
                Ok(*v as u64)
            }
        }
        Value::Int(v) => {
            if *v < 0 {
                Err(RuntimeError::TypeMismatch)
            } else {
                Ok(*v as u64)
            }
        }
        Value::DInt(v) => {
            if *v < 0 {
                Err(RuntimeError::TypeMismatch)
            } else {
                Ok(*v as u64)
            }
        }
        Value::LInt(v) => {
            if *v < 0 {
                Err(RuntimeError::TypeMismatch)
            } else {
                Ok(*v as u64)
            }
        }
        _ => Err(RuntimeError::TypeMismatch),
    }
}

pub fn to_f64(value: &Value) -> Result<f64, RuntimeError> {
    match value {
        Value::Real(v) => Ok(*v as f64),
        Value::LReal(v) => Ok(*v),
        Value::SInt(v) => Ok(*v as f64),
        Value::Int(v) => Ok(*v as f64),
        Value::DInt(v) => Ok(*v as f64),
        Value::LInt(v) => Ok(*v as f64),
        Value::USInt(v) => Ok(*v as f64),
        Value::UInt(v) => Ok(*v as f64),
        Value::UDInt(v) => Ok(*v as f64),
        Value::ULInt(v) => Ok(*v as f64),
        _ => Err(RuntimeError::TypeMismatch),
    }
}

pub fn signed_from_i128(target: NumericKind, value: i128) -> Result<Value, RuntimeError> {
    match target {
        NumericKind::SInt => i8::try_from(value)
            .map(Value::SInt)
            .map_err(|_| RuntimeError::Overflow),
        NumericKind::Int => i16::try_from(value)
            .map(Value::Int)
            .map_err(|_| RuntimeError::Overflow),
        NumericKind::DInt => i32::try_from(value)
            .map(Value::DInt)
            .map_err(|_| RuntimeError::Overflow),
        NumericKind::LInt => i64::try_from(value)
            .map(Value::LInt)
            .map_err(|_| RuntimeError::Overflow),
        _ => Err(RuntimeError::TypeMismatch),
    }
}

pub fn unsigned_from_u128(target: NumericKind, value: u128) -> Result<Value, RuntimeError> {
    match target {
        NumericKind::USInt => u8::try_from(value)
            .map(Value::USInt)
            .map_err(|_| RuntimeError::Overflow),
        NumericKind::UInt => u16::try_from(value)
            .map(Value::UInt)
            .map_err(|_| RuntimeError::Overflow),
        NumericKind::UDInt => u32::try_from(value)
            .map(Value::UDInt)
            .map_err(|_| RuntimeError::Overflow),
        NumericKind::ULInt => u64::try_from(value)
            .map(Value::ULInt)
            .map_err(|_| RuntimeError::Overflow),
        _ => Err(RuntimeError::TypeMismatch),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        NumericKind, numeric_kind, signed_from_i128, to_i64, to_u64, unsigned_from_u128,
        wider_numeric,
    };
    use crate::error::RuntimeError;
    use crate::value::Value;

    #[test]
    fn numeric_kind_identifies_supported_runtime_values() {
        assert_eq!(numeric_kind(&Value::DInt(1)), Some(NumericKind::DInt));
        assert_eq!(numeric_kind(&Value::LReal(1.0)), Some(NumericKind::LReal));
    }

    #[test]
    fn common_numeric_kind_is_accuracy_preserving_and_symmetric() {
        let accepted = [
            (NumericKind::Int, NumericKind::SInt, NumericKind::Int),
            (NumericKind::UInt, NumericKind::USInt, NumericKind::UInt),
            (NumericKind::Real, NumericKind::Int, NumericKind::Real),
            (NumericKind::LReal, NumericKind::DInt, NumericKind::LReal),
            (NumericKind::LReal, NumericKind::Real, NumericKind::LReal),
        ];
        for (left, right, expected) in accepted {
            assert_eq!(wider_numeric(left, right), Some(expected));
            assert_eq!(wider_numeric(right, left), Some(expected));
        }

        let rejected = [
            (NumericKind::UInt, NumericKind::SInt),
            (NumericKind::ULInt, NumericKind::Real),
            (NumericKind::LInt, NumericKind::LReal),
        ];
        for (left, right) in rejected {
            assert_eq!(wider_numeric(left, right), None);
            assert_eq!(wider_numeric(right, left), None);
        }
    }

    #[test]
    fn integer_conversions_preserve_overflow_and_signedness_errors() {
        assert_eq!(
            to_i64(&Value::ULInt(i64::MAX as u64 + 1)),
            Err(RuntimeError::Overflow)
        );
        assert_eq!(to_u64(&Value::DInt(-1)), Err(RuntimeError::TypeMismatch));
        assert_eq!(
            signed_from_i128(NumericKind::SInt, i128::from(i8::MAX) + 1),
            Err(RuntimeError::Overflow)
        );
        assert_eq!(
            unsigned_from_u128(NumericKind::USInt, u128::from(u8::MAX) + 1),
            Err(RuntimeError::Overflow)
        );
    }

    #[test]
    fn integer_i64_normalization_preserves_boundaries_and_rejects_invalid_inputs() {
        let representable = [
            (Value::SInt(i8::MIN), i64::from(i8::MIN)),
            (Value::SInt(i8::MAX), i64::from(i8::MAX)),
            (Value::Int(i16::MIN), i64::from(i16::MIN)),
            (Value::Int(i16::MAX), i64::from(i16::MAX)),
            (Value::DInt(i32::MIN), i64::from(i32::MIN)),
            (Value::DInt(i32::MAX), i64::from(i32::MAX)),
            (Value::LInt(i64::MIN), i64::MIN),
            (Value::LInt(i64::MAX), i64::MAX),
            (Value::USInt(u8::MAX), i64::from(u8::MAX)),
            (Value::UInt(u16::MAX), i64::from(u16::MAX)),
            (Value::UDInt(u32::MAX), i64::from(u32::MAX)),
            (Value::ULInt(i64::MAX as u64), i64::MAX),
        ];

        for (value, expected) in representable {
            assert_eq!(to_i64(&value), Ok(expected));
        }

        assert_eq!(
            to_i64(&Value::ULInt(i64::MAX as u64 + 1)),
            Err(RuntimeError::Overflow)
        );
        assert_eq!(to_i64(&Value::ULInt(u64::MAX)), Err(RuntimeError::Overflow));

        for value in [
            Value::Bool(false),
            Value::Real(0.0),
            Value::LReal(0.0),
            Value::Byte(0),
            Value::Word(0),
            Value::DWord(0),
            Value::LWord(0),
            Value::String("0".into()),
            Value::WString("0".into()),
            Value::Char(b'0'),
            Value::WChar(u16::from(b'0')),
            Value::Reference(None),
            Value::Null,
        ] {
            assert_eq!(to_i64(&value), Err(RuntimeError::TypeMismatch));
        }
    }

    #[test]
    fn signed_integer_materialization_preserves_boundaries_and_rejects_invalid_targets() {
        let signed_cases = [
            (
                NumericKind::SInt,
                i128::from(i8::MIN),
                i128::from(i8::MAX),
                Value::SInt(i8::MIN),
                Value::SInt(0),
                Value::SInt(i8::MAX),
            ),
            (
                NumericKind::Int,
                i128::from(i16::MIN),
                i128::from(i16::MAX),
                Value::Int(i16::MIN),
                Value::Int(0),
                Value::Int(i16::MAX),
            ),
            (
                NumericKind::DInt,
                i128::from(i32::MIN),
                i128::from(i32::MAX),
                Value::DInt(i32::MIN),
                Value::DInt(0),
                Value::DInt(i32::MAX),
            ),
            (
                NumericKind::LInt,
                i128::from(i64::MIN),
                i128::from(i64::MAX),
                Value::LInt(i64::MIN),
                Value::LInt(0),
                Value::LInt(i64::MAX),
            ),
        ];

        for (target, minimum, maximum, expected_minimum, expected_zero, expected_maximum) in
            signed_cases
        {
            assert_eq!(signed_from_i128(target, minimum), Ok(expected_minimum));
            assert_eq!(signed_from_i128(target, 0), Ok(expected_zero));
            assert_eq!(signed_from_i128(target, maximum), Ok(expected_maximum));
            assert_eq!(
                signed_from_i128(target, minimum - 1),
                Err(RuntimeError::Overflow)
            );
            assert_eq!(
                signed_from_i128(target, maximum + 1),
                Err(RuntimeError::Overflow)
            );
        }

        for target in [
            NumericKind::USInt,
            NumericKind::UInt,
            NumericKind::UDInt,
            NumericKind::ULInt,
            NumericKind::Real,
            NumericKind::LReal,
        ] {
            assert_eq!(signed_from_i128(target, 0), Err(RuntimeError::TypeMismatch));
        }
    }

    #[test]
    fn unsigned_integer_materialization_preserves_boundaries_and_rejects_invalid_targets() {
        let unsigned_cases = [
            (
                NumericKind::USInt,
                u128::from(u8::MAX),
                Value::USInt(0),
                Value::USInt(u8::MAX),
            ),
            (
                NumericKind::UInt,
                u128::from(u16::MAX),
                Value::UInt(0),
                Value::UInt(u16::MAX),
            ),
            (
                NumericKind::UDInt,
                u128::from(u32::MAX),
                Value::UDInt(0),
                Value::UDInt(u32::MAX),
            ),
            (
                NumericKind::ULInt,
                u128::from(u64::MAX),
                Value::ULInt(0),
                Value::ULInt(u64::MAX),
            ),
        ];

        for (target, maximum, expected_zero, expected_maximum) in unsigned_cases {
            assert_eq!(unsigned_from_u128(target, 0), Ok(expected_zero));
            assert_eq!(unsigned_from_u128(target, maximum), Ok(expected_maximum));
            assert_eq!(
                unsigned_from_u128(target, maximum + 1),
                Err(RuntimeError::Overflow)
            );
        }

        for target in [
            NumericKind::SInt,
            NumericKind::Int,
            NumericKind::DInt,
            NumericKind::LInt,
            NumericKind::Real,
            NumericKind::LReal,
        ] {
            assert_eq!(
                unsigned_from_u128(target, 0),
                Err(RuntimeError::TypeMismatch)
            );
        }
    }
}
