//! Bind constants before publication, using the same admitted layout as live values.
use super::const_type_entry;
use crate::bytecode::{TypeData, TypeTable};
use crate::collections::OrderedMap;
use crate::error::RuntimeError;
use crate::value::{overlap::PhysicalLayout, StructValue, Value};
use alloc::sync::Arc;
use smol_str::SmolStr;

pub(super) fn bind(
    value: &mut Value,
    type_id: u32,
    types: &TypeTable,
    layouts: &[Option<Arc<PhysicalLayout>>],
    charge: &mut dyn FnMut(usize, usize) -> Result<(), RuntimeError>,
    depth: usize,
) -> Result<(), RuntimeError> {
    charge(0, 1)?;
    if depth > crate::bytecode::BYTECODE_MAX_TYPE_NESTING {
        return Err(RuntimeError::TypeMismatch);
    }
    match &const_type_entry(types, type_id)?.data {
        TypeData::Alias { target_type_id } => {
            bind(value, *target_type_id, types, layouts, charge, depth + 1)?;
        }
        TypeData::Subrange { base_type_id, .. } => {
            bind(value, *base_type_id, types, layouts, charge, depth + 1)?;
        }
        TypeData::Array { elem_type_id, .. } => {
            let Value::Array(array) = value else {
                return Err(RuntimeError::TypeMismatch);
            };
            for element in array.elements_mut() {
                bind(element, *elem_type_id, types, layouts, charge, depth + 1)?;
            }
        }
        TypeData::Struct { fields } | TypeData::Union { fields } => {
            let Value::Struct(shared) = value else {
                return Err(RuntimeError::TypeMismatch);
            };
            // Decoded constants have not escaped or been shared. Do not hide a
            // violated ownership invariant behind an uncharged copy-on-write.
            let structure = Arc::get_mut(shared).ok_or(RuntimeError::TypeMismatch)?;
            if structure.fields.len() != fields.len() {
                return Err(RuntimeError::TypeMismatch);
            }
            for ((_, child), field) in structure.fields.iter_mut().zip(fields) {
                bind(child, field.type_id, types, layouts, charge, depth + 1)?;
            }
            if let Some(layout) = layouts.get(type_id as usize).and_then(Option::as_ref) {
                charge(
                    layout
                        .byte_len()?
                        .checked_add(layout.initialization_staging_allocation(structure)?)
                        .ok_or(RuntimeError::Overflow)?,
                    layout.construction_work()?,
                )?;
                let owned = core::mem::replace(
                    structure,
                    StructValue::from_canonical_parts(SmolStr::default(), OrderedMap::default()),
                );
                // Supplied constant aliases must agree. Declaration-ordered
                // default initialization is a different construction operation.
                *structure = owned.with_physical_layout(Some(Arc::clone(layout)))?;
            }
        }
        _ => {}
    }
    Ok(())
}
