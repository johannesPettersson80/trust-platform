//! Helpers for standard function implementations.

use crate::error::RuntimeError;
pub use crate::numeric::{
    numeric_kind, signed_from_i128, to_f64, to_i64, to_u64, unsigned_from_u128, wider_numeric,
    NumericKind,
};
use crate::value::{Duration, Value};
use alloc::string::ToString;
use smol_str::SmolStr;

/// Require exactly the declared number of arguments.
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

/// Require at least the declared number of arguments.
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

/// Date/time category used for common-type coercion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeKind {
    /// TIME duration.
    Time,
    /// LTIME duration.
    LTime,
    /// DATE calendar value.
    Date,
    /// LDATE calendar value.
    LDate,
    /// TIME_OF_DAY value.
    Tod,
    /// LTIME_OF_DAY value.
    LTod,
    /// DATE_AND_TIME value.
    Dt,
    /// LDATE_AND_TIME value.
    Ldt,
}

/// Common scalar category selected for standard-function operands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommonKind {
    /// Common numeric width and signedness.
    Numeric(NumericKind),
    /// Common bit-string width.
    Bit(u32),
    /// Narrow or wide string category.
    String {
        /// True for wide IEC strings.
        wide: bool,
    },
    /// Common duration or calendar category.
    Time(TimeKind),
    /// Enumeration category identified by its type name.
    Enum(SmolStr),
}

/// Comparison applied after common-type coercion.
#[derive(Debug, Clone, Copy)]
pub enum CmpOp {
    /// Less than.
    Lt,
    /// Less than or equal.
    Le,
    /// Greater than.
    Gt,
    /// Greater than or equal.
    Ge,
    /// Equal.
    Eq,
    /// Not equal.
    Ne,
}

