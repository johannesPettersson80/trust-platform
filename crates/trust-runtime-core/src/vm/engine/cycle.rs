//! Cooperative resource transaction, sharing readiness and ordering with the host.
use super::*;
use crate::cycle::{sort_ready_tasks_by_priority, ReadyTask};
use crate::fault::NO_TASK;
use crate::task::evaluate_task_readiness;

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
                self.capture_retained(&mut capture)?;
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
        // Preparation found the program root of the name and its construction root (checklist
        // P1-6).
        let entry = prepared.program_entry(name_idx).ok_or_else(|| {
            invalid_bytecode(smol_str::SmolStr::new_static("missing task program root"))
        })?;
        let pou = entry.pou.ok_or_else(|| {
            invalid_bytecode(smol_str::SmolStr::new_static("missing program template"))
        })?;
        let name = &prepared.vm.strings[entry.name_idx as usize];
        let observed = hooks.has_program_hooks();
        if observed {
            let before = self.allocator_operations();
            hooks.program_started(name);
            self.count_service_operations(before);
        }
        let result = (|| {
            // The program root holds no bytes and names its program's instance (C1-19e), which its
            // construction root reserved.
            let instance = entry
                .root
                .and_then(|root| self.data.construction.roots.get(root as usize).copied())
                .flatten()
                .ok_or_else(|| RuntimeError::NullReference)?;
            self.execute_pou(pou, Some(instance))
        })();
        if observed {
            let before = self.allocator_operations();
            hooks.program_finished(name);
            self.count_service_operations(before);
        }
        result
    }
}
