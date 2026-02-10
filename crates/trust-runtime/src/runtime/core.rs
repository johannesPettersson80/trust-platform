//! Core runtime state and accessors.

#![allow(missing_docs)]

use crate::debug::DebugControl;
use crate::eval::expr::Expr;
use crate::eval::{ClassDef, EvalContext, FunctionBlockDef, FunctionDef, InterfaceDef};
use crate::io::{IoDriver, IoDriverStatus, IoInterface, IoSafeState};
use crate::memory::{AccessMap, FrameId, InstanceId, VariableStorage};
use crate::metrics::RuntimeMetrics;
use crate::retain::{RetainManager, RetainStore};
use crate::stdlib::StandardLibrary;
use crate::task::{ProgramDef, TaskConfig, TaskState};
use crate::value::{DateTimeProfile, Duration, Value};
use crate::watchdog::{FaultDecision, FaultPolicy, WatchdogPolicy};
use crate::{error, eval, stdlib};
use indexmap::IndexMap;
use smol_str::SmolStr;
use trust_hir::types::TypeRegistry;
use trust_hir::Type;

use super::faults::FaultSubsystem;
use super::io_subsystem::IoSubsystem;
use super::metadata::{resolve_using_for_frame, RuntimeMetadata};
use super::metrics_subsystem::MetricsSubsystem;
use super::types::{GlobalInitValue, GlobalVarMeta, RetainPolicy};
use super::watchdog_subsystem::WatchdogSubsystem;

/// Minimal runtime entry point (extended later).
pub struct Runtime {
    pub(super) profile: DateTimeProfile,
    pub(super) storage: VariableStorage,
    pub(super) registry: TypeRegistry,
    pub(super) io: IoSubsystem,
    pub(super) access: AccessMap,
    pub(super) stdlib: StandardLibrary,
    pub(super) debug: Option<DebugControl>,
    pub(super) statement_index: IndexMap<u32, Vec<crate::debug::SourceLocation>>,
    pub(super) functions: IndexMap<SmolStr, FunctionDef>,
    pub(super) function_blocks: IndexMap<SmolStr, FunctionBlockDef>,
    pub(super) classes: IndexMap<SmolStr, ClassDef>,
    pub(super) interfaces: IndexMap<SmolStr, InterfaceDef>,
    pub(super) programs: IndexMap<SmolStr, ProgramDef>,
    pub(super) globals: IndexMap<SmolStr, GlobalVarMeta>,
    pub(super) tasks: Vec<TaskConfig>,
    pub(super) task_state: IndexMap<SmolStr, TaskState>,
    pub(super) task_thread_ids: IndexMap<SmolStr, u32>,
    pub(super) next_thread_id: u32,
    pub(super) background_thread_id: Option<u32>,
    pub(super) current_time: Duration,
    pub(super) cycle_counter: u64,
    pub(super) retain: RetainManager,
    pub(super) metrics: MetricsSubsystem,
    pub(super) watchdog: WatchdogSubsystem,
    pub(super) faults: FaultSubsystem,
    pub(super) execution_deadline: Option<std::time::Instant>,
}

