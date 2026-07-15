use crate::bytecode::{TypeData, TypeTable};
use crate::error::RuntimeError;
use crate::value::{normalize_assignment_for_target, truncate_string_elements, RefSegment, Value};

use super::VmModule;

const TYPE_POLICY_MAX_DEPTH: usize = 16;

pub(super) fn normalize_vm_value_for_type(
    module: &VmModule,
    type_idx: u32,
    value: Value,
) -> Result<Value, RuntimeError> {
    normalize_value_for_type_table(&module.types, type_idx, value, 0)
}

pub(super) fn normalize_vm_value_for_ref(
    module: &VmModule,
    ref_idx: u32,
    value: Value,
) -> Result<Value, RuntimeError> {
    let Some(type_idx) = module.ref_type(ref_idx) else {
        return Ok(value);
    };
    normalize_vm_value_for_type(module, type_idx, value)
}

pub(super) fn vm_string_primitive_for_type(module: &VmModule, type_idx: u32) -> Option<u16> {
    vm_string_shape_for_type(module, type_idx).map(|(prim_id, _)| prim_id)
}

pub(super) fn vm_string_shape_for_type(module: &VmModule, type_idx: u32) -> Option<(u16, u16)> {
    resolved_primitive_shape(&module.types, type_idx, 0)
        .filter(|(prim_id, _)| matches!(prim_id, 24 | 25))
}

pub(super) fn vm_type_for_path(
    module: &VmModule,
    type_idx: u32,
    path: &[RefSegment],
) -> Option<u32> {
    let mut current = type_idx;
    for segment in path {
        current = resolved_alias_type(&module.types, current, 0)?;
        let entry = module.types.entries.get(current as usize)?;
        current = match (segment, &entry.data) {
            (RefSegment::Index(_), TypeData::Array { elem_type_id, .. }) => *elem_type_id,
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

fn normalize_value_for_type_table(
    types: &TypeTable,
    type_idx: u32,
    value: Value,
    depth: usize,
) -> Result<Value, RuntimeError> {
    if depth >= TYPE_POLICY_MAX_DEPTH {
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
        } => {
            let value = normalize_string_primitive(*prim_id, *max_length, value);
            let value = match default_value_for_primitive(*prim_id) {
                Some(target) => normalize_assignment_for_target(&target, value),
                None => value,
            };
            if !primitive_value_matches(*prim_id, &value) {
                return Err(RuntimeError::TypeMismatch);
            }
            Ok(value)
        }
        TypeData::Reference { .. } => Ok(match value {
            Value::Null => Value::Reference(None),
            value => value,
        }),
        _ => Ok(value),
    }
}

fn normalize_string_primitive(prim_id: u16, max_length: u16, value: Value) -> Value {
    if max_length == 0 {
        return value;
    }
    match (prim_id, value) {
        (24, Value::String(text)) => {
            Value::String(truncate_string_elements(text.as_str(), u32::from(max_length)).into())
        }
        (25, Value::WString(text)) => {
            Value::WString(truncate_string_elements(&text, u32::from(max_length)))
        }
        (_, value) => value,
    }
}

fn resolved_primitive_id(types: &TypeTable, type_idx: u32, depth: usize) -> Option<u16> {
    if depth >= TYPE_POLICY_MAX_DEPTH {
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

fn resolved_primitive_shape(types: &TypeTable, type_idx: u32, depth: usize) -> Option<(u16, u16)> {
    if depth >= TYPE_POLICY_MAX_DEPTH {
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

fn resolved_alias_type(types: &TypeTable, type_idx: u32, depth: usize) -> Option<u32> {
    if depth >= TYPE_POLICY_MAX_DEPTH {
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

fn primitive_value_matches(prim_id: u16, value: &Value) -> bool {
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
