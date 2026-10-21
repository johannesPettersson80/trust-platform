//! TYPE_TABLE assignment policy shared by hosted and source-free execution.

use super::module::VmModule;
use crate::bytecode::{TypeData, TypeTable, BYTECODE_MAX_TYPE_NESTING};
use crate::error::RuntimeError;
use crate::memory::cells::Cell;
use crate::value::{normalize_assignment_for_target, RefSegment, Value};
use alloc::string::String;

/// Whether an array dimension is `ARRAY[*]`'s: `TYPE_TABLE` keeps the sentinel bounds
/// `(0, i64::MAX)` for an open dimension (spec 12 §6.3).
pub(crate) fn is_wildcard_dim(dim: (i64, i64)) -> bool {
    dim == (0, i64::MAX)
}

/// Normalize assignment to a declared module type.
pub fn normalize_vm_value_for_type(
    module: &VmModule,
    type_idx: u32,
    value: Value,
) -> Result<Value, RuntimeError> {
    normalize_value_for_type_table(&module.types, type_idx, value, 0)
}

/// Normalize assignment using optional reference type metadata.
#[cfg(feature = "hir")]
pub fn normalize_vm_value_for_ref(
    module: &VmModule,
    ref_idx: u32,
    value: Value,
) -> Result<Value, RuntimeError> {
    let Some(type_idx) = module.ref_type(ref_idx) else {
        return Ok(value);
    };
    normalize_vm_value_for_type(module, type_idx, value)
}

/// Resolve a declared string type to its primitive encoding.
pub fn vm_string_primitive_for_type(module: &VmModule, type_idx: u32) -> Option<u16> {
    vm_string_shape_for_type(module, type_idx).map(|(prim_id, _)| prim_id)
}

/// Resolve string encoding and declared maximum length.
pub fn vm_string_shape_for_type(module: &VmModule, type_idx: u32) -> Option<(u16, u16)> {
    resolved_primitive_shape(&module.types, type_idx, 0)
        .filter(|(prim_id, _)| matches!(prim_id, 24 | 25))
}

/// The declared capacity in storage units of a bounded `STRING`/`WSTRING` type, or the implementer
/// maximum when the type is not a string. A character write is cut to this capacity (spec 02 §6):
/// UTF-8 bytes for `STRING`, UTF-16 code units for `WSTRING`.
#[must_use]
pub fn vm_string_capacity_for_type(module: &VmModule, type_idx: u32) -> usize {
    match vm_string_shape_for_type(module, type_idx) {
        Some((_, max_length)) => usize::from(crate::value::string_capacity(max_length)),
        None => crate::value::MAX_STRING_CAPACITY,
    }
}

/// Resolve a declared type through array and aggregate selections.
pub fn vm_type_for_path(module: &VmModule, type_idx: u32, path: &[RefSegment]) -> Option<u32> {
    let mut current = type_idx;
    for segment in path {
        current = resolved_alias_type(&module.types, current, 0)?;
        let entry = module.types.entries.get(current as usize)?;
        current = match (segment, &entry.data) {
            (RefSegment::Index(_), TypeData::Array { elem_type_id, .. }) => *elem_type_id,
            (
                RefSegment::Index(indices),
                TypeData::Primitive {
                    prim_id: 24 | 25, ..
                },
            ) if indices.len() == 1 => {
                let TypeData::Primitive { prim_id, .. } = entry.data else {
                    return None;
                };
                let character = if prim_id == 24 { 26 } else { 27 };
                u32::try_from(module.types.entries.iter().position(|entry| matches!(entry.data, TypeData::Primitive { prim_id, .. } if prim_id == character))?).ok()?
            }
            (RefSegment::Field(name), TypeData::Struct { fields } | TypeData::Union { fields }) => {
                fields
                    .iter()
                    .find(|field| {
                        module
                            .strings
                            .get(field.name_idx as usize)
                            .is_some_and(|candidate| candidate.eq_ignore_ascii_case(name))
                    })?
                    .type_id
            }
            _ => return None,
        };
    }
    resolved_alias_type(&module.types, current, 0)
}

