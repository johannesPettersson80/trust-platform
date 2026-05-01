//! Runtime cycle execution.

#![allow(missing_docs)]

use smol_str::SmolStr;

use crate::error;
use crate::task::{ProgramDef, TaskConfig};
use crate::value::{Duration, Value};
use std::sync::Arc;

use super::core::Runtime;
use super::types::ReadyTask;

impl Runtime {
    pub fn execute_cycle(&mut self) -> Result<(), error::RuntimeError> {
        if self.faults.is_faulted() {
            return Err(error::RuntimeError::ResourceFaulted);
        }

        let cycle_timer = self.metrics.start_timer();
        let debug = self.debug.clone();
        if let Some(debug) = debug.as_ref() {
            for write in debug.drain_var_writes() {
                match write.target {
                    crate::debug::PendingVarTarget::Global(name) => {
                        self.storage.set_global(name, write.value);
                    }
                    crate::debug::PendingVarTarget::Retain(name) => {
                        self.storage.set_retain(name, write.value);
                    }
                    crate::debug::PendingVarTarget::Instance(id, name) => {
                        self.storage.set_instance_var(id, name, write.value);
                    }
                    crate::debug::PendingVarTarget::Local(frame_id, name) => {
                        let _ = self
                            .storage
                            .with_frame(frame_id, |storage| storage.set_local(name, write.value));
                    }
                }
            }
            for write in debug.drain_lvalue_writes() {
                let _ = if let Some(frame_id) = write.frame_id {
                    self.storage
                        .with_frame(frame_id, |storage| {
                            let instance_id =
                                storage.current_frame().and_then(|frame| frame.instance_id);
                            crate::helper_eval::write_storage_lvalue(
                                storage,
                                &self.registry,
                                &self.profile,
                                instance_id,
                                &write.target,
                                write.value.clone(),
                            )
                        })
                        .unwrap_or(Err(error::RuntimeError::InvalidFrame(frame_id.0)))
                } else {
                    crate::helper_eval::write_storage_lvalue(
                        &mut self.storage,
                        &self.registry,
                        &self.profile,
                        None,
                        &write.target,
                        write.value.clone(),
                    )
                };
            }
        }

        if let Some(debug) = &self.debug {
            debug.push_runtime_event(crate::debug::RuntimeEvent::CycleStart {
                cycle: self.cycle_counter,
                time: self.current_time,
            });
        }

        if let Err(err) = self.read_cycle_inputs() {
            return Err(self.record_fault(err));
        }

        let mut ready = std::mem::take(&mut self.ready_tasks_scratch);
        ready.clear();
        if let Err(err) = self.collect_ready_tasks_into(&mut ready) {
            self.ready_tasks_scratch = ready;
            return Err(self.record_fault(err));
        }
        ready.sort_by_key(|entry| {
            let task = &self.tasks[entry.index];
            (task.priority, entry.due_at.as_nanos(), entry.index)
        });
        for entry in &ready {
            let task = self.tasks[entry.index].clone();
            let task_timer = self.metrics.start_timer();
            if let Err(err) = self.execute_task(&task) {
                ready.clear();
                self.ready_tasks_scratch = ready;
                return Err(self.record_fault(err));
            }
            if let Some(start) = task_timer {
                self.metrics.record_task(&task.name, start.elapsed());
            }
        }
        ready.clear();
        self.ready_tasks_scratch = ready;
        if let Err(err) = self.execute_background_programs() {
            return Err(self.record_fault(err));
        }

        if let Err(err) = self.write_cycle_outputs() {
            return Err(self.record_fault(err));
        }

        if self.retain.has_store() {
            self.retain.mark_dirty();
            if let Err(err) = self.maybe_save_retain_store() {
                return Err(self.record_fault(err));
            }
        }

        if let Some(debug) = &self.debug {
            debug.push_runtime_event(crate::debug::RuntimeEvent::CycleEnd {
                cycle: self.cycle_counter,
                time: self.current_time,
            });
        }
        if let Some(start) = cycle_timer {
            self.metrics.record_cycle(start.elapsed());
        }
        self.cycle_counter = self.cycle_counter.saturating_add(1);
        Ok(())
    }

    fn apply_forced_values(
        &mut self,
        debug: &crate::debug::DebugControl,
    ) -> Result<(), error::RuntimeError> {
        let forced = debug.forced_snapshot();
        for (address, value) in forced.io {
            self.io.interface_mut().write(&address, value)?;
        }
        for entry in forced.vars {
            match entry.target {
                crate::debug::ForcedVarTarget::Global(name) => {
                    self.storage.set_global(name, entry.value);
                }
                crate::debug::ForcedVarTarget::Retain(name) => {
                    self.storage.set_retain(name, entry.value);
                }
                crate::debug::ForcedVarTarget::Instance(id, name) => {
                    self.storage.set_instance_var(id, name, entry.value);
                }
            }
        }
        Ok(())
    }

