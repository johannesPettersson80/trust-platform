//! Assignment normalization never evaluates initialization recipes.
use super::EngineState;
use crate::bytecode::{Field, TypeData};
use crate::collections::OrderedMap;
use crate::error::RuntimeError;
use crate::value::{StructValue, Value};
use alloc::{sync::Arc, vec::Vec};

mod canonical;
#[cfg(test)]
#[path = "assignment/primitive_fast_path_tests.rs"]
mod primitive_fast_path_tests;
#[cfg(test)]
mod tests;

impl EngineState<'_> {
    pub(in crate::vm::engine) fn normalize_assignment_value(
        &self,
        ty: u32,
        value: Value,
    ) -> Result<Value, RuntimeError> {
        #[cfg(all(feature = "std", feature = "test-support"))]
        let _timing = crate::vm::diagnostic_timing::begin(
            crate::vm::diagnostic_timing::Phase::AssignmentNormalize,
        );
        self.normalize_assignment_inner(ty, value, 0)
    }

    fn normalize_assignment_inner(
        &self,
        ty: u32,
        value: Value,
        depth: usize,
    ) -> Result<Value, RuntimeError> {
        if depth > crate::bytecode::BYTECODE_MAX_TYPE_NESTING {
            return Err(RuntimeError::TypeMismatch);
        }
        let Some(entry) = self.prepared.vm.types.entries.get(ty as usize) else {
            return Err(RuntimeError::TypeMismatch);
        };
        match &entry.data {
            TypeData::Alias { target_type_id } => {
                self.normalize_assignment_inner(*target_type_id, value, depth + 1)
            }
            TypeData::Array { elem_type_id, dims } => {
                self.normalize_assignment_array(*elem_type_id, dims, value, depth)
            }
            TypeData::Struct { fields } | TypeData::Union { fields } => {
                self.normalize_assignment_struct(ty, entry.name_idx, fields, value, depth)
            }
            TypeData::Enum { .. } => {
                self.validate_assignment_enum(ty, &value)?;
                Ok(value)
            }
            TypeData::Reference { .. } | TypeData::Pou { .. } | TypeData::Interface { .. } => {
                self.check_identity_value(ty, &value)?;
                super::super::type_policy::normalize_vm_value_for_type(&self.prepared.vm, ty, value)
            }
            TypeData::Primitive {
                prim_id,
                max_length,
            } => {
                if matches!(*prim_id, 24 | 25) && *max_length != 0 {
                    let text = match &value {
                        Value::String(text) => Some(text.as_str()),
                        Value::WString(text) => Some(text.as_str()),
                        _ => None,
                    };
                    if let Some(text) = text {
                        self.charge_work_units(
                            text.len()
                                .checked_mul(2)
                                .ok_or_else(|| RuntimeError::Overflow)?,
                        )?;
                        if matches!(value, Value::String(_))
                            && let Some((end, _)) =
                                text.char_indices().nth(usize::from(*max_length))
                        {
                            self.charge_allocation_bytes(end)?;
                        }
                    }
                }
                if !matches!(*prim_id, 24 | 25)
                    && super::super::type_policy::primitive_value_matches(*prim_id, &value)
                {
                    return Ok(value);
                }
                super::super::type_policy::normalize_primitive_value(*prim_id, *max_length, value)
            }
            _ => {
                super::super::type_policy::normalize_vm_value_for_type(&self.prepared.vm, ty, value)
            }
        }
    }

    fn normalize_assignment_array(
        &self,
        elem_type_id: u32,
        dims: &[(i64, i64)],
        value: Value,
        depth: usize,
    ) -> Result<Value, RuntimeError> {
        let Value::Array(mut array) = value else {
            return Err(RuntimeError::TypeMismatch);
        };
        if array.dimensions().len() != dims.len()
            || dims
                .iter()
                .zip(array.dimensions())
                .any(|(expected, actual)| *expected != (0, i64::MAX) && expected != actual)
        {
            return Err(RuntimeError::TypeMismatch);
        }
        let actual_dimensions = array.dimensions();
        let count = actual_dimensions
            .iter()
            .try_fold(1usize, |count, (low, high)| {
                high.checked_sub(*low)
                    .and_then(|n| n.checked_add(1))
                    .and_then(|n| usize::try_from(n).ok())
                    .and_then(|n| count.checked_mul(n))
                    .ok_or_else(|| RuntimeError::TypeMismatch)
            })?;
        if count != array.elements().len() {
            return Err(RuntimeError::TypeMismatch);
        }
        // Array ownership is already independent at an assignment boundary.
        // Reuse the consumed Box and vector instead of allocating a second full
        // array and cloning every nested payload. A failure discards only this
        // detached value; installed storage is still untouched.
        for child in array.elements_mut() {
            let owned = core::mem::replace(child, Value::Null);
            *child = self.normalize_assignment_inner(elem_type_id, owned, depth + 1)?;
        }
        Ok(Value::Array(array))
    }

    fn normalize_assignment_struct(
        &self,
        ty: u32,
        name_idx: Option<u32>,
        fields: &[Field],
        value: Value,
        depth: usize,
    ) -> Result<Value, RuntimeError> {
        let Value::Struct(structure) = value else {
            return Err(RuntimeError::TypeMismatch);
        };
        let name = name_idx
            .and_then(|id| self.prepared.vm.strings.get(id as usize))
            .ok_or_else(|| RuntimeError::TypeMismatch)?;
        if !structure.type_name().eq_ignore_ascii_case(name)
            || structure.fields().len() != fields.len()
        {
            return Err(RuntimeError::TypeMismatch);
        }
        if self.assignment_is_canonical(ty, &Value::Struct(Arc::clone(&structure)), depth)? {
            return Ok(Value::Struct(structure));
        }
        self.charge_allocation_bytes(
            core::mem::size_of::<StructValue>() + 2 * core::mem::size_of::<usize>(),
        )?;
        self.charge_allocation_bytes(
            fields
                .len()
                .checked_mul(4 * core::mem::size_of::<(smol_str::SmolStr, Value)>())
                .ok_or_else(|| RuntimeError::Overflow)?,
        )?;
        let mut in_order = true;
        for (field, (name, _)) in fields.iter().zip(structure.fields()) {
            let expected = &self.prepared.vm.strings[field.name_idx as usize];
            in_order &= name.eq_ignore_ascii_case(expected);
        }
        let positions = if in_order {
            None
        } else {
            self.charge_allocation_bytes(
                fields
                    .len()
                    .checked_mul(core::mem::size_of::<usize>())
                    .ok_or_else(|| RuntimeError::Overflow)?,
            )?;
            let mut positions = Vec::new();
            positions
                .try_reserve_exact(fields.len())
                .map_err(|_| RuntimeError::Overflow)?;
            positions.resize(fields.len(), usize::MAX);
            for (position, (name, _)) in structure.fields().iter().enumerate() {
                let (_, ordinal) = self
                    .prepared
                    .type_member_position(ty, name)
                    .ok_or_else(|| RuntimeError::TypeMismatch)?;
                let destination = positions
                    .get_mut(ordinal)
                    .ok_or_else(|| RuntimeError::TypeMismatch)?;
                if *destination != usize::MAX {
                    return Err(RuntimeError::TypeMismatch);
                }
                *destination = position;
            }
            Some(positions)
        };
        let mut normalized = OrderedMap::default();
        for (ordinal, field) in fields.iter().enumerate() {
            let name = &self.prepared.vm.strings[field.name_idx as usize];
            let position = positions
                .as_ref()
                .map_or(ordinal, |positions| positions[ordinal]);
            let (_, child) = structure
                .fields()
                .get_index(position)
                .ok_or_else(|| RuntimeError::TypeMismatch)?;
            self.charge_allocation_bytes(self.value_clone_charge(child, 0)?)?;
            normalized.insert(
                name.clone(),
                self.normalize_assignment_inner(field.type_id, child.clone(), depth + 1)?,
            );
        }
        let layout = self.prepared.vm.physical_layout(ty);
        let value = StructValue::from_canonical_parts(name.clone(), normalized);
        if let Some(layout) = &layout {
            self.charge_allocation_bytes(layout.byte_len()?)?;
            self.charge_allocation_bytes(layout.initialization_staging_allocation(&value)?)?;
        }
        let value = value.with_physical_layout_preserving(layout, &structure)?;
        Ok(Value::Struct(Arc::new(value)))
    }
}