/// Apply the shared declared-type assignment rules without HIR.
pub fn normalize_value_for_type_table(
    types: &TypeTable,
    type_idx: u32,
    value: Value,
    depth: usize,
) -> Result<Value, RuntimeError> {
    if depth > BYTECODE_MAX_TYPE_NESTING {
        return Err(RuntimeError::TypeMismatch);
    }
    let Some(entry) = types.entries.get(type_idx as usize) else {
        return Ok(value);
    };
    match &entry.data {
        TypeData::Alias { target_type_id } => {
            normalize_value_for_type_table(types, *target_type_id, value, depth + 1)
        }
        TypeData::Subrange {
            base_type_id,
            lower,
            upper,
        } => {
            let value = normalize_value_for_type_table(types, *base_type_id, value, depth + 1)?;
            let Some(prim_id) = resolved_primitive_id(types, *base_type_id, depth + 1) else {
                return Err(RuntimeError::TypeMismatch);
            };
            if !primitive_value_matches(prim_id, &value) {
                return Err(RuntimeError::TypeMismatch);
            }
            let Some(numeric) = integer_value(&value) else {
                return Err(RuntimeError::TypeMismatch);
            };
            let lower = i128::from(*lower);
            let upper = i128::from(*upper);
            if numeric < lower || numeric > upper {
                return Err(RuntimeError::SubrangeViolation {
                    value: numeric,
                    lower,
                    upper,
                });
            }
            Ok(value)
        }
        TypeData::Primitive {
            prim_id,
            max_length,
        } => normalize_primitive_value(*prim_id, *max_length, value),
        TypeData::Reference { .. } => Ok(match value {
            Value::Null => Value::Reference(None),
            value => value,
        }),
        _ => Ok(value),
    }
}

/// Normalize one primitive without resolving its TYPE_TABLE entry again.
/// Values that already match the target primitive need no conversion through a default
/// value after string bounds are applied. Mismatched primitive families retain
/// the existing shared coercion.
pub(crate) fn normalize_primitive_value(
    prim_id: u16,
    max_length: u16,
    value: Value,
) -> Result<Value, RuntimeError> {
    let value = normalize_string_primitive(prim_id, max_length, value);
    if primitive_value_matches(prim_id, &value) {
        return Ok(value);
    }
    let value = match default_value_for_primitive(prim_id) {
        Some(target) => normalize_assignment_for_target(&target, value),
        None => value,
    };
    if primitive_value_matches(prim_id, &value) {
        Ok(value)
    } else {
        Err(RuntimeError::TypeMismatch)
    }
}

/// Cut a stored text to its capacity: UTF-8 bytes for STRING, UTF-16 code units for WSTRING,
/// never splitting a character (`docs/IEC_DECISIONS.md` 2026-10-17).
fn normalize_string_primitive(prim_id: u16, max_length: u16, value: Value) -> Value {
    let capacity = usize::from(crate::value::string_capacity(max_length));
    match (prim_id, value) {
        (24, Value::String(text)) => {
            match crate::value::string_capacity_cut(&text, false, capacity) {
                Some(end) => Value::String(text[..end].into()),
                None => Value::String(text),
            }
        }
        (25, Value::WString(mut text)) => {
            if let Some(end) = crate::value::string_capacity_cut(&text, true, capacity) {
                text.truncate(end);
            }
            Value::WString(text)
        }
        (_, value) => value,
    }
}

fn resolved_primitive_id(types: &TypeTable, type_idx: u32, depth: usize) -> Option<u16> {
    if depth > BYTECODE_MAX_TYPE_NESTING {
        return None;
    }
    let entry = types.entries.get(type_idx as usize)?;
    match &entry.data {
        TypeData::Primitive { prim_id, .. } => Some(*prim_id),
        TypeData::Alias { target_type_id }
        | TypeData::Subrange {
            base_type_id: target_type_id,
            ..
        } => resolved_primitive_id(types, *target_type_id, depth + 1),
        _ => None,
    }
}

/// Resolve a primitive encoding and bound through aliases.
pub fn resolved_primitive_shape(
    types: &TypeTable,
    type_idx: u32,
    depth: usize,
) -> Option<(u16, u16)> {
    if depth > BYTECODE_MAX_TYPE_NESTING {
        return None;
    }
    let entry = types.entries.get(type_idx as usize)?;
    match &entry.data {
        TypeData::Primitive {
            prim_id,
            max_length,
        } => Some((*prim_id, *max_length)),
        TypeData::Alias { target_type_id }
        | TypeData::Subrange {
            base_type_id: target_type_id,
            ..
        } => resolved_primitive_shape(types, *target_type_id, depth + 1),
        _ => None,
    }
}

/// Resolve aliases within the type-policy recursion bound.
pub fn resolved_alias_type(types: &TypeTable, type_idx: u32, depth: usize) -> Option<u32> {
    if depth > BYTECODE_MAX_TYPE_NESTING {
        return None;
    }
    let entry = types.entries.get(type_idx as usize)?;
    match &entry.data {
        TypeData::Alias { target_type_id } => {
            resolved_alias_type(types, *target_type_id, depth + 1)
        }
        _ => Some(type_idx),
    }
}

