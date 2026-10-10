//! Scalar leaves of typed initialization, preserving hosted initializer coercion.
use crate::error::RuntimeError;
use crate::value::{
    truncate_string_elements, DateTimeProfile, DateTimeValue, DateValue, Duration, LDateTimeValue,
    LDateValue, LTimeOfDayValue, TimeOfDayValue, Value,
};
use alloc::string::{String, ToString};

pub(super) fn default(primitive: u16, profile: DateTimeProfile) -> Result<Value, RuntimeError> {
    Ok(match primitive {
        1 => Value::Bool(false),
        2 => Value::Byte(0),
        3 => Value::Word(0),
        4 => Value::DWord(0),
        5 => Value::LWord(0),
        6 => Value::SInt(0),
        7 => Value::Int(0),
        8 => Value::DInt(0),
        9 => Value::LInt(0),
        10 => Value::USInt(0),
        11 => Value::UInt(0),
        12 => Value::UDInt(0),
        13 => Value::ULInt(0),
        14 => Value::Real(0.0),
        15 => Value::LReal(0.0),
        16 => Value::Time(Duration::ZERO),
        17 => Value::LTime(Duration::ZERO),
        18 => Value::Date(DateValue::new(profile.epoch.ticks())),
        19 => Value::LDate(LDateValue::new(0)),
        20 => Value::Tod(TimeOfDayValue::new(0)),
        21 => Value::LTod(LTimeOfDayValue::new(0)),
        22 => Value::Dt(DateTimeValue::new(profile.epoch.ticks())),
        23 => Value::Ldt(LDateTimeValue::new(0)),
        24 => Value::String("".into()),
        25 => Value::WString(String::new()),
        26 => Value::Char(0),
        27 => Value::WChar(0),
        0x0100 => Value::Null,
        _ => return Err(RuntimeError::TypeMismatch),
    })
}

pub(super) fn integer(value: &Value) -> Option<i128> {
    Some(match value {
        Value::SInt(v) => i128::from(*v),
        Value::Int(v) => i128::from(*v),
        Value::DInt(v) => i128::from(*v),
        Value::LInt(v) => i128::from(*v),
        Value::USInt(v) => i128::from(*v),
        Value::UInt(v) => i128::from(*v),
        Value::UDInt(v) => i128::from(*v),
        Value::ULInt(v) => i128::from(*v),
        _ => return None,
    })
}

pub(in crate::vm) fn coerce(
    primitive: u16,
    max_length: u16,
    value: Value,
) -> Result<Value, RuntimeError> {
    macro_rules! narrow {
        ($number:expr,$ty:ty,$variant:ident) => {
            <$ty>::try_from($number)
                .map(Value::$variant)
                .map_err(|_| RuntimeError::TypeMismatch)
        };
    }
    match primitive {
        1 => match value {
            Value::Bool(_) => Ok(value),
            _ => Err(RuntimeError::TypeMismatch),
        },
        2..=5 => {
            let number = match &value {
                Value::Byte(v) => i128::from(*v),
                Value::Word(v) => i128::from(*v),
                Value::DWord(v) => i128::from(*v),
                Value::LWord(v) => i128::from(*v),
                _ => integer(&value).ok_or(RuntimeError::TypeMismatch)?,
            };
            match primitive {
                2 => narrow!(number, u8, Byte),
                3 => narrow!(number, u16, Word),
                4 => narrow!(number, u32, DWord),
                _ => narrow!(number, u64, LWord),
            }
        }
        6..=13 => {
            let number = integer(&value).ok_or(RuntimeError::TypeMismatch)?;
            match primitive {
                6 => narrow!(number, i8, SInt),
                7 => narrow!(number, i16, Int),
                8 => narrow!(number, i32, DInt),
                9 => narrow!(number, i64, LInt),
                10 => narrow!(number, u8, USInt),
                11 => narrow!(number, u16, UInt),
                12 => narrow!(number, u32, UDInt),
                _ => narrow!(number, u64, ULInt),
            }
        }
        14 | 15 => {
            let number = match value {
                Value::Real(v) => f64::from(v),
                Value::LReal(v) => v,
                _ => integer(&value).ok_or(RuntimeError::TypeMismatch)? as f64,
            };
            if !number.is_finite() {
                return Err(RuntimeError::TypeMismatch);
            }
            if primitive == 14 {
                let value = number as f32;
                if !value.is_finite() {
                    return Err(RuntimeError::TypeMismatch);
                }
                Ok(Value::Real(value))
            } else {
                Ok(Value::LReal(number))
            }
        }
        16 => match value {
            Value::Time(_) => Ok(value),
            Value::LTime(v) => Ok(Value::Time(v)),
            _ => Err(RuntimeError::TypeMismatch),
        },
        17 => match value {
            Value::LTime(_) => Ok(value),
            Value::Time(v) => Ok(Value::LTime(v)),
            _ => Err(RuntimeError::TypeMismatch),
        },
        18..=23 => match (primitive, &value) {
            (18, Value::Date(_))
            | (19, Value::LDate(_))
            | (20, Value::Tod(_))
            | (21, Value::LTod(_))
            | (22, Value::Dt(_))
            | (23, Value::Ldt(_)) => Ok(value),
            _ => Err(RuntimeError::TypeMismatch),
        },
        24 | 25 => {
            let value = match (primitive, value) {
                (24, v @ Value::String(_)) | (25, v @ Value::WString(_)) => v,
                (24, Value::WString(v)) => Value::String(v.into()),
                (24, Value::Char(v)) => Value::String(char::from(v).to_string().into()),
                (25, Value::String(v)) => Value::WString(v.to_string()),
                (25, Value::Char(v)) => Value::WString(char::from(v).to_string()),
                (25, Value::WChar(v)) => Value::WString(
                    char::from_u32(u32::from(v))
                        .unwrap_or('\u{FFFD}')
                        .to_string(),
                ),
                _ => return Err(RuntimeError::TypeMismatch),
            };
            if max_length == 0 {
                return Ok(value);
            }
            Ok(match value {
                Value::String(s) => {
                    Value::String(truncate_string_elements(&s, u32::from(max_length)).into())
                }
                Value::WString(s) => {
                    Value::WString(truncate_string_elements(&s, u32::from(max_length)))
                }
                _ => return Err(RuntimeError::TypeMismatch),
            })
        }
        26 | 27 => {
            let text = match value {
                v @ Value::Char(_) if primitive == 26 => return Ok(v),
                v @ Value::WChar(_) if primitive == 27 => return Ok(v),
                Value::String(s) => s.to_string(),
                Value::WString(s) => s,
                _ => return Err(RuntimeError::TypeMismatch),
            };
            let mut chars = text.chars();
            let ch = chars.next().ok_or(RuntimeError::TypeMismatch)?;
            if chars.next().is_some() {
                return Err(RuntimeError::TypeMismatch);
            }
            if primitive == 26 {
                if !ch.is_ascii() {
                    return Err(RuntimeError::TypeMismatch);
                }
                Ok(Value::Char(ch as u8))
            } else {
                narrow!(u32::from(ch), u16, WChar)
            }
        }
        0x0100 if matches!(value, Value::Null) || integer(&value).is_some() => Ok(value),
        _ => Err(RuntimeError::TypeMismatch),
    }
}