    /// Execute a program body in the runtime context.
    pub fn execute_program(&mut self, program: &ProgramDef) -> Result<(), error::RuntimeError> {
        self.ensure_vm_module_loaded()?;
        super::vm::execute_program(self, program)
    }

    /// Execute a function block once by its declared type name.
    pub fn execute_function_block_by_name(
        &mut self,
        name: &str,
    ) -> Result<(), error::RuntimeError> {
        let key = SmolStr::new(name.to_ascii_uppercase());
        let fb = self
            .function_blocks
            .get(&key)
            .cloned()
            .ok_or_else(|| error::RuntimeError::UndefinedFunctionBlock(name.into()))?;
        let instance_id = crate::instance::create_fb_instance(
            &mut self.storage,
            &self.registry,
            &self.profile,
            &self.classes,
            &self.function_blocks,
            &self.functions,
            &self.stdlib,
            &self.initializer_catalog,
            &fb,
        )?;

        const TEMP_FB_REF_OWNER: &str = "__TRUST_TEST_FB_EXEC";
        const TEMP_FB_REF_LOCAL: &str = "__trust_test_fb_ref";

        self.storage.push_frame(TEMP_FB_REF_OWNER);
        self.storage
            .set_local(TEMP_FB_REF_LOCAL, Value::Instance(instance_id));
        let reference = self
            .storage
            .ref_for_local(TEMP_FB_REF_LOCAL)
            .ok_or(error::RuntimeError::NullReference)?;
        let result = self.execute_function_block_ref(&reference);
        self.storage.pop_frame();
        result
    }

    fn execute_program_by_name(&mut self, name: &SmolStr) -> Result<(), error::RuntimeError> {
        let timer = self.metrics.start_timer();
        let program = self
            .programs
            .get(name)
            .cloned()
            .ok_or_else(|| error::RuntimeError::UndefinedProgram(name.clone()))?;
        let result = self.execute_program(&program);
        if let Some(start) = timer {
            self.metrics
                .record_profile_call("program", name, start.elapsed());
        }
        result
    }

    fn execute_task(&mut self, task: &TaskConfig) -> Result<(), error::RuntimeError> {
        if let Some(debug) = &self.debug {
            let thread_id = self.task_thread_ids.get(&task.name).copied();
            debug.set_current_thread(thread_id);
            debug.push_runtime_event(crate::debug::RuntimeEvent::TaskStart {
                name: task.name.clone(),
                priority: task.priority,
                time: self.current_time,
            });
        }
        for program in &task.programs {
            self.execute_program_by_name(program)?;
        }
        for fb_ref in &task.fb_instances {
            self.execute_function_block_ref(fb_ref)?;
        }
        if let Some(debug) = &self.debug {
            debug.push_runtime_event(crate::debug::RuntimeEvent::TaskEnd {
                name: task.name.clone(),
                priority: task.priority,
                time: self.current_time,
            });
        }
        Ok(())
    }

    fn execute_background_programs(&mut self) -> Result<(), error::RuntimeError> {
        let mut background = std::mem::take(&mut self.background_program_names_scratch);
        background.clear();
        for name in self.programs.keys() {
            if self.is_program_scheduled(name) {
                continue;
            }
            background.push(name.clone());
        }
        if background.is_empty() {
            self.background_program_names_scratch = background;
            return Ok(());
        }
        let debug = self.debug.clone();
        let thread_id = self.ensure_background_thread_id();
        if let Some(debug) = debug {
            debug.set_current_thread(thread_id);
        }
        for program_name in &background {
            self.execute_program_by_name(program_name)?;
        }
        background.clear();
        self.background_program_names_scratch = background;
        Ok(())
    }

    fn is_program_scheduled(&self, name: &SmolStr) -> bool {
        self.tasks
            .iter()
            .any(|task| task.programs.iter().any(|program| program == name))
    }