pub(crate) fn primitive_value_matches(prim_id: u16, value: &Value) -> bool {
    // STBC 2.0 native counter slots remain unbound until their first call.
    // Once bound, ANY_INT preserves the caller's integer width and signedness.
    if prim_id == 0x0100 {
        return matches!(value, Value::Null) || integer_value(value).is_some();
    }
    // The native duration slot of an overloaded timer holds a duration of either family
    // (specification 34 §6.2).
    if prim_id == 0x0101 {
        return matches!(value, Value::Time(_) | Value::LTime(_));
    }
    matches!(
        (prim_id, value),
        (1, Value::Bool(_))
            | (2, Value::Byte(_))
            | (3, Value::Word(_))
            | (4, Value::DWord(_))
            | (5, Value::LWord(_))
            | (6, Value::SInt(_))
            | (7, Value::Int(_))
            | (8, Value::DInt(_))
            | (9, Value::LInt(_))
            | (10, Value::USInt(_))
            | (11, Value::UInt(_))
            | (12, Value::UDInt(_))
            | (13, Value::ULInt(_))
            | (14, Value::Real(_))
            | (15, Value::LReal(_))
            | (16, Value::Time(_))
            | (17, Value::LTime(_))
            | (18, Value::Date(_))
            | (19, Value::LDate(_))
            | (20, Value::Tod(_))
            | (21, Value::LTod(_))
            | (22, Value::Dt(_))
            | (23, Value::Ldt(_))
            | (24, Value::String(_))
            | (25, Value::WString(_))
            | (26, Value::Char(_))
            | (27, Value::WChar(_))
    )
}

/// Whether `cell` holds a value of primitive `prim_id`, as [`primitive_value_matches`] decides it
/// for that value (checklist P1-9). A native counter slot (0x0100), which compact code never names,
/// matches no cell, which leaves its store to the general path.
pub(crate) fn primitive_cell_matches(prim_id: u16, cell: &Cell) -> bool {
    if prim_id == 0x0101 {
        return matches!(cell, Cell::Time(_) | Cell::LTime(_));
    }
    matches!(
        (prim_id, cell),
        (1, Cell::Bool(_))
            | (2, Cell::Byte(_))
            | (3, Cell::Word(_))
            | (4, Cell::DWord(_))
            | (5, Cell::LWord(_))
            | (6, Cell::SInt(_))
            | (7, Cell::Int(_))
            | (8, Cell::DInt(_))
            | (9, Cell::LInt(_))
            | (10, Cell::USInt(_))
            | (11, Cell::UInt(_))
            | (12, Cell::UDInt(_))
            | (13, Cell::ULInt(_))
            | (14, Cell::Real(_))
            | (15, Cell::LReal(_))
            | (16, Cell::Time(_))
            | (17, Cell::LTime(_))
            | (18, Cell::Date(_))
            | (19, Cell::LDate(_))
            | (20, Cell::Tod(_))
            | (21, Cell::LTod(_))
            | (22, Cell::Dt(_))
            | (23, Cell::Ldt(_))
            | (26, Cell::Char(_))
            | (27, Cell::WChar(_))
    )
}

fn integer_value(value: &Value) -> Option<i128> {
    match value {
        Value::SInt(value) => Some(i128::from(*value)),
        Value::Int(value) => Some(i128::from(*value)),
        Value::DInt(value) => Some(i128::from(*value)),
        Value::LInt(value) => Some(i128::from(*value)),
        Value::USInt(value) => Some(i128::from(*value)),
        Value::UInt(value) => Some(i128::from(*value)),
        Value::UDInt(value) => Some(i128::from(*value)),
        Value::ULInt(value) => Some(i128::from(*value)),
        _ => None,
    }
}

fn default_value_for_primitive(prim_id: u16) -> Option<Value> {
    match prim_id {
        1 => Some(Value::Bool(false)),
        2 => Some(Value::Byte(0)),
        3 => Some(Value::Word(0)),
        4 => Some(Value::DWord(0)),
        5 => Some(Value::LWord(0)),
        6 => Some(Value::SInt(0)),
        7 => Some(Value::Int(0)),
        8 => Some(Value::DInt(0)),
        9 => Some(Value::LInt(0)),
        10 => Some(Value::USInt(0)),
        11 => Some(Value::UInt(0)),
        12 => Some(Value::UDInt(0)),
        13 => Some(Value::ULInt(0)),
        14 => Some(Value::Real(0.0)),
        15 => Some(Value::LReal(0.0)),
        24 => Some(Value::String("".into())),
        25 => Some(Value::WString(String::new())),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
