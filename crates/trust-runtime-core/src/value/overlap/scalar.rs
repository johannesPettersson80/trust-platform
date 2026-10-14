//! Bit-preserving elementary codecs for admitted physical structure leaves.
use super::*;
use crate::value::{
    DateTimeValue, DateValue, Duration, EnumValue, LDateTimeValue, LDateValue, LTimeOfDayValue,
    TimeOfDayValue,
};
use alloc::boxed::Box;

pub(super) fn width(id: u16, abi: crate::value::StorageAbi) -> Option<u8> {
    Some(match id {
        1 => 1,
        2 | 6 | 10 | 26 => 8,
        3 | 7 | 11 | 27 => 16,
        4 | 8 | 12 | 14 => 32,
        16 | 18 | 20 | 22 => (abi.short_temporal_bytes() * 8) as u8,
        5 | 9 | 13 | 15 | 17 | 19 | 21 | 23 => 64,
        _ => return None,
    })
}

pub(super) fn bits(value: &Value) -> Option<u64> {
    Some(match value {
        Value::Bool(v) => u64::from(*v),
        Value::SInt(v) => *v as u64,
        Value::Int(v) => *v as u64,
        Value::DInt(v) => *v as u64,
        Value::LInt(v) => *v as u64,
        Value::USInt(v) | Value::Byte(v) | Value::Char(v) => u64::from(*v),
        Value::UInt(v) | Value::Word(v) | Value::WChar(v) => u64::from(*v),
        Value::UDInt(v) | Value::DWord(v) => u64::from(*v),
        Value::ULInt(v) | Value::LWord(v) => *v,
        Value::Real(v) => u64::from(v.to_bits()),
        Value::LReal(v) => v.to_bits(),
        Value::Time(v) | Value::LTime(v) => v.as_nanos() as u64,
        Value::Date(v) => v.ticks() as u64,
        Value::LDate(v) => v.nanos() as u64,
        Value::Tod(v) => v.ticks() as u64,
        Value::LTod(v) => v.nanos() as u64,
        Value::Dt(v) => v.ticks() as u64,
        Value::Ldt(v) => v.nanos() as u64,
        Value::Enum(v) => v.numeric_value() as u64,
        _ => return None,
    })
}

fn enum_number(bits: u64, leaf: &Leaf, enumeration: &Enumeration) -> i64 {
    if enumeration.signed {
        ((bits << (64 - leaf.width)) as i64) >> (64 - leaf.width)
    } else {
        bits as i64
    }
}

pub(super) fn validate(template: &Value, bits: u64, leaf: &Leaf) -> Result<(), RuntimeError> {
    if leaf.enumeration.is_none() && primitive(template) != Some(leaf.primitive_id) {
        return Err(RuntimeError::TypeMismatch);
    }
    if let Some((lower, upper)) = leaf.range {
        let numeric = match template {
            Value::SInt(_) => i64::from(bits as i8),
            Value::Int(_) => i64::from(bits as i16),
            Value::DInt(_) => i64::from(bits as i32),
            Value::LInt(_) => bits as i64,
            Value::USInt(_) => i64::from(bits as u8),
            Value::UInt(_) => i64::from(bits as u16),
            Value::UDInt(_) => i64::from(bits as u32),
            Value::ULInt(_) => match i64::try_from(bits) {
                Ok(value) => value,
                Err(_) => {
                    return Err(RuntimeError::SubrangeViolation {
                        value: i128::from(bits),
                        lower: i128::from(lower),
                        upper: i128::from(upper),
                    })
                }
            },
            Value::Enum(_) => match &leaf.enumeration {
                Some(enumeration) if enumeration.signed => enum_number(bits, leaf, enumeration),
                Some(_) => match i64::try_from(bits) {
                    Ok(value) => value,
                    Err(_) => {
                        return Err(RuntimeError::SubrangeViolation {
                            value: i128::from(bits),
                            lower: i128::from(lower),
                            upper: i128::from(upper),
                        })
                    }
                },
                None => return Err(RuntimeError::TypeMismatch),
            },
            _ => return Err(RuntimeError::TypeMismatch),
        };
        if numeric < lower || numeric > upper {
            return Err(RuntimeError::SubrangeViolation {
                value: i128::from(numeric),
                lower: i128::from(lower),
                upper: i128::from(upper),
            });
        }
    }
    if let Some(enumeration) = &leaf.enumeration {
        if !matches!(template, Value::Enum(_))
            || !enumeration
                .variants
                .iter()
                .any(|(_, value)| *value == enum_number(bits, leaf, enumeration))
        {
            return Err(RuntimeError::TypeMismatch);
        }
    } else if self::bits(template).is_none() {
        return Err(RuntimeError::TypeMismatch);
    }
    Ok(())
}

