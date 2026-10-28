//! The value an HMI command writes (specification 39, §12.2.4, §17.3): the requested value is
//! converted to the type of the point it is written to, and refused with the reason when it does
//! not fit: the wrong kind of value, outside the type's capacity (an integer's width, a REAL's
//! range), or not a finite number. The point's current value gives its type.

use serde_json::Value as Json;
use trust_hir::types::TypeRegistry;
use trust_hir::Type;

use crate::value::{EnumValue, Value};

#[cfg(test)]
mod tests;

/// The value to write to a point that holds `current`, with the program's types for an
/// enumeration: one of its value names, ignoring case, or the number of one of its values
/// (§5.8).
pub fn value_for_target_in(
    registry: &TypeRegistry,
    current: &Value,
    requested: &Json,
) -> Result<Value, String> {
    value_for_target_with(current, requested, |type_name| {
        registry
            .lookup(type_name)
            .and_then(|id| enumeration_values(registry, id))
            .map(<[_]>::to_vec)
    })
}

/// The value to write to a point that holds `current`, an enumeration's values, by their names
/// and numbers, given by `values_of` for the enumeration's name: one of its value names, ignoring
/// case, or the number of one of its values (§5.8).
pub fn value_for_target_with(
    current: &Value,
    requested: &Json,
    values_of: impl FnOnce(&str) -> Option<Vec<(smol_str::SmolStr, i64)>>,
) -> Result<Value, String> {
    let Value::Enum(current) = current else {
        return value_for_target(current, requested);
    };
    let type_name = current.type_name();
    let values =
        values_of(type_name).ok_or_else(|| format!("the values of {type_name} are not known"))?;
    let chosen = match requested {
        Json::String(name) => values
            .iter()
            .find(|(variant, _)| variant.eq_ignore_ascii_case(name)),
        Json::Number(number) => number
            .as_i64()
            .and_then(|number| values.iter().find(|(_, value)| *value == number)),
        _ => {
            return Err(format!(
                "a {type_name} takes a value name or its number, not {requested}"
            ))
        }
    };
    let (variant, number) =
        chosen.ok_or_else(|| format!("{requested} is not a value of {type_name}"))?;
    Ok(Value::Enum(Box::new(EnumValue::from_canonical_parts(
        type_name.clone(),
        variant.clone(),
        *number,
    ))))
}

/// The value names and numbers of an enumeration type, through its aliases.
pub(crate) fn enumeration_values(
    registry: &TypeRegistry,
    id: trust_hir::TypeId,
) -> Option<&[(smol_str::SmolStr, i64)]> {
    let mut current = id;
    for _ in 0..16 {
        match registry.get(current)? {
            Type::Alias { target, .. } => current = *target,
            Type::Enum { values, .. } => return Some(values.as_slice()),
            _ => return None,
        }
    }
    None
}

/// The value to write to a point that holds `current`, or why `requested` cannot be written.
pub fn value_for_target(current: &Value, requested: &Json) -> Result<Value, String> {
    match current {
        Value::Bool(_) => match requested {
            Json::Bool(flag) => Ok(Value::Bool(*flag)),
            _ => Err(format!("a BOOL takes TRUE or FALSE, not {requested}")),
        },
        Value::SInt(_) => {
            integer(requested, "SINT", i8::MIN.into(), i8::MAX.into()).map(|v| Value::SInt(v as i8))
        }
        Value::Int(_) => integer(requested, "INT", i16::MIN.into(), i16::MAX.into())
            .map(|v| Value::Int(v as i16)),
        Value::DInt(_) => integer(requested, "DINT", i32::MIN.into(), i32::MAX.into())
            .map(|v| Value::DInt(v as i32)),
        Value::LInt(_) => integer(requested, "LINT", i64::MIN.into(), i64::MAX.into())
            .map(|v| Value::LInt(v as i64)),
        Value::USInt(_) => {
            integer(requested, "USINT", 0, u8::MAX.into()).map(|v| Value::USInt(v as u8))
        }
        Value::UInt(_) => {
            integer(requested, "UINT", 0, u16::MAX.into()).map(|v| Value::UInt(v as u16))
        }
        Value::UDInt(_) => {
            integer(requested, "UDINT", 0, u32::MAX.into()).map(|v| Value::UDInt(v as u32))
        }
        Value::ULInt(_) => {
            integer(requested, "ULINT", 0, u64::MAX.into()).map(|v| Value::ULInt(v as u64))
        }
        Value::Byte(_) => {
            integer(requested, "BYTE", 0, u8::MAX.into()).map(|v| Value::Byte(v as u8))
        }
        Value::Word(_) => {
            integer(requested, "WORD", 0, u16::MAX.into()).map(|v| Value::Word(v as u16))
        }
        Value::DWord(_) => {
            integer(requested, "DWORD", 0, u32::MAX.into()).map(|v| Value::DWord(v as u32))
        }
        Value::LWord(_) => {
            integer(requested, "LWORD", 0, u64::MAX.into()).map(|v| Value::LWord(v as u64))
        }
        Value::Real(_) => {
            let value = finite(requested, "REAL")?;
            if value.abs() > f64::from(f32::MAX) {
                return Err(format!("{value} is outside the range of a REAL"));
            }
            Ok(Value::Real(value as f32))
        }
        Value::LReal(_) => finite(requested, "LREAL").map(Value::LReal),
        Value::Time(_) => seconds(requested, "TIME").map(Value::Time),
        Value::LTime(_) => seconds(requested, "LTIME").map(Value::LTime),
        Value::String(_) => match requested.as_str() {
            Some(text) if text.chars().count() <= MAX_TEXT => Ok(Value::String(text.into())),
            Some(_) => Err(format!(
                "a STRING from the HMI holds at most {MAX_TEXT} characters"
            )),
            None => Err(format!("a STRING takes text, not {requested}")),
        },
        Value::WString(_) => match requested.as_str() {
            Some(text) if text.chars().count() <= MAX_TEXT => Ok(Value::WString(text.into())),
            Some(_) => Err(format!(
                "a WSTRING from the HMI holds at most {MAX_TEXT} characters"
            )),
            None => Err(format!("a WSTRING takes text, not {requested}")),
        },
        other => Err(format!(
            "a {} cannot be written from the HMI",
            crate::debug::dap::value_type_name(other)
                .unwrap_or_else(|| "value of this type".to_string())
        )),
    }
}

