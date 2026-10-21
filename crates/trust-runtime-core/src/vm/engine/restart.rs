//! Staged in-process restart, preserving the active state on construction failure.
use super::*;
use crate::bytecode::{StorageOwner, StorageRole};
use crate::memory::view::{StoreAccess, StoreMutate, ValueView};
use crate::retain::codec::view_is_retainable;
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
        // A warm or cold restart constructs the next generation, so a reference taken
        // before it names no state afterwards (spec 34 §11.3, MEM-07, UPDATE-04).
        let generation = self.next_generation()?;
        let mut next_data = Self::build(
            self.prepared,
            self.data.resource_index,
            self.services,
            generation,
            true,
            (mode == RestartMode::Warm).then_some(&*self),
            Some(&self.data.images),
            self.data.now,
            // The restart reconstructs the store it replaces, with the profile it ran with.
            Construction {
                profile: self.data.profile,
            },
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

    /// Builds the data of a distinct installation that replaces this one, at a quiescent owner
    /// boundary (specification 34 §11.3; specification 11 §6.7.1). It is the one body of
    /// `OwnedRuntimeState::prepare_replacement`, which a computer's hosted state calls, and of
    /// `RuntimeState::prepare_replacement`, which a chip's borrowed state calls, so the two replace
    /// their program by the same rules and cannot drift apart.
    ///
    /// - The replacement continues this installation's lineage with the next generation, so no
    ///   earlier reference names its state (MEM-07, UPDATE-04). An exhausted counter refuses it
    ///   before anything changes.
    /// - The replacement is constructed with its own initial values, and then every variable it
    ///   declares at the same place and by the same name as this installation takes this
    ///   installation's value, retained or not, the members of function-block instances included;
    ///   a variable whose declaration changed keeps what fits, and the rest keeps its initial value
    ///   (decision OC-1 and "keep what fits"; [`carry_running_values`](Self::carry_running_values)).
    ///   No value refuses the change.
    /// - The process images and the logical time are carried over, and the replacement's tasks
    ///   continue this installation's schedule: a task of the same name keeps its last activation,
    ///   the level of its event input and its overruns, so the first cycle after the change runs the
    ///   tasks that are due (specification 34 §11.3, decision OC-2).
    /// - No retain file or checkpoint is applied: it holds older values than the program does, and
    ///   stays the source of a restart after a stop or a power loss (decision OC-1).
    ///
    /// Nothing of this installation changes, whether the replacement is built or refused. The
    /// replacement holds no borrow of this installation: it is built from copies.
    ///
    /// # Errors
    ///
    /// The [`RuntimeError`] of a replacement that cannot be prepared: the generation counter is
    /// exhausted, or an allowance or the physical deadline is exceeded.
    pub(super) fn prepare_replacement_data<'b>(
        &mut self,
        prepared: &'b PreparedModule,
        resource_index: usize,
        services: &'b dyn ExecutionServices,
    ) -> Result<(Box<EngineData>, Vec<super::CarriedValueEvent>), RuntimeError> {
        let generation = self.next_generation()?;
        let mut data = EngineState::build(
            prepared,
            resource_index,
            services,
            generation,
            true,
            None,
            Some(&self.data.images),
            self.data.now,
            // The replacement continues the installation on the store it runs on, with its profile.
            super::Construction {
                profile: self.data.profile,
            },
        )?;
        let mut candidate = EngineState::borrow(&mut data, prepared, services);
        candidate.carry_task_schedule(self);
        let events = candidate.carry_running_values(self)?;
        candidate.check_entry_deadline()?;
        Ok((data, events))
    }

    /// Give each task of this replacement the scheduling state of `source`'s task of the same name,
    /// IEC names compared without regard to ASCII case: its last activation, the level of its event
    /// input and its overruns. A task that `source` does not have keeps the state construction gave
    /// it, which counts its first activation from the replacement's logical time, `source`'s time:
    /// the start of its last cycle, since the program's time advances when a cycle starts
    /// (specification 34 §11.3, decision OC-2).
    fn carry_task_schedule(&mut self, source: &EngineState<'_>) {
        for (index, task) in self.data.resource.tasks.iter().enumerate() {
            let Some(name) = self.prepared.vm.strings.get(task.name_idx as usize) else {
                continue;
            };
            let running = source.data.resource.tasks.iter().position(|old| {
                source
                    .prepared
                    .vm
                    .strings
                    .get(old.name_idx as usize)
                    .is_some_and(|old| old.eq_ignore_ascii_case(name))
            });
            if let (Some(state), Some(old)) = (
                self.data.tasks.get_mut(index),
                running.and_then(|old| source.data.tasks.get(old)),
            ) {
                state.clone_from(old);
            }
        }
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
                if let Some(view) = source.read_instance_var(old, name) {
                    // On the flat store the value is copied from the source's cells, checked and
                    // charged as its value is; nothing builds the value (C1-14b).
                    if self.copies_cells_from(&view) {
                        self.check_retention_cells(view, 0)?;
                        if view_is_retainable(&view)? {
                            self.restore_program_cells(new, name, view)?;
                        }
                    } else {
                        let value = view.materialize()?;
                        self.check_retention_walk(&value, 0)?;
                        if value_is_retainable(&value) {
                            self.restore_program_value(new, name, &value)?;
                        }
                    }
                }
                for &phase_id in source.prepared.edge_declarations(index as u32) {
                    let phase = &source.prepared.layout.entries[phase_id as usize];
                    let phase_name = &source.prepared.vm.strings[phase.name_idx as usize];
                    if let Some(view) = source.read_instance_var(old, phase_name) {
                        if self.copies_cells_from(&view) {
                            self.restore_program_cells(new, phase_name, view)?;
                        } else {
                            let value = view.materialize()?;
                            self.restore_program_value(new, phase_name, &value)?;
                        }
                    }
                }
            }
        }
        // A retained program variable located at %M keeps its retained value, so the marker image
        // the first scan loads must hold it (specification 11 6.7).
        self.publish_marker_image()
    }

    pub(in crate::vm::engine) fn copy_process_images(
        &mut self,
        source: &ProcessImages,
    ) -> Result<(), RuntimeError> {
        // Sampling and publication of the images are bookkeeping and are not charged (TIME-08);
        // the overflow guard stays.
        source
            .inputs
            .len()
            .checked_add(source.outputs.len())
            .and_then(|n| n.checked_add(source.memory.len()))
            .ok_or(RuntimeError::Overflow)?;
        for (target, source) in [
            (&mut self.data.images.inputs, &source.inputs),
            (&mut self.data.images.outputs, &source.outputs),
            (&mut self.data.images.memory, &source.memory),
        ] {
            let shared = target.len().min(source.len());
            target[..shared].copy_from_slice(&source[..shared]);
        }
        for (key, value) in &source.hierarchical {
            // The key copy and the entry map are bookkeeping and are not charged; the overflow
            // guard stays. The value is a copy and pays the copy rule (TIME-08).
            let key_bytes = key.clone_allocation_bytes().ok_or(RuntimeError::Overflow)?;
            key_bytes.checked_add(1).ok_or(RuntimeError::Overflow)?;
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
        name: &str,
        value: &Value,
    ) -> Result<(), RuntimeError> {
        self.charge_allocation_bytes(self.value_clone_charge(value, 0)?)?;
        if !StoreMutate::write_instance_var(self, instance, name, value.clone()) {
            return Err(RuntimeError::InvalidExecutionState);
        }
        Ok(())
    }

    /// Whether this store takes the source value `view` as its cells: the value lies in cells (C1-14b).
    fn copies_cells_from(&self, view: &ValueView<'_>) -> bool {
        matches!(view, ValueView::Cells { .. })
    }

    /// `restore_program_value` from the source's cells: the copy charged as the copy of the value
    /// they hold, then their bytes written into the same member of `instance`, which congruent stores
    /// lay out alike. The value holds no instance and no reference, so its bytes are the same in both.
    fn restore_program_cells(
        &mut self,
        instance: InstanceId,
        name: &str,
        view: ValueView<'_>,
    ) -> Result<(), RuntimeError> {
        self.charge_allocation_bytes(self.view_clone_charge(&view)?)?;
        let ValueView::Cells { cells, range, .. } = view else {
            return Err(RuntimeError::InvalidExecutionState);
        };
        let Some(ValueView::Cells { range: at, .. }) = self.read_instance_var(instance, name)
        else {
            return Err(RuntimeError::InvalidExecutionState);
        };
        let bytes = cells
            .bytes(range)
            .ok_or(RuntimeError::InvalidExecutionState)?;
        let arm = &mut *self.data.store;
        if arm.write_bytes(at, bytes) {
            Ok(())
        } else {
            Err(RuntimeError::InvalidExecutionState)
        }
    }

    /// `check_retention_walk` over the cells of `view`: the same depth check of each array and
    /// structure it holds.
    fn check_retention_cells(&self, view: ValueView<'_>, depth: usize) -> Result<(), RuntimeError> {
        if depth > crate::bytecode::BYTECODE_MAX_TYPE_NESTING {
            return Err(RuntimeError::TypeMismatch);
        }
        if let Some(count) = view.cell_child_count()? {
            for ordinal in 0..count {
                self.check_retention_cells(view.cell_child(ordinal)?, depth + 1)?;
            }
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
