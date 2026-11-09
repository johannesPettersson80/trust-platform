//! A starting value's bytes at the program's layout (specification 44 §6.7): the encoding of a value that
//! holds no instance, in its own module so that `init.rs` stays below the 1,000 lines of a file.

use super::super::model::Model;
use super::super::types::{Elem, Kind, Ty};
use super::super::NativeError;
use crate::value::Value;

pub(super) fn put(bytes: &mut [u8], offset: usize, data: &[u8]) -> Result<(), NativeError> {
    bytes
        .get_mut(offset..offset + data.len())
        .ok_or_else(|| NativeError::Compile("a starting value outside its area".into()))?
        .copy_from_slice(data);
    Ok(())
}

/// The bytes, at the program's layout, of `value` of type `ty`: a frame variable's starting value
/// (specification 44 §6.7).
///
/// # Errors
///
/// [`NativeError`] when the value does not fit its type.
pub(in super::super) fn value_bytes(
    model: &mut Model<'_>,
    value: &Value,
    ty: Ty,
) -> Result<Vec<u8>, NativeError> {
    let mut bytes = vec![0u8; model.types.info(ty).size as usize];
    encode(model, value, ty, &mut bytes, 0)?;
    Ok(bytes)
}

/// Writes `value`, which holds no instance, of type `ty` at `offset`.
pub(super) fn encode(
    model: &Model<'_>,
    value: &Value,
    ty: Ty,
    bytes: &mut [u8],
    offset: usize,
) -> Result<(), NativeError> {
    let info = model.types.info(ty).clone();
    match (&info.kind, value) {
        (_, Value::Null) => Ok(()),
        // A reference and an interface start `NULL` (see `Start::write`).
        (Kind::Ref { .. } | Kind::Interface, Value::Reference(None)) => Ok(()),
        (
            Kind::Elem(elem) | Kind::Enum { base: elem, .. } | Kind::Subrange { base: elem, .. },
            value,
        ) => {
            let data = scalar(*elem, value).ok_or_else(|| {
                NativeError::Compile(format!("a starting value {value:?} of {}", elem.name()))
            })?;
            put(bytes, offset, &data[..elem.size() as usize])
        }
        (Kind::Str(capacity), Value::String(text)) => {
            let mut end = text.len().min(*capacity as usize);
            while !text.is_char_boundary(end) {
                end -= 1;
            }
            let length = u16::try_from(end).unwrap_or(u16::MAX);
            put(bytes, offset, &length.to_le_bytes())?;
            put(bytes, offset + 2, &text.as_bytes()[..end])
        }
        (Kind::WStr(capacity), Value::WString(text)) => {
            // At most `capacity` UTF-16 code units, never half of a pair (specification 10
            // §2.3.1).
            let end =
                trust_runtime_core::value::string_capacity_cut(text, true, *capacity as usize)
                    .unwrap_or(text.len());
            let units: Vec<u8> = text[..end]
                .encode_utf16()
                .flat_map(u16::to_le_bytes)
                .collect();
            let length = u16::try_from(units.len() / 2).unwrap_or(u16::MAX);
            put(bytes, offset, &length.to_le_bytes())?;
            put(bytes, offset + 2, &units)
        }
        (Kind::Array { elem, .. }, Value::Array(array)) => {
            let size = model.types.info(*elem).size as usize;
            for (index, element) in array.elements().iter().enumerate() {
                encode(model, element, *elem, bytes, offset + index * size)?;
            }
            Ok(())
        }
        (Kind::Struct { members }, Value::Struct(structure)) => {
            for member in members {
                if let Some(field) = structure.field(&member.name) {
                    encode(
                        model,
                        field,
                        member.ty,
                        bytes,
                        offset + member.offset as usize,
                    )?;
                }
            }
            Ok(())
        }
        // A `STRUCT OVERLAP` starts at zero: no member has an initial value (specification 02
        // §3.6; IEC §6.4.4.7.2).
        (Kind::Overlap { .. }, _) => Ok(()),
        (kind, value) => Err(NativeError::Compile(format!(
            "a starting value {value:?} for a {kind:?}"
        ))),
    }
}

/// The little-endian bytes of a scalar's value, eight long.
pub(in super::super) fn scalar(elem: Elem, value: &Value) -> Option<[u8; 8]> {
    let wide: i128 = match value {
        Value::Bool(value) => i128::from(*value),
        Value::SInt(value) => i128::from(*value),
        Value::Int(value) => i128::from(*value),
        Value::DInt(value) => i128::from(*value),
        Value::LInt(value) => i128::from(*value),
        Value::USInt(value) | Value::Byte(value) | Value::Char(value) => i128::from(*value),
        Value::UInt(value) | Value::Word(value) | Value::WChar(value) => i128::from(*value),
        Value::UDInt(value) | Value::DWord(value) => i128::from(*value),
        Value::ULInt(value) | Value::LWord(value) => i128::from(*value),
        Value::Time(value) | Value::LTime(value) => i128::from(value.as_nanos()),
        Value::Date(value) => i128::from(value.ticks()),
        Value::Tod(value) => i128::from(value.ticks()),
        Value::Dt(value) => i128::from(value.ticks()),
        Value::LDate(value) => i128::from(value.nanos()),
        Value::LTod(value) => i128::from(value.nanos()),
        Value::Ldt(value) => i128::from(value.nanos()),
        Value::Enum(value) => i128::from(value.numeric_value()),
        Value::Real(value) => {
            return match elem {
                Elem::Real => Some(pad(&value.to_le_bytes())),
                Elem::LReal => Some(f64::from(*value).to_le_bytes()),
                _ => None,
            };
        }
        Value::LReal(value) => {
            return match elem {
                Elem::LReal => Some(value.to_le_bytes()),
                #[allow(clippy::cast_possible_truncation)]
                Elem::Real => Some(pad(&(*value as f32).to_le_bytes())),
                _ => None,
            };
        }
        _ => return None,
    };
    if elem.is_real() {
        #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
        return Some(if elem == Elem::Real {
            pad(&(wide as f32).to_le_bytes())
        } else {
            (wide as f64).to_le_bytes()
        });
    }
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    Some((wide as u64).to_le_bytes())
}

fn pad(bytes: &[u8; 4]) -> [u8; 8] {
    let mut out = [0u8; 8];
    out[..4].copy_from_slice(bytes);
    out
}
