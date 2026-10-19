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

/// What a leaf's bits are checked as: a value of the kind of a template, or an enumeration, which
/// needs no template value (C1-15).
#[derive(Clone, Copy)]
pub(super) enum Kind<'a> {
    Value(&'a Value),
    Enumeration,
}

impl<'a> Kind<'a> {
    /// The kind of `template`.
    fn of(template: &'a Value) -> Self {
        match template {
            Value::Enum(_) => Self::Enumeration,
            other => Self::Value(other),
        }
    }
}

pub(super) fn validate(template: &Value, bits: u64, leaf: &Leaf) -> Result<(), RuntimeError> {
    validate_kind(Kind::of(template), bits, leaf)
}

/// [`validate`] of the bits `bits` of `leaf` as the leaf's own kind, without a template value: the
/// check of an alias of a write made in place in a backing (C1-15).
pub(super) fn validate_leaf(bits: u64, leaf: &Leaf) -> Result<(), RuntimeError> {
    match leaf.enumeration {
        Some(_) => validate_kind(Kind::Enumeration, bits, leaf),
        None => {
            let template = zero(leaf.primitive_id).ok_or(RuntimeError::TypeMismatch)?;
            validate_kind(Kind::Value(&template), bits, leaf)
        }
    }
}

fn validate_kind(kind: Kind<'_>, bits: u64, leaf: &Leaf) -> Result<(), RuntimeError> {
    let template_primitive = match kind {
        Kind::Value(template) => primitive(template),
        Kind::Enumeration => None,
    };
    if leaf.enumeration.is_none() && template_primitive != Some(leaf.primitive_id) {
        return Err(RuntimeError::TypeMismatch);
    }
    if let Some((lower, upper)) = leaf.range {
        let numeric = match kind {
            Kind::Value(Value::SInt(_)) => i64::from(bits as i8),
            Kind::Value(Value::Int(_)) => i64::from(bits as i16),
            Kind::Value(Value::DInt(_)) => i64::from(bits as i32),
            Kind::Value(Value::LInt(_)) => bits as i64,
            Kind::Value(Value::USInt(_)) => i64::from(bits as u8),
            Kind::Value(Value::UInt(_)) => i64::from(bits as u16),
            Kind::Value(Value::UDInt(_)) => i64::from(bits as u32),
            Kind::Value(Value::ULInt(_)) => match i64::try_from(bits) {
                Ok(value) => value,
                Err(_) => {
                    return Err(RuntimeError::SubrangeViolation {
                        value: i128::from(bits),
                        lower: i128::from(lower),
                        upper: i128::from(upper),
                    })
                }
            },
            Kind::Enumeration => match &leaf.enumeration {
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
    match (&leaf.enumeration, kind) {
        (Some(enumeration), Kind::Enumeration) => {
            if !enumeration
                .variants
                .iter()
                .any(|(_, value)| *value == enum_number(bits, leaf, enumeration))
            {
                return Err(RuntimeError::TypeMismatch);
            }
        }
        (Some(_), Kind::Value(_)) => return Err(RuntimeError::TypeMismatch),
        (None, Kind::Value(template)) if self::bits(template).is_none() => {
            return Err(RuntimeError::TypeMismatch);
        }
        (None, _) => {}
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
    checked_kind_bits(value, Kind::of(template), leaf, abi)
}

/// [`checked_bits`] of `value` written into `leaf` as the leaf's own kind, without a template value:
/// an enumeration of the type named `enum_type`, or a value of the leaf's primitive (C1-15).
pub(super) fn checked_leaf_bits(
    value: &Value,
    leaf: &Leaf,
    abi: crate::value::StorageAbi,
    enum_type: &str,
) -> Result<u64, RuntimeError> {
    match (&leaf.enumeration, value) {
        (Some(_), Value::Enum(enumeration)) if enumeration.type_name().as_str() == enum_type => {
            checked_kind_bits(value, Kind::Enumeration, leaf, abi)
        }
        (Some(_), _) => Err(RuntimeError::TypeMismatch),
        (None, _) => {
            let template = zero(leaf.primitive_id).ok_or(RuntimeError::TypeMismatch)?;
            if core::mem::discriminant(value) != core::mem::discriminant(&template) {
                return Err(RuntimeError::TypeMismatch);
            }
            checked_kind_bits(value, Kind::Value(&template), leaf, abi)
        }
    }
}

fn checked_kind_bits(
    value: &Value,
    kind: Kind<'_>,
    leaf: &Leaf,
    abi: crate::value::StorageAbi,
) -> Result<u64, RuntimeError> {
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
    validate_kind(kind, bits, leaf)?;
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
        Value::Enum(value) => return enum_value(value.type_name().clone(), bits, leaf),
        _ => return None,
    })
}

/// The value of the enumeration leaf `leaf` whose bits are `bits`, of the enumeration type named
/// `type_name`, with no template value: `None` when the bits name no variant.
pub(super) fn enum_value(type_name: SmolStr, bits: u64, leaf: &Leaf) -> Option<Value> {
    let enumeration = leaf.enumeration.as_ref()?;
    let numeric = enum_number(bits, leaf, enumeration);
    let (name, _) = enumeration.variants.iter().find(|(_, n)| *n == numeric)?;
    Some(Value::Enum(Box::new(EnumValue::from_canonical_parts(
        type_name,
        name.clone(),
        numeric,
    ))))
}

fn temporal_number(bits: u64, leaf: &Leaf) -> i64 {
    if leaf.width == 32 {
        i64::from(bits as i32)
    } else {
        bits as i64
    }
}

/// A value of the primitive `id`, whose kind tells [`value`] how to read a leaf's bits.
pub(super) fn zero(id: u16) -> Option<Value> {
    Some(match id {
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
        16 => Value::Time(Duration::from_nanos(0)),
        17 => Value::LTime(Duration::from_nanos(0)),
        18 => Value::Date(DateValue::new(0)),
        19 => Value::LDate(LDateValue::new(0)),
        20 => Value::Tod(TimeOfDayValue::new(0)),
        21 => Value::LTod(LTimeOfDayValue::new(0)),
        22 => Value::Dt(DateTimeValue::new(0)),
        23 => Value::Ldt(LDateTimeValue::new(0)),
        26 => Value::Char(0),
        27 => Value::WChar(0),
        _ => return None,
    })
}
