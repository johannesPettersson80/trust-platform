//! Staged in-process restart, preserving the active state on construction failure.
use super::*;
use crate::bytecode::{StorageOwner, StorageRole};
use crate::retain::{value_is_retainable, RestartMode};

impl EngineState<'_> {
    /// Reconstruct this application, committing only after every initializer succeeds.
    pub fn restart(&mut self, mode: RestartMode) -> Result<(), RuntimeError> {
        self.restart_with_snapshot(mode, None).map(|_| ())
    }

    pub(super) fn restart_with_snapshot(
        &mut self,
        mode: RestartMode,
        snapshot: Option<&crate::retain::RetainSnapshot>,
    ) -> Result<Vec<super::RetainApplyEvent>, RuntimeError> {
        let mut next_data = Self::build(
            self.prepared,
            self.data.resource_index,
            self.services,
            true,
            (mode == RestartMode::Warm).then_some(&*self),
            Some(&self.data.images),
            self.data.now,
        )?;
        let mut next = Self::borrow(&mut next_data, self.prepared, self.services);
        if mode == RestartMode::Warm {
            next.restore_retained_program_values(self)?;
        }
        // Image carry-over and initialization already ran in the staged build.
        // Expiry during retained restoration also leaves the previous state intact.
        let events = match snapshot {
            Some(snapshot) => next.apply_retained_snapshot(snapshot)?,
            None => Vec::new(),
        };
        next.check_entry_deadline()?;
        // Same-module restart preserves selected plans and profiling, as the
        // hosted lifecycle did. Move reserved state only after candidate success.
        #[cfg(feature = "register-ir")]
        core::mem::swap(
            &mut next_data.resources.register,
            &mut self.data.resources.register,
        );
        *self.data = *next_data;
        Ok(events)
    }

    pub(super) fn restore_retained_program_values(
        &mut self,
        source: &EngineState<'_>,
    ) -> Result<(), RuntimeError> {
        for (index, declaration) in source.prepared.layout.entries.iter().enumerate() {
            if declaration.role == StorageRole::External
                || declaration.owner != StorageOwner::Instance
                || !declaration.is_retained()
            {
                continue;
            }
            let name = &source.prepared.vm.strings[declaration.name_idx as usize];
            let owner = declaration
                .owner_pou_id
                .ok_or(RuntimeError::InvalidExecutionState)?;
            for &root_index in source.prepared.program_roots(owner) {
                let Some(old) = source.data.construction.roots[root_index] else {
                    continue;
                };
                let new = self.data.construction.roots[root_index]
                    .ok_or(RuntimeError::InvalidExecutionState)?;
                if let Some(value) = source.data.storage.get_instance_var(old, name) {
                    self.check_retention_walk(value, 0)?;
                    if value_is_retainable(value) {
                        self.restore_program_value(new, name, value)?;
                    }
                }
                for &phase_id in source.prepared.edge_declarations(index as u32) {
                    let phase = &source.prepared.layout.entries[phase_id as usize];
                    let phase_name = &source.prepared.vm.strings[phase.name_idx as usize];
                    if let Some(value) = source.data.storage.get_instance_var(old, phase_name) {
                        self.restore_program_value(new, phase_name, value)?;
                    }
                }
            }
        }
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
        self.charge_allocation_bytes(image_bytes)?;
        for (target, source) in [
            (&mut self.data.images.inputs, &source.inputs),
            (&mut self.data.images.outputs, &source.outputs),
            (&mut self.data.images.memory, &source.memory),
        ] {
            let shared = target.len().min(source.len());
            target[..shared].copy_from_slice(&source[..shared]);
        }
        self.charge_allocation_bytes(
            source
                .hierarchical
                .len()
                .checked_mul(core::mem::size_of::<(crate::io_image::IoAddressKey, Value)>() * 4)
                .ok_or(RuntimeError::Overflow)?,
        )?;
        for (key, value) in &source.hierarchical {
            let key_bytes = key.clone_allocation_bytes().ok_or(RuntimeError::Overflow)?;
            // The overflow guard stays; the key copy is bookkeeping and is not charged.
            key_bytes.checked_add(1).ok_or(RuntimeError::Overflow)?;
            self.charge_allocation_bytes(key_bytes)?;
            self.charge_allocation_bytes(self.value_clone_charge(value, 0)?)?;
            self.data
                .images
                .hierarchical
                .insert(key.clone(), value.clone());
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
            .data
            .storage
            .set_instance_var(instance, name.clone(), value.clone())
        {
            return Err(RuntimeError::InvalidExecutionState);
        }
        Ok(())
    }

    /// Depth-check a retained value before it is copied. The walk is bookkeeping:
    /// the copy itself is charged by `value_clone_charge` (TIME-08).
    pub(super) fn check_retention_walk(
        &self,
        value: &Value,
        depth: usize,
    ) -> Result<(), RuntimeError> {
        if depth > crate::bytecode::BYTECODE_MAX_TYPE_NESTING {
            return Err(RuntimeError::TypeMismatch);
        }
        match value {
            Value::Array(array) => {
                for child in array.elements() {
                    self.check_retention_walk(child, depth + 1)?;
                }
            }
            Value::Struct(structure) => {
                for child in structure.fields().values() {
                    self.check_retention_walk(child, depth + 1)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
}

/// Index-based object preservation is valid only for the same admitted identity
/// graph. Changed executable bodies/default code are allowed; changed physical
/// declarations, types, inheritance or interface identities require migration.
#[cfg(feature = "std")]
pub(super) fn warm_graph_congruent(old: &PreparedModule, new: &PreparedModule) -> bool {
    old.layout == new.layout
        && old.roots == new.roots
        && old.vm.types.entries == new.vm.types.entries
        && old.vm.physical_layouts.len() == new.vm.physical_layouts.len()
        && old
            .vm
            .physical_layouts
            .iter()
            .zip(&new.vm.physical_layouts)
            .all(|(old, new)| match (old, new) {
                (None, None) => true,
                (Some(old), Some(new)) => old.storage_compatible(new),
                _ => false,
            })
        && old
            .vm
            .strings
            .iter()
            .zip(&new.vm.strings)
            .all(|(a, b)| a == b)
        && old.vm.parent_pou_ids == new.vm.parent_pou_ids
        && old.vm.interface_type_ids_by_pou == new.vm.interface_type_ids_by_pou
        && old.vm.pou_by_id.len() == new.vm.pou_by_id.len()
        && old.vm.pou_by_id.iter().all(|(id, pou)| {
            new.vm.pou(*id).is_some_and(|next| {
                pou.kind == next.kind
                    && pou.name == next.name
                    && pou.local_ref_start == next.local_ref_start
                    && pou.local_ref_count == next.local_ref_count
                    && pou.primary_instance_owner == next.primary_instance_owner
            })
        })
}
