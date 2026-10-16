use alloc::{string::String, vec::Vec};

/// Immutable optimization-preparation outcome for one admitted module.
/// Resetting runtime counters never changes this installation evidence.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VmRegisterPreparationSnapshot {
    /// POU bodies considered during preparation.
    pub pou_count: usize,
    /// Successfully lowered register bodies, including bodies with stack fallback instructions.
    pub lowered_pous: usize,
    /// Bodies whose lowering failed and therefore retain the shared stack fallback.
    pub lowering_failures: usize,
    /// Specialized blocks compiled before installation.
    pub specialized_blocks: usize,
    /// Blocks whose specialization was refused and retain the general shared path.
    pub specialization_failures: usize,
    /// Prepared refusal reasons in lexical order; independent of resettable RUN counters.
    pub specialization_failure_reasons: Vec<VmTier1SpecializedExecutorCompileFailureReason>,
}

/// VM register-executor profiling snapshot captured from runtime state.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VmRegisterProfileSnapshot {
    /// Whether this observation or execution facility is enabled.
    pub enabled: bool,
    /// POU entries completed through the register representation.
    pub register_programs_executed: u64,
    /// POU entries routed to the shared stack fallback.
    pub register_program_fallbacks: u64,
    /// Observed fallback causes and their counts.
    pub fallback_reasons: Vec<VmRegisterFallbackReason>,
    /// Observed register-block entry counts.
    pub hot_blocks: Vec<VmRegisterHotBlock>,
    /// Block entries run by the interpreter, counted apart from Tier-1 block executions.
    pub interpreted_block_entries: u64,
    /// Reference operations observed since the last profile reset.
    pub ref_ops: VmRegisterRefOpCounters,
    /// Call and frame operations observed since the last profile reset.
    pub call_ops: VmRegisterCallOpCounters,
    /// Value ownership operations observed since the last profile reset.
    pub value_ops: VmRegisterValueOpCounters,
}

/// Fallback reason counter for register-executor eligibility.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VmRegisterFallbackReason {
    /// Stable preparation, fallback or deoptimization reason.
    pub reason: String,
    /// Number of observations for this reason.
    pub count: u64,
}

/// Per-block hotness counter in register-executor runs.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VmRegisterHotBlock {
    /// Admitted POU identity.
    pub pou_id: u32,
    /// Block identity within the POU.
    pub block_id: u32,
    /// Original bytecode position of the block.
    pub start_pc: u32,
    /// Observed entries or successful prepared-plan lookups.
    pub hits: u64,
}

/// Ref-operation counters captured from register-executor runs.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VmRegisterRefOpCounters {
    /// Static-reference value loads.
    pub load_ref: u64,
    /// Static-reference value stores.
    pub store_ref: u64,
    /// Owned reference-address materializations.
    pub load_ref_addr: u64,
    /// Dynamic field reference selections.
    pub ref_field: u64,
    /// Dynamic index reference selections.
    pub ref_index: u64,
    /// Dynamic-reference value loads.
    pub load_dynamic: u64,
    /// Dynamic-reference value stores.
    pub store_dynamic: u64,
    /// Instance member-resolution operations.
    pub instance_field_lookups: u64,
}

/// Call/frame-operation counters captured from VM execution paths.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VmRegisterCallOpCounters {
    /// Execution frames entered.
    pub frame_pushes: u64,
    /// Execution frames retired.
    pub frame_pops: u64,
    /// Function-block call entries.
    pub function_block_call_entries: u64,
    /// Call parameter bindings applied.
    pub parameter_bindings: u64,
    /// Call output values copied back to destinations.
    pub output_copy_backs: u64,
}

/// Value-clone/move counters captured from VM execution paths.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VmRegisterValueOpCounters {
    /// Materialized constant-value copies.
    pub const_load_clones: u64,
    /// Register payload reads requiring a copy.
    pub register_read_clones: u64,
    /// Register payload reads transferring ownership.
    pub register_read_moves: u64,
    /// Storage/reference reads requiring a value copy.
    pub read_value_clones: u64,
    /// Argument binding copies.
    pub binding_expr_clones: u64,
    /// Output publication/copy-back value copies.
    pub output_value_clones: u64,
}

/// Tier-1 specialized register-executor runtime snapshot.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VmTier1SpecializedExecutorSnapshot {
    /// Whether this observation or execution facility is enabled.
    pub enabled: bool,
    /// Hits required before selecting an existing specialized plan.
    pub hot_block_threshold: u64,
    /// Configured maximum active selection count; plans remain prepared.
    pub cache_capacity: usize,
    /// Currently selected specialized blocks.
    pub cached_blocks: usize,
    /// Preparation compilation attempts observed since reset.
    pub compile_attempts: u64,
    /// Successful preparation compilations observed since reset.
    pub compile_successes: u64,
    /// Rejected preparation compilations observed since reset.
    pub compile_failures: u64,
    /// Observed preparation failure reasons.
    pub compile_failure_reasons: Vec<VmTier1SpecializedExecutorCompileFailureReason>,
    /// Prepared block selections evicted from the active LRU.
    pub cache_evictions: u64,
    /// Completed specialized-block executions.
    pub block_executions: u64,
}

/// Tier-1 specialized register-executor compile failure reason counter.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VmTier1SpecializedExecutorCompileFailureReason {
    /// Stable preparation, fallback or deoptimization reason.
    pub reason: String,
    /// Number of observations for this reason.
    pub count: u64,
}

/// Register-IR lowering cache snapshot captured from runtime state.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VmRegisterLoweringCacheSnapshot {
    /// Whether this observation or execution facility is enabled.
    pub enabled: bool,
    /// Configured maximum active selection count; plans remain prepared.
    pub cache_capacity: usize,
    /// Immutable lowered POU entries available after preparation.
    pub cached_entries: usize,
    /// Observed entries or successful prepared-plan lookups.
    pub hits: u64,
    /// Preparation entries observed since reset, not lazy RUN compilation.
    pub misses: u64,
    /// Preparation lowering errors observed since reset.
    pub build_errors: u64,
    /// Prepared block selections evicted from the active LRU.
    pub cache_evictions: u64,
    /// Prepared-entry observations invalidated by lifecycle changes.
    pub invalidations: u64,
}
