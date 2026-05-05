//! Core runtime state and accessors.

#![allow(missing_docs)]

use crate::debug::DebugControl;
use crate::execution_backend::ExecutionBackend;
use crate::io::{IoDriver, IoDriverStatus, IoInterface, IoSafeState};
use crate::memory::{AccessMap, FrameId, InstanceId, VariableStorage};
use crate::metrics::RuntimeMetrics;
use crate::program_model::{
    ClassDef, FunctionBlockDef, FunctionDef, InitializerCatalog, InterfaceDef,
};
use crate::program_model::{Expr, LValue};
use crate::retain::{RetainManager, RetainStore};
use crate::stdlib::StandardLibrary;
use crate::task::{ProgramDef, TaskConfig, TaskState};
use crate::value::{DateTimeProfile, Duration, Value};
use crate::watchdog::{FaultDecision, FaultPolicy, WatchdogPolicy};
use crate::{error, stdlib};
use indexmap::IndexMap;
use smol_str::SmolStr;
use std::collections::HashMap;
use std::sync::Arc;
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
    pub(super) execution_backend: ExecutionBackend,
    pub(super) vm_module: Option<Arc<super::vm::VmModule>>,
    pub(super) profile: DateTimeProfile,
    pub(super) storage: VariableStorage,
    pub(super) registry: TypeRegistry,
    pub(super) initializer_catalog: InitializerCatalog,
    pub(super) io: IoSubsystem,
    pub(super) access: AccessMap,
    pub(super) stdlib: StandardLibrary,
    pub(super) debug: Option<DebugControl>,
    pub(super) statement_index: IndexMap<u32, Vec<crate::debug::SourceLocation>>,
    pub(super) source_text_index: IndexMap<u32, String>,
    pub(super) source_label_index: HashMap<SmolStr, u32>,
    pub(super) functions: IndexMap<SmolStr, FunctionDef>,
    pub(super) function_blocks: IndexMap<SmolStr, FunctionBlockDef>,
    pub(super) classes: IndexMap<SmolStr, ClassDef>,
    pub(super) interfaces: IndexMap<SmolStr, InterfaceDef>,
    pub(super) programs: IndexMap<SmolStr, ProgramDef>,
    pub(super) globals: IndexMap<SmolStr, GlobalVarMeta>,
    pub(super) tasks: Vec<TaskConfig>,
    pub(super) ready_tasks_scratch: Vec<super::types::ReadyTask>,
    pub(super) background_program_names_scratch: Vec<SmolStr>,
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
    pub(super) output_commit_deadline: Option<std::time::Instant>,
    pub(super) vm_local_init_plan_cache: super::vm::VmLocalInitPlanCacheState,
    pub(super) vm_register_lowering_cache: super::vm::RegisterLoweringCacheState,
    pub(super) vm_register_profile: super::vm::RegisterProfileState,
    pub(super) vm_tier1_specialized_executor: super::vm::RegisterTier1SpecializedExecutorState,
}

impl std::fmt::Debug for Runtime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Runtime")
            .field("execution_backend", &self.execution_backend)
            .field("vm_module_loaded", &self.vm_module.is_some())
            .field("profile", &self.profile)
            .field("storage", &self.storage)
            .field("registry", &self.registry)
            .field("initializer_catalog", &self.initializer_catalog)
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
            .field(
                "vm_register_lowering_cache_enabled",
                &self.vm_register_lowering_cache.snapshot().enabled,
            )
            .field(
                "vm_tier1_specialized_executor_enabled",
                &self.vm_tier1_specialized_executor.snapshot().enabled,
            )
            .finish()
    }
}

include!("core/lifecycle.rs");
include!("core/diagnostics.rs");
include!("core/accessors.rs");
include!("core/evaluation.rs");
include!("core/scheduling.rs");

impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}
