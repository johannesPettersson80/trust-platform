//! Borrowed validation avoids reconstructing already canonical immutable values.
use super::super::EngineState;
use crate::bytecode::TypeData;
use crate::error::RuntimeError;
use crate::value::Value;
use crate::vm::type_policy::{primitive_value_matches, resolved_primitive_shape};

impl EngineState<'_> {
    pub(in crate::vm::engine) fn assignment_is_canonical(
        &self,
        ty: u32,
        value: &Value,
        depth: usize,
    ) -> Result<bool, RuntimeError> {
        if depth > crate::bytecode::BYTECODE_MAX_TYPE_NESTING {
            return Err(RuntimeError::TypeMismatch);
        }
        let Some(entry) = self.prepared.vm.types.entries.get(ty as usize) else {
            return Err(RuntimeError::TypeMismatch);
        };
        match (&entry.data, value) {
            (TypeData::Alias { target_type_id }, _) => {
                self.assignment_is_canonical(*target_type_id, value, depth + 1)
            }
            (
                TypeData::Subrange {
                    base_type_id,
                    lower,
                    upper,
                },
                _,
            ) => {
                if !self.assignment_is_canonical(*base_type_id, value, depth + 1)? {
                    return Ok(false);
                }
                let numeric = crate::vm::construction::values::scalar::integer(value)
                    .ok_or(RuntimeError::TypeMismatch)?;
                if numeric < i128::from(*lower) || numeric > i128::from(*upper) {
                    return Err(RuntimeError::SubrangeViolation {
                        value: numeric,
                        lower: i128::from(*lower),
                        upper: i128::from(*upper),
                    });
                }
                Ok(true)
            }
            (
                TypeData::Primitive {
                    prim_id,
                    max_length,
                },
                _,
            ) => {
                if !primitive_value_matches(*prim_id, value) {
                    return Ok(false);
                }
                let text = match value {
                    Value::String(value) => Some(value.as_str()),
                    Value::WString(value) => Some(value.as_str()),
                    _ => None,
                };
                if let Some(text) = text.filter(|_| *max_length != 0) {
                    self.charge_work_units(text.len())?;
                    return Ok(text.chars().nth(usize::from(*max_length)).is_none());
                }
                Ok(true)
            }
            (TypeData::Array { elem_type_id, dims }, Value::Array(array)) => {
                if dims.len() != array.dimensions().len()
                    || dims
                        .iter()
                        .zip(array.dimensions())
                        .any(|(expected, actual)| *expected != (0, i64::MAX) && expected != actual)
                {
                    return Ok(false);
                }
                let count = array
                    .dimensions()
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
                for child in array.elements() {
                    if !self.assignment_is_canonical(*elem_type_id, child, depth + 1)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            (
                TypeData::Struct { fields } | TypeData::Union { fields },
                Value::Struct(structure),
            ) => {
                let name = entry
                    .name_idx
                    .and_then(|id| self.prepared.vm.strings.get(id as usize))
                    .ok_or(RuntimeError::TypeMismatch)?;
                if structure.type_name() != name || structure.fields().len() != fields.len() {
                    return Ok(false);
                }
                if !structure.has_physical_layout(self.prepared.vm.physical_layout(ty).as_ref()) {
                    return Ok(false);
                }
                for (field, (name, child)) in fields.iter().zip(structure.fields()) {
                    if name != &self.prepared.vm.strings[field.name_idx as usize]
                        || !self.assignment_is_canonical(field.type_id, child, depth + 1)?
                    {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            (TypeData::Enum { .. }, Value::Enum(_)) => {
                self.validate_assignment_enum(ty, value)?;
                Ok(true)
            }
            (TypeData::Reference { .. } | TypeData::Pou { .. } | TypeData::Interface { .. }, _) => {
                self.check_identity_value(ty, value)?;
                Ok(!matches!(value, Value::Null))
            }
            _ => Ok(false),
        }
    }

    pub(super) fn validate_assignment_enum(
        &self,
        ty: u32,
        value: &Value,
    ) -> Result<(), RuntimeError> {
        let entry = self
            .prepared
            .vm
            .types
            .entries
            .get(ty as usize)
            .ok_or(RuntimeError::TypeMismatch)?;
        let (
            TypeData::Enum {
                base_type_id,
                variants,
            },
            Value::Enum(enumeration),
        ) = (&entry.data, value)
        else {
            return Err(RuntimeError::TypeMismatch);
        };
        let name = entry
            .name_idx
            .and_then(|id| self.prepared.vm.strings.get(id as usize))
            .ok_or(RuntimeError::TypeMismatch)?;
        if !enumeration.type_name().eq_ignore_ascii_case(name)
            || !variants.iter().any(|variant| {
                variant.value == enumeration.numeric_value()
                    && self.prepared.vm.strings[variant.name_idx as usize]
                        .eq_ignore_ascii_case(enumeration.variant_name())
            })
        {
            return Err(RuntimeError::TypeMismatch);
        }
        let (prim_id, max_length) =
            resolved_primitive_shape(&self.prepared.vm.types, *base_type_id, 0)
                .ok_or(RuntimeError::TypeMismatch)?;
        if !matches!(prim_id, 6..=13) {
            return Err(RuntimeError::TypeMismatch);
        }
        let primitive = crate::vm::construction::values::scalar::coerce(
            prim_id,
            max_length,
            Value::LInt(enumeration.numeric_value()),
        )?;
        // The ordinal must fit both its primitive representation and every
        // declared base subrange; resolving to a primitive cannot erase bounds.
        self.normalize_assignment_value(*base_type_id, primitive)
            .map(|_| ())
    }
}
