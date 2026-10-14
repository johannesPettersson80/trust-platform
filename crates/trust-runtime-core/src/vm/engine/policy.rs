//! Runtime checks for mutable references and initialization visibility.
use super::*;
use crate::bytecode::{InitializerBodyKind, StorageOwner, StorageRole};
use crate::memory::MemoryLocation;
use crate::value::ValueRefView;
use crate::vm::context::ExecutionContext;

/// Arrays are homogeneous: an empty array, or one whose first element is a primitive
/// scalar, cannot hold a reference or instance, so its lifetime check visits nothing.
fn array_holds_only_primitives(array: &crate::value::ArrayValue) -> bool {
    array.elements().first().is_none_or(|first| {
        !matches!(
            first,
            Value::Reference(_)
                | Value::Instance(_)
                | Value::Array(_)
                | Value::Struct(_)
                | Value::Null
        )
    })
}

impl EngineState<'_> {
    pub(super) fn check_entry_deadline(&self) -> Result<(), RuntimeError> {
        self.check_execution_deadline()
    }
    #[cfg_attr(all(feature = "std", feature = "test-support"), track_caller)]
    pub(in crate::vm::engine) fn charge_work_units(
        &self,
        units: usize,
    ) -> Result<(), RuntimeError> {
        // Empty paths do not consume fuel or advance the deadline stride.
        if units == 0 {
            return Ok(());
        }
        self.charge_execution_work(units)
    }

    /// Whether `check_value_lifetime` admits `value` without visiting anything or
    /// charging. Exhaustive, so a new value kind must be classified here too.
    /// Arrays are excluded even when they hold only primitives.
    pub(in crate::vm::engine) fn lifetime_check_is_trivial(value: &Value) -> bool {
        match value {
            Value::Array(_) | Value::Struct(_) | Value::Reference(Some(_)) | Value::Instance(_) => {
                false
            }
            Value::Bool(_)
            | Value::SInt(_)
            | Value::Int(_)
            | Value::DInt(_)
            | Value::LInt(_)
            | Value::USInt(_)
            | Value::UInt(_)
            | Value::UDInt(_)
            | Value::ULInt(_)
            | Value::Real(_)
            | Value::LReal(_)
            | Value::Byte(_)
            | Value::Word(_)
            | Value::DWord(_)
            | Value::LWord(_)
            | Value::Time(_)
            | Value::LTime(_)
            | Value::Date(_)
            | Value::LDate(_)
            | Value::Tod(_)
            | Value::LTod(_)
            | Value::Dt(_)
            | Value::Ldt(_)
            | Value::String(_)
            | Value::WString(_)
            | Value::Char(_)
            | Value::WChar(_)
            | Value::Enum(_)
            | Value::Reference(None)
            | Value::Null => true,
        }
    }

    #[cfg_attr(all(feature = "std", feature = "test-support"), track_caller)]
    pub(in crate::vm::engine) fn check_value_lifetime(
        &self,
        value: &Value,
        destination: Option<FrameId>,
    ) -> Result<(), RuntimeError> {
        // TIME-08 LIFETIME: scalars, and arrays of primitive scalars, visit nothing
        // that can hold a reference, so they are admitted without a charge.
        match value {
            Value::Array(array) if array_holds_only_primitives(array) => Ok(()),
            Value::Reference(Some(_)) | Value::Instance(_) | Value::Array(_) | Value::Struct(_) => {
                self.check_value_lifetime_inner(value, destination, 0, &mut InstanceSet::default())
            }
            Value::Bool(_)
            | Value::SInt(_)
            | Value::Int(_)
            | Value::DInt(_)
            | Value::LInt(_)
            | Value::USInt(_)
            | Value::UInt(_)
            | Value::UDInt(_)
            | Value::ULInt(_)
            | Value::Real(_)
            | Value::LReal(_)
            | Value::Byte(_)
            | Value::Word(_)
            | Value::DWord(_)
            | Value::LWord(_)
            | Value::Time(_)
            | Value::LTime(_)
            | Value::Date(_)
            | Value::LDate(_)
            | Value::Tod(_)
            | Value::LTod(_)
            | Value::Dt(_)
            | Value::Ldt(_)
            | Value::String(_)
            | Value::WString(_)
            | Value::Char(_)
            | Value::WChar(_)
            | Value::Enum(_)
            | Value::Reference(None)
            | Value::Null => Ok(()),
        }
    }

    #[cfg_attr(all(feature = "std", feature = "test-support"), track_caller)]
    pub(in crate::vm::engine) fn check_value_lifetime_inner(
        &self,
        value: &Value,
        destination: Option<FrameId>,
        depth: usize,
        visited: &mut InstanceSet,
    ) -> Result<(), RuntimeError> {
        self.charge_work_units(1)?;
        if depth > crate::bytecode::BYTECODE_MAX_TYPE_NESTING {
            return Err(RuntimeError::TypeMismatch);
        }
        match value {
            Value::Reference(Some(reference)) => {
                if let MemoryLocation::Local(source) = reference.location {
                    self.check_local_lifetime(source, destination)?;
                }
                if let MemoryLocation::Instance(instance) = reference.location {
                    self.check_instance_lifetime(instance, destination)?;
                }
            }
            Value::Instance(instance) => {
                self.check_instance_lifetime(*instance, destination)?;
                self.charge_sorted_growth(visited.insertion_demand(*instance))?;
                if visited.insert(*instance)? {
                    let data = self
                        .data
                        .storage
                        .get_instance(*instance)
                        .ok_or_else(|| RuntimeError::NullReference)?;
                    if let Some(parent) = data.parent {
                        self.check_value_lifetime_inner(
                            &Value::Instance(parent),
                            destination,
                            depth + 1,
                            visited,
                        )?;
                    }
                    for value in data.variables.values() {
                        self.check_value_lifetime_inner(value, destination, depth + 1, visited)?;
                    }
                }
            }
            Value::Array(array) if !array_holds_only_primitives(array) => {
                for value in array.elements() {
                    self.check_value_lifetime_inner(value, destination, depth + 1, visited)?;
                }
            }
            Value::Struct(value) => {
                for value in value.fields().values() {
                    self.check_value_lifetime_inner(value, destination, depth + 1, visited)?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    pub(in crate::vm::engine) fn check_local_lifetime(
        &self,
        source: FrameId,
        destination: Option<FrameId>,
    ) -> Result<(), RuntimeError> {
        if !self.data.storage.execution_activation_is_live(source) {
            return Err(RuntimeError::ReferenceLifetime);
        }
        let Some(destination) =
            destination.filter(|id| self.data.storage.execution_activation_is_live(*id))
        else {
            return Err(RuntimeError::ReferenceLifetime);
        };
        // Activated IDs are monotonic, so identity order is ancestry order.
        if source.0 > destination.0 {
            return Err(RuntimeError::ReferenceLifetime);
        }
        Ok(())
    }

    pub(in crate::vm::engine) fn check_instance_lifetime(
        &self,
        instance: InstanceId,
        destination: Option<FrameId>,
    ) -> Result<(), RuntimeError> {
        if self.data.storage.get_instance(instance).is_none() {
            return Err(RuntimeError::ReferenceLifetime);
        }
        if let Some(Some(owner)) = self.data.lifetimes.instance_lifetimes.get(&instance) {
            self.check_local_lifetime(owner, destination)?;
        }
        Ok(())
    }

    pub(in crate::vm::engine) fn check_write(
        &self,
        reference: ValueRefView<'_>,
        value: &Value,
    ) -> Result<(), RuntimeError> {
        self.check_writable_declaration(reference)?;
        let destination = match reference.location {
            MemoryLocation::Local(id) => Some(id),
            MemoryLocation::Instance(id) => {
                self.data.lifetimes.instance_lifetimes.get(&id).flatten()
            }
            _ => None,
        };
        if let Some(initializer) = self.data.construction.initializers.last() {
            let staging = reference.location == MemoryLocation::Local(initializer.result)
                || matches!(reference.location, MemoryLocation::Instance(id) if self.data.lifetimes.instance_lifetimes.get(&id) == Some(Some(initializer.result)) || initializer.writable_instances.contains(&id));
            if !staging {
                return Err(RuntimeError::StagingViolation);
            }
        }
        self.check_value_lifetime(value, destination)
    }

    pub(in crate::vm::engine) fn check_read(
        &self,
        reference: ValueRefView<'_>,
    ) -> Result<(), RuntimeError> {
        if let MemoryLocation::Local(id) = reference.location
            && !self.data.storage.execution_frame_is_live(id)
        {
            return Err(RuntimeError::ReferenceLifetime);
        }
        let Some(active) = self.data.construction.initializers.last() else {
            return Ok(());
        };
        if reference.location == MemoryLocation::Local(active.result) {
            return Ok(());
        }
        match reference.location {
            MemoryLocation::Local(id) => {
                if active.entry.body_kind != InitializerBodyKind::Action
                    || Some(id) != active.lexical_frame
                    || reference.offset >= active.entry.visible_local_count as usize
                {
                    return Err(RuntimeError::VisibilityViolation);
                }
            }
            MemoryLocation::Global => {
                if let Some((index, declaration)) =
                    self.prepared
                        .declaration(StorageOwner::Global, None, reference.offset)
                    && declaration.role == StorageRole::Static
                {
                    if active.entry.body_kind != InitializerBodyKind::Action
                        || declaration.owner_pou_id != active.entry.owner_pou_id
                    {
                        return Err(RuntimeError::VisibilityViolation);
                    }
                    let ordinal = self
                        .prepared
                        .static_ordinal(index)
                        .ok_or_else(|| RuntimeError::InvalidExecutionState)?;
                    if ordinal >= active.entry.visible_static_count as usize {
                        return Err(RuntimeError::VisibilityViolation);
                    }
                }
            }
            MemoryLocation::Instance(instance) => {
                let template = self
                    .data
                    .construction
                    .instance_templates
                    .get(&instance)
                    .copied()
                    .ok_or_else(|| RuntimeError::VisibilityViolation)?;
                if let Some((index, declaration)) = self.prepared.declaration(
                    StorageOwner::Instance,
                    Some(template),
                    reference.offset,
                ) {
                    if declaration.role == StorageRole::Static {
                        if active.entry.body_kind != InitializerBodyKind::Action
                            || declaration.owner_pou_id != active.entry.owner_pou_id
                        {
                            return Err(RuntimeError::VisibilityViolation);
                        }
                        let ordinal = self
                            .prepared
                            .static_ordinal(index)
                            .ok_or_else(|| RuntimeError::InvalidExecutionState)?;
                        if ordinal >= active.entry.visible_static_count as usize {
                            return Err(RuntimeError::VisibilityViolation);
                        }
                    }
                    let marker = (index, Some(instance));
                    if !self
                        .data
                        .construction
                        .initialized_declarations
                        .contains(&marker)?
                    {
                        return Err(RuntimeError::VisibilityViolation);
                    }
                }
            }
            MemoryLocation::Io(_) | MemoryLocation::Retain => {
                return Err(RuntimeError::VisibilityViolation)
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "policy/lifetime_tests.rs"]
mod lifetime_tests;
#[cfg(test)]
#[path = "policy/work_allowance_lifetime_tests.rs"]
mod work_allowance_lifetime_tests;

impl EngineState<'_> {
    pub(in crate::vm::engine) fn normalize_reference_value(
        &self,
        reference: ValueRefView<'_>,
        value: Value,
    ) -> Result<Value, RuntimeError> {
        let Some(type_id) = self.reference_type(reference)? else {
            return Err(RuntimeError::TypeMismatch);
        };
        self.normalize_assignment_value(type_id, value)
    }

    pub(in crate::vm::engine) fn check_typed_value(
        &self,
        type_id: u32,
        value: &Value,
    ) -> Result<(), RuntimeError> {
        if self.assignment_is_canonical(type_id, value, 0)? {
            return Ok(());
        }
        self.charge_allocation_bytes(self.value_clone_charge(value, 0)?)?;
        self.normalize_assignment_value(type_id, value.clone())
            .map(|_| ())
    }

    pub(in crate::vm::engine) fn check_identity_value(
        &self,
        type_id: u32,
        value: &Value,
    ) -> Result<(), RuntimeError> {
        use crate::bytecode::TypeData;
        let type_id =
            super::super::type_policy::resolved_alias_type(&self.prepared.vm.types, type_id, 0)
                .ok_or_else(|| RuntimeError::TypeMismatch)?;
        let entry = self
            .prepared
            .vm
            .types
            .entries
            .get(type_id as usize)
            .ok_or_else(|| RuntimeError::TypeMismatch)?;
        match (&entry.data, value) {
            (TypeData::Pou { pou_id }, Value::Instance(instance)) => {
                let mut actual = *self
                    .data
                    .construction
                    .instance_templates
                    .get(instance)
                    .ok_or_else(|| RuntimeError::NullReference)?;
                for _ in 0..=self.prepared.vm.pou_by_id.len() {
                    if actual == *pou_id {
                        return Ok(());
                    }
                    let Some(parent) = self.prepared.vm.parent_pou_ids.get(&actual) else {
                        break;
                    };
                    actual = *parent;
                }
                Err(RuntimeError::TypeMismatch)
            }
            (TypeData::Interface { .. }, Value::Instance(instance)) => {
                let mut actual = *self
                    .data
                    .construction
                    .instance_templates
                    .get(instance)
                    .ok_or_else(|| RuntimeError::NullReference)?;
                for _ in 0..=self.prepared.vm.pou_by_id.len() {
                    if self
                        .prepared
                        .vm
                        .interface_type_ids_by_pou
                        .get(&actual)
                        .is_some_and(|interfaces| interfaces.contains(&type_id))
                    {
                        return Ok(());
                    }
                    let Some(parent) = self.prepared.vm.parent_pou_ids.get(&actual) else {
                        break;
                    };
                    actual = *parent;
                }
                Err(RuntimeError::TypeMismatch)
            }
            (TypeData::Interface { .. }, Value::Null)
            | (TypeData::Reference { .. }, Value::Null | Value::Reference(None)) => Ok(()),
            (TypeData::Reference { target_type_id }, Value::Reference(Some(reference))) => {
                let actual = self
                    .reference_type(reference.as_view())?
                    .and_then(|id| {
                        super::super::type_policy::resolved_alias_type(
                            &self.prepared.vm.types,
                            id,
                            0,
                        )
                    })
                    .ok_or_else(|| RuntimeError::TypeMismatch)?;
                let target = super::super::type_policy::resolved_alias_type(
                    &self.prepared.vm.types,
                    *target_type_id,
                    0,
                )
                .ok_or_else(|| RuntimeError::TypeMismatch)?;
                if super::super::reference_attempt::runtime_type_is_compatible(
                    &self.prepared.vm,
                    actual,
                    target,
                )? {
                    Ok(())
                } else {
                    Err(RuntimeError::TypeMismatch)
                }
            }
            (TypeData::Pou { .. } | TypeData::Interface { .. } | TypeData::Reference { .. }, _) => {
                Err(RuntimeError::TypeMismatch)
            }
            _ => Ok(()),
        }
    }
}
