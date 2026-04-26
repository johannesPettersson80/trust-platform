//! Helpers for standard function implementations.

#![allow(missing_docs)]

use crate::error::RuntimeError;
pub use crate::numeric::{
    numeric_kind, signed_from_i128, to_f64, to_i64, to_u64, unsigned_from_u128, wider_numeric,
    NumericKind,
};
use crate::value::{Duration, Value};
use smol_str::SmolStr;

pub fn require_arity(args: &[Value], expected: usize) -> Result<(), RuntimeError> {
    if args.len() == expected {
        Ok(())
    } else {
        Err(RuntimeError::InvalidArgumentCount {
            expected,
            got: args.len(),
        })
    }
}

pub fn require_min(args: &[Value], min: usize) -> Result<(), RuntimeError> {
    if args.len() >= min {
        Ok(())
    } else {
        Err(RuntimeError::InvalidArgumentCount {
            expected: min,
            got: args.len(),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeKind {
    Time,
    LTime,
    Date,
    LDate,
    Tod,
    LTod,
    Dt,
    Ldt,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommonKind {
    Numeric(NumericKind),
    Bit(u32),
    String { wide: bool },
    Time(TimeKind),
    Enum(SmolStr),
}

#[derive(Debug, Clone, Copy)]
pub enum CmpOp {
    Lt,
    Le,
    Gt,
    Ge,
    Eq,
    Ne,
}

pub fn common_kind(values: &[Value]) -> Result<CommonKind, RuntimeError> {
    let mut common: Option<CommonKind> = None;
    for value in values {
        let next = classify_value(value).ok_or(RuntimeError::TypeMismatch)?;
        common = Some(match (common.take(), next) {
            (None, kind) => kind,
            (Some(CommonKind::Numeric(a)), CommonKind::Numeric(b)) => {
                CommonKind::Numeric(wider_numeric(a, b))
            }
            (Some(CommonKind::Bit(a)), CommonKind::Bit(b)) => CommonKind::Bit(a.max(b)),
            (Some(CommonKind::String { wide: a }), CommonKind::String { wide: b }) => {
                if a != b {
                    return Err(RuntimeError::TypeMismatch);
                }
                CommonKind::String { wide: a }
            }
            (Some(CommonKind::Time(a)), CommonKind::Time(b)) => {
                if a != b {
                    return Err(RuntimeError::TypeMismatch);
                }
                CommonKind::Time(a)
            }
            (Some(CommonKind::Enum(a)), CommonKind::Enum(b)) => {
                if a != b {
                    return Err(RuntimeError::TypeMismatch);
                }
                CommonKind::Enum(a)
            }
            _ => return Err(RuntimeError::TypeMismatch),
        });
    }
    common.ok_or(RuntimeError::TypeMismatch)
}

pub fn coerce_to_common(value: &Value, kind: &CommonKind) -> Result<Value, RuntimeError> {
    match kind {
        CommonKind::Numeric(target) => match target {
            NumericKind::Real => Ok(Value::Real(to_f64(value)? as f32)),
            NumericKind::LReal => Ok(Value::LReal(to_f64(value)?)),
            NumericKind::SInt | NumericKind::Int | NumericKind::DInt | NumericKind::LInt => {
                let value = i128::from(to_i64(value)?);
                signed_from_i128(*target, value)
            }
            NumericKind::USInt | NumericKind::UInt | NumericKind::UDInt | NumericKind::ULInt => {
                let value = u128::from(to_u64(value)?);
                unsigned_from_u128(*target, value)
            }
        },
        CommonKind::Bit(width) => {
            let (value, _) = bit_value(value)?;
            let mask = mask_for(*width);
            Ok(bit_value_to_result(value & mask, *width))
        }
        CommonKind::String { wide } => {
            if *wide {
                match value {
                    Value::WString(_) => Ok(value.clone()),
                    Value::WChar(value) => {
                        let ch = std::char::from_u32(u32::from(*value))
                            .ok_or(RuntimeError::TypeMismatch)?;
                        Ok(Value::WString(ch.to_string()))
                    }
                    _ => Err(RuntimeError::TypeMismatch),
                }
            } else {
                match value {
                    Value::String(_) => Ok(value.clone()),
                    Value::Char(value) => {
                        Ok(Value::String((char::from(*value)).to_string().into()))
                    }
                    _ => Err(RuntimeError::TypeMismatch),
                }
            }
        }
        CommonKind::Time(kind) => match (kind, value) {
            (TimeKind::Time, Value::Time(_))
            | (TimeKind::LTime, Value::LTime(_))
            | (TimeKind::Date, Value::Date(_))
            | (TimeKind::LDate, Value::LDate(_))
            | (TimeKind::Tod, Value::Tod(_))
            | (TimeKind::LTod, Value::LTod(_))
            | (TimeKind::Dt, Value::Dt(_))
            | (TimeKind::Ldt, Value::Ldt(_)) => Ok(value.clone()),
            _ => Err(RuntimeError::TypeMismatch),
        },
        CommonKind::Enum(type_name) => match value {
            Value::Enum(enum_value) if enum_value.type_name() == type_name => Ok(value.clone()),
            _ => Err(RuntimeError::TypeMismatch),
        },
    }
}

pub fn compare_common(
    a: &Value,
    b: &Value,
    kind: &CommonKind,
    op: CmpOp,
) -> Result<bool, RuntimeError> {
    match kind {
        CommonKind::Numeric(target) => match target {
            NumericKind::Real | NumericKind::LReal => {
                let left = to_f64(a)?;
                let right = to_f64(b)?;
                Ok(compare_float(left, right, op))
            }
            NumericKind::SInt | NumericKind::Int | NumericKind::DInt | NumericKind::LInt => {
                let left = i128::from(to_i64(a)?);
                let right = i128::from(to_i64(b)?);
                Ok(compare_ord(left, right, op))
            }
            NumericKind::USInt | NumericKind::UInt | NumericKind::UDInt | NumericKind::ULInt => {
                let left = u128::from(to_u64(a)?);
                let right = u128::from(to_u64(b)?);
                Ok(compare_ord(left, right, op))
            }
        },
        CommonKind::Bit(width) => {
            let (left, _) = bit_value(a)?;
            let (right, _) = bit_value(b)?;
            let mask = mask_for(*width);
            Ok(compare_ord(left & mask, right & mask, op))
        }
        CommonKind::String { wide } => {
            if *wide {
                let left = match a {
                    Value::WString(value) => value,
                    _ => return Err(RuntimeError::TypeMismatch),
                };
                let right = match b {
                    Value::WString(value) => value,
                    _ => return Err(RuntimeError::TypeMismatch),
                };
                Ok(compare_ord(left, right, op))
            } else {
                let left = match a {
                    Value::String(value) => value.as_str(),
                    _ => return Err(RuntimeError::TypeMismatch),
                };
                let right = match b {
                    Value::String(value) => value.as_str(),
                    _ => return Err(RuntimeError::TypeMismatch),
                };
                Ok(compare_ord(left, right, op))
            }
        }
        CommonKind::Time(kind) => {
            let left = time_value_as_i128(a, *kind)?;
            let right = time_value_as_i128(b, *kind)?;
            Ok(compare_ord(left, right, op))
        }
        CommonKind::Enum(type_name) => {
            let left = match a {
                Value::Enum(value) if value.type_name() == type_name => value.numeric_value(),
                _ => return Err(RuntimeError::TypeMismatch),
            };
            let right = match b {
                Value::Enum(value) if value.type_name() == type_name => value.numeric_value(),
                _ => return Err(RuntimeError::TypeMismatch),
            };
            Ok(compare_ord(left, right, op))
        }
    }
}

fn classify_value(value: &Value) -> Option<CommonKind> {
    if let Some(kind) = numeric_kind(value) {
        return Some(CommonKind::Numeric(kind));
    }
    if let Ok((_, width)) = bit_value(value) {
        return Some(CommonKind::Bit(width));
    }
    match value {
        Value::String(_) => return Some(CommonKind::String { wide: false }),
        Value::WString(_) => return Some(CommonKind::String { wide: true }),
        Value::Char(_) => return Some(CommonKind::String { wide: false }),
        Value::WChar(_) => return Some(CommonKind::String { wide: true }),
        _ => {}
    }
    match value {
        Value::Time(_) => Some(CommonKind::Time(TimeKind::Time)),
        Value::LTime(_) => Some(CommonKind::Time(TimeKind::LTime)),
        Value::Date(_) => Some(CommonKind::Time(TimeKind::Date)),
        Value::LDate(_) => Some(CommonKind::Time(TimeKind::LDate)),
        Value::Tod(_) => Some(CommonKind::Time(TimeKind::Tod)),
        Value::LTod(_) => Some(CommonKind::Time(TimeKind::LTod)),
        Value::Dt(_) => Some(CommonKind::Time(TimeKind::Dt)),
        Value::Ldt(_) => Some(CommonKind::Time(TimeKind::Ldt)),
        Value::Enum(value) => Some(CommonKind::Enum(value.type_name().clone())),
        _ => None,
    }
}

fn compare_ord<T: Ord>(left: T, right: T, op: CmpOp) -> bool {
    match op {
        CmpOp::Lt => left < right,
        CmpOp::Le => left <= right,
        CmpOp::Gt => left > right,
        CmpOp::Ge => left >= right,
        CmpOp::Eq => left == right,
        CmpOp::Ne => left != right,
    }
}

fn compare_float(left: f64, right: f64, op: CmpOp) -> bool {
    match op {
        CmpOp::Lt => left < right,
        CmpOp::Le => left <= right,
        CmpOp::Gt => left > right,
        CmpOp::Ge => left >= right,
        CmpOp::Eq => left == right,
        CmpOp::Ne => left != right,
    }
}

fn time_value_as_i128(value: &Value, kind: TimeKind) -> Result<i128, RuntimeError> {
    match (kind, value) {
        (TimeKind::Time, Value::Time(duration)) => Ok(duration.as_nanos() as i128),
        (TimeKind::LTime, Value::LTime(duration)) => Ok(duration.as_nanos() as i128),
        (TimeKind::Date, Value::Date(date)) => Ok(i128::from(date.ticks())),
        (TimeKind::LDate, Value::LDate(date)) => Ok(i128::from(date.nanos())),
        (TimeKind::Tod, Value::Tod(tod)) => Ok(i128::from(tod.ticks())),
        (TimeKind::LTod, Value::LTod(tod)) => Ok(i128::from(tod.nanos())),
        (TimeKind::Dt, Value::Dt(dt)) => Ok(i128::from(dt.ticks())),
        (TimeKind::Ldt, Value::Ldt(dt)) => Ok(i128::from(dt.nanos())),
        _ => Err(RuntimeError::TypeMismatch),
    }
}

pub fn bit_value(value: &Value) -> Result<(u64, u32), RuntimeError> {
    match value {
        Value::Bool(v) => Ok((if *v { 1 } else { 0 }, 1)),
        Value::Byte(v) => Ok((*v as u64, 8)),
        Value::Word(v) => Ok((*v as u64, 16)),
        Value::DWord(v) => Ok((*v as u64, 32)),
        Value::LWord(v) => Ok((*v, 64)),
        _ => Err(RuntimeError::TypeMismatch),
    }
}

pub fn bit_value_to_result(value: u64, width: u32) -> Value {
    match width {
        1 => Value::Bool(value & 0x1 == 1),
        8 => Value::Byte(value as u8),
        16 => Value::Word(value as u16),
        32 => Value::DWord(value as u32),
        64 => Value::LWord(value),
        _ => Value::LWord(value),
    }
}

pub fn mask_for(width: u32) -> u64 {
    if width >= 64 {
        u64::MAX
    } else {
        (1u64 << width) - 1
    }
}

pub fn scale_time(
    duration: Duration,
    factor: &Value,
    multiply: bool,
) -> Result<Duration, RuntimeError> {
    let factor = to_f64(factor)?;
    if !factor.is_finite() {
        return Err(RuntimeError::Overflow);
    }
    if !multiply && factor == 0.0 {
        return Err(RuntimeError::DivisionByZero);
    }
    let nanos = duration.as_nanos() as f64;
    let result = if multiply {
        nanos * factor
    } else {
        nanos / factor
    };
    let result = round_ties_to_even(result);
    if !result.is_finite() {
        return Err(RuntimeError::Overflow);
    }
    let nanos = i64::try_from(result as i128).map_err(|_| RuntimeError::Overflow)?;
    Ok(Duration::from_nanos(nanos))
}

pub fn round_ties_to_even(value: f64) -> f64 {
    let truncated = value.trunc();
    let frac = value - truncated;
    if frac.abs() == 0.5 {
        let is_even = truncated.rem_euclid(2.0) == 0.0;
        if is_even {
            truncated
        } else {
            truncated + frac.signum()
        }
    } else {
        value.round()
    }
}
