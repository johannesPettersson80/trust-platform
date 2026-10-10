//! Runtime checks for mutable references and initialization visibility.
use super::*;
use crate::bytecode::{InitializerBodyKind, StorageOwner, StorageRole};
use crate::memory::MemoryLocation;
use crate::value::ValueRefView;
use crate::vm::context::ExecutionContext;

impl EngineState<'_> {
    pub(super) fn check_entry_deadline(&self) -> Result<(), RuntimeError> {
        self.check_execution_deadline()
    }
    pub(in crate::vm::engine) fn charge_work_units(
        &self,
        units: usize,
    ) -> Result<(), RuntimeError> {
        self.charge_execution_work(units)
    }

    pub(in crate::vm::engine) fn check_value_lifetime(
        &self,
        value: &Value,
        destination: Option<FrameId>,
    ) -> Result<(), RuntimeError> {
        self.check_value_lifetime_inner(value, destination, 0, &mut InstanceSet::default())
    }

    pub(in crate::vm::engine) fn check_value_lifetime_inner(
        &self,
        value: &Value,
        destination: Option<FrameId>,
        depth: usize,
        visited: &mut InstanceSet,
    ) -> Result<(), RuntimeError> {
        self.charge_work_units(1)?;
        if depth >= self.prepared.limits.max_call_depth {
            return Err(super::super::VmTrap::CallStackOverflow.into_runtime_error());
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
                self.charge_work_units(visited.len().max(1).ilog2() as usize + 1)?;
                self.charge_sorted_growth(visited.insertion_demand(*instance))?;
                if visited.insert(*instance)? {
                    let data = self
                        .storage
                        .get_instance(*instance)
                        .ok_or(RuntimeError::NullReference)?;
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
            Value::Array(array) => {
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
        let source_index = self
            .lifetimes
            .live_activations
            .iter()
            .position(|id| *id == source)
            .ok_or(RuntimeError::ReferenceLifetime)?;
        let destination_index = destination
            .and_then(|id| {
                self.lifetimes
                    .live_activations
                    .iter()
                    .position(|candidate| *candidate == id)
            })
            .ok_or(RuntimeError::ReferenceLifetime)?;
        if source_index > destination_index {
            return Err(RuntimeError::ReferenceLifetime);
        }
        Ok(())
    }

    pub(in crate::vm::engine) fn check_instance_lifetime(
        &self,
        instance: InstanceId,
        destination: Option<FrameId>,
    ) -> Result<(), RuntimeError> {
        if self.storage.get_instance(instance).is_none() {
            return Err(RuntimeError::ReferenceLifetime);
        }
        if let Some(Some(owner)) = self.lifetimes.instance_lifetimes.get(&instance) {
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
            MemoryLocation::Instance(id) => self.lifetimes.instance_lifetimes.get(&id).flatten(),
            _ => None,
        };
        if let Some(initializer) = self.construction.initializers.last() {
            let staging = reference.location == MemoryLocation::Local(initializer.result)
                || matches!(reference.location, MemoryLocation::Instance(id) if self.lifetimes.instance_lifetimes.get(&id) == Some(Some(initializer.result)) || initializer.writable_instances.contains(&id));
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
        if let MemoryLocation::Local(id) = reference.location {
            if !self.storage.execution_frame_is_live(id) {
                return Err(RuntimeError::ReferenceLifetime);
            }
        }
        let Some(active) = self.construction.initializers.last() else {
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
                self.charge_work_units(self.prepared.lookup_work())?;
                if let Some((index, declaration)) =
                    self.prepared
                        .declaration(StorageOwner::Global, None, reference.offset)
                {
                    if declaration.role == StorageRole::Static {
                        if active.entry.body_kind != InitializerBodyKind::Action
                            || declaration.owner_pou_id != active.entry.owner_pou_id
                        {
                            return Err(RuntimeError::VisibilityViolation);
                        }
                        let ordinal = self
                            .prepared
                            .static_ordinal(index)
                            .ok_or(RuntimeError::InvalidExecutionState)?;
                        if ordinal >= active.entry.visible_static_count as usize {
                            return Err(RuntimeError::VisibilityViolation);
                        }
                    }
                }
            }
            MemoryLocation::Instance(instance) => {
                let template = self
                    .construction
                    .instance_templates
                    .get(&instance)
                    .copied()
                    .ok_or(RuntimeError::VisibilityViolation)?;
                self.charge_work_units(self.prepared.lookup_work())?;
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
                            .ok_or(RuntimeError::InvalidExecutionState)?;
                        if ordinal >= active.entry.visible_static_count as usize {
                            return Err(RuntimeError::VisibilityViolation);
                        }
                    }
                    if !self
                        .construction
                        .initialized_declarations
                        .contains(&(index, Some(instance)))?
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
                .ok_or(RuntimeError::TypeMismatch)?;
        let entry = self
            .prepared
            .vm
            .types
            .entries
            .get(type_id as usize)
            .ok_or(RuntimeError::TypeMismatch)?;
        match (&entry.data, value) {
            (TypeData::Pou { pou_id }, Value::Instance(instance)) => {
                let mut actual = *self
                    .construction
                    .instance_templates
                    .get(instance)
                    .ok_or(RuntimeError::NullReference)?;
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
                    .construction
                    .instance_templates
                    .get(instance)
                    .ok_or(RuntimeError::NullReference)?;
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
                    .ok_or(RuntimeError::TypeMismatch)?;
                let target = super::super::type_policy::resolved_alias_type(
                    &self.prepared.vm.types,
                    *target_type_id,
                    0,
                )
                .ok_or(RuntimeError::TypeMismatch)?;
                if super::super::reference_attempt::runtime_type_is_compatible(
                    &self.prepared.vm,
                    actual,
                    target,
                ) {
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
