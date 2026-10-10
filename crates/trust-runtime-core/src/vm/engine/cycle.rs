//! Cooperative resource transaction, sharing readiness and ordering with the host.
use super::*;
use crate::bytecode::{StorageOwner, StorageRole};
use crate::cycle::{sort_ready_tasks_by_priority, ReadyTask};
use crate::retain::RetainSnapshot;
use crate::task::evaluate_task_readiness;
use crate::vm::construction::values::ValueConstructionContext;
use alloc::format;

impl EngineState<'_> {
    pub(super) fn seed_task_inputs(&mut self) -> Result<(), RuntimeError> {
        for index in 0..self.resource.tasks.len() {
            if let Some(name) = self.resource.tasks[index].single_name_idx {
                self.tasks[index].last_single = self.read_named_bool(name)?;
            }
        }
        Ok(())
    }

    pub(super) fn execute_cycle_inner(&mut self) -> Result<(), RuntimeError> {
        self.sample_input_image()?;
        let ready_bytes = self
            .resource
            .tasks
            .len()
            .checked_mul(core::mem::size_of::<ReadyTask>())
            .ok_or(RuntimeError::Overflow)?;
        self.charge_allocation(ready_bytes)?;
        let mut ready = Vec::new();
        ready
            .try_reserve_exact(self.resource.tasks.len())
            .map_err(|_| super::super::VmTrap::BudgetExceeded.into_runtime_error())?;
        for (index, task) in self.resource.tasks.iter().enumerate() {
            self.charge_work_units(1)?;
            let single = match task.single_name_idx {
                Some(name) => self.read_named_bool(name)?,
                None => false,
            };
            let readiness = evaluate_task_readiness(
                &mut self.tasks[index],
                Duration::from_nanos(task.interval_nanos),
                single,
                self.now,
            );
            if let Some(due_at) = readiness.due_at {
                ready.push(ReadyTask { index, due_at });
            }
        }
        sort_ready_tasks_by_priority(&mut ready, |index| self.resource.tasks[index].priority);
        for entry in ready {
            let task = self.resource.tasks[entry.index].clone();
            for name in task.program_name_idx {
                self.execute_program_root(name)?;
            }
            for reference in task.fb_ref_idx {
                self.execute_task_fb(reference)?;
            }
        }
        let prepared = self.prepared;
        for declaration in &prepared.layout.entries {
            if declaration.role != StorageRole::ProgramRoot {
                continue;
            }
            self.charge_work_units(1)?;
            let name = &prepared.vm.strings[declaration.name_idx as usize];
            let scheduled = self.resource.tasks.iter().any(|task| {
                task.program_name_idx
                    .iter()
                    .any(|index| prepared.vm.strings[*index as usize].eq_ignore_ascii_case(name))
            });
            if !scheduled {
                self.execute_program_root(declaration.name_idx)?;
            }
        }
        if self.services.has_retain_store() {
            let snapshot = self.cycle_retain_snapshot()?;
            self.services.save_retain_snapshot(&snapshot)?;
        }
        self.publish_output_image()
    }

    fn execute_program_root(&mut self, name_idx: u32) -> Result<(), RuntimeError> {
        let prepared = self.prepared;
        let name = &prepared.vm.strings[name_idx as usize];
        self.charge_work_units(prepared.name_lookup_work(name))?;
        let (_, declaration) = prepared
            .global(name)
            .filter(|(_, declaration)| declaration.role == StorageRole::ProgramRoot)
            .ok_or_else(|| invalid_bytecode("missing task program root"))?;
        let pou = declaration
            .owner_pou_id
            .ok_or_else(|| invalid_bytecode("missing program template"))?;
        let name = &prepared.vm.strings[declaration.name_idx as usize];
        let instance = match self.storage.get_global(name) {
            Some(Value::Instance(instance)) => *instance,
            _ => return Err(RuntimeError::NullReference),
        };
        self.execute_pou(pou, Some(instance))
    }

    fn cycle_retain_snapshot(&mut self) -> Result<RetainSnapshot, RuntimeError> {
        let prepared = self.prepared;
        let mut snapshot = RetainSnapshot::default();
        self.charge_work_units(prepared.lookup_work())?;
        for &id in prepared.retained_declarations(StorageOwner::Global, None) {
            self.charge_work_units(1)?;
            let declaration = &prepared.layout.entries[id as usize];
            let name = &prepared.vm.strings[declaration.name_idx as usize];
            if let Some(value) = self.storage.get_global(name) {
                if self.snapshot_value_is_retainable(value, 0)? {
                    let bytes = self.snapshot_entry_charge(value, name.len())?;
                    self.charge_allocation(bytes)?;
                    snapshot.insert(
                        name.clone(),
                        self.storage
                            .get_global(name)
                            .ok_or(RuntimeError::NullReference)?
                            .clone(),
                    );
                }
            }
        }
        for (index, root) in prepared.roots.entries.iter().enumerate() {
            self.charge_work_units(1)?;
            let root_declaration = &prepared.layout.entries[root.declaration_idx as usize];
            if root.parent_root_idx.is_some() || root_declaration.role != StorageRole::ProgramRoot {
                continue;
            }
            let instance = self.construction.roots[index].ok_or(RuntimeError::NullReference)?;
            let program_name = &prepared.vm.strings[root_declaration.name_idx as usize];
            self.charge_work_units(prepared.lookup_work())?;
            for &id in prepared.retained_declarations(StorageOwner::Instance, root.template_pou_id)
            {
                self.charge_work_units(1)?;
                let declaration = &prepared.layout.entries[id as usize];
                let name = &prepared.vm.strings[declaration.name_idx as usize];
                if let Some(value) = self.storage.get_instance_var(instance, name) {
                    if self.snapshot_value_is_retainable(value, 0)? {
                        let key_len = program_name
                            .len()
                            .checked_add(name.len())
                            .and_then(|length| length.checked_add(10))
                            .ok_or(RuntimeError::Overflow)?;
                        let bytes = self.snapshot_entry_charge(value, key_len)?;
                        self.charge_allocation(bytes)?;
                        let key = format!("@program/{program_name}/{name}");
                        snapshot.insert(
                            key,
                            self.storage
                                .get_instance_var(instance, name)
                                .ok_or(RuntimeError::NullReference)?
                                .clone(),
                        );
                    }
                }
            }
        }
        Ok(snapshot)
    }
    fn snapshot_entry_charge(
        &self,
        value: &Value,
        name_bytes: usize,
    ) -> Result<usize, RuntimeError> {
        self.value_clone_charge(value, 0)?
            .checked_add(name_bytes)
            .and_then(|bytes| {
                bytes.checked_add(4 * core::mem::size_of::<(smol_str::SmolStr, Value)>())
            })
            .ok_or(RuntimeError::Overflow)
    }

    fn snapshot_value_is_retainable(
        &self,
        value: &Value,
        depth: usize,
    ) -> Result<bool, RuntimeError> {
        self.charge_work_units(1)?;
        if depth >= self.prepared.limits.max_call_depth {
            return Err(super::super::VmTrap::CallStackOverflow.into_runtime_error());
        }
        match value {
            Value::Reference(_) | Value::Instance(_) => Ok(false),
            Value::Array(array) => {
                for value in array.elements() {
                    if !self.snapshot_value_is_retainable(value, depth + 1)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            Value::Struct(structure) => {
                for value in structure.fields().values() {
                    if !self.snapshot_value_is_retainable(value, depth + 1)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            _ => Ok(true),
        }
    }
}