/// Legacy narrowing is a checked physical boundary, never a wrapping logical conversion.
pub(super) fn checked_bits(
    value: &Value,
    template: &Value,
    leaf: &Leaf,
    abi: crate::value::StorageAbi,
) -> Result<u64, RuntimeError> {
    if core::mem::discriminant(value) != core::mem::discriminant(template) {
        return Err(RuntimeError::TypeMismatch);
    }
    if let (Value::Enum(value), Value::Enum(template)) = (value, template)
        && value.type_name() != template.type_name()
    {
        return Err(RuntimeError::TypeMismatch);
    }
    let temporal = match value {
        Value::Time(v) => Some(v.as_nanos()),
        Value::Date(v) => Some(v.ticks()),
        Value::Tod(v) => Some(v.ticks()),
        Value::Dt(v) => Some(v.ticks()),
        _ => None,
    };
    if abi == crate::value::StorageAbi::LegacyTemporal32
        && temporal.is_some_and(|n| i32::try_from(n).is_err())
    {
        return Err(RuntimeError::Overflow);
    }
    let bits = self::bits(value).ok_or(RuntimeError::TypeMismatch)?;
    validate(template, bits, leaf)?;
    if let (Value::Enum(value), Some(enumeration)) = (value, &leaf.enumeration)
        && !enumeration.variants.iter().any(|(name, ordinal)| {
            *ordinal == value.numeric_value() && name.eq_ignore_ascii_case(value.variant_name())
        })
    {
        return Err(RuntimeError::TypeMismatch);
    }
    Ok(bits)
}

fn primitive(value: &Value) -> Option<u16> {
    Some(match value {
        Value::Bool(_) => 1,
        Value::Byte(_) => 2,
        Value::Word(_) => 3,
        Value::DWord(_) => 4,
        Value::LWord(_) => 5,
        Value::SInt(_) => 6,
        Value::Int(_) => 7,
        Value::DInt(_) => 8,
        Value::LInt(_) => 9,
        Value::USInt(_) => 10,
        Value::UInt(_) => 11,
        Value::UDInt(_) => 12,
        Value::ULInt(_) => 13,
        Value::Real(_) => 14,
        Value::LReal(_) => 15,
        Value::Time(_) => 16,
        Value::LTime(_) => 17,
        Value::Date(_) => 18,
        Value::LDate(_) => 19,
        Value::Tod(_) => 20,
        Value::LTod(_) => 21,
        Value::Dt(_) => 22,
        Value::Ldt(_) => 23,
        Value::Char(_) => 26,
        Value::WChar(_) => 27,
        _ => return None,
    })
}

pub(super) fn value(template: &Value, bits: u64, leaf: &Leaf) -> Option<Value> {
    Some(match template {
        Value::Bool(_) => Value::Bool(bits & 1 != 0),
        Value::SInt(_) => Value::SInt(bits as i8),
        Value::Int(_) => Value::Int(bits as i16),
        Value::DInt(_) => Value::DInt(bits as i32),
        Value::LInt(_) => Value::LInt(bits as i64),
        Value::USInt(_) => Value::USInt(bits as u8),
        Value::UInt(_) => Value::UInt(bits as u16),
        Value::UDInt(_) => Value::UDInt(bits as u32),
        Value::ULInt(_) => Value::ULInt(bits),
        Value::Byte(_) => Value::Byte(bits as u8),
        Value::Word(_) => Value::Word(bits as u16),
        Value::DWord(_) => Value::DWord(bits as u32),
        Value::LWord(_) => Value::LWord(bits),
        Value::Char(_) => Value::Char(bits as u8),
        Value::WChar(_) => Value::WChar(bits as u16),
        Value::Real(_) => Value::Real(f32::from_bits(bits as u32)),
        Value::LReal(_) => Value::LReal(f64::from_bits(bits)),
        Value::Time(_) => Value::Time(Duration::from_nanos(temporal_number(bits, leaf))),
        Value::LTime(_) => Value::LTime(Duration::from_nanos(bits as i64)),
        Value::Date(_) => Value::Date(DateValue::new(temporal_number(bits, leaf))),
        Value::LDate(_) => Value::LDate(LDateValue::new(bits as i64)),
        Value::Tod(_) => Value::Tod(TimeOfDayValue::new(temporal_number(bits, leaf))),
        Value::LTod(_) => Value::LTod(LTimeOfDayValue::new(bits as i64)),
        Value::Dt(_) => Value::Dt(DateTimeValue::new(temporal_number(bits, leaf))),
        Value::Ldt(_) => Value::Ldt(LDateTimeValue::new(bits as i64)),
        Value::Enum(value) => {
            let enumeration = leaf.enumeration.as_ref()?;
            let numeric = enum_number(bits, leaf, enumeration);
            let (name, _) = enumeration.variants.iter().find(|(_, n)| *n == numeric)?;
            Value::Enum(Box::new(EnumValue::from_canonical_parts(
                value.type_name().clone(),
                name.clone(),
                numeric,
            )))
        }
        _ => return None,
    })
}

fn temporal_number(bits: u64, leaf: &Leaf) -> i64 {
    if leaf.width == 32 {
        i64::from(bits as i32)
    } else {
        bits as i64
    }
}