/// Select one compatible category across all supplied operands.
pub fn common_kind(values: &[Value]) -> Result<CommonKind, RuntimeError> {
    let mut common: Option<CommonKind> = None;
    for value in values {
        let next = classify_value(value).ok_or(RuntimeError::TypeMismatch)?;
        common = Some(match (common.take(), next) {
            (None, kind) => kind,
            (Some(CommonKind::Numeric(a)), CommonKind::Numeric(b)) => {
                CommonKind::Numeric(wider_numeric(a, b).ok_or(RuntimeError::TypeMismatch)?)
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

/// Convert an operand to the previously selected common category.
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
                        let ch = core::char::from_u32(u32::from(*value))
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

/// Compare common-category operands using the requested relation.
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
                let left = to_i64(a)?;
                let right = to_i64(b)?;
                Ok(compare_ord(left, right, op))
            }
            NumericKind::USInt | NumericKind::UInt | NumericKind::UDInt | NumericKind::ULInt => {
                let left = to_u64(a)?;
                let right = to_u64(b)?;
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
            let left = time_value_as_i64(a, *kind)?;
            let right = time_value_as_i64(b, *kind)?;
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

fn time_value_as_i64(value: &Value, kind: TimeKind) -> Result<i64, RuntimeError> {
    match (kind, value) {
        (TimeKind::Time, Value::Time(duration)) => Ok(duration.as_nanos()),
        (TimeKind::LTime, Value::LTime(duration)) => Ok(duration.as_nanos()),
        (TimeKind::Date, Value::Date(date)) => Ok(date.ticks()),
        (TimeKind::LDate, Value::LDate(date)) => Ok(date.nanos()),
        (TimeKind::Tod, Value::Tod(tod)) => Ok(tod.ticks()),
        (TimeKind::LTod, Value::LTod(tod)) => Ok(tod.nanos()),
        (TimeKind::Dt, Value::Dt(dt)) => Ok(dt.ticks()),
        (TimeKind::Ldt, Value::Ldt(dt)) => Ok(dt.nanos()),
        _ => Err(RuntimeError::TypeMismatch),
    }
}

/// Return a bit-string value and its IEC width.
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

/// Construct the IEC bit-string value matching a width.
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

/// Return the low-bit mask for an IEC bit-string width.
pub fn mask_for(width: u32) -> u64 {
    if width >= 64 {
        u64::MAX
    } else {
        (1u64 << width) - 1
    }
}

/// Scale a duration using shared numeric conversion and truncation policy.
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
    // f64 rounds i64::MAX to 2^63, so the upper bound must be exclusive.
    if !(-9_223_372_036_854_775_808.0..9_223_372_036_854_775_808.0).contains(&result) {
        return Err(RuntimeError::Overflow);
    }
    Ok(Duration::from_nanos(result as i64))
}

/// Round a real value to the nearest integer, resolving ties to even.
pub fn round_ties_to_even(value: f64) -> f64 {
    let truncated = crate::numeric::math::trunc(value);
    let frac = value - truncated;
    if frac.abs() == 0.5 {
        let is_even = truncated % 2.0 == 0.0;
        if is_even {
            truncated
        } else {
            truncated + frac.signum()
        }
    } else {
        crate::numeric::math::round(value)
    }
}

#[cfg(test)]
mod width_tests {
    use super::*;
    use crate::value::{
        DateTimeValue, DateValue, LDateTimeValue, LDateValue, LTimeOfDayValue, TimeOfDayValue,
    };

    fn wide_scale_time(
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
        let result = round_ties_to_even(if multiply {
            nanos * factor
        } else {
            nanos / factor
        });
        if !result.is_finite() {
            return Err(RuntimeError::Overflow);
        }
        i64::try_from(result as i128)
            .map(Duration::from_nanos)
            .map_err(|_| RuntimeError::Overflow)
    }

    #[test]
    fn time_function_rounding_matches_wide_cast_bounds_and_error_precedence() {
        let upper = 9_223_372_036_854_775_808.0f64;
        let factors = [
            Value::LInt(i64::MIN),
            Value::ULInt(u64::MAX),
            Value::Bool(false),
            Value::Real(0.5),
            Value::LReal(f64::NEG_INFINITY),
            Value::LReal(f64::NAN),
            Value::LReal(f64::INFINITY),
            Value::LReal(-0.0),
            Value::LReal(0.0),
            Value::LReal(-0.5),
            Value::LReal(0.5),
            Value::LReal(1.0),
            Value::LReal(2.0),
            Value::LReal(upper.next_down()),
            Value::LReal(upper),
            Value::LReal(upper.next_up()),
            Value::LReal((-upper).next_down()),
            Value::LReal(-upper),
            Value::LReal((-upper).next_up()),
        ];
        for nanos in [
            i64::MIN,
            i64::MIN + 1,
            -5,
            -3,
            -1,
            0,
            1,
            3,
            5,
            i64::MAX - 1,
            i64::MAX,
        ] {
            for factor in &factors {
                for multiply in [false, true] {
                    let duration = Duration::from_nanos(nanos);
                    assert_eq!(
                        scale_time(duration, factor, multiply),
                        wide_scale_time(duration, factor, multiply),
                        "nanos={nanos}, factor={factor:?}, multiply={multiply}"
                    );
                }
            }
        }
    }

    #[test]
    fn narrow_integer_and_time_comparisons_keep_all_relations_at_extremes() {
        let operations = [
            CmpOp::Lt,
            CmpOp::Le,
            CmpOp::Gt,
            CmpOp::Ge,
            CmpOp::Eq,
            CmpOp::Ne,
        ];
        for left in [i64::MIN, -1, 0, 1, i64::MAX] {
            for right in [i64::MIN, -1, 0, 1, i64::MAX] {
                for op in operations {
                    let expected = compare_ord(i128::from(left), i128::from(right), op);
                    for kind in [
                        NumericKind::SInt,
                        NumericKind::Int,
                        NumericKind::DInt,
                        NumericKind::LInt,
                    ] {
                        assert_eq!(
                            compare_common(
                                &Value::LInt(left),
                                &Value::LInt(right),
                                &CommonKind::Numeric(kind),
                                op
                            ),
                            Ok(expected)
                        );
                    }
                    let pairs = [
                        (
                            TimeKind::Time,
                            Value::Time(Duration::from_nanos(left)),
                            Value::Time(Duration::from_nanos(right)),
                        ),
                        (
                            TimeKind::LTime,
                            Value::LTime(Duration::from_nanos(left)),
                            Value::LTime(Duration::from_nanos(right)),
                        ),
                        (
                            TimeKind::Date,
                            Value::Date(DateValue::new(left)),
                            Value::Date(DateValue::new(right)),
                        ),
                        (
                            TimeKind::LDate,
                            Value::LDate(LDateValue::new(left)),
                            Value::LDate(LDateValue::new(right)),
                        ),
                        (
                            TimeKind::Tod,
                            Value::Tod(TimeOfDayValue::new(left)),
                            Value::Tod(TimeOfDayValue::new(right)),
                        ),
                        (
                            TimeKind::LTod,
                            Value::LTod(LTimeOfDayValue::new(left)),
                            Value::LTod(LTimeOfDayValue::new(right)),
                        ),
                        (
                            TimeKind::Dt,
                            Value::Dt(DateTimeValue::new(left)),
                            Value::Dt(DateTimeValue::new(right)),
                        ),
                        (
                            TimeKind::Ldt,
                            Value::Ldt(LDateTimeValue::new(left)),
                            Value::Ldt(LDateTimeValue::new(right)),
                        ),
                    ];
                    for (kind, a, b) in pairs {
                        assert_eq!(
                            compare_common(&a, &b, &CommonKind::Time(kind), op),
                            Ok(expected)
                        );
                        assert_eq!(
                            compare_common(&a, &Value::Bool(false), &CommonKind::Time(kind), op),
                            Err(RuntimeError::TypeMismatch)
                        );
                    }
                }
            }
        }
        for left in [0, 1, 1u64 << 63, u64::MAX] {
            for right in [0, 1, 1u64 << 63, u64::MAX] {
                for op in operations {
                    for kind in [
                        NumericKind::USInt,
                        NumericKind::UInt,
                        NumericKind::UDInt,
                        NumericKind::ULInt,
                    ] {
                        assert_eq!(
                            compare_common(
                                &Value::ULInt(left),
                                &Value::ULInt(right),
                                &CommonKind::Numeric(kind),
                                op
                            ),
                            Ok(compare_ord(u128::from(left), u128::from(right), op))
                        );
                    }
                }
            }
        }
    }
}