impl std::fmt::Debug for Runtime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Runtime")
            .field("profile", &self.profile)
            .field("storage", &self.storage)
            .field("registry", &self.registry)
            .field("io", &"<io>")
            .field("access", &self.access)
            .field("stdlib", &self.stdlib)
            .field("debug", &self.debug.is_some())
            .field("statement_index", &self.statement_index)
            .field("functions", &self.functions)
            .field("function_blocks", &self.function_blocks)
            .field("classes", &self.classes)
            .field("interfaces", &self.interfaces)
            .field("programs", &self.programs)
            .field("globals", &self.globals)
            .field("tasks", &self.tasks)
            .field("task_state", &self.task_state)
            .field("current_time", &self.current_time)
            .field("cycle_counter", &self.cycle_counter)
            .field("faulted", &self.faults.is_faulted())
            .field("last_fault", &self.faults.last_fault())
            .finish()
    }
}
impl Runtime {
    /// Create a new runtime with default profile and empty storage.
    #[must_use]
    pub fn new() -> Self {
        let mut runtime = Self {
            profile: DateTimeProfile::default(),
            storage: VariableStorage::default(),
            registry: TypeRegistry::new(),
            io: IoSubsystem::new(),
            access: AccessMap::default(),
            stdlib: StandardLibrary::new(),
            debug: None,
            statement_index: IndexMap::new(),
            functions: IndexMap::new(),
            function_blocks: IndexMap::new(),
            classes: IndexMap::new(),
            interfaces: IndexMap::new(),
            programs: IndexMap::new(),
            globals: IndexMap::new(),
            tasks: Vec::new(),
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

    /// Update configured safe-state outputs.
    pub fn set_io_safe_state(&mut self, safe_state: IoSafeState) {
        self.io.set_safe_state(safe_state);
    }

    /// Attach a metrics sink for runtime statistics.
    pub fn set_metrics_sink(&mut self, metrics: std::sync::Arc<std::sync::Mutex<RuntimeMetrics>>) {
        self.metrics.set_sink(metrics);
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
        if decision.apply_safe_state {
            let _ = self.io.apply_safe_state();
        }
        self.faults.record(err.clone());
        self.metrics.record_fault();
        if let Some(debug) = &self.debug {
            debug.push_runtime_event(crate::debug::RuntimeEvent::Fault {
                error: err.to_string(),
                time: self.current_time,
            });
        }
        err
    }

    /// Get the current debug control handle, if set.
    #[must_use]
    pub fn debug_control(&self) -> Option<crate::debug::DebugControl> {
        self.debug.clone()
    }

    /// Register statement locations for a file id.
    pub fn register_statement_locations(
        &mut self,
        file_id: u32,
        locations: Vec<crate::debug::SourceLocation>,
    ) {
        self.statement_index.insert(file_id, locations);
    }

    /// Get the statement locations for a file id.
    #[must_use]
    pub fn statement_locations(&self, file_id: u32) -> Option<&[crate::debug::SourceLocation]> {
        self.statement_index.get(&file_id).map(Vec::as_slice)
    }

    /// Resolve a breakpoint to a statement location for the given file and source.
    #[must_use]
    pub fn resolve_breakpoint_location(
        &self,
        source: &str,
        file_id: u32,
        line: u32,
        column: u32,
    ) -> Option<crate::debug::SourceLocation> {
        let locations = self.statement_index.get(&file_id)?;
        crate::debug::resolve_breakpoint_location(source, file_id, locations, line, column)
    }

    /// Resolve a breakpoint and return its adjusted line/column.
    #[must_use]
    pub fn resolve_breakpoint_position(
        &self,
        source: &str,
        file_id: u32,
        line: u32,
        column: u32,
    ) -> Option<(crate::debug::SourceLocation, u32, u32)> {
        let location = self.resolve_breakpoint_location(source, file_id, line, column)?;
        let (resolved_line, resolved_col) = crate::debug::location_to_line_col(source, &location);
        Some((location, resolved_line, resolved_col))
    }

    /// Mutable access to variable storage (temporary API).
    pub fn storage_mut(&mut self) -> &mut VariableStorage {
        &mut self.storage
    }

    /// Access variable storage.
    #[must_use]
    pub fn storage(&self) -> &VariableStorage {
        &self.storage
    }

    #[must_use]
    /// Access the type registry.
    pub fn registry(&self) -> &TypeRegistry {
        &self.registry
    }

    /// Mutable access to the type registry.
    pub fn registry_mut(&mut self) -> &mut TypeRegistry {
        &mut self.registry
    }

    /// Access the registered functions.
    #[must_use]
    pub fn functions(&self) -> &IndexMap<SmolStr, FunctionDef> {
        &self.functions
    }

    /// Access the registered function blocks.
    #[must_use]
    pub fn function_blocks(&self) -> &IndexMap<SmolStr, FunctionBlockDef> {
        &self.function_blocks
    }

    /// Access the registered classes.
    #[must_use]
    pub fn classes(&self) -> &IndexMap<SmolStr, ClassDef> {
        &self.classes
    }

    /// Access the registered interfaces.
    #[must_use]
    pub fn interfaces(&self) -> &IndexMap<SmolStr, InterfaceDef> {
        &self.interfaces
    }

    /// Access the registered programs.
    #[must_use]
    pub fn programs(&self) -> &IndexMap<SmolStr, ProgramDef> {
        &self.programs
    }

    pub(crate) fn globals(&self) -> &IndexMap<SmolStr, GlobalVarMeta> {
        &self.globals
    }

    /// Access the standard library.
    #[must_use]
    pub fn stdlib(&self) -> &StandardLibrary {
        &self.stdlib
    }

    /// Register a function definition by name.
    pub fn register_function(&mut self, function: FunctionDef) {
        let key = function.name.to_ascii_uppercase();
        self.functions.insert(key.into(), function);
    }

    /// Register a function block definition by name.
    pub fn register_function_block(&mut self, function_block: FunctionBlockDef) {
        let key = function_block.name.to_ascii_uppercase();
        self.function_blocks.insert(key.into(), function_block);
    }

    /// Register a class definition by name.
    pub fn register_class(&mut self, class_def: ClassDef) {
        let key = class_def.name.to_ascii_uppercase();
        self.classes.insert(key.into(), class_def);
    }

    /// Register an interface definition by name.
    pub fn register_interface(&mut self, interface_def: InterfaceDef) {
        let key = interface_def.name.to_ascii_uppercase();
        self.interfaces.insert(key.into(), interface_def);
    }

    fn register_builtin_function_blocks(&mut self) {
        for fb in stdlib::fbs::standard_function_blocks() {
            if self.registry.lookup(fb.name.as_ref()).is_none() {
                let name = fb.name.clone();
                self.registry
                    .register(name.clone(), Type::FunctionBlock { name });
            }
            self.register_function_block(fb);
        }
    }

    /// Gets the current simulation time.
    #[must_use]
    pub fn current_time(&self) -> Duration {
        self.current_time
    }

    /// Access the I/O interface.
    pub fn io(&self) -> &IoInterface {
        self.io.interface()
    }

    /// Mutable access to the I/O interface.
    pub fn io_mut(&mut self) -> &mut IoInterface {
        self.io.interface_mut()
    }

    /// Register an I/O driver invoked at cycle boundaries.
    pub fn add_io_driver(&mut self, name: impl Into<SmolStr>, driver: Box<dyn IoDriver>) {
        self.io.add_driver(name, driver);
    }

    /// Clear all registered I/O drivers.
    pub fn clear_io_drivers(&mut self) {
        self.io.clear_drivers();
    }

    /// Set the sink for I/O driver health snapshots.
    pub fn set_io_health_sink(
        &mut self,
        sink: Option<std::sync::Arc<std::sync::Mutex<Vec<IoDriverStatus>>>>,
    ) {
        self.io.set_health_sink(sink);
    }

    pub(super) fn update_io_health(&self) {
        self.io.update_health();
    }

    /// Access the current cycle counter.
    #[must_use]
    pub fn cycle_counter(&self) -> u64 {
        self.cycle_counter
    }

    /// Returns the VAR_ACCESS binding map.
    #[must_use]
    pub fn access_map(&self) -> &AccessMap {
        &self.access
    }

    /// Returns a mutable VAR_ACCESS binding map.
    pub fn access_map_mut(&mut self) -> &mut AccessMap {
        &mut self.access
    }

    /// Resolve USING directives for the given frame id.
    #[must_use]
    pub fn using_for_frame(&self, frame_id: FrameId) -> Option<Vec<SmolStr>> {
        let frame = self
            .storage
            .frames()
            .iter()
            .find(|frame| frame.id == frame_id)?;
        resolve_using_for_frame(
            frame,
            &self.storage,
            &self.functions,
            &self.function_blocks,
            &self.classes,
            &self.programs,
        )
        .map(|using| using.to_vec())
    }

    /// Reads a VAR_ACCESS binding by name.
    #[must_use]
    pub fn read_access(&self, name: &str) -> Option<Value> {
        let binding = self.access.get(name)?;
        let value = self.storage.read_by_ref(binding.reference.clone())?.clone();
        if let Some(partial) = binding.partial {
            crate::value::read_partial_access(&value, partial).ok()
        } else {
            Some(value)
        }
    }

    /// Writes a VAR_ACCESS binding by name.
    pub fn write_access(&mut self, name: &str, value: Value) -> Result<(), error::RuntimeError> {
        let Some(binding) = self.access.get(name) else {
            return Err(error::RuntimeError::UndefinedVariable(name.into()));
        };
        if let Some(partial) = binding.partial {
            let current = self
                .storage
                .read_by_ref(binding.reference.clone())
                .cloned()
                .ok_or(error::RuntimeError::NullReference)?;
            let updated = crate::value::write_partial_access(current, partial, value).map_err(
                |err| match err {
                    crate::value::PartialAccessError::IndexOutOfBounds {
                        index,
                        lower,
                        upper,
                    } => error::RuntimeError::IndexOutOfBounds {
                        index,
                        lower,
                        upper,
                    },
                    crate::value::PartialAccessError::TypeMismatch => {
                        error::RuntimeError::TypeMismatch
                    }
                },
            )?;
            if self
                .storage
                .write_by_ref(binding.reference.clone(), updated)
            {
                Ok(())
            } else {
                Err(error::RuntimeError::NullReference)
            }
        } else if self.storage.write_by_ref(binding.reference.clone(), value) {
            Ok(())
        } else {
            Err(error::RuntimeError::NullReference)
        }
    }

    /// Evaluate a debug expression within the current runtime context.
    pub fn evaluate_expression(
        &mut self,
        expr: &Expr,
        frame_id: Option<FrameId>,
    ) -> Result<Value, error::RuntimeError> {
        let profile = self.profile;
        let now = self.current_time;
        let registry = &self.registry;
        let functions = &self.functions;
        let stdlib = &self.stdlib;
        let function_blocks = &self.function_blocks;
        let classes = &self.classes;
        let access = &self.access;
        let execution_deadline = self.execution_deadline;
        let eval = |storage: &mut VariableStorage, instance_id: Option<InstanceId>| {
            let mut ctx = EvalContext {
                storage,
                registry,
                profile,
                now,
                debug: None,
                call_depth: 0,
                functions: Some(functions),
                stdlib: Some(stdlib),
                function_blocks: Some(function_blocks),
                classes: Some(classes),
                using: None,
                access: Some(access),
                current_instance: instance_id,
                return_name: None,
                loop_depth: 0,
                pause_requested: false,
                execution_deadline,
            };
            eval::eval_expr(&mut ctx, expr)
        };

        if let Some(frame_id) = frame_id {
            self.storage
                .with_frame(frame_id, |storage| {
                    let instance_id = storage.current_frame().and_then(|frame| frame.instance_id);
                    eval(storage, instance_id)
                })
                .ok_or(error::RuntimeError::InvalidFrame(frame_id.0))?
        } else {
            eval(&mut self.storage, None)
        }
    }

    /// Run a closure with an evaluation context, optionally scoped to a frame.
    pub fn with_eval_context<T>(
        &mut self,
        frame_id: Option<FrameId>,
        using: Option<&[SmolStr]>,
        f: impl FnOnce(&mut EvalContext<'_>) -> Result<T, error::RuntimeError>,
    ) -> Result<T, error::RuntimeError> {
        let profile = self.profile;
        let now = self.current_time;
        let registry = &self.registry;
        let functions = &self.functions;
        let stdlib = &self.stdlib;
        let function_blocks = &self.function_blocks;
        let classes = &self.classes;
        let access = &self.access;
        let execution_deadline = self.execution_deadline;
        let eval = |storage: &mut VariableStorage, instance_id: Option<InstanceId>| {
            let mut ctx = EvalContext {
                storage,
                registry,
                profile,
                now,
                debug: None,
                call_depth: 0,
                functions: Some(functions),
                stdlib: Some(stdlib),
                function_blocks: Some(function_blocks),
                classes: Some(classes),
                using,
                access: Some(access),
                current_instance: instance_id,
                return_name: None,
                loop_depth: 0,
                pause_requested: false,
                execution_deadline,
            };
            f(&mut ctx)
        };

        if let Some(frame_id) = frame_id {
            self.storage
                .with_frame(frame_id, |storage| {
                    let instance_id = storage.current_frame().and_then(|frame| frame.instance_id);
                    eval(storage, instance_id)
                })
                .ok_or(error::RuntimeError::InvalidFrame(frame_id.0))?
        } else {
            eval(&mut self.storage, None)
        }
    }

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
            &program,
        )?;
        self.storage
            .set_global(program.name.clone(), Value::Instance(instance_id));
        self.programs.insert(program.name.clone(), program);
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

impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}
