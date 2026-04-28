impl Runtime {
    /// Register a program definition by name.
    pub fn register_program(&mut self, program: ProgramDef) -> Result<(), error::RuntimeError> {
        let instance_id = crate::instance::create_program_instance(
            &mut self.storage,
            &self.registry,
            &self.profile,
            &self.classes,
            &self.function_blocks,
            &self.functions,
            &self.stdlib,
            &self.initializer_catalog,
            &program,
        )?;
        self.storage
            .set_global(program.name.clone(), Value::Instance(instance_id));
        self.programs.insert(program.name.clone(), program);
        self.vm_local_init_plan_cache.invalidate_all();
        Ok(())
    }

    /// Register metadata for a global variable.
    pub(crate) fn register_global_meta(
        &mut self,
        name: SmolStr,
        type_id: trust_hir::TypeId,
        retain: RetainPolicy,
        init: GlobalInitValue,
    ) {
        self.globals.insert(
            name,
            GlobalVarMeta {
                type_id,
                retain,
                init,
            },
        );
    }

    /// Register a task configuration.
    pub fn register_task(&mut self, task: TaskConfig) {
        let mut state = TaskState::new(self.current_time);
        if let Some(single) = &task.single {
            if let Some(Value::Bool(value)) = self.storage.get_global(single.as_ref()) {
                state.last_single = *value;
            }
        }
        if !self.task_thread_ids.contains_key(&task.name) {
            let id = self.next_thread_id;
            self.next_thread_id = self.next_thread_id.saturating_add(1);
            self.task_thread_ids.insert(task.name.clone(), id);
        }
        self.task_state.insert(task.name.clone(), state);
        self.tasks.push(task);
    }

    /// Ensure a stable background thread id when background programs exist.
    pub fn ensure_background_thread_id(&mut self) -> Option<u32> {
        if !self.has_background_programs() {
            return None;
        }
        if self.background_thread_id.is_none() {
            let id = self.next_thread_id;
            self.next_thread_id = self.next_thread_id.saturating_add(1);
            self.background_thread_id = Some(id);
        }
        self.background_thread_id
    }
    /// Access configured tasks.
    #[must_use]
    pub fn tasks(&self) -> &[TaskConfig] {
        &self.tasks
    }

    /// Determine whether any programs run outside configured tasks.
    #[must_use]
    pub fn has_background_programs(&self) -> bool {
        let mut scheduled = IndexMap::new();
        for task in &self.tasks {
            for program in &task.programs {
                scheduled.insert(program.clone(), ());
            }
        }
        self.programs
            .keys()
            .any(|name| !scheduled.contains_key(name))
    }

    /// Advance the runtime clock by the given duration.
    pub fn advance_time(&mut self, delta: Duration) {
        let next = self.current_time.as_nanos() + delta.as_nanos();
        self.current_time = Duration::from_nanos(next);
    }

    /// Set the current simulation time.
    pub fn set_current_time(&mut self, time: Duration) {
        self.current_time = time;
    }

    /// Return whether the resource is currently faulted.
    #[must_use]
    pub fn faulted(&self) -> bool {
        self.faults.is_faulted()
    }

    /// Get the last recorded fault, if any.
    #[must_use]
    pub fn last_fault(&self) -> Option<&error::RuntimeError> {
        self.faults.last_fault()
    }

    /// Clear the faulted state (used by tests and tooling).
    pub fn clear_fault(&mut self) {
        self.faults.clear();
    }

    /// Get the overrun count for a task by name.
    #[must_use]
    pub fn task_overrun_count(&self, name: &str) -> Option<u64> {
        self.task_state.get(name).map(|state| state.overrun_count)
    }
}
