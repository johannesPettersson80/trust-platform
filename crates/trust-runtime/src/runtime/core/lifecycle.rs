impl Runtime {
    /// Create a new runtime with default profile and empty storage.
    #[must_use]
    pub fn new() -> Self {
        let mut runtime = Self {
            execution_backend: crate::execution_backend::ExecutionBackend::BytecodeVm,
            vm_module: None,
            profile: DateTimeProfile::default(),
            storage: VariableStorage::default(),
            registry: TypeRegistry::new(),
            initializer_catalog: InitializerCatalog::default(),
            io: IoSubsystem::new(),
            access: AccessMap::default(),
            stdlib: StandardLibrary::new(),
            debug: None,
            statement_index: IndexMap::new(),
            source_text_index: IndexMap::new(),
            source_label_index: std::collections::HashMap::new(),
            functions: IndexMap::new(),
            function_blocks: IndexMap::new(),
            classes: IndexMap::new(),
            interfaces: IndexMap::new(),
            programs: IndexMap::new(),
            globals: IndexMap::new(),
            tasks: Vec::new(),
            ready_tasks_scratch: Vec::new(),
            background_program_names_scratch: Vec::new(),
            task_state: IndexMap::new(),
            task_thread_ids: IndexMap::new(),
            next_thread_id: 1,
            background_thread_id: None,
            current_time: Duration::ZERO,
            cycle_counter: 0,
            retain: RetainManager::default(),
            metrics: MetricsSubsystem::new(),
            watchdog: WatchdogSubsystem::new(),
            faults: FaultSubsystem::new(),
            execution_deadline: None,
            output_commit_deadline: None,
            vm_local_init_plan_cache: super::vm::VmLocalInitPlanCacheState::default(),
            vm_register_lowering_cache: super::vm::RegisterLoweringCacheState::from_env(),
            vm_register_profile: super::vm::RegisterProfileState::default(),
            vm_tier1_specialized_executor:
                super::vm::RegisterTier1SpecializedExecutorState::from_env(),
        };
        runtime.register_builtin_function_blocks();
        runtime
    }

    /// Access the active date/time profile.
    #[must_use]
    pub fn profile(&self) -> DateTimeProfile {
        self.profile
    }

    /// Enable debugging and return a shared control handle.
    #[must_use]
    pub fn enable_debug(&mut self) -> crate::debug::DebugControl {
        let control = crate::debug::DebugControl::new();
        self.debug = Some(control.clone());
        control
    }

    /// Set an external debug control handle.
    pub fn set_debug_control(&mut self, control: crate::debug::DebugControl) {
        self.debug = Some(control);
    }

    /// Snapshot static metadata for external tooling.
    #[must_use]
    pub fn metadata_snapshot(&self) -> RuntimeMetadata {
        RuntimeMetadata {
            profile: self.profile,
            registry: self.registry.clone(),
            stdlib: self.stdlib.clone(),
            access: self.access.clone(),
            functions: self.functions.clone(),
            function_blocks: self.function_blocks.clone(),
            classes: self.classes.clone(),
            interfaces: self.interfaces.clone(),
            programs: self.programs.clone(),
            tasks: self.tasks.clone(),
            task_thread_ids: self
                .tasks
                .iter()
                .filter_map(|task| {
                    self.task_thread_ids
                        .get(&task.name)
                        .copied()
                        .map(|id| (task.name.clone(), id))
                })
                .collect(),
            background_thread_id: self.background_thread_id,
            statement_index: self.statement_index.clone(),
        }
    }

    /// Clear the active debug control.
    pub fn clear_debug_control(&mut self) {
        self.debug = None;
    }

    /// Configure the retain store and save cadence.
    pub fn set_retain_store(
        &mut self,
        store: Option<Box<dyn RetainStore>>,
        save_interval: Option<Duration>,
    ) {
        self.retain
            .configure(store, save_interval, self.current_time);
    }

    /// Update the watchdog policy.
    pub fn set_watchdog_policy(&mut self, policy: WatchdogPolicy) {
        self.watchdog.set_policy(policy);
    }

    /// Update the fault policy.
    pub fn set_fault_policy(&mut self, policy: FaultPolicy) {
        self.faults.set_policy(policy);
    }

    /// Current watchdog policy.
    #[must_use]
    pub fn watchdog_policy(&self) -> WatchdogPolicy {
        self.watchdog.policy()
    }

    /// Current fault policy.
    #[must_use]
    pub fn fault_policy(&self) -> FaultPolicy {
        self.faults.policy()
    }

    /// Set an optional execution deadline enforced by the evaluator.
    pub fn set_execution_deadline(&mut self, deadline: Option<std::time::Instant>) {
        self.execution_deadline = deadline;
    }

    /// Get the current execution deadline.
    #[must_use]
    pub fn execution_deadline(&self) -> Option<std::time::Instant> {
        self.execution_deadline
    }

    pub(crate) fn set_output_commit_deadline(&mut self, deadline: Option<std::time::Instant>) {
        self.output_commit_deadline = deadline;
    }

    #[must_use]
    pub(crate) fn output_commit_deadline(&self) -> Option<std::time::Instant> {
        self.output_commit_deadline
    }

    /// Update configured safe-state outputs.
    pub fn set_io_safe_state(&mut self, safe_state: IoSafeState) {
        self.io.set_safe_state(safe_state);
    }

    /// Attach a metrics sink for runtime statistics.
    pub fn set_metrics_sink(&mut self, metrics: std::sync::Arc<std::sync::Mutex<RuntimeMetrics>>) {
        self.metrics.set_sink(metrics);
        self.metrics.set_execution_backend(self.execution_backend);
    }

    /// Update retain save interval without changing the backend.
    pub fn set_retain_save_interval(&mut self, interval: Option<Duration>) {
        self.retain.set_save_interval(interval);
    }

    /// Mark retain values as dirty so they will be persisted on the next save tick.
    pub fn mark_retain_dirty(&mut self) {
        self.retain.mark_dirty();
    }

    /// Record a watchdog timeout fault.
    pub fn watchdog_timeout(&mut self) -> error::RuntimeError {
        let err = error::RuntimeError::WatchdogTimeout;
        self.apply_fault(err, self.watchdog.decision())
    }

    /// Record a scripted simulation fault.
    pub fn simulation_fault(
        &mut self,
        message: impl Into<smol_str::SmolStr>,
    ) -> error::RuntimeError {
        let err = error::RuntimeError::SimulationFault(message.into());
        self.apply_fault(err, self.faults.decision())
    }

    pub(super) fn apply_fault(
        &mut self,
        err: error::RuntimeError,
        decision: FaultDecision,
    ) -> error::RuntimeError {
        let mut reported = err.clone();
        if decision.apply_safe_state {
            if let Err(safe_state_err) = self.io.apply_safe_state() {
                reported = error::RuntimeError::SafeStateFailed {
                    root: smol_str::SmolStr::new(err.to_string()),
                    error: smol_str::SmolStr::new(safe_state_err.to_string()),
                };
                if let Some(debug) = &self.debug {
                    debug.push_runtime_event(crate::debug::RuntimeEvent::SafeStateFailed {
                        root: err.to_string(),
                        error: safe_state_err.to_string(),
                        time: self.current_time,
                    });
                }
            }
        }
        self.faults.record(err.clone());
        self.metrics.record_fault();
        if let Some(debug) = &self.debug {
            debug.push_runtime_event(crate::debug::RuntimeEvent::Fault {
                error: err.to_string(),
                time: self.current_time,
            });
        }
        reported
    }

}
