//! Assignment normalization never evaluates initialization recipes.
use super::*;
use crate::bytecode::{Field, TypeData};
use crate::collections::OrderedMap;
use crate::value::{ArrayValue, StructValue};
use alloc::{boxed::Box, sync::Arc};

impl EngineState<'_> {
    pub(in crate::vm::engine) fn normalize_assignment_value(
        &self,
        ty: u32,
        value: Value,
    ) -> Result<Value, RuntimeError> {
        self.normalize_assignment_inner(ty, value, 0)
    }

    fn normalize_assignment_inner(
        &self,
        ty: u32,
        value: Value,
        depth: usize,
    ) -> Result<Value, RuntimeError> {
        self.charge_work_units(1)?;
        if depth >= self.prepared.limits.max_call_depth.min(128) {
            return Err(super::super::VmTrap::CallStackOverflow.into_runtime_error());
        }
        let entry = self
            .prepared
            .vm
            .types
            .entries
            .get(ty as usize)
            .ok_or(RuntimeError::TypeMismatch)?;
        match &entry.data {
            TypeData::Alias { target_type_id } => {
                self.normalize_assignment_inner(*target_type_id, value, depth + 1)
            }
            TypeData::Array { elem_type_id, dims } => {
                self.normalize_assignment_array(*elem_type_id, dims, value, depth)
            }
            TypeData::Struct { fields } | TypeData::Union { fields } => {
                self.normalize_assignment_struct(entry.name_idx, fields, value, depth)
            }
            TypeData::Enum {
                base_type_id,
                variants,
            } => {
                let Value::Enum(enumeration) = &value else {
                    return Err(RuntimeError::TypeMismatch);
                };
                let name = entry
                    .name_idx
                    .and_then(|id| self.prepared.vm.strings.get(id as usize))
                    .ok_or(RuntimeError::TypeMismatch)?;
                self.charge_work_units(variants.len())?;
                if !enumeration.type_name().eq_ignore_ascii_case(name)
                    || !variants.iter().any(|variant| {
                        variant.value == enumeration.numeric_value()
                            && self.prepared.vm.strings[variant.name_idx as usize]
                                .eq_ignore_ascii_case(enumeration.variant_name())
                    })
                {
                    return Err(RuntimeError::TypeMismatch);
                }
                let base = super::super::type_policy::resolved_alias_type(
                    &self.prepared.vm.types,
                    *base_type_id,
                    0,
                )
                .ok_or(RuntimeError::TypeMismatch)?;
                let TypeData::Primitive {
                    prim_id,
                    max_length,
                } = self.prepared.vm.types.entries[base as usize].data
                else {
                    return Err(RuntimeError::TypeMismatch);
                };
                super::super::construction::values::scalar::coerce(
                    prim_id,
                    max_length,
                    Value::LInt(enumeration.numeric_value()),
                )?;
                Ok(value)
            }
            TypeData::Reference { .. } | TypeData::Pou { .. } | TypeData::Interface { .. } => {
                self.check_identity_value(ty, &value)?;
                super::super::type_policy::normalize_vm_value_for_type(&self.prepared.vm, ty, value)
            }
            _ => {
                if let TypeData::Primitive {
                    prim_id: 24 | 25,
                    max_length,
                } = entry.data
                {
                    if max_length != 0 {
                        let text = match &value {
                            Value::String(text) => Some(text.as_str()),
                            Value::WString(text) => Some(text.as_str()),
                            _ => None,
                        };
                        if let Some(text) = text {
                            // Truncation constructs a String; SmolStr may then copy it again.
                            self.charge_work_units(text.len())?;
                            self.charge_allocation_bytes(
                                text.len().checked_mul(2).ok_or(RuntimeError::Overflow)?,
                            )?;
                        }
                    }
                }
                self.charge_allocation_bytes(self.value_clone_charge(&value, 0)?)?;
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
        let Value::Array(array) = value else {
            return Err(RuntimeError::TypeMismatch);
        };
        self.charge_work_units(dims.len())?;
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
                    .ok_or(RuntimeError::TypeMismatch)
            })?;
        if count != array.elements().len() {
            return Err(RuntimeError::TypeMismatch);
        }
        self.charge_allocation_bytes(
            count
                .checked_mul(core::mem::size_of::<Value>())
                .and_then(|n| n.checked_add(core::mem::size_of_val(dims)))
                .ok_or(RuntimeError::Overflow)?,
        )?;
        let mut elements = Vec::new();
        elements
            .try_reserve_exact(count)
            .map_err(|_| RuntimeError::Overflow)?;
        for child in array.elements() {
            self.charge_allocation_bytes(self.value_clone_charge(child, 0)?)?;
            elements.push(self.normalize_assignment_inner(
                elem_type_id,
                child.clone(),
                depth + 1,
            )?);
        }
        Ok(Value::Array(Box::new(ArrayValue::from_canonical_parts(
            elements,
            actual_dimensions.to_vec(),
        ))))
    }

    fn normalize_assignment_struct(
        &self,
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
            .ok_or(RuntimeError::TypeMismatch)?;
        if !structure.type_name().eq_ignore_ascii_case(name)
            || structure.fields().len() != fields.len()
        {
            return Err(RuntimeError::TypeMismatch);
        }
        self.charge_allocation_bytes(
            core::mem::size_of::<StructValue>() + 2 * core::mem::size_of::<usize>(),
        )?;
        self.charge_allocation_bytes(
            fields
                .len()
                .checked_mul(4 * core::mem::size_of::<(smol_str::SmolStr, Value)>())
                .ok_or(RuntimeError::Overflow)?,
        )?;
        let mut normalized = OrderedMap::default();
        for field in fields {
            let name = &self.prepared.vm.strings[field.name_idx as usize];
            let comparisons = structure.fields().keys().try_fold(0usize, |total, key| {
                total.checked_add(key.len()).ok_or(RuntimeError::Overflow)
            })?;
            self.charge_work_units(
                comparisons
                    .checked_add(
                        name.len()
                            .checked_mul(structure.fields().len())
                            .ok_or(RuntimeError::Overflow)?,
                    )
                    .ok_or(RuntimeError::Overflow)?,
            )?;
            let mut candidates = structure
                .fields()
                .iter()
                .filter(|(key, _)| key.eq_ignore_ascii_case(name));
            let (_, child) = candidates.next().ok_or(RuntimeError::TypeMismatch)?;
            if candidates.next().is_some() {
                return Err(RuntimeError::TypeMismatch);
            }
            self.charge_allocation_bytes(self.value_clone_charge(child, 0)?)?;
            normalized.insert(
                name.clone(),
                self.normalize_assignment_inner(field.type_id, child.clone(), depth + 1)?,
            );
        }
        Ok(Value::Struct(Arc::new(StructValue::from_canonical_parts(
            name.clone(),
            normalized,
        ))))
    }
}
