//! Staged in-process restart, preserving the active state on construction failure.
use super::*;
use crate::bytecode::{StorageOwner, StorageRole};
use crate::retain::{value_is_retainable, RestartMode};

impl EngineState<'_> {
    /// Reconstruct this application, committing only after every initializer succeeds.
    pub fn restart(&mut self, mode: RestartMode) -> Result<(), RuntimeError> {
        let mut next = Self::build(
            self.prepared,
            self.resource_index,
            self.services,
            true,
            (mode == RestartMode::Warm).then_some(&*self),
            Some(&self.images),
            self.now,
        )?;
        if mode == RestartMode::Warm {
            for (index, declaration) in self.prepared.layout.entries.iter().enumerate() {
                next.charge_work_units(1)?;
                if declaration.role == StorageRole::External
                    || declaration.owner != StorageOwner::Instance
                    || !declaration.is_retained()
                {
                    continue;
                }
                let name = &self.prepared.vm.strings[declaration.name_idx as usize];
                let owner = declaration
                    .owner_pou_id
                    .ok_or(RuntimeError::InvalidExecutionState)?;
                next.charge_work_units(self.prepared.lookup_work())?;
                for &root_index in self.prepared.program_roots(owner) {
                    next.charge_work_units(1)?;
                    let Some(old) = self.construction.roots[root_index] else {
                        continue;
                    };
                    let new = next.construction.roots[root_index]
                        .ok_or(RuntimeError::InvalidExecutionState)?;
                    if let Some(value) = self.storage.get_instance_var(old, name) {
                        next.charge_retention_walk(value, 0)?;
                        if value_is_retainable(value) {
                            next.restore_program_value(new, name, value)?;
                        }
                    }
                    next.charge_work_units(self.prepared.lookup_work())?;
                    for &phase_id in self.prepared.edge_declarations(index as u32) {
                        next.charge_work_units(1)?;
                        let phase = &self.prepared.layout.entries[phase_id as usize];
                        let phase_name = &self.prepared.vm.strings[phase.name_idx as usize];
                        if let Some(value) = self.storage.get_instance_var(old, phase_name) {
                            next.restore_program_value(new, phase_name, value)?;
                        }
                    }
                }
            }
        }
        // Image carry-over and initialization already ran in the staged build.
        // Expiry during retained restoration also leaves the previous state intact.
        next.check_entry_deadline()?;
        *self = next;
        Ok(())
    }

    pub(in crate::vm::engine) fn copy_process_images(
        &mut self,
        source: &ProcessImages,
    ) -> Result<(), RuntimeError> {
        let image_bytes = source
            .inputs
            .len()
            .checked_add(source.outputs.len())
            .and_then(|n| n.checked_add(source.memory.len()))
            .ok_or(RuntimeError::Overflow)?;
        self.charge_work_units(image_bytes)?;
        self.charge_allocation_bytes(image_bytes)?;
        self.images.inputs.copy_from_slice(&source.inputs);
        self.images.outputs.copy_from_slice(&source.outputs);
        self.images.memory.copy_from_slice(&source.memory);
        self.charge_allocation_bytes(
            source
                .hierarchical
                .len()
                .checked_mul(core::mem::size_of::<(crate::io_image::IoAddressKey, Value)>() * 4)
                .ok_or(RuntimeError::Overflow)?,
        )?;
        for (key, value) in &source.hierarchical {
            let key_bytes = key.clone_allocation_bytes().ok_or(RuntimeError::Overflow)?;
            self.charge_work_units(key_bytes.checked_add(1).ok_or(RuntimeError::Overflow)?)?;
            self.charge_allocation_bytes(key_bytes)?;
            self.charge_allocation_bytes(self.value_clone_charge(value, 0)?)?;
            self.images.hierarchical.insert(key.clone(), value.clone());
        }
        Ok(())
    }

    fn restore_program_value(
        &mut self,
        instance: InstanceId,
        name: &smol_str::SmolStr,
        value: &Value,
    ) -> Result<(), RuntimeError> {
        self.charge_allocation_bytes(self.value_clone_charge(value, 0)?)?;
        if !self
            .storage
            .set_instance_var(instance, name.clone(), value.clone())
        {
            return Err(RuntimeError::InvalidExecutionState);
        }
        Ok(())
    }

    fn charge_retention_walk(&self, value: &Value, depth: usize) -> Result<(), RuntimeError> {
        self.charge_work_units(1)?;
        if depth >= self.prepared.limits.max_call_depth.min(128) {
            return Err(super::super::VmTrap::CallStackOverflow.into_runtime_error());
        }
        match value {
            Value::Array(array) => {
                for child in array.elements() {
                    self.charge_retention_walk(child, depth + 1)?;
                }
            }
            Value::Struct(structure) => {
                for child in structure.fields().values() {
                    self.charge_retention_walk(child, depth + 1)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
}
