use alloc::{format, vec::Vec};

use crate::bytecode::{TypeData, TypeTable};
use crate::error::RuntimeError;

const POINTER_REFERENCE_HANDLE_SIZE_BYTES: u64 = core::mem::size_of::<usize>() as u64;
const SIZEOF_TYPE_MAX_DEPTH: usize = 128;

/// Compute `SIZEOF` for a bytecode type-table entry.
pub fn sizeof_type_from_table(types: &TypeTable, type_idx: u32) -> Result<u64, RuntimeError> {
    let mut stack = Vec::new();
    sizeof_type_from_table_inner(types, type_idx, &mut stack)
}

fn sizeof_type_from_table_inner(
    types: &TypeTable,
    type_idx: u32,
    stack: &mut Vec<u32>,
) -> Result<u64, RuntimeError> {
    if stack.len() >= SIZEOF_TYPE_MAX_DEPTH {
        return Err(RuntimeError::InvalidBytecode(
            format!(
                "SIZEOF type nesting exceeds max depth {SIZEOF_TYPE_MAX_DEPTH} at index {type_idx}"
            )
            .into(),
        ));
    }
    if stack.contains(&type_idx) {
        return Err(RuntimeError::InvalidBytecode(
            format!("SIZEOF type recursion at index {type_idx}").into(),
        ));
    }
    let entry = types.entries.get(type_idx as usize).ok_or_else(|| {
        RuntimeError::InvalidBytecode(format!("invalid type index {type_idx} for SIZEOF").into())
    })?;
    stack.push(type_idx);
    let result = (|| match &entry.data {
        TypeData::Primitive {
            prim_id,
            max_length,
        } => sizeof_primitive_type(*prim_id, *max_length),
        TypeData::Array { elem_type_id, dims } => {
            if dims
                .iter()
                .any(|(lower, upper)| *lower == 0 && *upper == i64::MAX)
            {
                return Err(RuntimeError::TypeMismatch);
            }
            let elem_size = sizeof_type_from_table_inner(types, *elem_type_id, stack)?;
            let len = type_array_len(dims).ok_or(RuntimeError::TypeMismatch)?;
            elem_size.checked_mul(len).ok_or(RuntimeError::Overflow)
        }
        TypeData::Struct { fields } => {
            let mut total = 0u64;
            for field in fields {
                let size = sizeof_type_from_table_inner(types, field.type_id, stack)?;
                total = total.checked_add(size).ok_or(RuntimeError::Overflow)?;
            }
            Ok(total)
        }
        TypeData::Union { fields } => {
            let mut max_size = 0u64;
            for field in fields {
                let size = sizeof_type_from_table_inner(types, field.type_id, stack)?;
                max_size = max_size.max(size);
            }
            Ok(max_size)
        }
        TypeData::Enum { base_type_id, .. } => {
            sizeof_type_from_table_inner(types, *base_type_id, stack)
        }
        TypeData::Alias { target_type_id } => {
            sizeof_type_from_table_inner(types, *target_type_id, stack)
        }
        TypeData::Subrange { base_type_id, .. } => {
            sizeof_type_from_table_inner(types, *base_type_id, stack)
        }
        TypeData::Reference { .. } => Ok(POINTER_REFERENCE_HANDLE_SIZE_BYTES),
        TypeData::Pou { .. } | TypeData::Interface { .. } => Err(RuntimeError::TypeMismatch),
    })();
    let _ = stack.pop();
    result
}

fn sizeof_primitive_type(prim_id: u16, max_length: u16) -> Result<u64, RuntimeError> {
    match prim_id {
        1 | 2 | 6 | 10 | 26 => Ok(1),
        3 | 7 | 11 | 27 => Ok(2),
        4 | 8 | 12 | 14 => Ok(4),
        5 | 9 | 13 | 15 => Ok(8),
        16 | 18 | 20 | 22 => Ok(4),
        17 | 19 | 21 | 23 => Ok(8),
        24 => {
            if max_length == 0 {
                Err(RuntimeError::TypeMismatch)
            } else {
                Ok(u64::from(max_length))
            }
        }
        25 => {
            if max_length == 0 {
                Err(RuntimeError::TypeMismatch)
            } else {
                u64::from(max_length)
                    .checked_mul(2)
                    .ok_or(RuntimeError::Overflow)
            }
        }
        other => Err(RuntimeError::InvalidBytecode(
            format!("unsupported primitive type id {other} for SIZEOF").into(),
        )),
    }
}

fn type_array_len(dimensions: &[(i64, i64)]) -> Option<u64> {
    let mut total: i128 = 1;
    for (lower, upper) in dimensions {
        let len = i128::from(*upper) - i128::from(*lower) + 1;
        if len <= 0 {
            return None;
        }
        total = total.checked_mul(len)?;
    }
    u64::try_from(total).ok()
}