/// The longest text the HMI writes to a STRING or a WSTRING; the point's declared capacity then
/// decides (specification 39, §12.8 item 5).
const MAX_TEXT: usize = 255;

/// A duration given as a finite number of seconds, from zero.
fn seconds(requested: &Json, type_name: &str) -> Result<crate::value::Duration, String> {
    let value = requested
        .as_f64()
        .filter(|value| value.is_finite() && *value >= 0.0 && *value <= 9.0e9)
        .ok_or_else(|| {
            format!("a {type_name} takes a number of seconds from 0, not {requested}")
        })?;
    Ok(crate::value::Duration::from_nanos(
        (value * 1.0e9).round() as i64
    ))
}

/// A finite number.
fn finite(requested: &Json, type_name: &str) -> Result<f64, String> {
    requested
        .as_f64()
        .filter(|value| value.is_finite())
        .ok_or_else(|| format!("a {type_name} takes a finite number, not {requested}"))
}

/// A whole number within `min..=max`, kept wide enough for every integer type.
fn integer(requested: &Json, type_name: &str, min: i128, max: i128) -> Result<i128, String> {
    let whole = match requested {
        Json::Number(number) => number
            .as_i64()
            .map(i128::from)
            .or_else(|| number.as_u64().map(i128::from))
            .or_else(|| {
                number
                    .as_f64()
                    .filter(|value| value.is_finite() && value.fract() == 0.0 && value.abs() < 1e19)
                    .map(|value| value as i128)
            }),
        _ => {
            return Err(format!(
                "a {type_name} takes a whole number, not {requested}"
            ))
        }
    }
    .ok_or_else(|| format!("a {type_name} takes a whole number, not {requested}"))?;
    if whole < min || whole > max {
        return Err(format!(
            "{whole} does not fit a {type_name} ({min} to {max})"
        ));
    }
    Ok(whole)
}

/// A written point's value as JSON, for answers and the audit log: the kinds of value the HMI
/// writes, including enum labels received from a remote owner; `None` for unsupported aggregates.
pub fn json_of(value: &Value) -> Option<Json> {
    Some(match value {
        Value::Bool(flag) => Json::from(*flag),
        Value::SInt(v) => Json::from(*v),
        Value::Int(v) => Json::from(*v),
        Value::DInt(v) => Json::from(*v),
        Value::LInt(v) => Json::from(*v),
        Value::USInt(v) | Value::Byte(v) => Json::from(*v),
        Value::UInt(v) | Value::Word(v) => Json::from(*v),
        Value::UDInt(v) | Value::DWord(v) => Json::from(*v),
        Value::ULInt(v) | Value::LWord(v) => Json::from(*v),
        Value::Real(v) => serde_json::Number::from_f64(f64::from(*v)).map(Json::Number)?,
        Value::LReal(v) => serde_json::Number::from_f64(*v).map(Json::Number)?,
        Value::Enum(value) => Json::from(value.variant_name().as_str()),
        Value::String(value) => Json::from(value.as_str()),
        Value::WString(value) => Json::from(value.as_str()),
        _ => return None,
    })
}
