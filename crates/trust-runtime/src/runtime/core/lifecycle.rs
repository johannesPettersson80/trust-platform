impl Runtime {
    /// Create a new runtime with default profile and empty storage.
    #[must_use]
    pub fn new() -> Self {
        let mut runtime = Self {
            resource_name: SmolStr::new("RESOURCE"),
            execution_backend: crate::execution_backend::ExecutionBackend::BytecodeVm,
            vm_module: None,
            profile: DateTimeProfile::default(),
            storage: VariableStorage::default(),
            registry: TypeRegistry::new(),
            initializer_catalog: InitializerCatalog::default(),
            ads: AdsSubsystem::new(),
            opcua_client: OpcUaClientSubsystem::new(),
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
            edge_inputs: IndexMap::new(),
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
            faults: FaultSubsystem::new(),
            openot_telemetry: OpenOtTelemetrySubsystem::default(),
            execution_deadline: None,
            output_commit_deadline: None,
            execution_deadline_pause_baseline: std::time::Duration::ZERO,
            output_commit_deadline_pause_baseline: std::time::Duration::ZERO,
            vm_local_init_plan_cache: super::vm::VmLocalInitPlanCacheState::default(),
            vm_register_lowering_cache: super::vm::RegisterLoweringCacheState::from_env(),
            vm_register_profile: super::vm::RegisterProfileState::default(),
            vm_execution_budget: trust_runtime_core::vm::hosted::budget::ExecutionBudget::new(
                trust_runtime_core::vm::VM_MAX_EXECUTED_INSTRUCTIONS,
            ),
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

    /// Snapshot static metadata for external tooling.
    #[must_use]
    pub fn metadata_snapshot(&self) -> RuntimeMetadata {
        RuntimeMetadata {
            storage_abi: crate::value::StorageAbi::LegacyTemporal32,
            profile: self.profile,
            registry: self.registry.clone(),
            artifact_types: Vec::new(),
            io_display_names: Vec::new(),
            retained_bindings: IndexMap::new(),
            global_declarations: IndexMap::new(),
            stdlib: self.stdlib.clone(),
            access: self.access.clone(),
            functions: self.functions.clone(),
            function_blocks: self.function_blocks.clone(),
            classes: self.classes.clone(),
            interfaces: self.interfaces.clone(),
            programs: self.programs.clone(),
            program_templates: self.programs.clone(),
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
            source_text_index: self.source_text_index.clone(),
            source_label_index: self.source_label_index.clone(),
        }
    }

    /// Set an optional execution deadline enforced by the evaluator.
    pub fn set_execution_deadline(&mut self, deadline: Option<std::time::Instant>) {
        self.execution_deadline = deadline;
        self.execution_deadline_pause_baseline = self.debug_watchdog_pause_elapsed();
    }

    /// Get the current execution deadline.
    #[must_use]
    pub fn execution_deadline(&self) -> Option<std::time::Instant> {
        self.execution_deadline
    }

    pub(crate) fn effective_execution_deadline(&self) -> Option<std::time::Instant> {
        self.deadline_with_debug_pause(
            self.execution_deadline,
            self.execution_deadline_pause_baseline,
        )
    }

    pub(crate) fn set_output_commit_deadline(&mut self, deadline: Option<std::time::Instant>) {
        self.output_commit_deadline = deadline;
        self.output_commit_deadline_pause_baseline = self.debug_watchdog_pause_elapsed();
    }

    #[cfg(test)]
    #[must_use]
    pub(crate) fn output_commit_deadline(&self) -> Option<std::time::Instant> {
        self.output_commit_deadline
    }

    pub(crate) fn effective_output_commit_deadline(&self) -> Option<std::time::Instant> {
        self.deadline_with_debug_pause(
            self.output_commit_deadline,
            self.output_commit_deadline_pause_baseline,
        )
    }

    pub(crate) fn debug_watchdog_pause_elapsed(&self) -> std::time::Duration {
        self.debug
            .as_ref()
            .map(crate::debug::DebugControl::watchdog_pause_elapsed)
            .unwrap_or_default()
    }

    fn deadline_with_debug_pause(
        &self,
        deadline: Option<std::time::Instant>,
        baseline: std::time::Duration,
    ) -> Option<std::time::Instant> {
        let pause_delta = self.debug_watchdog_pause_elapsed().saturating_sub(baseline);
        deadline.map(|value| value.checked_add(pause_delta).unwrap_or(value))
    }

    /// Attach a metrics sink for runtime statistics.
    pub fn set_metrics_sink(&mut self, metrics: std::sync::Arc<std::sync::Mutex<RuntimeMetrics>>) {
        self.metrics.set_sink(metrics);
        self.metrics.set_execution_backend(self.execution_backend);
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
        if let Some(debug) = &self.debug {
            debug.clear_runtime_mutations();
        }
        let mut reported = err.clone();
        if decision.apply_safe_state
            && let Err(safe_state_err) = self.io.apply_safe_state()
        {
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
