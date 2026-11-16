//! The defaults of layer 1's bindings of the project's data types (specification 48 RU-39): the
//! value a variable of the type starts at, a structure's fields' declared initial values or their
//! types' defaults and an enumeration's declared initial value or its first value (IEC §6.4.4.2.2,
//! §6.4.4.3.2, §6.4.4.6.2, Table 11), as the build works it out for the starting memory
//! (specification 44 §6.7), written as a constant of the binding.

use super::super::frame_init::{scalar, starting_bytes};
use super::super::lower::{byte_string, unit_string};
use super::super::model::Model;
use super::super::types::{Elem, Kind, Ty, Types};
use super::super::NativeError;
use super::st_bindings::{elementary, field_name};
use trust_plc_decl::Elementary;

/// The binding's default of the Structured Text type `ty`, a constant expression of its binding:
/// the value a variable of the type starts at.
///
/// # Errors
///
/// [`NativeError`] for a starting value the build cannot work out, naming the type.
pub(super) fn default(model: &mut Model<'_>, ty: Ty) -> Result<String, NativeError> {
    let name = model.types.info(ty).name.clone();
    let Some(type_id) = model.types.registry().lookup(&name) else {
        return Err(NativeError::Compile(format!(
            "the type {name}, which the project does not declare"
        )));
    };
    let bytes = starting_bytes(model, ty, type_id, None, &name)?;
    constant(&model.types, ty, &bytes)
        .ok_or_else(|| NativeError::unsupported(format!("the default of the type {name}")))
}

/// The value of `ty` whose bytes at the program's layout are `bytes`, as a constant of the Rust type
/// a binding passes it as; `None` for a type no binding passes.
fn constant(types: &Types<'_>, ty: Ty, bytes: &[u8]) -> Option<String> {
    let info = types.info(ty);
    match &info.kind {
        Kind::Elem(elem) => Some(elementary_constant(*elem, bytes)),
        Kind::Str(capacity) => {
            let length = usize::from(u16::from_le_bytes([*bytes.first()?, *bytes.get(1)?]));
            let text = byte_string(bytes.get(2..2 + length)?);
            Some(format!(
                "trust_plc::Str::<{capacity}>::__from_bytes({text})"
            ))
        }
        Kind::WStr(capacity) => {
            let length = usize::from(u16::from_le_bytes([*bytes.first()?, *bytes.get(1)?]));
            let units: Vec<u16> = bytes
                .get(2..2 + 2 * length)?
                .as_chunks::<2>()
                .0
                .iter()
                .map(|unit| u16::from_le_bytes(*unit))
                .collect();
            Some(format!(
                "trust_plc::WStr::<{capacity}>::__from_units({})",
                unit_string(&units)
            ))
        }
        Kind::Array { elem, dims } => array(types, *elem, dims, bytes),
        // A reference that names no variable, `NULL`; one that names a variable has no constant.
        Kind::Ref { .. } => bytes
            .iter()
            .all(|byte| *byte == 0)
            .then(|| "trust_plc::Ref::NULL".to_string()),
        Kind::Enum {
            base,
            values,
            named,
            ..
        } => {
            if super::st_types::is_open(values, *named) {
                return Some(format!(
                    "{}({})",
                    info.name,
                    elementary_constant(*base, bytes)
                ));
            }
            let number = integer(*base, bytes)?;
            let (value, _) = values
                .iter()
                .find(|(_, value)| *value == number)
                .or_else(|| values.first())?;
            Some(format!("{}::{}", info.name, field_name(value)))
        }
        Kind::Struct { members } => {
            let fields = members
                .iter()
                .map(|member| {
                    let start = member.offset as usize;
                    let size = types.info(member.ty).size as usize;
                    let value = constant(types, member.ty, bytes.get(start..start + size)?)?;
                    Some(format!("{}: {value}", field_name(&member.name)))
                })
                .collect::<Option<Vec<_>>>()?;
            Some(format!("{} {{ {} }}", info.name, fields.join(", ")))
        }
        _ => None,
    }
}

/// The array of `dims` over `elem` whose bytes are `bytes`, one dimension at a time, as the array
/// of arrays a binding passes it as.
fn array(types: &Types<'_>, elem: Ty, dims: &[(i64, i64)], bytes: &[u8]) -> Option<String> {
    let Some(((lower, upper), inner)) = dims.split_first() else {
        return constant(types, elem, bytes);
    };
    let length = usize::try_from(upper.checked_sub(*lower)?.checked_add(1)?).ok()?;
    if length == 0 || !bytes.len().is_multiple_of(length) {
        return None;
    }
    let items = bytes
        .chunks_exact(bytes.len() / length)
        .map(|item| array(types, elem, inner, item))
        .collect::<Option<Vec<_>>>()?;
    Some(format!(
        "trust_plc::Array::from_items([{}])",
        items.join(", ")
    ))
}

/// The value of the elementary type `elem` in the first bytes of `bytes`, as RU-25's type of it,
/// whose SDK types hold the value the areas store.
pub(super) fn elementary_constant(elem: Elem, bytes: &[u8]) -> String {
    if matches!(elem, Elem::Bool) {
        return (bytes.first().copied().unwrap_or(0) != 0).to_string();
    }
    let stored = scalar(elem, bytes);
    let rust = elementary(elem).rust();
    if rust == elem.rust() {
        return stored;
    }
    match elementary(elem) {
        Elementary::Time
        | Elementary::LTime
        | Elementary::LDate
        | Elementary::LTod
        | Elementary::Ldt => format!("trust_plc::{rust}::from_nanos({stored})"),
        Elementary::Date | Elementary::Tod | Elementary::Dt => {
            format!("trust_plc::{rust}::from_millis({stored})")
        }
        _ => format!("trust_plc::{rust}({stored})"),
    }
}

/// The integer of the integer type `elem` in the first bytes of `bytes`, as its cell stores it.
fn integer(elem: Elem, bytes: &[u8]) -> Option<i64> {
    let size = elem.size() as usize;
    let mut wide = [0u8; 8];
    wide.get_mut(..size)?.copy_from_slice(bytes.get(..size)?);
    let raw = u64::from_le_bytes(wide);
    let shift = 64 - 8 * size;
    #[allow(clippy::cast_possible_wrap)]
    let value = if elem.stored().starts_with('i') {
        ((raw << shift) as i64) >> shift
    } else {
        raw as i64
    };
    Some(value)
}
