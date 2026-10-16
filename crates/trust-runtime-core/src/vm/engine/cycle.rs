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
        for index in 0..self.data.resource.tasks.len() {
            if let Some(name) = self.data.resource.tasks[index].single_name_idx {
                self.data.tasks[index].last_single = self.read_named_bool(name)?;
            }
        }
        Ok(())
    }

    pub(super) fn execute_cycle_inner(
        &mut self,
        hooks: &mut dyn CycleHooks,
    ) -> Result<(), RuntimeError> {
        let mut ready = core::mem::take(&mut self.data.resources.ready_tasks);
        ready.clear();
        let result = self.execute_cycle_with_ready(&mut ready, hooks);
        ready.clear();
        self.data.resources.ready_tasks = ready;
        result
    }

    fn execute_cycle_with_ready(
        &mut self,
        ready: &mut Vec<ReadyTask>,
        hooks: &mut dyn CycleHooks,
    ) -> Result<(), RuntimeError> {
        self.cycle_hook(hooks, CyclePhase::CycleStart)?;
        self.cycle_hook(hooks, CyclePhase::BeforeInputSample)?;
        self.sample_input_image()?;
        self.cycle_hook(hooks, CyclePhase::AfterInputSample)?;
        let prepared = self.prepared;
        let resource = &prepared.resources.resources[self.data.resource_index];
        for (index, task) in resource.tasks.iter().enumerate() {
            let single = match task.single_name_idx {
                Some(name) => self.read_named_bool(name)?,
                None => false,
            };
            let readiness = evaluate_task_readiness(
                &mut self.data.tasks[index],
                Duration::from_nanos(task.interval_nanos),
                single,
                self.data.now,
            );
            if readiness.missed_intervals > 0 {
                self.cycle_hook(
                    hooks,
                    CyclePhase::TaskOverrun {
                        index,
                        missed: readiness.missed_intervals,
                    },
                )?;
            }
            if let Some(due_at) = readiness.due_at {
                ready.push(ReadyTask { index, due_at });
            }
        }
        sort_ready_tasks_by_priority(ready, |index| resource.tasks[index].priority);
        for entry in ready.iter() {
            self.cycle_hook(hooks, CyclePhase::TaskStart(entry.index))?;
            let task = &resource.tasks[entry.index];
            for &name in &task.program_name_idx {
                self.begin_top_level_invocation();
                self.execute_program_root(name, hooks)?;
            }
            for &reference in &task.fb_ref_idx {
                self.begin_top_level_invocation();
                self.execute_task_fb(reference)?;
            }
            self.cycle_hook(hooks, CyclePhase::TaskEnd(entry.index))?;
        }
        for &name in prepared.background_programs(self.data.resource_index) {
            self.cycle_hook(hooks, CyclePhase::BackgroundStart(name))?;
            self.begin_top_level_invocation();
            self.execute_program_root(name, hooks)?;
            self.cycle_hook(hooks, CyclePhase::BackgroundEnd(name))?;
        }
        self.cycle_hook(hooks, CyclePhase::BeforeOutputSample)?;
        if self.services.has_retain_store() {
            let snapshot = self.cycle_retain_snapshot()?;
            self.services.save_retain_snapshot(&snapshot)?;
        }
        self.publish_output_image_with_hooks(hooks)
    }

    /// Each top-level program or task function-block invocation in a cycle has its
    /// own work allowance (specification 10; spec 34 TIME-09). Nested calls share it.
    fn begin_top_level_invocation(&self) {
        self.data
            .resources
            .work_budget
            .reset(self.prepared.limits.max_work);
    }

    fn execute_program_root(
        &mut self,
        name_idx: u32,
        hooks: &mut dyn CycleHooks,
    ) -> Result<(), RuntimeError> {
        let prepared = self.prepared;
        let name = &prepared.vm.strings[name_idx as usize];
        let (_, declaration) = prepared
            .global(name)
            .filter(|(_, declaration)| declaration.role == StorageRole::ProgramRoot)
            .ok_or_else(|| {
                invalid_bytecode(smol_str::SmolStr::new_static("missing task program root"))
            })?;
        let pou = declaration.owner_pou_id.ok_or_else(|| {
            invalid_bytecode(smol_str::SmolStr::new_static("missing program template"))
        })?;
        let name = &prepared.vm.strings[declaration.name_idx as usize];
        let observed = hooks.has_program_hooks();
        if observed {
            hooks.program_started(name);
        }
        let result = (|| {
            let instance = match self.data.storage.get_global(name) {
                Some(Value::Instance(instance)) => *instance,
                _ => return Err(RuntimeError::NullReference),
            };
            self.execute_pou(pou, Some(instance))
        })();
        if observed {
            hooks.program_finished(name);
        }
        result
    }

    pub(super) fn cycle_retain_snapshot(&mut self) -> Result<RetainSnapshot, RuntimeError> {
        let prepared = self.prepared;
        let mut snapshot = RetainSnapshot::default();
        for &id in prepared.retained_declarations(StorageOwner::Global, None) {
            let declaration = &prepared.layout.entries[id as usize];
            let name = &prepared.vm.strings[declaration.name_idx as usize];
            if let Some(value) = self.data.storage.get_global(name)
                && self.snapshot_value_is_retainable(value, 0)?
            {
                let bytes = self.snapshot_entry_charge(value, name.len())?;
                self.charge_allocation(bytes)?;
                snapshot.insert(
                    name.clone(),
                    self.data
                        .storage
                        .get_global(name)
                        .ok_or_else(|| RuntimeError::NullReference)?
                        .clone(),
                );
            }
        }
        for (index, root) in prepared.roots.entries.iter().enumerate() {
            let root_declaration = &prepared.layout.entries[root.declaration_idx as usize];
            if root.parent_root_idx.is_some() || root_declaration.role != StorageRole::ProgramRoot {
                continue;
            }
            let instance =
                self.data.construction.roots[index].ok_or_else(|| RuntimeError::NullReference)?;
            let program_name = &prepared.vm.strings[root_declaration.name_idx as usize];
            for &id in prepared.retained_declarations(StorageOwner::Instance, root.template_pou_id)
            {
                let declaration = &prepared.layout.entries[id as usize];
                let name = &prepared.vm.strings[declaration.name_idx as usize];
                if let Some(value) = self.data.storage.get_instance_var(instance, name)
                    && self.snapshot_value_is_retainable(value, 0)?
                {
                    let key_len = program_name
                        .len()
                        .checked_add(name.len())
                        .and_then(|length| length.checked_add(10))
                        .ok_or_else(|| RuntimeError::Overflow)?;
                    let bytes = self.snapshot_entry_charge(value, key_len)?;
                    self.charge_allocation(bytes)?;
                    let key = format!("@program/{program_name}/{name}");
                    if snapshot.values().contains_key(key.as_str()) {
                        return Err(RuntimeError::RetainMigration(
                            "ambiguous retained checkpoint label".into(),
                        ));
                    }
                    snapshot.insert(
                        key,
                        self.data
                            .storage
                            .get_instance_var(instance, name)
                            .ok_or_else(|| RuntimeError::NullReference)?
                            .clone(),
                    );
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
            .ok_or_else(|| RuntimeError::Overflow)
    }

    fn snapshot_value_is_retainable(
        &self,
        value: &Value,
        depth: usize,
    ) -> Result<bool, RuntimeError> {
        if depth > crate::bytecode::BYTECODE_MAX_TYPE_NESTING {
            return Err(RuntimeError::TypeMismatch);
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
