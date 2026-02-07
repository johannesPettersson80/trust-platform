//! Runtime cycle execution.

#![allow(missing_docs)]

use indexmap::IndexMap;
use smol_str::SmolStr;

use crate::eval::{self, EvalContext};
use crate::task::{ProgramDef, TaskConfig};
use crate::value::{Duration, Value};
use crate::{error, stdlib};
use trust_hir::symbols::ParamDirection;

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
                let using_ref = (!write.using.is_empty()).then_some(write.using.as_slice());
                let _ = self.with_eval_context(write.frame_id, using_ref, |ctx| {
                    crate::eval::expr::write_lvalue(ctx, &write.target, write.value.clone())
                });
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

        let mut ready = match self.collect_ready_tasks() {
            Ok(ready) => ready,
            Err(err) => return Err(self.record_fault(err)),
        };
        ready.sort_by_key(|entry| {
            let task = &self.tasks[entry.index];
            (task.priority, entry.due_at.as_nanos(), entry.index)
        });
        for entry in ready {
            let task = self.tasks[entry.index].clone();
            let task_timer = self.metrics.start_timer();
            if let Err(err) = self.execute_task(&task) {
                return Err(self.record_fault(err));
            }
            if let Some(start) = task_timer {
                self.metrics.record_task(&task.name, start.elapsed());
            }
        }
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
        let mut debug = self.debug.take();
        let instance_id = match self.storage.get_global(program.name.as_ref()) {
            Some(Value::Instance(id)) => Some(*id),
            _ => None,
        };
        let mut ctx = EvalContext {
            storage: &mut self.storage,
            registry: &self.registry,
            profile: self.profile,
            now: self.current_time,
            debug: debug
                .as_mut()
                .map(|hook| hook as &mut dyn crate::debug::DebugHook),
            call_depth: 0,
            functions: Some(&self.functions),
            stdlib: Some(&self.stdlib),
            function_blocks: Some(&self.function_blocks),
            classes: Some(&self.classes),
            using: Some(&program.using),
            access: Some(&self.access),
            current_instance: instance_id,
            return_name: None,
            loop_depth: 0,
            pause_requested: false,
        };
        let mut has_frame = false;
        if instance_id.is_some() || !program.temps.is_empty() {
            if let Some(instance_id) = instance_id {
                ctx.storage
                    .push_frame_with_instance(program.name.clone(), instance_id);
            } else {
                ctx.storage.push_frame(program.name.clone());
            }
            if !program.temps.is_empty() {
                if let Err(err) = eval::init_locals_in_frame(&mut ctx, &program.temps) {
                    ctx.storage.pop_frame();
                    self.debug = debug;
                    return Err(err);
                }
            }
            has_frame = true;
        }
        let result = match eval::exec_block(&mut ctx, &program.body) {
            Ok(result) => result,
            Err(err) => {
                if has_frame {
                    ctx.storage.pop_frame();
                }
                self.debug = debug;
                return Err(err);
            }
        };
        if has_frame {
            ctx.storage.pop_frame();
        }
        self.debug = debug;
        match result {
            eval::stmt::StmtResult::Continue => Ok(()),
            _ => Err(error::RuntimeError::InvalidControlFlow),
        }
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
        let mut scheduled = IndexMap::new();
        for task in &self.tasks {
            for program in &task.programs {
                scheduled.insert(program.clone(), ());
            }
        }
        let mut background = Vec::new();
        for (name, program) in &self.programs {
            if scheduled.contains_key(name) {
                continue;
            }
            background.push(program.clone());
        }
        if background.is_empty() {
            return Ok(());
        }
        let debug = self.debug.clone();
        let thread_id = self.ensure_background_thread_id();
        if let Some(debug) = debug {
            debug.set_current_thread(thread_id);
        }
        for program in background {
            self.execute_program(&program)?;
        }
        Ok(())
    }

    fn collect_ready_tasks(&mut self) -> Result<Vec<ReadyTask>, error::RuntimeError> {
        let mut ready = Vec::new();
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
        Ok(ready)
    }

    fn execute_function_block_ref(
        &mut self,
        reference: &crate::value::ValueRef,
    ) -> Result<(), error::RuntimeError> {
        let timer = self.metrics.start_timer();
        let instance_id = match self.storage.read_by_ref(reference.clone()) {
            Some(Value::Instance(id)) => *id,
            Some(_) => return Err(error::RuntimeError::TypeMismatch),
            None => return Err(error::RuntimeError::NullReference),
        };
        let instance = self
            .storage
            .get_instance(instance_id)
            .ok_or(error::RuntimeError::NullReference)?;
        let key = SmolStr::new(instance.type_name.to_ascii_uppercase());
        let fb = self.function_blocks.get(&key).ok_or_else(|| {
            error::RuntimeError::UndefinedFunctionBlock(instance.type_name.clone())
        })?;
        let mut debug = self.debug.take();
        let mut ctx = EvalContext {
            storage: &mut self.storage,
            registry: &self.registry,
            profile: self.profile,
            now: self.current_time,
            debug: debug
                .as_mut()
                .map(|hook| hook as &mut dyn crate::debug::DebugHook),
            call_depth: 0,
            functions: Some(&self.functions),
            stdlib: Some(&self.stdlib),
            function_blocks: Some(&self.function_blocks),
            classes: Some(&self.classes),
            using: Some(&fb.using),
            access: Some(&self.access),
            current_instance: Some(instance_id),
            return_name: None,
            loop_depth: 0,
            pause_requested: false,
        };
        ctx.storage
            .push_frame_with_instance(fb.name.clone(), instance_id);

        if fb.params.iter().any(|param| {
            param.name.eq_ignore_ascii_case("EN") && matches!(param.direction, ParamDirection::In)
        }) {
            if let Some(Value::Bool(false)) = ctx.storage.get_instance_var(instance_id, "EN") {
                if fb.params.iter().any(|param| {
                    param.name.eq_ignore_ascii_case("ENO")
                        && matches!(param.direction, ParamDirection::Out)
                }) {
                    ctx.storage
                        .set_instance_var(instance_id, "ENO", Value::Bool(false));
                }
                ctx.storage.pop_frame();
                self.debug = debug;
                return Ok(());
            }
        }

        let builtin_kind = stdlib::fbs::builtin_kind(fb.name.as_ref());
        let result = if let Some(kind) = builtin_kind {
            stdlib::fbs::execute_builtin(&mut ctx, instance_id, kind)
        } else {
            crate::eval::init_locals_in_frame(&mut ctx, &fb.temps)?;
            crate::eval::exec_block(&mut ctx, &fb.body).map(|_| ())
        };

        ctx.storage.pop_frame();
        self.debug = debug;
        if let Some(start) = timer {
            self.metrics
                .record_profile_call("fb", &fb.name, start.elapsed());
        }
        result
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
