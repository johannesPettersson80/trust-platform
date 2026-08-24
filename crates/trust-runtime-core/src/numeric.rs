//! Shared numeric helpers.

#![allow(missing_docs)]

mod kind;

#[cfg(test)]
use kind::is_accuracy_preserving_widening;

pub use kind::{numeric_kind, wider_numeric, NumericKind};

use crate::error::RuntimeError;
use crate::value::Value;

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
        is_accuracy_preserving_widening, numeric_kind, signed_from_i128, to_f64, to_i64, to_u64,
        unsigned_from_u128, wider_numeric, NumericKind,
    };
    use crate::error::RuntimeError;
    use crate::value::Value;

    #[test]
    fn numeric_kind_identifies_supported_runtime_values() {
        assert_eq!(numeric_kind(&Value::DInt(1)), Some(NumericKind::DInt));
        assert_eq!(numeric_kind(&Value::LReal(1.0)), Some(NumericKind::LReal));
    }

    #[test]
    fn numeric_kind_and_widening_matrix_cover_every_runtime_numeric_tag() {
        let tagged_values = [
            (Value::SInt(0), NumericKind::SInt),
            (Value::Int(0), NumericKind::Int),
            (Value::DInt(0), NumericKind::DInt),
            (Value::LInt(0), NumericKind::LInt),
            (Value::USInt(0), NumericKind::USInt),
            (Value::UInt(0), NumericKind::UInt),
            (Value::UDInt(0), NumericKind::UDInt),
            (Value::ULInt(0), NumericKind::ULInt),
            (Value::Real(0.0), NumericKind::Real),
            (Value::LReal(0.0), NumericKind::LReal),
        ];
        for (value, expected) in tagged_values {
            assert_eq!(numeric_kind(&value), Some(expected), "{value:?}");
        }
        for value in [
            Value::Bool(false),
            Value::Byte(0),
            Value::Word(0),
            Value::DWord(0),
            Value::LWord(0),
            Value::String("0".into()),
            Value::Reference(None),
            Value::Null,
        ] {
            assert_eq!(numeric_kind(&value), None, "{value:?}");
        }

        let kinds = [
            NumericKind::SInt,
            NumericKind::Int,
            NumericKind::DInt,
            NumericKind::LInt,
            NumericKind::USInt,
            NumericKind::UInt,
            NumericKind::UDInt,
            NumericKind::ULInt,
            NumericKind::Real,
            NumericKind::LReal,
        ];
        let accepted_widenings = [
            (NumericKind::Int, NumericKind::SInt),
            (NumericKind::DInt, NumericKind::SInt),
            (NumericKind::DInt, NumericKind::Int),
            (NumericKind::LInt, NumericKind::SInt),
            (NumericKind::LInt, NumericKind::Int),
            (NumericKind::LInt, NumericKind::DInt),
            (NumericKind::UInt, NumericKind::USInt),
            (NumericKind::UDInt, NumericKind::USInt),
            (NumericKind::UDInt, NumericKind::UInt),
            (NumericKind::ULInt, NumericKind::USInt),
            (NumericKind::ULInt, NumericKind::UInt),
            (NumericKind::ULInt, NumericKind::UDInt),
            (NumericKind::Real, NumericKind::SInt),
            (NumericKind::Real, NumericKind::Int),
            (NumericKind::LReal, NumericKind::SInt),
            (NumericKind::LReal, NumericKind::Int),
            (NumericKind::LReal, NumericKind::DInt),
            (NumericKind::LReal, NumericKind::Real),
        ];

        for target in kinds {
            for source in kinds {
                let expected_widening = accepted_widenings.contains(&(target, source));
                assert_eq!(
                    is_accuracy_preserving_widening(target, source),
                    expected_widening,
                    "target={target:?}, source={source:?}"
                );

                let expected_common = if target == source || expected_widening {
                    Some(target)
                } else if accepted_widenings.contains(&(source, target)) {
                    Some(source)
                } else {
                    None
                };
                assert_eq!(
                    wider_numeric(target, source),
                    expected_common,
                    "left={target:?}, right={source:?}"
                );
            }
        }
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

    #[test]
    fn unsigned_and_float_normalization_preserve_values_and_reject_non_numeric_tags() {
        let unsigned_cases = [
            (Value::USInt(u8::MAX), u64::from(u8::MAX)),
            (Value::UInt(u16::MAX), u64::from(u16::MAX)),
            (Value::UDInt(u32::MAX), u64::from(u32::MAX)),
            (Value::ULInt(u64::MAX), u64::MAX),
            (Value::SInt(i8::MAX), i8::MAX as u64),
            (Value::Int(i16::MAX), i16::MAX as u64),
            (Value::DInt(i32::MAX), i32::MAX as u64),
            (Value::LInt(i64::MAX), i64::MAX as u64),
        ];
        for (value, expected) in unsigned_cases {
            assert_eq!(to_u64(&value), Ok(expected));
        }
        for value in [
            Value::SInt(-1),
            Value::Int(-1),
            Value::DInt(-1),
            Value::LInt(-1),
        ] {
            assert_eq!(to_u64(&value), Err(RuntimeError::TypeMismatch));
        }

        let float_cases = [
            (Value::Real(1.25), 1.25_f64),
            (Value::LReal(-2.5), -2.5_f64),
            (Value::SInt(i8::MIN), i8::MIN as f64),
            (Value::Int(i16::MIN), i16::MIN as f64),
            (Value::DInt(i32::MIN), i32::MIN as f64),
            (Value::LInt(i64::MIN), i64::MIN as f64),
            (Value::USInt(u8::MAX), u8::MAX as f64),
            (Value::UInt(42), 42.0_f64),
            (Value::UDInt(u32::MAX), u32::MAX as f64),
            (Value::ULInt(u64::MAX), u64::MAX as f64),
        ];
        for (value, expected) in float_cases {
            assert_eq!(to_f64(&value), Ok(expected));
        }

        for value in [Value::Bool(false), Value::Word(0), Value::Null] {
            assert_eq!(to_u64(&value), Err(RuntimeError::TypeMismatch));
            assert_eq!(to_f64(&value), Err(RuntimeError::TypeMismatch));
        }
    }
}