    fn collect_ready_tasks_into(
        &mut self,
        ready: &mut Vec<ReadyTask>,
    ) -> Result<(), error::RuntimeError> {
        let now = self.current_time;
        for (idx, task) in self.tasks.iter().enumerate() {
            let state = self
                .task_state
                .get_mut(&task.name)
                .ok_or_else(|| error::RuntimeError::UndefinedTask(task.name.clone()))?;
            let single_now = match &task.single {
                Some(name) => match self.storage.get_global(name.as_ref()) {
                    Some(Value::Bool(value)) => *value,
                    Some(_) => return Err(error::RuntimeError::InvalidTaskSingle(name.clone())),
                    None => return Err(error::RuntimeError::UndefinedVariable(name.clone())),
                },
                None => false,
            };
            let event_due = !state.last_single && single_now;
            let interval_nanos = task.interval.as_nanos();
            let elapsed = now.as_nanos().saturating_sub(state.last_run.as_nanos());
            let periodic_due = interval_nanos > 0 && !single_now && elapsed >= interval_nanos;
            let mut due_at = None;
            if event_due {
                due_at = Some(now);
            }
            if periodic_due {
                let intervals = elapsed / interval_nanos;
                if intervals > 1 {
                    let missed = (intervals - 1) as u64;
                    state.overrun_count = state.overrun_count.saturating_add(missed);
                    if let Some(debug) = &self.debug {
                        debug.push_runtime_event(crate::debug::RuntimeEvent::TaskOverrun {
                            name: task.name.clone(),
                            missed,
                            time: now,
                        });
                    }
                    self.metrics.record_overrun(&task.name, missed);
                }
                let due_time =
                    Duration::from_nanos(state.last_run.as_nanos().saturating_add(interval_nanos));
                due_at = Some(match due_at {
                    Some(existing) if existing.as_nanos() <= due_time.as_nanos() => existing,
                    _ => due_time,
                });
                state.last_run = now;
            }
            state.last_single = single_now;
            if let Some(due_at) = due_at {
                ready.push(ReadyTask { index: idx, due_at });
            }
        }
        Ok(())
    }

    fn execute_function_block_ref(
        &mut self,
        reference: &crate::value::ValueRef,
    ) -> Result<(), error::RuntimeError> {
        self.ensure_vm_module_loaded()?;
        super::vm::execute_function_block_ref(self, reference)
    }

    fn ensure_vm_module_loaded(&mut self) -> Result<(), error::RuntimeError> {
        if self.vm_module.is_some() {
            return Ok(());
        }
        let module = self.build_vm_module()?;
        module
            .validate()
            .map_err(|err| error::RuntimeError::InvalidBytecode(err.to_string().into()))?;
        let vm_module = Arc::new(super::vm::VmModule::from_bytecode(&module)?);
        self.vm_module = Some(vm_module);
        Ok(())
    }

    fn build_vm_module(&self) -> Result<crate::bytecode::BytecodeModule, error::RuntimeError> {
        if self.source_text_index.is_empty() {
            return crate::bytecode::build_module_from_runtime(self).map_err(|err| {
                error::RuntimeError::InvalidBytecode(
                    format!("vm module build failed: {err}").into(),
                )
            });
        }

        let max_file_id = self.source_text_index.keys().copied().max().unwrap_or(0);
        let sources = (0..=max_file_id)
            .map(|file_id| {
                self.source_text_index
                    .get(&file_id)
                    .map(String::as_str)
                    .unwrap_or("")
            })
            .collect::<Vec<_>>();
        crate::bytecode::build_module_from_runtime_with_sources(self, &sources).map_err(|err| {
            error::RuntimeError::InvalidBytecode(format!("vm module build failed: {err}").into())
        })
    }

    fn read_cycle_inputs(&mut self) -> Result<(), error::RuntimeError> {
        {
            let (interface, drivers) = self.io.interface_and_drivers_mut();
            for entry in drivers {
                entry.driver.read_inputs(interface.inputs_mut())?;
            }
        }
        if let Some(debug) = self.debug.clone() {
            for (address, value) in debug.drain_io_writes() {
                self.io.interface_mut().write(&address, value)?;
            }
            self.apply_forced_values(&debug)?;
        }
        self.io.interface_mut().read_inputs(&mut self.storage)?;
        #[cfg(feature = "debug")]
        self.emit_io_snapshot();
        self.update_io_health();
        Ok(())
    }

    fn write_cycle_outputs(&mut self) -> Result<(), error::RuntimeError> {
        self.io.interface_mut().write_outputs(&self.storage)?;
        if let Some(debug) = self.debug.clone() {
            self.apply_forced_values(&debug)?;
        }
        #[cfg(feature = "debug")]
        self.emit_io_snapshot();
        {
            let (interface, drivers) = self.io.interface_and_drivers_mut();
            for entry in drivers {
                entry.driver.write_outputs(interface.outputs())?;
            }
        }
        self.update_io_health();
        Ok(())
    }

    fn record_fault(&mut self, err: error::RuntimeError) -> error::RuntimeError {
        self.apply_fault(err, self.faults.decision())
    }

    #[cfg(feature = "debug")]
    fn emit_io_snapshot(&self) {
        if let Some(debug) = &self.debug {
            debug.push_io_snapshot(self.io.snapshot());
        }
    }
}
