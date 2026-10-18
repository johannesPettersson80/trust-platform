//! Cooperative resource transaction, sharing readiness and ordering with the host.
use super::*;
use crate::bytecode::{StorageOwner, StorageRole};
use crate::cycle::{sort_ready_tasks_by_priority, ReadyTask};
use crate::fault::NO_TASK;
use crate::memory::layout::CELL_BYTES;
use crate::memory::view::StoreAccess;
use crate::retain::capture::RetainCapture;
use crate::retain::codec::{decode_snapshot_payload_bytes, PayloadWriter};
use crate::retain::RetainSnapshot;
use crate::task::evaluate_task_readiness;
use crate::vm::construction::values::ValueConstructionContext;

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
            // The fault record names the task that was running (specification 34 MEM-05). A
            // task index beyond the record's field is not an admitted one to name.
            self.data.task = u16::try_from(entry.index).unwrap_or(NO_TASK);
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
            self.data.task = NO_TASK;
        }
        for &name in prepared.background_programs(self.data.resource_index) {
            // An unscheduled program runs in no task.
            self.data.task = NO_TASK;
            self.cycle_hook(hooks, CyclePhase::BackgroundStart(name))?;
            self.begin_top_level_invocation();
            self.execute_program_root(name, hooks)?;
            self.cycle_hook(hooks, CyclePhase::BackgroundEnd(name))?;
        }
        self.cycle_hook(hooks, CyclePhase::BeforeOutputSample)?;
        let services = self.services;
        if let Some(retain) = services.retain_capture() {
            let now = self.data.now;
            let mut capture = retain.borrow_mut();
            capture.mark_dirty();
            if capture.should_save(now) {
                capture.refill(|payload| self.fill_retained_payload(payload))?;
                if capture.image_changed() {
                    services.save_retain_payload(capture.staged_payload())?;
                    capture.accept(now);
                } else {
                    capture.unchanged(now);
                }
            }
        } else if self.services.has_retain_store() {
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
            let view = self
                .data
                .storage
                .read_global(name)
                .ok_or(RuntimeError::NullReference)?;
            let instance = match self.data.storage.borrow(&view) {
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

    /// Capture the retained image this resource would persist, in the registration and declaration
    /// order of specification 11 §6.7, through the shared capture: its reserved staging buffer
    /// holds the STRN v3 payload bytes the codec writes, so a steady-state capture allocates
    /// nothing (specification 34 MEM-04).
    pub(super) fn capture_retained(
        &mut self,
        capture: &mut RetainCapture,
    ) -> Result<(), RuntimeError> {
        capture.refill(|payload| self.fill_retained_payload(payload))
    }

    /// An explicitly requested retained checkpoint as a fresh snapshot, through the shared capture.
    pub(super) fn cycle_retain_snapshot(&mut self) -> Result<RetainSnapshot, RuntimeError> {
        let mut capture = RetainCapture::new();
        self.capture_retained(&mut capture)?;
        decode_snapshot_payload_bytes(capture.staged_payload())
    }

    /// Append the retained values to `payload`: the resource's retained globals and then each root's
    /// retained program variables, in registration and declaration order (specification 11 §6.7).
    fn fill_retained_payload(
        &mut self,
        payload: &mut PayloadWriter<'_>,
    ) -> Result<(), RuntimeError> {
        let prepared = self.prepared;
        for &id in prepared.retained_declarations(StorageOwner::Global, None) {
            let declaration = &prepared.layout.entries[id as usize];
            let name = &prepared.vm.strings[declaration.name_idx as usize];
            if let Some(view) = self.data.storage.read_global(name)
                && let Some(value) = self.data.storage.borrow(&view)
                && self.snapshot_value_is_retainable(value, 0)?
            {
                let bytes = self.snapshot_entry_charge(value, name.len())?;
                self.charge_allocation(bytes)?;
                let stored = self
                    .data
                    .storage
                    .read_global(name)
                    .ok_or(RuntimeError::NullReference)?;
                let value = self
                    .data
                    .storage
                    .borrow(&stored)
                    .ok_or(RuntimeError::NullReference)?;
                payload.entry(name.as_str(), value)?;
            }
        }
        for (index, root) in prepared.roots.entries.iter().enumerate() {
            let root_declaration = &prepared.layout.entries[root.declaration_idx as usize];
            if root.parent_root_idx.is_some() || root_declaration.role != StorageRole::ProgramRoot {
                continue;
            }
            let instance =
                self.data.construction.roots[index].ok_or(RuntimeError::NullReference)?;
            let program_name = &prepared.vm.strings[root_declaration.name_idx as usize];
            for &id in prepared.retained_declarations(StorageOwner::Instance, root.template_pou_id)
            {
                let declaration = &prepared.layout.entries[id as usize];
                let name = &prepared.vm.strings[declaration.name_idx as usize];
                if let Some(view) = self.data.storage.read_instance_var(instance, name)
                    && let Some(value) = self.data.storage.borrow(&view)
                    && self.snapshot_value_is_retainable(value, 0)?
                {
                    let key_len = program_name
                        .len()
                        .checked_add(name.len())
                        .and_then(|length| length.checked_add(10))
                        .ok_or(RuntimeError::Overflow)?;
                    let bytes = self.snapshot_entry_charge(value, key_len)?;
                    self.charge_allocation(bytes)?;
                    if self.retained_program_key_conflicts(program_name.as_str(), name.as_str()) {
                        return Err(RuntimeError::RetainMigration(
                            "ambiguous retained checkpoint label".into(),
                        ));
                    }
                    let stored = self
                        .data
                        .storage
                        .read_instance_var(instance, name)
                        .ok_or(RuntimeError::NullReference)?;
                    let value = self
                        .data
                        .storage
                        .borrow(&stored)
                        .ok_or(RuntimeError::NullReference)?;
                    // The reserved `@program/<program-identity>/<variable-identity>` key is
                    // appended as pieces, so an identity of any length builds no key string
                    // (specification 11 §6.7).
                    payload.entry_pieces(
                        &["@program/", program_name.as_str(), "/", name.as_str()],
                        value,
                    )?;
                }
            }
        }
        Ok(())
    }

    /// Whether a retained global's declared name is exactly the `@program/<program>/<name>` key of
    /// the program value about to be appended (specification 11 §6.7).
    ///
    /// A program identity and variable identity pair is unique, so only a retained global's
    /// declared name can collide with a program key. The `@program/` prefix cannot be an IEC
    /// identifier, but a wire name is opaque, so the capture refuses a collision instead of writing
    /// two entries under one name.
    fn retained_program_key_conflicts(&self, program: &str, name: &str) -> bool {
        let prepared = self.prepared;
        prepared
            .retained_declarations(StorageOwner::Global, None)
            .iter()
            .any(|&id| {
                let declaration = &prepared.layout.entries[id as usize];
                let global = &prepared.vm.strings[declaration.name_idx as usize];
                global
                    .as_str()
                    .strip_prefix("@program/")
                    .and_then(|rest| rest.rsplit_once('/'))
                    == Some((program, name))
            })
    }

    fn snapshot_entry_charge(
        &self,
        value: &Value,
        name_bytes: usize,
    ) -> Result<usize, RuntimeError> {
        // A retained entry costs four cells of allocation (TIME-08 retained-value snapshot row).
        self.value_clone_charge(value, 0)?
            .checked_add(name_bytes)
            .and_then(|bytes| bytes.checked_add(4 * CELL_BYTES))
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
