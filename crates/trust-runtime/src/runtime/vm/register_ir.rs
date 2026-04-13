#![allow(dead_code)]

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};
use std::sync::Arc;
use std::time::Instant;

use crate::debug::DebugHook;
use crate::error::RuntimeError;
use crate::eval::ops::{apply_binary, apply_unary, BinaryOp, UnaryOp};
use crate::execution_backend::{
    VmRegisterCallOpCounters, VmRegisterFallbackReason, VmRegisterHotBlock,
    VmRegisterLoweringCacheSnapshot, VmRegisterProfileSnapshot, VmRegisterRefOpCounters,
    VmRegisterValueOpCounters, VmTier1SpecializedExecutorCompileFailureReason,
    VmTier1SpecializedExecutorDeoptReason, VmTier1SpecializedExecutorSnapshot,
};
use crate::memory::InstanceId;
use crate::value::{size_of_value, Value};

use super::super::core::Runtime;
use super::call::{execute_native_call, push_call_frame};
use super::dispatch_refs::{
    dynamic_ref_field, dynamic_ref_index, dynamic_store_ref, index_to_i64, load_ref_addr,
    peek_dynamic_ref, peek_ref, store_ref,
};
use super::dispatch_sizeof::{sizeof_error_to_runtime, sizeof_type_from_table};
use super::errors::VmTrap;
use super::frames::{ensure_global_call_depth, FrameStack};
use super::stack::OperandStack;
use super::{invalid_bytecode, materialize_borrowed_value, opcode_operand_len, VmModule};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(super) struct RegisterId(u32);

impl RegisterId {
    pub(super) fn index(self) -> u32 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum BlockTarget {
    Block(u32),
    Exit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum RegisterInstr {
    Nop,
    LoadConst {
        dest: RegisterId,
        const_idx: u32,
    },
    LoadRef {
        dest: RegisterId,
        ref_idx: u32,
    },
    LoadRefAddr {
        dest: RegisterId,
        ref_idx: u32,
    },
    StoreRef {
        ref_idx: u32,
        src: RegisterId,
    },
    LoadNull {
        dest: RegisterId,
    },
    LoadSelf {
        dest: RegisterId,
    },
    LoadSuper {
        dest: RegisterId,
    },
    Move {
        src: RegisterId,
        dest: RegisterId,
    },
    CallNative {
        kind: u32,
        symbol_idx: u32,
        args: Vec<RegisterId>,
        dest: RegisterId,
    },
    SizeOfType {
        type_idx: u32,
        dest: RegisterId,
    },
    SizeOfValue {
        src: RegisterId,
        dest: RegisterId,
    },
    RefField {
        base: RegisterId,
        field_idx: u32,
        dest: RegisterId,
    },
    RefIndex {
        base: RegisterId,
        index: RegisterId,
        dest: RegisterId,
    },
    LoadDynamic {
        reference: RegisterId,
        dest: RegisterId,
    },
    StoreDynamic {
        reference: RegisterId,
        value: RegisterId,
    },
    Unary {
        op: UnaryOp,
        src: RegisterId,
        dest: RegisterId,
    },
    Binary {
        op: BinaryOp,
        left: RegisterId,
        right: RegisterId,
        dest: RegisterId,
    },
    BinaryRefToRef {
        op: BinaryOp,
        left_ref_idx: u32,
        right_ref_idx: u32,
        dest_ref_idx: u32,
    },
    BinaryRefConstToRef {
        op: BinaryOp,
        left_ref_idx: u32,
        const_idx: u32,
        dest_ref_idx: u32,
    },
    BinaryConstRefToRef {
        op: BinaryOp,
        const_idx: u32,
        right_ref_idx: u32,
        dest_ref_idx: u32,
    },
    CmpRefConstJumpIf {
        op: BinaryOp,
        ref_idx: u32,
        const_idx: u32,
        jump_if_true: bool,
        target: BlockTarget,
    },
    Jump {
        target: BlockTarget,
    },
    JumpIf {
        cond: RegisterId,
        jump_if_true: bool,
        target: BlockTarget,
    },
    Return,
    VmFallback {
        opcode: u8,
        operands: Vec<u8>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RegisterBlock {
    pub(super) id: u32,
    pub(super) start_pc: usize,
    pub(super) end_pc: usize,
    pub(super) entry_stack_depth: u32,
    pub(super) instructions: Vec<RegisterInstr>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RegisterProgram {
    pub(super) pou_id: u32,
    pub(super) entry_block: u32,
    pub(super) max_registers: u32,
    pub(super) blocks: Vec<RegisterBlock>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RegisterExecutionOutcome {
    Executed,
    FallbackToStack,
}

#[derive(Debug, Clone)]
pub(super) struct RegisterPouExecutionResult {
    pub(super) return_value: Option<Value>,
    pub(super) locals: Vec<Value>,
}

const REGISTER_DEADLINE_CHECK_STRIDE: usize = 32;
const REGISTER_EXECUTION_POOL_LIMIT: usize = 64;

thread_local! {
    static VM_REGISTER_FILE_POOL: RefCell<Vec<Vec<Value>>> = const { RefCell::new(Vec::new()) };
    static VM_REGISTER_READ_COUNTS_POOL: RefCell<Vec<Vec<u32>>> = const { RefCell::new(Vec::new()) };
    static VM_REGISTER_NATIVE_CALL_STACK_POOL: RefCell<Vec<OperandStack>> = const { RefCell::new(Vec::new()) };
}

#[derive(Debug)]
struct RegisterExecutionBuffers {
    registers: Option<Vec<Value>>,
    remaining_register_reads: Option<Vec<u32>>,
    native_call_stack: Option<OperandStack>,
}

impl RegisterExecutionBuffers {
    fn acquire(max_registers: usize) -> Self {
        let mut registers = VM_REGISTER_FILE_POOL
            .with(|pool| pool.borrow_mut().pop())
            .unwrap_or_default();
        registers.resize(max_registers, Value::Null);
        registers.fill(Value::Null);
        let mut remaining_register_reads = VM_REGISTER_READ_COUNTS_POOL
            .with(|pool| pool.borrow_mut().pop())
            .unwrap_or_default();
        remaining_register_reads.resize(max_registers, 0);
        remaining_register_reads.fill(0);
        let native_call_stack = VM_REGISTER_NATIVE_CALL_STACK_POOL
            .with(|pool| pool.borrow_mut().pop())
            .unwrap_or_default();
        Self {
            registers: Some(registers),
            remaining_register_reads: Some(remaining_register_reads),
            native_call_stack: Some(native_call_stack),
        }
    }

    fn buffers_mut(&mut self) -> (&mut [Value], &mut [u32], &mut OperandStack) {
        let registers = self
            .registers
            .as_mut()
            .expect("register execution buffers missing register file");
        let remaining_register_reads = self
            .remaining_register_reads
            .as_mut()
            .expect("register execution buffers missing remaining reads");
        let native_call_stack = self
            .native_call_stack
            .as_mut()
            .expect("register execution buffers missing native-call stack");
        (
            registers.as_mut_slice(),
            remaining_register_reads.as_mut_slice(),
            native_call_stack,
        )
    }
}

impl Drop for RegisterExecutionBuffers {
    fn drop(&mut self) {
        if let Some(mut registers) = self.registers.take() {
            registers.clear();
            VM_REGISTER_FILE_POOL.with(|pool| {
                let mut pool = pool.borrow_mut();
                if pool.len() < REGISTER_EXECUTION_POOL_LIMIT {
                    pool.push(registers);
                }
            });
        }
        if let Some(mut remaining_register_reads) = self.remaining_register_reads.take() {
            remaining_register_reads.clear();
            VM_REGISTER_READ_COUNTS_POOL.with(|pool| {
                let mut pool = pool.borrow_mut();
                if pool.len() < REGISTER_EXECUTION_POOL_LIMIT {
                    pool.push(remaining_register_reads);
                }
            });
        }
        if let Some(mut native_call_stack) = self.native_call_stack.take() {
            native_call_stack.clear();
            VM_REGISTER_NATIVE_CALL_STACK_POOL.with(|pool| {
                let mut pool = pool.borrow_mut();
                if pool.len() < REGISTER_EXECUTION_POOL_LIMIT {
                    pool.push(native_call_stack);
                }
            });
        }
    }
}

#[derive(Debug, Clone, Default)]
pub(in crate::runtime) struct RegisterProfileState {
    enabled: bool,
    register_programs_executed: u64,
    register_program_fallbacks: u64,
    fallback_reasons: BTreeMap<String, u64>,
    block_hits: BTreeMap<(u32, u32, u32), u64>,
    ref_ops: VmRegisterRefOpCounters,
    call_ops: VmRegisterCallOpCounters,
    value_ops: VmRegisterValueOpCounters,
}

impl RegisterProfileState {
    pub(in crate::runtime) fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub(in crate::runtime) fn reset(&mut self) {
        self.register_programs_executed = 0;
        self.register_program_fallbacks = 0;
        self.fallback_reasons.clear();
        self.block_hits.clear();
        self.ref_ops = VmRegisterRefOpCounters::default();
        self.call_ops = VmRegisterCallOpCounters::default();
        self.value_ops = VmRegisterValueOpCounters::default();
    }

    pub(in crate::runtime) fn snapshot(&self) -> VmRegisterProfileSnapshot {
        let fallback_reasons = self
            .fallback_reasons
            .iter()
            .map(|(reason, count)| VmRegisterFallbackReason {
                reason: reason.clone(),
                count: *count,
            })
            .collect();
        let hot_blocks = self
            .block_hits
            .iter()
            .map(|((pou_id, block_id, start_pc), hits)| VmRegisterHotBlock {
                pou_id: *pou_id,
                block_id: *block_id,
                start_pc: *start_pc,
                hits: *hits,
            })
            .collect();
        VmRegisterProfileSnapshot {
            enabled: self.enabled,
            register_programs_executed: self.register_programs_executed,
            register_program_fallbacks: self.register_program_fallbacks,
            fallback_reasons,
            hot_blocks,
            ref_ops: self.ref_ops.clone(),
            call_ops: self.call_ops.clone(),
            value_ops: self.value_ops.clone(),
        }
    }

    fn record_executed(&mut self) {
        if !self.enabled {
            return;
        }
        self.register_programs_executed = self.register_programs_executed.saturating_add(1);
    }

    fn record_fallback(&mut self, reason: impl Into<String>) {
        if !self.enabled {
            return;
        }
        self.register_program_fallbacks = self.register_program_fallbacks.saturating_add(1);
        let reason = reason.into();
        let entry = self.fallback_reasons.entry(reason).or_insert(0);
        *entry = entry.saturating_add(1);
    }

    fn record_block_hit(&mut self, pou_id: u32, block_id: u32, start_pc: u32) {
        if !self.enabled {
            return;
        }
        let entry = self
            .block_hits
            .entry((pou_id, block_id, start_pc))
            .or_insert(0);
        *entry = entry.saturating_add(1);
    }

    pub(super) fn record_ref_op(&mut self, kind: RegisterRefOpKind) {
        if !self.enabled {
            return;
        }
        let counter = match kind {
            RegisterRefOpKind::LoadRef => &mut self.ref_ops.load_ref,
            RegisterRefOpKind::StoreRef => &mut self.ref_ops.store_ref,
            RegisterRefOpKind::LoadRefAddr => &mut self.ref_ops.load_ref_addr,
            RegisterRefOpKind::RefField => &mut self.ref_ops.ref_field,
            RegisterRefOpKind::RefIndex => &mut self.ref_ops.ref_index,
            RegisterRefOpKind::LoadDynamic => &mut self.ref_ops.load_dynamic,
            RegisterRefOpKind::StoreDynamic => &mut self.ref_ops.store_dynamic,
            RegisterRefOpKind::InstanceFieldLookup => &mut self.ref_ops.instance_field_lookups,
        };
        *counter = counter.saturating_add(1);
    }

    pub(super) fn record_call_op(&mut self, kind: RegisterCallOpKind) {
        if !self.enabled {
            return;
        }
        let counter = match kind {
            RegisterCallOpKind::FramePush => &mut self.call_ops.frame_pushes,
            RegisterCallOpKind::FramePop => &mut self.call_ops.frame_pops,
            RegisterCallOpKind::FunctionBlockCallEntry => {
                &mut self.call_ops.function_block_call_entries
            }
            RegisterCallOpKind::ParameterBinding => &mut self.call_ops.parameter_bindings,
            RegisterCallOpKind::OutputCopyBack => &mut self.call_ops.output_copy_backs,
        };
        *counter = counter.saturating_add(1);
    }

    pub(super) fn record_value_op(&mut self, kind: RegisterValueOpKind) {
        if !self.enabled {
            return;
        }
        let counter = match kind {
            RegisterValueOpKind::ConstLoadClone => &mut self.value_ops.const_load_clones,
            RegisterValueOpKind::RegisterReadClone => &mut self.value_ops.register_read_clones,
            RegisterValueOpKind::RegisterReadMove => &mut self.value_ops.register_read_moves,
            RegisterValueOpKind::ReadValueClone => &mut self.value_ops.read_value_clones,
            RegisterValueOpKind::BindingExprClone => &mut self.value_ops.binding_expr_clones,
            RegisterValueOpKind::OutputValueClone => &mut self.value_ops.output_value_clones,
        };
        *counter = counter.saturating_add(1);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RegisterRefOpKind {
    LoadRef,
    StoreRef,
    LoadRefAddr,
    RefField,
    RefIndex,
    LoadDynamic,
    StoreDynamic,
    InstanceFieldLookup,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RegisterCallOpKind {
    FramePush,
    FramePop,
    FunctionBlockCallEntry,
    ParameterBinding,
    OutputCopyBack,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RegisterValueOpKind {
    ConstLoadClone,
    RegisterReadClone,
    RegisterReadMove,
    ReadValueClone,
    BindingExprClone,
    OutputValueClone,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
struct RegisterLoweringCacheKey {
    module_ptr: usize,
    pou_id: u32,
}

#[derive(Debug, Clone)]
struct CachedRegisterProgram {
    program: Arc<RegisterProgram>,
    register_read_counts_by_block: Arc<Vec<Vec<u32>>>,
    block_has_register_reads: Arc<Vec<bool>>,
    fallback_opcode: Option<u8>,
}

#[derive(Debug, Clone)]
enum RegisterLoweringCacheEntry {
    Ready(CachedRegisterProgram),
    LoweringError { message: String },
}

#[derive(Debug, Clone)]
pub(in crate::runtime) struct RegisterLoweringCacheState {
    enabled: bool,
    cache_capacity: usize,
    entries: BTreeMap<RegisterLoweringCacheKey, Arc<RegisterLoweringCacheEntry>>,
    entry_order: VecDeque<RegisterLoweringCacheKey>,
    hits: u64,
    misses: u64,
    build_errors: u64,
    cache_evictions: u64,
    invalidations: u64,
}

impl Default for RegisterLoweringCacheState {
    fn default() -> Self {
        Self {
            enabled: true,
            cache_capacity: 256,
            entries: BTreeMap::new(),
            entry_order: VecDeque::new(),
            hits: 0,
            misses: 0,
            build_errors: 0,
            cache_evictions: 0,
            invalidations: 0,
        }
    }
}

impl RegisterLoweringCacheState {
    pub(in crate::runtime) fn from_env() -> Self {
        let mut state = Self::default();
        state.enabled = parse_env_bool("TRUST_VM_REGISTER_LOWERING_CACHE", state.enabled);
        state.cache_capacity =
            parse_env_usize("TRUST_VM_REGISTER_LOWERING_CACHE_CAP", state.cache_capacity).max(1);
        state
    }

    pub(in crate::runtime) fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub(in crate::runtime) fn reset(&mut self) {
        self.invalidate_all();
        self.hits = 0;
        self.misses = 0;
        self.build_errors = 0;
        self.cache_evictions = 0;
        self.invalidations = 0;
    }

    pub(in crate::runtime) fn invalidate_all(&mut self) {
        let removed = self.entries.len() as u64;
        if removed > 0 {
            self.invalidations = self.invalidations.saturating_add(removed);
        }
        self.entries.clear();
        self.entry_order.clear();
    }

    pub(in crate::runtime) fn snapshot(&self) -> VmRegisterLoweringCacheSnapshot {
        VmRegisterLoweringCacheSnapshot {
            enabled: self.enabled,
            cache_capacity: self.cache_capacity,
            cached_entries: self.entries.len(),
            hits: self.hits,
            misses: self.misses,
            build_errors: self.build_errors,
            cache_evictions: self.cache_evictions,
            invalidations: self.invalidations,
        }
    }

    fn get_or_build(&mut self, module: &VmModule, pou_id: u32) -> Arc<RegisterLoweringCacheEntry> {
        let key = RegisterLoweringCacheKey {
            module_ptr: module as *const VmModule as usize,
            pou_id,
        };
        if self.enabled {
            if let Some(entry) = self.entries.get(&key).cloned() {
                self.hits = self.hits.saturating_add(1);
                self.touch_entry(key);
                return entry;
            }
        }

        self.misses = self.misses.saturating_add(1);
        let built = match build_cached_register_program(module, pou_id) {
            Ok(program) => Arc::new(RegisterLoweringCacheEntry::Ready(program)),
            Err(err) => {
                self.build_errors = self.build_errors.saturating_add(1);
                Arc::new(RegisterLoweringCacheEntry::LoweringError {
                    message: err.to_string(),
                })
            }
        };

        if self.enabled {
            self.insert_entry(key, Arc::clone(&built));
        }
        built
    }

    fn touch_entry(&mut self, key: RegisterLoweringCacheKey) {
        self.entry_order.retain(|entry| *entry != key);
        self.entry_order.push_back(key);
    }

    fn insert_entry(
        &mut self,
        key: RegisterLoweringCacheKey,
        entry: Arc<RegisterLoweringCacheEntry>,
    ) {
        if self.entries.insert(key, entry).is_some() {
            self.entry_order.retain(|existing| *existing != key);
        }
        self.entry_order.push_back(key);

        while self.entries.len() > self.cache_capacity {
            if let Some(evicted) = self.entry_order.pop_front() {
                if self.entries.remove(&evicted).is_some() {
                    self.cache_evictions = self.cache_evictions.saturating_add(1);
                }
            } else {
                break;
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
struct Tier1BlockKey {
    module_ptr: usize,
    pou_id: u32,
    block_id: u32,
    start_pc: u32,
}

#[derive(Debug, Clone)]
struct Tier1CompiledBlock {
    key: Tier1BlockKey,
    instructions: Vec<Tier1CompiledInstr>,
}

#[derive(Debug, Clone)]
enum Tier1CompiledInstr {
    Nop,
    LoadConst {
        dest: RegisterId,
        value: Value,
    },
    LoadNull {
        dest: RegisterId,
    },
    LoadSelf {
        dest: RegisterId,
    },
    LoadSuper {
        dest: RegisterId,
    },
    Move {
        src: RegisterId,
        dest: RegisterId,
    },
    CallNative {
        kind: u32,
        symbol_idx: u32,
        args: Box<[RegisterId]>,
        dest: RegisterId,
    },
    LoadRef {
        dest: RegisterId,
        ref_idx: u32,
    },
    LoadRefAddr {
        dest: RegisterId,
        ref_idx: u32,
    },
    StoreRef {
        ref_idx: u32,
        src: RegisterId,
    },
    RefField {
        base: RegisterId,
        field: smol_str::SmolStr,
        dest: RegisterId,
    },
    RefIndex {
        base: RegisterId,
        index: RegisterId,
        dest: RegisterId,
    },
    LoadDynamic {
        reference: RegisterId,
        dest: RegisterId,
    },
    StoreDynamic {
        reference: RegisterId,
        value: RegisterId,
    },
    Unary {
        op: UnaryOp,
        src: RegisterId,
        dest: RegisterId,
    },
    BinaryDIntGuard {
        op: BinaryOp,
        left: RegisterId,
        right: RegisterId,
        dest: RegisterId,
    },
    BinaryRefToRefDIntGuard {
        op: BinaryOp,
        left_ref_idx: u32,
        right_ref_idx: u32,
        dest_ref_idx: u32,
    },
    BinaryRefConstToRefDIntGuard {
        op: BinaryOp,
        left_ref_idx: u32,
        const_idx: u32,
        dest_ref_idx: u32,
    },
    BinaryConstRefToRefDIntGuard {
        op: BinaryOp,
        const_idx: u32,
        right_ref_idx: u32,
        dest_ref_idx: u32,
    },
    CmpRefConstJumpIfDIntGuard {
        op: BinaryOp,
        ref_idx: u32,
        const_idx: u32,
        jump_if_true: bool,
        target: BlockTarget,
    },
    Jump {
        target: BlockTarget,
    },
    JumpIf {
        cond: RegisterId,
        jump_if_true: bool,
        target: BlockTarget,
    },
    Return,
}

#[derive(Debug, Clone)]
pub(in crate::runtime) struct RegisterTier1SpecializedExecutorState {
    enabled: bool,
    hot_block_threshold: u64,
    cache_capacity: usize,
    block_hits: BTreeMap<Tier1BlockKey, u64>,
    compiled_order: VecDeque<Tier1BlockKey>,
    compiled_blocks: BTreeMap<Tier1BlockKey, Arc<Tier1CompiledBlock>>,
    compile_attempts: u64,
    compile_successes: u64,
    compile_failures: u64,
    compile_failure_reasons: BTreeMap<String, u64>,
    cache_evictions: u64,
    block_executions: u64,
    deopt_count: u64,
    deopt_reasons: BTreeMap<String, u64>,
}

impl Default for RegisterTier1SpecializedExecutorState {
    fn default() -> Self {
        Self {
            enabled: false,
            hot_block_threshold: 64,
            cache_capacity: 128,
            block_hits: BTreeMap::new(),
            compiled_order: VecDeque::new(),
            compiled_blocks: BTreeMap::new(),
            compile_attempts: 0,
            compile_successes: 0,
            compile_failures: 0,
            compile_failure_reasons: BTreeMap::new(),
            cache_evictions: 0,
            block_executions: 0,
            deopt_count: 0,
            deopt_reasons: BTreeMap::new(),
        }
    }
}

impl RegisterTier1SpecializedExecutorState {
    pub(in crate::runtime) fn from_env() -> Self {
        let mut state = Self::default();
        state.enabled = parse_env_bool("TRUST_VM_TIER1_SPECIALIZED_EXECUTOR", false);
        state.hot_block_threshold = parse_env_u64(
            "TRUST_VM_TIER1_SPECIALIZED_EXECUTOR_HOT_THRESHOLD",
            state.hot_block_threshold,
        );
        state.cache_capacity = parse_env_usize(
            "TRUST_VM_TIER1_SPECIALIZED_EXECUTOR_CACHE_CAP",
            state.cache_capacity,
        )
        .max(1);
        state
    }

    pub(in crate::runtime) fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub(in crate::runtime) fn reset(&mut self) {
        self.invalidate_all();
        self.compile_attempts = 0;
        self.compile_successes = 0;
        self.compile_failures = 0;
        self.compile_failure_reasons.clear();
        self.cache_evictions = 0;
        self.block_executions = 0;
        self.deopt_count = 0;
        self.deopt_reasons.clear();
    }

    pub(in crate::runtime) fn invalidate_all(&mut self) {
        self.block_hits.clear();
        self.compiled_order.clear();
        self.compiled_blocks.clear();
    }

    pub(in crate::runtime) fn snapshot(&self) -> VmTier1SpecializedExecutorSnapshot {
        let compile_failure_reasons = self
            .compile_failure_reasons
            .iter()
            .map(
                |(reason, count)| VmTier1SpecializedExecutorCompileFailureReason {
                    reason: reason.clone(),
                    count: *count,
                },
            )
            .collect::<Vec<_>>();
        let deopt_reasons = self
            .deopt_reasons
            .iter()
            .map(|(reason, count)| VmTier1SpecializedExecutorDeoptReason {
                reason: reason.clone(),
                count: *count,
            })
            .collect::<Vec<_>>();
        VmTier1SpecializedExecutorSnapshot {
            enabled: self.enabled,
            hot_block_threshold: self.hot_block_threshold,
            cache_capacity: self.cache_capacity,
            cached_blocks: self.compiled_blocks.len(),
            compile_attempts: self.compile_attempts,
            compile_successes: self.compile_successes,
            compile_failures: self.compile_failures,
            compile_failure_reasons,
            cache_evictions: self.cache_evictions,
            block_executions: self.block_executions,
            deopt_count: self.deopt_count,
            deopt_reasons,
        }
    }

    fn enabled(&self) -> bool {
        self.enabled
    }

    fn track_block_hit(&mut self, key: Tier1BlockKey) -> u64 {
        let entry = self.block_hits.entry(key).or_insert(0);
        *entry = entry.saturating_add(1);
        *entry
    }

    fn can_attempt_compile(&self, hits: u64, key: &Tier1BlockKey) -> bool {
        hits >= self.hot_block_threshold && !self.compiled_blocks.contains_key(key)
    }

    fn compiled_block(&self, key: &Tier1BlockKey) -> Option<&Arc<Tier1CompiledBlock>> {
        self.compiled_blocks.get(key)
    }

    fn record_compile_attempt(&mut self) {
        self.compile_attempts = self.compile_attempts.saturating_add(1);
    }

    fn record_compile_success(&mut self) {
        self.compile_successes = self.compile_successes.saturating_add(1);
    }

    fn record_compile_failure(&mut self, reason: impl Into<String>) {
        self.compile_failures = self.compile_failures.saturating_add(1);
        let entry = self
            .compile_failure_reasons
            .entry(reason.into())
            .or_insert(0);
        *entry = entry.saturating_add(1);
    }

    fn insert_compiled_block(&mut self, block: Arc<Tier1CompiledBlock>) {
        let key = block.key;
        if self.compiled_blocks.contains_key(&key) {
            return;
        }
        self.compiled_blocks.insert(key, block);
        self.compiled_order.push_back(key);
        while self.compiled_blocks.len() > self.cache_capacity {
            if let Some(evicted) = self.compiled_order.pop_front() {
                if self.compiled_blocks.remove(&evicted).is_some() {
                    self.cache_evictions = self.cache_evictions.saturating_add(1);
                }
            } else {
                break;
            }
        }
    }

    fn remove_compiled_block(&mut self, key: &Tier1BlockKey) {
        if self.compiled_blocks.remove(key).is_none() {
            return;
        }
        self.compiled_order.retain(|entry| entry != key);
    }

    fn record_block_execution(&mut self) {
        self.block_executions = self.block_executions.saturating_add(1);
    }

    fn record_deopt(&mut self, reason: impl Into<String>) {
        self.deopt_count = self.deopt_count.saturating_add(1);
        let entry = self.deopt_reasons.entry(reason.into()).or_insert(0);
        *entry = entry.saturating_add(1);
    }
}

fn parse_env_bool(name: &str, default: bool) -> bool {
    match std::env::var(name) {
        Ok(value) => match value.trim().to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "on" => true,
            "0" | "false" | "no" | "off" => false,
            _ => default,
        },
        Err(_) => default,
    }
}

fn parse_env_u64(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .unwrap_or(default)
}

fn parse_env_usize(name: &str, default: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
        .unwrap_or(default)
}

fn build_cached_register_program(
    module: &VmModule,
    pou_id: u32,
) -> Result<CachedRegisterProgram, RuntimeError> {
    let program = lower_pou_to_register_ir(module, pou_id)?;
    let register_read_counts_by_block = register_read_counts_by_block(&program);
    let block_has_register_reads = register_read_counts_by_block
        .iter()
        .map(|counts| counts.iter().any(|count| *count != 0))
        .collect::<Vec<_>>();
    let fallback_opcode = first_fallback_opcode(&program);
    Ok(CachedRegisterProgram {
        program: Arc::new(program),
        register_read_counts_by_block: Arc::new(register_read_counts_by_block),
        block_has_register_reads: Arc::new(block_has_register_reads),
        fallback_opcode,
    })
}

pub(super) fn try_execute_pou_with_register_ir(
    runtime: &mut Runtime,
    module: &VmModule,
    pou_id: u32,
    entry_instance: Option<InstanceId>,
) -> Result<RegisterExecutionOutcome, RuntimeError> {
    let result = try_execute_pou_with_register_ir_with_locals(
        runtime,
        module,
        pou_id,
        entry_instance,
        None,
        false,
        0,
        None,
    )?;
    if result.is_some() {
        Ok(RegisterExecutionOutcome::Executed)
    } else {
        Ok(RegisterExecutionOutcome::FallbackToStack)
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn try_execute_pou_with_register_ir_with_locals(
    runtime: &mut Runtime,
    module: &VmModule,
    pou_id: u32,
    entry_instance: Option<InstanceId>,
    initial_locals: Option<&[Value]>,
    capture_return: bool,
    depth_offset: u32,
    shared_budget: Option<&mut usize>,
) -> Result<Option<RegisterPouExecutionResult>, RuntimeError> {
    // Keep stack execution as the single source of truth while debug stepping is active.
    if runtime.debug.is_some() {
        runtime.vm_register_profile.record_fallback("debug_mode");
        return Ok(None);
    }

    let lowered = runtime
        .vm_register_lowering_cache
        .get_or_build(module, pou_id);
    let lowered = match lowered.as_ref() {
        RegisterLoweringCacheEntry::Ready(program) => program,
        RegisterLoweringCacheEntry::LoweringError { message } => {
            let pou_name = module.pou_name(pou_id).unwrap_or("<unknown>");
            runtime
                .vm_register_profile
                .record_fallback(format!("lowering_error:{pou_name}: {message}"));
            return Ok(None);
        }
    };

    if let Some(opcode) = lowered.fallback_opcode {
        runtime
            .vm_register_profile
            .record_fallback(format!("unsupported_opcode_0x{opcode:02X}"));
        return Ok(None);
    }
    let result = execute_register_program(
        runtime,
        module,
        lowered.program.as_ref(),
        lowered.register_read_counts_by_block.as_ref(),
        lowered.block_has_register_reads.as_ref(),
        entry_instance,
        initial_locals,
        capture_return,
        depth_offset,
        shared_budget,
    )?;
    runtime.vm_register_profile.record_executed();
    Ok(Some(result))
}

fn canonical_stack_register(slot: u32) -> RegisterId {
    RegisterId(slot)
}

fn pop_stack_depth(depth: &mut u32, opcode: u8) -> Result<(), RuntimeError> {
    if *depth == 0 {
        return Err(invalid_bytecode(format!(
            "register-ir lowering stack underflow while decoding opcode 0x{opcode:02X}",
        )));
    }
    *depth -= 1;
    Ok(())
}

fn propagate_block_entry_stack_depth(
    entry_depths: &mut [Option<u32>],
    block_index: usize,
    depth: u32,
    worklist: &mut VecDeque<usize>,
) -> Result<(), RuntimeError> {
    match entry_depths.get_mut(block_index) {
        Some(slot @ None) => {
            *slot = Some(depth);
            worklist.push_back(block_index);
        }
        Some(Some(existing)) if *existing != depth => {
            return Err(invalid_bytecode(format!(
                "register-ir inconsistent block-entry stack depth for block {block_index}: {existing} vs {depth}",
            )));
        }
        Some(Some(_)) => {}
        None => {
            return Err(invalid_bytecode(format!(
                "register-ir missing block {block_index} while propagating stack depth",
            )));
        }
    }
    Ok(())
}

fn apply_decoded_stack_effect(depth: &mut u32, instr: &DecodedInstr) -> Result<(), RuntimeError> {
    match instr.opcode {
        0x00 | 0x01 | 0x02 | 0x05 | 0x06 | 0x07 | 0x08 | 0x70 => {}
        0x03 | 0x04 | 0x12 => pop_stack_depth(depth, instr.opcode)?,
        0x10 | 0x20 | 0x22 | 0x23 | 0x24 | 0x25 | 0x60 => {
            *depth = depth.saturating_add(1);
        }
        0x09 => {
            let (_, _, arg_count) = operand_native_call(instr)?;
            for _ in 0..arg_count {
                pop_stack_depth(depth, instr.opcode)?;
            }
            *depth = depth.saturating_add(1);
        }
        0x11 => {
            if *depth == 0 {
                return Err(invalid_bytecode(
                    "register-ir lowering stack underflow on DUP",
                ));
            }
            *depth = depth.saturating_add(1);
        }
        0x13 => {
            if *depth < 2 {
                return Err(invalid_bytecode(
                    "register-ir lowering stack underflow on SWAP",
                ));
            }
        }
        0x14 => {
            if *depth < 3 {
                return Err(invalid_bytecode(
                    "register-ir lowering stack underflow on ROT3",
                ));
            }
        }
        0x15 => {
            if *depth < 4 {
                return Err(invalid_bytecode(
                    "register-ir lowering stack underflow on ROT4",
                ));
            }
        }
        0x16 | 0x30 | 0x32 | 0x45 | 0x49 | 0x61 | 0x62 => {
            if *depth == 0 {
                return Err(invalid_bytecode(format!(
                    "register-ir lowering stack underflow while decoding opcode 0x{:02X}",
                    instr.opcode,
                )));
            }
        }
        0x21 => pop_stack_depth(depth, instr.opcode)?,
        0x31 | 0x40..=0x44 | 0x46..=0x48 | 0x4A..=0x4E | 0x50..=0x55 | 0x63 => {
            pop_stack_depth(depth, instr.opcode)?;
        }
        0x33 => {
            pop_stack_depth(depth, instr.opcode)?;
            pop_stack_depth(depth, instr.opcode)?;
        }
        _ => {
            return Err(invalid_bytecode(format!(
                "register-ir unsupported stack-effect analysis for opcode 0x{:02X}",
                instr.opcode,
            )));
        }
    }
    Ok(())
}

fn compute_block_entry_stack_depths(
    decoded: &[DecodedInstr],
    leaders: &[usize],
    code_start: usize,
    code_end: usize,
) -> Result<HashMap<usize, u32>, RuntimeError> {
    let mut start_to_index = HashMap::new();
    for (index, start_pc) in leaders.iter().copied().enumerate() {
        start_to_index.insert(start_pc, index);
    }

    let mut entry_depths = vec![None; leaders.len()];
    let mut worklist = VecDeque::new();
    if !leaders.is_empty() {
        entry_depths[0] = Some(0);
        worklist.push_back(0);
    }

    while let Some(block_index) = worklist.pop_front() {
        let start_pc = leaders[block_index];
        let end_pc = leaders.get(block_index + 1).copied().unwrap_or(code_end);
        let mut depth = entry_depths[block_index].unwrap_or(0);
        let mut terminated = false;

        for instr in decoded
            .iter()
            .filter(|instr| instr.pc >= start_pc && instr.pc < end_pc)
        {
            match instr.opcode {
                0x02 => {
                    let offset = operand_i32(instr)?;
                    let target_pc = jump_target_pc(instr.next_pc, offset, code_start, code_end)?;
                    if target_pc < code_end {
                        let target_index = *start_to_index.get(&target_pc).ok_or_else(|| {
                            invalid_bytecode(format!(
                                "register-ir jump target {target_pc} is not a block leader"
                            ))
                        })?;
                        propagate_block_entry_stack_depth(
                            &mut entry_depths,
                            target_index,
                            depth,
                            &mut worklist,
                        )?;
                    }
                    terminated = true;
                    break;
                }
                0x03 | 0x04 => {
                    pop_stack_depth(&mut depth, instr.opcode)?;
                    let offset = operand_i32(instr)?;
                    let target_pc = jump_target_pc(instr.next_pc, offset, code_start, code_end)?;
                    if target_pc < code_end {
                        let target_index = *start_to_index.get(&target_pc).ok_or_else(|| {
                            invalid_bytecode(format!(
                                "register-ir jump target {target_pc} is not a block leader"
                            ))
                        })?;
                        propagate_block_entry_stack_depth(
                            &mut entry_depths,
                            target_index,
                            depth,
                            &mut worklist,
                        )?;
                    }
                    if instr.next_pc < code_end {
                        let fallthrough_index =
                            *start_to_index.get(&instr.next_pc).ok_or_else(|| {
                                invalid_bytecode(format!(
                                    "register-ir fallthrough target {} is not a block leader",
                                    instr.next_pc,
                                ))
                            })?;
                        propagate_block_entry_stack_depth(
                            &mut entry_depths,
                            fallthrough_index,
                            depth,
                            &mut worklist,
                        )?;
                    }
                    terminated = true;
                    break;
                }
                0x06 => {
                    terminated = true;
                    break;
                }
                _ => apply_decoded_stack_effect(&mut depth, instr)?,
            }
        }

        if !terminated {
            if let Some(next_start_pc) = leaders.get(block_index + 1).copied() {
                let next_index = *start_to_index.get(&next_start_pc).ok_or_else(|| {
                    invalid_bytecode(format!(
                        "register-ir fallthrough target {next_start_pc} is not a block leader",
                    ))
                })?;
                propagate_block_entry_stack_depth(
                    &mut entry_depths,
                    next_index,
                    depth,
                    &mut worklist,
                )?;
            }
        }
    }

    let mut resolved = HashMap::new();
    for (index, start_pc) in leaders.iter().copied().enumerate() {
        if let Some(depth) = entry_depths[index] {
            resolved.insert(start_pc, depth);
        }
    }
    Ok(resolved)
}

fn normalize_stack_for_block_exit(
    next_register: &mut u32,
    instructions: &mut Vec<RegisterInstr>,
    stack: &[RegisterId],
    protected: Option<RegisterId>,
) -> Option<RegisterId> {
    let mut protected = protected;
    if let Some(register) = protected {
        let clobbered = stack.iter().enumerate().any(|(slot, src)| {
            let dest = canonical_stack_register(slot as u32);
            dest == register && *src != register
        });
        if clobbered {
            let temp = alloc_register(next_register);
            instructions.push(RegisterInstr::Move {
                src: register,
                dest: temp,
            });
            protected = Some(temp);
        }
    }

    let mut pending = stack
        .iter()
        .enumerate()
        .filter_map(|(slot, src)| {
            let dest = canonical_stack_register(slot as u32);
            (*src != dest).then_some((*src, dest))
        })
        .collect::<Vec<_>>();
    let mut scratch = None;

    while !pending.is_empty() {
        if let Some(index) = pending
            .iter()
            .position(|(_, dest)| !pending.iter().any(|(other_src, _)| *other_src == *dest))
        {
            let (src, dest) = pending.remove(index);
            instructions.push(RegisterInstr::Move { src, dest });
            continue;
        }

        let (src, dest) = pending.remove(0);
        let temp = *scratch.get_or_insert_with(|| alloc_register(next_register));
        instructions.push(RegisterInstr::Move { src, dest: temp });
        for (other_src, _) in &mut pending {
            if *other_src == src {
                *other_src = temp;
            }
        }
        pending.push((temp, dest));
    }

    protected
}

pub(super) fn lower_pou_to_register_ir(
    module: &VmModule,
    pou_id: u32,
) -> Result<RegisterProgram, RuntimeError> {
    let pou = module
        .pou(pou_id)
        .ok_or_else(|| invalid_bytecode(format!("vm missing pou id {pou_id}")))?;
    let decoded = decode_pou(module, pou.code_start, pou.code_end)?;
    let leaders = collect_block_leaders(&decoded, pou.code_start, pou.code_end)?;
    let entry_stack_depths =
        compute_block_entry_stack_depths(&decoded, &leaders, pou.code_start, pou.code_end)?;
    let mut start_to_block = HashMap::new();
    for (idx, start) in leaders.iter().copied().enumerate() {
        start_to_block.insert(start, idx as u32);
    }

    let max_entry_stack_depth = entry_stack_depths.values().copied().max().unwrap_or(0);
    let mut next_register = max_entry_stack_depth;
    let mut blocks = Vec::with_capacity(leaders.len());
    for (idx, start_pc) in leaders.iter().copied().enumerate() {
        let end_pc = leaders.get(idx + 1).copied().unwrap_or(pou.code_end);
        let entry_stack_depth = entry_stack_depths.get(&start_pc).copied().unwrap_or(0);
        let mut stack = (0..entry_stack_depth)
            .map(canonical_stack_register)
            .collect::<Vec<_>>();
        let mut opaque_mode = false;
        let mut instructions = Vec::new();

        for instr in decoded
            .iter()
            .filter(|instr| instr.pc >= start_pc && instr.pc < end_pc)
        {
            if opaque_mode {
                instructions.push(RegisterInstr::VmFallback {
                    opcode: instr.opcode,
                    operands: instr.operands.clone(),
                });
                continue;
            }

            match instr.opcode {
                0x00 => instructions.push(RegisterInstr::Nop),
                0x02 => {
                    let offset = operand_i32(instr)?;
                    let target_pc =
                        jump_target_pc(instr.next_pc, offset, pou.code_start, pou.code_end)?;
                    let target = pc_to_block_target(target_pc, pou.code_end, &start_to_block)?;
                    normalize_stack_for_block_exit(
                        &mut next_register,
                        &mut instructions,
                        &stack,
                        None,
                    );
                    instructions.push(RegisterInstr::Jump { target });
                }
                0x03 | 0x04 => {
                    let cond = pop_stack(&mut stack, instr.opcode)?;
                    let offset = operand_i32(instr)?;
                    let target_pc =
                        jump_target_pc(instr.next_pc, offset, pou.code_start, pou.code_end)?;
                    let target = pc_to_block_target(target_pc, pou.code_end, &start_to_block)?;
                    let cond = normalize_stack_for_block_exit(
                        &mut next_register,
                        &mut instructions,
                        &stack,
                        Some(cond),
                    )
                    .unwrap_or(cond);
                    instructions.push(RegisterInstr::JumpIf {
                        cond,
                        jump_if_true: instr.opcode == 0x03,
                        target,
                    });
                }
                0x06 => instructions.push(RegisterInstr::Return),
                0x09 => {
                    let (kind, symbol_idx, arg_count) = operand_native_call(instr)?;
                    let arg_count = usize::try_from(arg_count).map_err(|_| {
                        invalid_bytecode("register-ir lowering arg_count overflow on CALL_NATIVE")
                    })?;
                    if stack.len() < arg_count {
                        return Err(invalid_bytecode(
                            "register-ir lowering stack underflow on CALL_NATIVE",
                        ));
                    }
                    let split = stack.len() - arg_count;
                    let args = stack.split_off(split);
                    let dest = alloc_register(&mut next_register);
                    stack.push(dest);
                    instructions.push(RegisterInstr::CallNative {
                        kind,
                        symbol_idx,
                        args,
                        dest,
                    });
                }
                0x10 => {
                    let const_idx = operand_u32(instr)?;
                    let dest = alloc_register(&mut next_register);
                    stack.push(dest);
                    instructions.push(RegisterInstr::LoadConst { dest, const_idx });
                }
                0x11 => {
                    let top = stack.last().copied().ok_or_else(|| {
                        invalid_bytecode("register-ir lowering stack underflow on DUP")
                    })?;
                    stack.push(top);
                }
                0x12 => {
                    let _ = pop_stack(&mut stack, instr.opcode)?;
                }
                0x13 => {
                    if stack.len() < 2 {
                        return Err(invalid_bytecode(
                            "register-ir lowering stack underflow on SWAP",
                        ));
                    }
                    let len = stack.len();
                    stack.swap(len - 1, len - 2);
                }
                0x20 => {
                    let ref_idx = operand_u32(instr)?;
                    let dest = alloc_register(&mut next_register);
                    stack.push(dest);
                    instructions.push(RegisterInstr::LoadRef { dest, ref_idx });
                }
                0x21 => {
                    let src = pop_stack(&mut stack, instr.opcode)?;
                    let ref_idx = operand_u32(instr)?;
                    instructions.push(RegisterInstr::StoreRef { ref_idx, src });
                }
                0x22 => {
                    let ref_idx = operand_u32(instr)?;
                    let dest = alloc_register(&mut next_register);
                    stack.push(dest);
                    instructions.push(RegisterInstr::LoadRefAddr { dest, ref_idx });
                }
                0x25 => {
                    let dest = alloc_register(&mut next_register);
                    stack.push(dest);
                    instructions.push(RegisterInstr::LoadNull { dest });
                }
                0x23 => {
                    let dest = alloc_register(&mut next_register);
                    stack.push(dest);
                    instructions.push(RegisterInstr::LoadSelf { dest });
                }
                0x24 => {
                    let dest = alloc_register(&mut next_register);
                    stack.push(dest);
                    instructions.push(RegisterInstr::LoadSuper { dest });
                }
                0x30 => {
                    let field_idx = operand_u32(instr)?;
                    let base = pop_stack(&mut stack, instr.opcode)?;
                    let dest = alloc_register(&mut next_register);
                    stack.push(dest);
                    instructions.push(RegisterInstr::RefField {
                        base,
                        field_idx,
                        dest,
                    });
                }
                0x31 => {
                    let index = pop_stack(&mut stack, instr.opcode)?;
                    let base = pop_stack(&mut stack, instr.opcode)?;
                    let dest = alloc_register(&mut next_register);
                    stack.push(dest);
                    instructions.push(RegisterInstr::RefIndex { base, index, dest });
                }
                0x32 => {
                    let reference = pop_stack(&mut stack, instr.opcode)?;
                    let dest = alloc_register(&mut next_register);
                    stack.push(dest);
                    instructions.push(RegisterInstr::LoadDynamic { reference, dest });
                }
                0x33 => {
                    let value = pop_stack(&mut stack, instr.opcode)?;
                    let reference = pop_stack(&mut stack, instr.opcode)?;
                    instructions.push(RegisterInstr::StoreDynamic { reference, value });
                }
                0x40 => lower_binary(
                    &mut next_register,
                    &mut stack,
                    &mut instructions,
                    BinaryOp::Add,
                    instr.opcode,
                )?,
                0x41 => lower_binary(
                    &mut next_register,
                    &mut stack,
                    &mut instructions,
                    BinaryOp::Sub,
                    instr.opcode,
                )?,
                0x42 => lower_binary(
                    &mut next_register,
                    &mut stack,
                    &mut instructions,
                    BinaryOp::Mul,
                    instr.opcode,
                )?,
                0x43 => lower_binary(
                    &mut next_register,
                    &mut stack,
                    &mut instructions,
                    BinaryOp::Div,
                    instr.opcode,
                )?,
                0x44 => lower_binary(
                    &mut next_register,
                    &mut stack,
                    &mut instructions,
                    BinaryOp::Mod,
                    instr.opcode,
                )?,
                0x45 => lower_unary(
                    &mut next_register,
                    &mut stack,
                    &mut instructions,
                    UnaryOp::Neg,
                    instr.opcode,
                )?,
                0x46 => lower_binary(
                    &mut next_register,
                    &mut stack,
                    &mut instructions,
                    BinaryOp::And,
                    instr.opcode,
                )?,
                0x47 => lower_binary(
                    &mut next_register,
                    &mut stack,
                    &mut instructions,
                    BinaryOp::Or,
                    instr.opcode,
                )?,
                0x48 => lower_binary(
                    &mut next_register,
                    &mut stack,
                    &mut instructions,
                    BinaryOp::Xor,
                    instr.opcode,
                )?,
                0x49 => lower_unary(
                    &mut next_register,
                    &mut stack,
                    &mut instructions,
                    UnaryOp::Not,
                    instr.opcode,
                )?,
                0x50 => lower_binary(
                    &mut next_register,
                    &mut stack,
                    &mut instructions,
                    BinaryOp::Eq,
                    instr.opcode,
                )?,
                0x51 => lower_binary(
                    &mut next_register,
                    &mut stack,
                    &mut instructions,
                    BinaryOp::Ne,
                    instr.opcode,
                )?,
                0x52 => lower_binary(
                    &mut next_register,
                    &mut stack,
                    &mut instructions,
                    BinaryOp::Lt,
                    instr.opcode,
                )?,
                0x53 => lower_binary(
                    &mut next_register,
                    &mut stack,
                    &mut instructions,
                    BinaryOp::Le,
                    instr.opcode,
                )?,
                0x54 => lower_binary(
                    &mut next_register,
                    &mut stack,
                    &mut instructions,
                    BinaryOp::Gt,
                    instr.opcode,
                )?,
                0x55 => lower_binary(
                    &mut next_register,
                    &mut stack,
                    &mut instructions,
                    BinaryOp::Ge,
                    instr.opcode,
                )?,
                0x60 => {
                    let type_idx = operand_u32(instr)?;
                    let dest = alloc_register(&mut next_register);
                    stack.push(dest);
                    instructions.push(RegisterInstr::SizeOfType { type_idx, dest });
                }
                0x61 => {
                    let src = pop_stack(&mut stack, instr.opcode)?;
                    let dest = alloc_register(&mut next_register);
                    stack.push(dest);
                    instructions.push(RegisterInstr::SizeOfValue { src, dest });
                }
                _ => {
                    instructions.push(RegisterInstr::VmFallback {
                        opcode: instr.opcode,
                        operands: instr.operands.clone(),
                    });
                    opaque_mode = true;
                }
            }
        }

        let terminates_control_flow = instructions.last().is_some_and(|instruction| {
            matches!(
                instruction,
                RegisterInstr::Jump { .. } | RegisterInstr::JumpIf { .. } | RegisterInstr::Return
            )
        });
        if !terminates_control_flow {
            normalize_stack_for_block_exit(&mut next_register, &mut instructions, &stack, None);
        }

        let instructions = fuse_register_block_instructions(&instructions);
        blocks.push(RegisterBlock {
            id: idx as u32,
            start_pc,
            end_pc,
            entry_stack_depth,
            instructions,
        });
    }

    let lowered = RegisterProgram {
        pou_id,
        entry_block: 0,
        max_registers: next_register,
        blocks,
    };
    verify_register_program(&lowered)?;
    Ok(lowered)
}

fn fuse_register_block_instructions(instructions: &[RegisterInstr]) -> Vec<RegisterInstr> {
    if instructions.len() < 4 {
        return instructions.to_vec();
    }
    let mut fused = Vec::with_capacity(instructions.len());
    let mut index = 0usize;
    while index < instructions.len() {
        if let Some((instruction, consumed)) = try_fuse_instruction_window(instructions, index) {
            fused.push(instruction);
            index += consumed;
            continue;
        }
        fused.push(instructions[index].clone());
        index += 1;
    }
    fused
}

fn try_fuse_instruction_window(
    instructions: &[RegisterInstr],
    index: usize,
) -> Option<(RegisterInstr, usize)> {
    if index + 3 >= instructions.len() {
        return None;
    }

    if let (
        RegisterInstr::LoadRef {
            dest: left_reg,
            ref_idx: left_ref_idx,
        },
        RegisterInstr::LoadRef {
            dest: right_reg,
            ref_idx: right_ref_idx,
        },
        RegisterInstr::Binary {
            op,
            left,
            right,
            dest,
        },
        RegisterInstr::StoreRef { ref_idx, src },
    ) = (
        &instructions[index],
        &instructions[index + 1],
        &instructions[index + 2],
        &instructions[index + 3],
    ) {
        if left == left_reg
            && right == right_reg
            && src == dest
            && !register_used_after(instructions, index + 4, *left_reg)
            && !register_used_after(instructions, index + 4, *right_reg)
            && !register_used_after(instructions, index + 4, *dest)
        {
            return Some((
                RegisterInstr::BinaryRefToRef {
                    op: *op,
                    left_ref_idx: *left_ref_idx,
                    right_ref_idx: *right_ref_idx,
                    dest_ref_idx: *ref_idx,
                },
                4,
            ));
        }
    }

    if let (
        RegisterInstr::LoadRef {
            dest: left_reg,
            ref_idx: left_ref_idx,
        },
        RegisterInstr::LoadConst {
            dest: const_reg,
            const_idx,
        },
        RegisterInstr::Binary {
            op,
            left,
            right,
            dest,
        },
        RegisterInstr::StoreRef { ref_idx, src },
    ) = (
        &instructions[index],
        &instructions[index + 1],
        &instructions[index + 2],
        &instructions[index + 3],
    ) {
        if left == left_reg
            && right == const_reg
            && src == dest
            && !register_used_after(instructions, index + 4, *left_reg)
            && !register_used_after(instructions, index + 4, *const_reg)
            && !register_used_after(instructions, index + 4, *dest)
        {
            return Some((
                RegisterInstr::BinaryRefConstToRef {
                    op: *op,
                    left_ref_idx: *left_ref_idx,
                    const_idx: *const_idx,
                    dest_ref_idx: *ref_idx,
                },
                4,
            ));
        }
    }

    if let (
        RegisterInstr::LoadConst {
            dest: const_reg,
            const_idx,
        },
        RegisterInstr::LoadRef {
            dest: right_reg,
            ref_idx: right_ref_idx,
        },
        RegisterInstr::Binary {
            op,
            left,
            right,
            dest,
        },
        RegisterInstr::StoreRef { ref_idx, src },
    ) = (
        &instructions[index],
        &instructions[index + 1],
        &instructions[index + 2],
        &instructions[index + 3],
    ) {
        if left == const_reg
            && right == right_reg
            && src == dest
            && !register_used_after(instructions, index + 4, *const_reg)
            && !register_used_after(instructions, index + 4, *right_reg)
            && !register_used_after(instructions, index + 4, *dest)
        {
            return Some((
                RegisterInstr::BinaryConstRefToRef {
                    op: *op,
                    const_idx: *const_idx,
                    right_ref_idx: *right_ref_idx,
                    dest_ref_idx: *ref_idx,
                },
                4,
            ));
        }
    }

    if let (
        RegisterInstr::LoadRef {
            dest: ref_reg,
            ref_idx,
        },
        RegisterInstr::LoadConst {
            dest: const_reg,
            const_idx,
        },
        RegisterInstr::Binary {
            op,
            left,
            right,
            dest,
        },
        RegisterInstr::JumpIf {
            cond,
            jump_if_true,
            target,
        },
    ) = (
        &instructions[index],
        &instructions[index + 1],
        &instructions[index + 2],
        &instructions[index + 3],
    ) {
        if is_cmp_binary_op(*op)
            && left == ref_reg
            && right == const_reg
            && cond == dest
            && !register_used_after(instructions, index + 4, *ref_reg)
            && !register_used_after(instructions, index + 4, *const_reg)
            && !register_used_after(instructions, index + 4, *dest)
        {
            return Some((
                RegisterInstr::CmpRefConstJumpIf {
                    op: *op,
                    ref_idx: *ref_idx,
                    const_idx: *const_idx,
                    jump_if_true: *jump_if_true,
                    target: *target,
                },
                4,
            ));
        }
    }

    None
}

fn register_used_after(
    instructions: &[RegisterInstr],
    start_index: usize,
    register: RegisterId,
) -> bool {
    instructions[start_index..]
        .iter()
        .any(|instruction| instruction_reads_register(instruction, register))
}

fn instruction_reads_register(instruction: &RegisterInstr, register: RegisterId) -> bool {
    match instruction {
        RegisterInstr::CallNative { args, .. } => args.contains(&register),
        RegisterInstr::SizeOfValue { src, .. } => *src == register,
        RegisterInstr::RefField { base, .. } => *base == register,
        RegisterInstr::RefIndex { base, index, .. } => *base == register || *index == register,
        RegisterInstr::LoadDynamic { reference, .. } => *reference == register,
        RegisterInstr::StoreDynamic { reference, value } => {
            *reference == register || *value == register
        }
        RegisterInstr::Unary { src, .. } => *src == register,
        RegisterInstr::Binary { left, right, .. } => *left == register || *right == register,
        RegisterInstr::StoreRef { src, .. } => *src == register,
        RegisterInstr::JumpIf { cond, .. } => *cond == register,
        _ => false,
    }
}

fn is_cmp_binary_op(op: BinaryOp) -> bool {
    matches!(
        op,
        BinaryOp::Eq | BinaryOp::Ne | BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge
    )
}

pub(super) fn verify_register_program(program: &RegisterProgram) -> Result<(), RuntimeError> {
    if program.blocks.is_empty() {
        return Err(invalid_bytecode("register-ir program has no blocks"));
    }
    let known_blocks = program
        .blocks
        .iter()
        .map(|block| block.id)
        .collect::<HashSet<_>>();
    if !known_blocks.contains(&program.entry_block) {
        return Err(invalid_bytecode(format!(
            "register-ir entry block {} missing",
            program.entry_block
        )));
    }

    for block in &program.blocks {
        let mut defined = (0..block.entry_stack_depth)
            .map(RegisterId)
            .collect::<BTreeSet<_>>();
        for instr in &block.instructions {
            match instr {
                RegisterInstr::LoadConst { dest, .. }
                | RegisterInstr::LoadRef { dest, .. }
                | RegisterInstr::LoadRefAddr { dest, .. }
                | RegisterInstr::LoadNull { dest }
                | RegisterInstr::LoadSelf { dest }
                | RegisterInstr::LoadSuper { dest }
                | RegisterInstr::SizeOfType { dest, .. } => {
                    verify_dest(dest, program.max_registers, &mut defined)?;
                }
                RegisterInstr::Move { src, dest } => {
                    verify_src(src, &defined)?;
                    verify_move_dest(dest, program.max_registers, &mut defined)?;
                }
                RegisterInstr::SizeOfValue { src, dest } => {
                    verify_src(src, &defined)?;
                    verify_dest(dest, program.max_registers, &mut defined)?;
                }
                RegisterInstr::RefField { base, dest, .. } => {
                    verify_src(base, &defined)?;
                    verify_dest(dest, program.max_registers, &mut defined)?;
                }
                RegisterInstr::RefIndex {
                    base, index, dest, ..
                } => {
                    verify_src(base, &defined)?;
                    verify_src(index, &defined)?;
                    verify_dest(dest, program.max_registers, &mut defined)?;
                }
                RegisterInstr::LoadDynamic { reference, dest } => {
                    verify_src(reference, &defined)?;
                    verify_dest(dest, program.max_registers, &mut defined)?;
                }
                RegisterInstr::StoreDynamic { reference, value } => {
                    verify_src(reference, &defined)?;
                    verify_src(value, &defined)?;
                }
                RegisterInstr::CallNative { args, dest, .. } => {
                    for arg in args {
                        verify_src(arg, &defined)?;
                    }
                    verify_dest(dest, program.max_registers, &mut defined)?;
                }
                RegisterInstr::Unary { src, dest, .. } => {
                    verify_src(src, &defined)?;
                    verify_dest(dest, program.max_registers, &mut defined)?;
                }
                RegisterInstr::Binary {
                    left, right, dest, ..
                } => {
                    verify_src(left, &defined)?;
                    verify_src(right, &defined)?;
                    verify_dest(dest, program.max_registers, &mut defined)?;
                }
                RegisterInstr::CmpRefConstJumpIf { target, .. } => {
                    verify_target(target, &known_blocks)?;
                }
                RegisterInstr::StoreRef { src, .. } => {
                    verify_src(src, &defined)?;
                }
                RegisterInstr::Jump { target } => verify_target(target, &known_blocks)?,
                RegisterInstr::JumpIf { cond, target, .. } => {
                    verify_src(cond, &defined)?;
                    verify_target(target, &known_blocks)?;
                }
                RegisterInstr::BinaryRefToRef { .. }
                | RegisterInstr::BinaryRefConstToRef { .. }
                | RegisterInstr::BinaryConstRefToRef { .. }
                | RegisterInstr::Nop
                | RegisterInstr::Return
                | RegisterInstr::VmFallback { .. } => {}
            }
        }
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn execute_register_program(
    runtime: &mut Runtime,
    module: &VmModule,
    program: &RegisterProgram,
    register_read_counts_by_block: &[Vec<u32>],
    block_has_register_reads: &[bool],
    entry_instance: Option<InstanceId>,
    initial_locals: Option<&[Value]>,
    capture_return: bool,
    depth_offset: u32,
    shared_budget: Option<&mut usize>,
) -> Result<RegisterPouExecutionResult, RuntimeError> {
    ensure_global_call_depth(depth_offset, 1).map_err(VmTrap::into_runtime_error)?;
    let mut frames = FrameStack::default();
    let _ = push_call_frame(
        &mut frames,
        module,
        program.pou_id,
        usize::MAX,
        entry_instance,
    )
    .map_err(VmTrap::into_runtime_error)?;
    runtime
        .vm_register_profile
        .record_call_op(RegisterCallOpKind::FramePush);
    if let Some(initial_locals) = initial_locals {
        let frame = frames
            .current_mut()
            .ok_or_else(|| VmTrap::CallStackUnderflow.into_runtime_error())?;
        if initial_locals.len() > frame.locals.len() {
            return Err(VmTrap::BytecodeDecode(
                "register-ir call initial local payload exceeds frame local capacity".into(),
            )
            .into_runtime_error());
        }
        for (index, value) in initial_locals.iter().cloned().enumerate() {
            frame.locals[index] = value;
        }
    }
    let mut register_execution_buffers =
        RegisterExecutionBuffers::acquire(program.max_registers as usize);
    let (registers, remaining_register_reads, native_call_stack) =
        register_execution_buffers.buffers_mut();
    let mut current_block = program.entry_block;
    let mut local_budget = module.instruction_budget;
    let budget = shared_budget.unwrap_or(&mut local_budget);
    let profile_enabled = runtime.vm_register_profile.enabled;
    let tier1_enabled = runtime.vm_tier1_specialized_executor.enabled();
    loop {
        if frames.is_empty() {
            return Ok(RegisterPouExecutionResult {
                return_value: None,
                locals: Vec::new(),
            });
        }
        let block_index = block_index_from_id(program, current_block)?;
        let block = &program.blocks[block_index];
        if *block_has_register_reads.get(block_index).unwrap_or(&false) {
            remaining_register_reads.copy_from_slice(&register_read_counts_by_block[block_index]);
        }
        if profile_enabled {
            runtime.vm_register_profile.record_block_hit(
                program.pou_id,
                block.id,
                block.start_pc.try_into().unwrap_or(u32::MAX),
            );
        }
        if runtime.debug.is_some() {
            if let Some(location) =
                register_statement_location(runtime, module, program.pou_id, block.start_pc)
            {
                if let Some(mut debug) = runtime.debug.clone() {
                    let call_depth =
                        depth_offset.saturating_add(frames.len().saturating_sub(1) as u32);
                    debug.refresh_snapshot_from_storage(runtime.storage(), runtime.current_time);
                    debug.on_statement(Some(&location), call_depth);
                }
            }
        }

        let outcome = if tier1_enabled {
            match maybe_execute_tier1_block(
                runtime,
                module,
                program,
                block,
                &mut frames,
                registers,
                native_call_stack,
                budget,
                depth_offset,
            )? {
                Some(outcome) => outcome,
                None => execute_register_block_interpreted(
                    runtime,
                    module,
                    program,
                    &mut frames,
                    registers,
                    remaining_register_reads,
                    native_call_stack,
                    block,
                    budget,
                    depth_offset,
                )?,
            }
        } else {
            execute_register_block_interpreted(
                runtime,
                module,
                program,
                &mut frames,
                registers,
                remaining_register_reads,
                native_call_stack,
                block,
                budget,
                depth_offset,
            )?
        };

        match outcome {
            RegisterBlockExecutionOutcome::ReturnFromPou => {
                let finished = frames.pop().map_err(VmTrap::into_runtime_error)?;
                runtime
                    .vm_register_profile
                    .record_call_op(RegisterCallOpKind::FramePop);
                if frames.is_empty() {
                    return Ok(build_register_pou_result(finished, capture_return));
                }
                return Err(invalid_bytecode(format!(
                    "register-ir executor unsupported nested call return_pc={}",
                    finished.return_pc
                )));
            }
            RegisterBlockExecutionOutcome::Continue(control_target) => {
                let next_target = match control_target {
                    Some(target) => target,
                    None => match program.blocks.get(block_index + 1) {
                        Some(next) => BlockTarget::Block(next.id),
                        None => BlockTarget::Exit,
                    },
                };
                match next_target {
                    BlockTarget::Block(next_block) => current_block = next_block,
                    BlockTarget::Exit => {
                        let finished = frames.pop().map_err(VmTrap::into_runtime_error)?;
                        runtime
                            .vm_register_profile
                            .record_call_op(RegisterCallOpKind::FramePop);
                        return Ok(build_register_pou_result(finished, capture_return));
                    }
                }
            }
        }
    }
}

fn build_register_pou_result(
    frame: super::frames::VmFrame,
    capture_return: bool,
) -> RegisterPouExecutionResult {
    let return_value = if capture_return {
        frame.locals.first().cloned()
    } else {
        None
    };
    RegisterPouExecutionResult {
        return_value,
        locals: frame.locals,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RegisterBlockExecutionOutcome {
    Continue(Option<BlockTarget>),
    ReturnFromPou,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tier1BlockExecutionOutcome {
    Executed(RegisterBlockExecutionOutcome),
    Deopt(&'static str),
}

enum BorrowedBinaryEval {
    GuardHit(Value),
    Materialized {
        left: Value,
        left_cloned: bool,
        right: Value,
        right_cloned: bool,
    },
}

#[allow(clippy::too_many_arguments)]
fn maybe_execute_tier1_block(
    runtime: &mut Runtime,
    module: &VmModule,
    program: &RegisterProgram,
    block: &RegisterBlock,
    frames: &mut FrameStack,
    registers: &mut [Value],
    native_call_stack: &mut OperandStack,
    budget: &mut usize,
    depth_offset: u32,
) -> Result<Option<RegisterBlockExecutionOutcome>, RuntimeError> {
    if !runtime.vm_tier1_specialized_executor.enabled() {
        return Ok(None);
    }

    let key = tier1_block_key(module, program.pou_id, block);
    let mut compiled = runtime
        .vm_tier1_specialized_executor
        .compiled_block(&key)
        .map(Arc::clone);
    let hits = runtime.vm_tier1_specialized_executor.track_block_hit(key);
    if compiled.is_none()
        && runtime
            .vm_tier1_specialized_executor
            .can_attempt_compile(hits, &key)
    {
        runtime
            .vm_tier1_specialized_executor
            .record_compile_attempt();
        match compile_tier1_block(module, block, key) {
            Ok(compiled_block) => {
                let compiled_block = Arc::new(compiled_block);
                runtime
                    .vm_tier1_specialized_executor
                    .record_compile_success();
                runtime
                    .vm_tier1_specialized_executor
                    .insert_compiled_block(Arc::clone(&compiled_block));
                compiled = Some(compiled_block);
            }
            Err(reason) => {
                runtime
                    .vm_tier1_specialized_executor
                    .record_compile_failure(reason);
            }
        }
    }

    let Some(compiled) = compiled else {
        return Ok(None);
    };

    match execute_tier1_compiled_block(
        runtime,
        module,
        program,
        block,
        frames,
        registers,
        native_call_stack,
        compiled.as_ref(),
        budget,
        depth_offset,
    )? {
        Tier1BlockExecutionOutcome::Executed(outcome) => {
            runtime
                .vm_tier1_specialized_executor
                .record_block_execution();
            Ok(Some(outcome))
        }
        Tier1BlockExecutionOutcome::Deopt(reason) => {
            runtime.vm_tier1_specialized_executor.record_deopt(reason);
            runtime
                .vm_tier1_specialized_executor
                .remove_compiled_block(&key);
            Ok(None)
        }
    }
}

fn tier1_block_key(module: &VmModule, pou_id: u32, block: &RegisterBlock) -> Tier1BlockKey {
    Tier1BlockKey {
        module_ptr: module as *const VmModule as usize,
        pou_id,
        block_id: block.id,
        start_pc: block.start_pc.try_into().unwrap_or(u32::MAX),
    }
}

fn compile_tier1_block(
    module: &VmModule,
    block: &RegisterBlock,
    key: Tier1BlockKey,
) -> Result<Tier1CompiledBlock, String> {
    let mut instructions = Vec::with_capacity(block.instructions.len());
    for instruction in &block.instructions {
        let compiled = match instruction {
            RegisterInstr::Nop => Tier1CompiledInstr::Nop,
            RegisterInstr::LoadConst { dest, const_idx } => {
                let value = module
                    .consts
                    .get(*const_idx as usize)
                    .cloned()
                    .ok_or_else(|| "invalid_const_idx".to_string())?;
                Tier1CompiledInstr::LoadConst { dest: *dest, value }
            }
            RegisterInstr::LoadNull { dest } => Tier1CompiledInstr::LoadNull { dest: *dest },
            RegisterInstr::LoadSelf { dest } => Tier1CompiledInstr::LoadSelf { dest: *dest },
            RegisterInstr::LoadSuper { dest } => Tier1CompiledInstr::LoadSuper { dest: *dest },
            RegisterInstr::Move { src, dest } => Tier1CompiledInstr::Move {
                src: *src,
                dest: *dest,
            },
            RegisterInstr::CallNative {
                kind,
                symbol_idx,
                args,
                dest,
            } => Tier1CompiledInstr::CallNative {
                kind: *kind,
                symbol_idx: *symbol_idx,
                args: args.clone().into_boxed_slice(),
                dest: *dest,
            },
            RegisterInstr::LoadRef { dest, ref_idx } => Tier1CompiledInstr::LoadRef {
                dest: *dest,
                ref_idx: *ref_idx,
            },
            RegisterInstr::LoadRefAddr { dest, ref_idx } => Tier1CompiledInstr::LoadRefAddr {
                dest: *dest,
                ref_idx: *ref_idx,
            },
            RegisterInstr::StoreRef { ref_idx, src } => Tier1CompiledInstr::StoreRef {
                ref_idx: *ref_idx,
                src: *src,
            },
            RegisterInstr::RefField {
                base,
                field_idx,
                dest,
            } => {
                let field = module
                    .strings
                    .get(*field_idx as usize)
                    .cloned()
                    .ok_or_else(|| "invalid_string_idx".to_string())?;
                Tier1CompiledInstr::RefField {
                    base: *base,
                    field,
                    dest: *dest,
                }
            }
            RegisterInstr::RefIndex { base, index, dest } => Tier1CompiledInstr::RefIndex {
                base: *base,
                index: *index,
                dest: *dest,
            },
            RegisterInstr::LoadDynamic { reference, dest } => Tier1CompiledInstr::LoadDynamic {
                reference: *reference,
                dest: *dest,
            },
            RegisterInstr::StoreDynamic { reference, value } => Tier1CompiledInstr::StoreDynamic {
                reference: *reference,
                value: *value,
            },
            RegisterInstr::Unary { op, src, dest } => Tier1CompiledInstr::Unary {
                op: *op,
                src: *src,
                dest: *dest,
            },
            RegisterInstr::Binary {
                op,
                left,
                right,
                dest,
            } => {
                if !is_tier1_supported_binary_op(*op) {
                    return Err(format!("unsupported_binary_op:{op:?}").to_ascii_lowercase());
                }
                Tier1CompiledInstr::BinaryDIntGuard {
                    op: *op,
                    left: *left,
                    right: *right,
                    dest: *dest,
                }
            }
            RegisterInstr::BinaryRefToRef {
                op,
                left_ref_idx,
                right_ref_idx,
                dest_ref_idx,
            } => {
                if !is_tier1_supported_binary_op(*op) {
                    return Err(format!("unsupported_binary_op:{op:?}").to_ascii_lowercase());
                }
                Tier1CompiledInstr::BinaryRefToRefDIntGuard {
                    op: *op,
                    left_ref_idx: *left_ref_idx,
                    right_ref_idx: *right_ref_idx,
                    dest_ref_idx: *dest_ref_idx,
                }
            }
            RegisterInstr::BinaryRefConstToRef {
                op,
                left_ref_idx,
                const_idx,
                dest_ref_idx,
            } => {
                if !is_tier1_supported_binary_op(*op) {
                    return Err(format!("unsupported_binary_op:{op:?}").to_ascii_lowercase());
                }
                Tier1CompiledInstr::BinaryRefConstToRefDIntGuard {
                    op: *op,
                    left_ref_idx: *left_ref_idx,
                    const_idx: *const_idx,
                    dest_ref_idx: *dest_ref_idx,
                }
            }
            RegisterInstr::BinaryConstRefToRef {
                op,
                const_idx,
                right_ref_idx,
                dest_ref_idx,
            } => {
                if !is_tier1_supported_binary_op(*op) {
                    return Err(format!("unsupported_binary_op:{op:?}").to_ascii_lowercase());
                }
                Tier1CompiledInstr::BinaryConstRefToRefDIntGuard {
                    op: *op,
                    const_idx: *const_idx,
                    right_ref_idx: *right_ref_idx,
                    dest_ref_idx: *dest_ref_idx,
                }
            }
            RegisterInstr::CmpRefConstJumpIf {
                op,
                ref_idx,
                const_idx,
                jump_if_true,
                target,
            } => {
                if !is_cmp_binary_op(*op) {
                    return Err(format!("unsupported_cmp_op:{op:?}").to_ascii_lowercase());
                }
                Tier1CompiledInstr::CmpRefConstJumpIfDIntGuard {
                    op: *op,
                    ref_idx: *ref_idx,
                    const_idx: *const_idx,
                    jump_if_true: *jump_if_true,
                    target: *target,
                }
            }
            RegisterInstr::Jump { target } => Tier1CompiledInstr::Jump { target: *target },
            RegisterInstr::JumpIf {
                cond,
                jump_if_true,
                target,
            } => Tier1CompiledInstr::JumpIf {
                cond: *cond,
                jump_if_true: *jump_if_true,
                target: *target,
            },
            RegisterInstr::Return => Tier1CompiledInstr::Return,
            RegisterInstr::SizeOfType { .. } => {
                return Err("unsupported_instr:size_of_type".to_string())
            }
            RegisterInstr::SizeOfValue { .. } => {
                return Err("unsupported_instr:size_of_value".to_string())
            }
            RegisterInstr::VmFallback { opcode, .. } => {
                return Err(format!(
                    "unsupported_instr:vm_fallback_opcode_{opcode:#04x}"
                ));
            }
        };
        instructions.push(compiled);
    }

    Ok(Tier1CompiledBlock { key, instructions })
}

#[allow(clippy::too_many_arguments)]
fn execute_tier1_compiled_block(
    runtime: &mut Runtime,
    module: &VmModule,
    program: &RegisterProgram,
    source_block: &RegisterBlock,
    frames: &mut FrameStack,
    registers: &mut [Value],
    native_call_stack: &mut OperandStack,
    block: &Tier1CompiledBlock,
    budget: &mut usize,
    depth_offset: u32,
) -> Result<Tier1BlockExecutionOutcome, RuntimeError> {
    let mut control_target = None;
    for (instruction_index, instruction) in block.instructions.iter().enumerate() {
        if should_check_register_deadline(instruction_index)
            && deadline_exceeded(runtime.execution_deadline)
        {
            return Err(VmTrap::DeadlineExceeded.into_runtime_error());
        }

        match instruction {
            Tier1CompiledInstr::Nop => {}
            Tier1CompiledInstr::LoadConst { dest, value } => {
                let (value, cloned) = materialize_borrowed_value(value);
                if cloned {
                    runtime
                        .vm_register_profile
                        .record_value_op(RegisterValueOpKind::ConstLoadClone);
                }
                write_register(registers, *dest, value)?;
            }
            Tier1CompiledInstr::LoadNull { dest } => {
                write_register(registers, *dest, Value::Null)?;
            }
            Tier1CompiledInstr::LoadSelf { dest } => {
                let frame = frames
                    .current()
                    .ok_or_else(|| VmTrap::CallStackUnderflow.into_runtime_error())?;
                let self_instance = frame.runtime_instance.ok_or_else(|| {
                    VmTrap::Runtime(RuntimeError::TypeMismatch).into_runtime_error()
                })?;
                write_register(registers, *dest, Value::Instance(self_instance))?;
            }
            Tier1CompiledInstr::LoadSuper { dest } => {
                let frame = frames
                    .current()
                    .ok_or_else(|| VmTrap::CallStackUnderflow.into_runtime_error())?;
                let self_instance = frame.runtime_instance.ok_or_else(|| {
                    VmTrap::Runtime(RuntimeError::TypeMismatch).into_runtime_error()
                })?;
                let instance = runtime
                    .storage
                    .get_instance(self_instance)
                    .ok_or_else(|| VmTrap::NullReference.into_runtime_error())?;
                let super_instance = instance.parent.ok_or_else(|| {
                    VmTrap::Runtime(RuntimeError::TypeMismatch).into_runtime_error()
                })?;
                write_register(registers, *dest, Value::Instance(super_instance))?;
            }
            Tier1CompiledInstr::Move { src, dest } => {
                let value = read_register(registers, *src)?;
                write_register(registers, *dest, value)?;
            }
            Tier1CompiledInstr::CallNative {
                kind,
                symbol_idx,
                args,
                dest,
            } => {
                native_call_stack.clear();
                for arg in args.iter() {
                    let value = read_register(registers, *arg)?;
                    native_call_stack
                        .push(value)
                        .map_err(VmTrap::into_runtime_error)?;
                }
                let arg_count = u32::try_from(args.len())
                    .map_err(|_| invalid_bytecode("tier-1 call-native arg_count overflow"))?;
                let caller_depth =
                    depth_offset.saturating_add(frames.len().saturating_sub(1) as u32);
                let frame = frames
                    .current_mut()
                    .ok_or_else(|| VmTrap::CallStackUnderflow.into_runtime_error())?;
                let result = execute_native_call(
                    runtime,
                    module,
                    frame,
                    native_call_stack,
                    caller_depth,
                    budget,
                    *kind,
                    *symbol_idx,
                    arg_count,
                )
                .map_err(VmTrap::into_runtime_error)?;
                write_register(registers, *dest, result)?;
            }
            Tier1CompiledInstr::LoadRef { dest, ref_idx } => {
                runtime
                    .vm_register_profile
                    .record_ref_op(RegisterRefOpKind::LoadRef);
                let value = {
                    let value = peek_ref(runtime, module, frames, *ref_idx)
                        .map_err(VmTrap::into_runtime_error)?;
                    let (value, cloned) = materialize_borrowed_value(value);
                    if cloned {
                        runtime
                            .vm_register_profile
                            .record_value_op(RegisterValueOpKind::ReadValueClone);
                    }
                    value
                };
                write_register(registers, *dest, value)?;
            }
            Tier1CompiledInstr::LoadRefAddr { dest, ref_idx } => {
                runtime
                    .vm_register_profile
                    .record_ref_op(RegisterRefOpKind::LoadRefAddr);
                let reference =
                    load_ref_addr(module, frames, *ref_idx).map_err(VmTrap::into_runtime_error)?;
                write_register(registers, *dest, Value::Reference(Some(reference)))?;
            }
            Tier1CompiledInstr::StoreRef { ref_idx, src } => {
                runtime
                    .vm_register_profile
                    .record_ref_op(RegisterRefOpKind::StoreRef);
                let value = read_register(registers, *src)?;
                store_ref(runtime, module, frames, *ref_idx, value)
                    .map_err(VmTrap::into_runtime_error)?;
            }
            Tier1CompiledInstr::RefField { base, field, dest } => {
                runtime
                    .vm_register_profile
                    .record_ref_op(RegisterRefOpKind::RefField);
                let next = match read_register_ref(registers, *base)? {
                    Value::Reference(Some(reference)) => {
                        dynamic_ref_field(runtime, frames, reference.clone(), field.clone())
                            .map_err(VmTrap::into_runtime_error)?
                    }
                    Value::Reference(None) => {
                        return Err(VmTrap::NullReference.into_runtime_error());
                    }
                    Value::Instance(instance_id) => {
                        runtime
                            .vm_register_profile
                            .record_ref_op(RegisterRefOpKind::InstanceFieldLookup);
                        runtime
                            .storage
                            .ref_for_instance_recursive(*instance_id, field.as_str())
                            .ok_or_else(|| {
                                VmTrap::Runtime(RuntimeError::UndefinedField(field.clone()))
                                    .into_runtime_error()
                            })?
                    }
                    _ => {
                        return Err(VmTrap::Runtime(RuntimeError::TypeMismatch).into_runtime_error())
                    }
                };
                write_register(registers, *dest, Value::Reference(Some(next)))?;
            }
            Tier1CompiledInstr::RefIndex { base, index, dest } => {
                runtime
                    .vm_register_profile
                    .record_ref_op(RegisterRefOpKind::RefIndex);
                let index = index_to_i64(read_register(registers, *index)?)
                    .map_err(VmTrap::into_runtime_error)?;
                let reference = read_reference_register(registers, *base)?;
                let next = dynamic_ref_index(runtime, frames, reference, index)
                    .map_err(VmTrap::into_runtime_error)?;
                write_register(registers, *dest, Value::Reference(Some(next)))?;
            }
            Tier1CompiledInstr::LoadDynamic { reference, dest } => {
                runtime
                    .vm_register_profile
                    .record_ref_op(RegisterRefOpKind::LoadDynamic);
                let reference = read_reference_register(registers, *reference)?;
                let value = {
                    let value = peek_dynamic_ref(runtime, frames, &reference)
                        .map_err(VmTrap::into_runtime_error)?;
                    let (value, cloned) = materialize_borrowed_value(value);
                    if cloned {
                        runtime
                            .vm_register_profile
                            .record_value_op(RegisterValueOpKind::ReadValueClone);
                    }
                    value
                };
                write_register(registers, *dest, value)?;
            }
            Tier1CompiledInstr::StoreDynamic { reference, value } => {
                runtime
                    .vm_register_profile
                    .record_ref_op(RegisterRefOpKind::StoreDynamic);
                let reference = read_reference_register(registers, *reference)?;
                let value = read_register(registers, *value)?;
                dynamic_store_ref(runtime, frames, &reference, value)
                    .map_err(VmTrap::into_runtime_error)?;
            }
            Tier1CompiledInstr::Unary { op, src, dest } => {
                let source = read_register(registers, *src)?;
                let result = apply_unary(*op, source)?;
                write_register(registers, *dest, result)?;
            }
            Tier1CompiledInstr::BinaryDIntGuard {
                op,
                left,
                right,
                dest,
            } => {
                let left = read_register(registers, *left)?;
                let right = read_register(registers, *right)?;
                let result =
                    if let Some(result) = apply_dint_binary_guard_borrowed(*op, &left, &right)? {
                        result
                    } else {
                        apply_binary(*op, left, right, &runtime.profile)?
                    };
                write_register(registers, *dest, result)?;
            }
            Tier1CompiledInstr::BinaryRefToRefDIntGuard {
                op,
                left_ref_idx,
                right_ref_idx,
                dest_ref_idx,
            } => {
                let eval = {
                    let left = peek_ref(runtime, module, frames, *left_ref_idx)
                        .map_err(VmTrap::into_runtime_error)?;
                    let right = peek_ref(runtime, module, frames, *right_ref_idx)
                        .map_err(VmTrap::into_runtime_error)?;
                    prepare_borrowed_binary_eval(*op, left, right)?
                };
                let result = match eval {
                    BorrowedBinaryEval::GuardHit(result) => result,
                    BorrowedBinaryEval::Materialized {
                        left,
                        left_cloned,
                        right,
                        right_cloned,
                    } => {
                        if left_cloned {
                            runtime
                                .vm_register_profile
                                .record_value_op(RegisterValueOpKind::ReadValueClone);
                        }
                        if right_cloned {
                            runtime
                                .vm_register_profile
                                .record_value_op(RegisterValueOpKind::ReadValueClone);
                        }
                        apply_binary(*op, left, right, &runtime.profile)?
                    }
                };
                store_ref(runtime, module, frames, *dest_ref_idx, result)
                    .map_err(VmTrap::into_runtime_error)?;
            }
            Tier1CompiledInstr::BinaryRefConstToRefDIntGuard {
                op,
                left_ref_idx,
                const_idx,
                dest_ref_idx,
            } => {
                let eval = {
                    let left = peek_ref(runtime, module, frames, *left_ref_idx)
                        .map_err(VmTrap::into_runtime_error)?;
                    let right = module
                        .consts
                        .get(*const_idx as usize)
                        .ok_or(VmTrap::InvalidConstIndex(*const_idx))
                        .map_err(VmTrap::into_runtime_error)?;
                    prepare_borrowed_binary_eval(*op, left, right)?
                };
                let result = match eval {
                    BorrowedBinaryEval::GuardHit(result) => result,
                    BorrowedBinaryEval::Materialized {
                        left,
                        left_cloned,
                        right,
                        right_cloned,
                    } => {
                        if left_cloned {
                            runtime
                                .vm_register_profile
                                .record_value_op(RegisterValueOpKind::ReadValueClone);
                        }
                        if right_cloned {
                            runtime
                                .vm_register_profile
                                .record_value_op(RegisterValueOpKind::ConstLoadClone);
                        }
                        apply_binary(*op, left, right, &runtime.profile)?
                    }
                };
                store_ref(runtime, module, frames, *dest_ref_idx, result)
                    .map_err(VmTrap::into_runtime_error)?;
            }
            Tier1CompiledInstr::BinaryConstRefToRefDIntGuard {
                op,
                const_idx,
                right_ref_idx,
                dest_ref_idx,
            } => {
                let eval = {
                    let left = module
                        .consts
                        .get(*const_idx as usize)
                        .ok_or(VmTrap::InvalidConstIndex(*const_idx))
                        .map_err(VmTrap::into_runtime_error)?;
                    let right = peek_ref(runtime, module, frames, *right_ref_idx)
                        .map_err(VmTrap::into_runtime_error)?;
                    prepare_borrowed_binary_eval(*op, left, right)?
                };
                let result = match eval {
                    BorrowedBinaryEval::GuardHit(result) => result,
                    BorrowedBinaryEval::Materialized {
                        left,
                        left_cloned,
                        right,
                        right_cloned,
                    } => {
                        if left_cloned {
                            runtime
                                .vm_register_profile
                                .record_value_op(RegisterValueOpKind::ConstLoadClone);
                        }
                        if right_cloned {
                            runtime
                                .vm_register_profile
                                .record_value_op(RegisterValueOpKind::ReadValueClone);
                        }
                        apply_binary(*op, left, right, &runtime.profile)?
                    }
                };
                store_ref(runtime, module, frames, *dest_ref_idx, result)
                    .map_err(VmTrap::into_runtime_error)?;
            }
            Tier1CompiledInstr::CmpRefConstJumpIfDIntGuard {
                op,
                ref_idx,
                const_idx,
                jump_if_true,
                target,
            } => {
                let eval = {
                    let left = peek_ref(runtime, module, frames, *ref_idx)
                        .map_err(VmTrap::into_runtime_error)?;
                    let right = module
                        .consts
                        .get(*const_idx as usize)
                        .ok_or(VmTrap::InvalidConstIndex(*const_idx))
                        .map_err(VmTrap::into_runtime_error)?;
                    prepare_borrowed_binary_eval(*op, left, right)?
                };
                let result = match eval {
                    BorrowedBinaryEval::GuardHit(result) => result,
                    BorrowedBinaryEval::Materialized {
                        left,
                        left_cloned,
                        right,
                        right_cloned,
                    } => {
                        if left_cloned {
                            runtime
                                .vm_register_profile
                                .record_value_op(RegisterValueOpKind::ReadValueClone);
                        }
                        if right_cloned {
                            runtime
                                .vm_register_profile
                                .record_value_op(RegisterValueOpKind::ConstLoadClone);
                        }
                        apply_binary(*op, left, right, &runtime.profile)?
                    }
                };
                let condition = match result {
                    Value::Bool(value) => value,
                    _ => return Err(VmTrap::ConditionNotBool.into_runtime_error()),
                };
                if condition == *jump_if_true {
                    consume_loop_budget_for_block_target(program, source_block, *target, budget)?;
                    control_target = Some(*target);
                    break;
                }
            }
            Tier1CompiledInstr::Jump { target } => {
                consume_loop_budget_for_block_target(program, source_block, *target, budget)?;
                control_target = Some(*target);
                break;
            }
            Tier1CompiledInstr::JumpIf {
                cond,
                jump_if_true,
                target,
            } => {
                let condition = read_bool_register(registers, *cond)?;
                if condition == *jump_if_true {
                    consume_loop_budget_for_block_target(program, source_block, *target, budget)?;
                    control_target = Some(*target);
                    break;
                }
            }
            Tier1CompiledInstr::Return => {
                return Ok(Tier1BlockExecutionOutcome::Executed(
                    RegisterBlockExecutionOutcome::ReturnFromPou,
                ));
            }
        }
    }
    Ok(Tier1BlockExecutionOutcome::Executed(
        RegisterBlockExecutionOutcome::Continue(control_target),
    ))
}

fn apply_dint_binary_guard_borrowed(
    op: BinaryOp,
    left: &Value,
    right: &Value,
) -> Result<Option<Value>, RuntimeError> {
    let (left, right) = match (left, right) {
        (Value::DInt(left), Value::DInt(right)) => (*left, *right),
        _ => return Ok(None),
    };

    let value = match op {
        BinaryOp::Add => Value::DInt(left.checked_add(right).ok_or(RuntimeError::Overflow)?),
        BinaryOp::Sub => Value::DInt(left.checked_sub(right).ok_or(RuntimeError::Overflow)?),
        BinaryOp::Mul => Value::DInt(left.checked_mul(right).ok_or(RuntimeError::Overflow)?),
        BinaryOp::Div => {
            if right == 0 {
                return Err(RuntimeError::DivisionByZero);
            }
            Value::DInt(left.checked_div(right).ok_or(RuntimeError::Overflow)?)
        }
        BinaryOp::Mod => {
            if right == 0 {
                return Err(RuntimeError::DivisionByZero);
            }
            Value::DInt(left.checked_rem(right).ok_or(RuntimeError::Overflow)?)
        }
        BinaryOp::Eq => Value::Bool(left == right),
        BinaryOp::Ne => Value::Bool(left != right),
        BinaryOp::Lt => Value::Bool(left < right),
        BinaryOp::Le => Value::Bool(left <= right),
        BinaryOp::Gt => Value::Bool(left > right),
        BinaryOp::Ge => Value::Bool(left >= right),
        _ => return Ok(None),
    };
    Ok(Some(value))
}

fn prepare_borrowed_binary_eval(
    op: BinaryOp,
    left: &Value,
    right: &Value,
) -> Result<BorrowedBinaryEval, RuntimeError> {
    if let Some(result) = apply_dint_binary_guard_borrowed(op, left, right)? {
        return Ok(BorrowedBinaryEval::GuardHit(result));
    }

    let (left, left_cloned) = materialize_borrowed_value(left);
    let (right, right_cloned) = materialize_borrowed_value(right);
    Ok(BorrowedBinaryEval::Materialized {
        left,
        left_cloned,
        right,
        right_cloned,
    })
}

fn apply_dint_binary_guard(
    op: BinaryOp,
    left: Value,
    right: Value,
) -> Result<Option<Value>, RuntimeError> {
    apply_dint_binary_guard_borrowed(op, &left, &right)
}

fn is_tier1_supported_binary_op(op: BinaryOp) -> bool {
    matches!(
        op,
        BinaryOp::Add
            | BinaryOp::Sub
            | BinaryOp::Mul
            | BinaryOp::Div
            | BinaryOp::Mod
            | BinaryOp::Pow
            | BinaryOp::And
            | BinaryOp::Or
            | BinaryOp::Xor
            | BinaryOp::Eq
            | BinaryOp::Ne
            | BinaryOp::Lt
            | BinaryOp::Le
            | BinaryOp::Gt
            | BinaryOp::Ge
    )
}

#[allow(clippy::too_many_arguments)]
fn execute_register_block_interpreted(
    runtime: &mut Runtime,
    module: &VmModule,
    program: &RegisterProgram,
    frames: &mut FrameStack,
    registers: &mut [Value],
    remaining_register_reads: &mut [u32],
    native_call_stack: &mut OperandStack,
    block: &RegisterBlock,
    budget: &mut usize,
    depth_offset: u32,
) -> Result<RegisterBlockExecutionOutcome, RuntimeError> {
    let mut control_target = None;
    for (instruction_index, instruction) in block.instructions.iter().enumerate() {
        if should_check_register_deadline(instruction_index)
            && deadline_exceeded(runtime.execution_deadline)
        {
            return Err(VmTrap::DeadlineExceeded.into_runtime_error());
        }

        match instruction {
            RegisterInstr::Nop => {}
            RegisterInstr::LoadConst { dest, const_idx } => {
                let value = module
                    .consts
                    .get(*const_idx as usize)
                    .map(|value| {
                        let (value, cloned) = materialize_borrowed_value(value);
                        if cloned {
                            runtime
                                .vm_register_profile
                                .record_value_op(RegisterValueOpKind::ConstLoadClone);
                        }
                        value
                    })
                    .ok_or(VmTrap::InvalidConstIndex(*const_idx))
                    .map_err(VmTrap::into_runtime_error)?;
                write_register(registers, *dest, value)?;
            }
            RegisterInstr::LoadNull { dest } => {
                write_register(registers, *dest, Value::Null)?;
            }
            RegisterInstr::LoadSelf { dest } => {
                let frame = frames
                    .current()
                    .ok_or_else(|| VmTrap::CallStackUnderflow.into_runtime_error())?;
                let self_instance = frame.runtime_instance.ok_or_else(|| {
                    VmTrap::Runtime(RuntimeError::TypeMismatch).into_runtime_error()
                })?;
                write_register(registers, *dest, Value::Instance(self_instance))?;
            }
            RegisterInstr::LoadSuper { dest } => {
                let frame = frames
                    .current()
                    .ok_or_else(|| VmTrap::CallStackUnderflow.into_runtime_error())?;
                let self_instance = frame.runtime_instance.ok_or_else(|| {
                    VmTrap::Runtime(RuntimeError::TypeMismatch).into_runtime_error()
                })?;
                let instance = runtime
                    .storage
                    .get_instance(self_instance)
                    .ok_or_else(|| VmTrap::NullReference.into_runtime_error())?;
                let super_instance = instance.parent.ok_or_else(|| {
                    VmTrap::Runtime(RuntimeError::TypeMismatch).into_runtime_error()
                })?;
                write_register(registers, *dest, Value::Instance(super_instance))?;
            }
            RegisterInstr::Move { src, dest } => {
                let value = read_register_with_counts(
                    &mut runtime.vm_register_profile,
                    registers,
                    remaining_register_reads,
                    *src,
                )?;
                write_register(registers, *dest, value)?;
            }
            RegisterInstr::LoadRef { dest, ref_idx } => {
                runtime
                    .vm_register_profile
                    .record_ref_op(RegisterRefOpKind::LoadRef);
                let value = {
                    let value = peek_ref(runtime, module, frames, *ref_idx)
                        .map_err(VmTrap::into_runtime_error)?;
                    let (value, cloned) = materialize_borrowed_value(value);
                    if cloned {
                        runtime
                            .vm_register_profile
                            .record_value_op(RegisterValueOpKind::ReadValueClone);
                    }
                    value
                };
                write_register(registers, *dest, value)?;
            }
            RegisterInstr::LoadRefAddr { dest, ref_idx } => {
                runtime
                    .vm_register_profile
                    .record_ref_op(RegisterRefOpKind::LoadRefAddr);
                let reference =
                    load_ref_addr(module, frames, *ref_idx).map_err(VmTrap::into_runtime_error)?;
                write_register(registers, *dest, Value::Reference(Some(reference)))?;
            }
            RegisterInstr::StoreRef { ref_idx, src } => {
                runtime
                    .vm_register_profile
                    .record_ref_op(RegisterRefOpKind::StoreRef);
                let value = read_register_with_counts(
                    &mut runtime.vm_register_profile,
                    registers,
                    remaining_register_reads,
                    *src,
                )?;
                store_ref(runtime, module, frames, *ref_idx, value)
                    .map_err(VmTrap::into_runtime_error)?;
            }
            RegisterInstr::CallNative {
                kind,
                symbol_idx,
                args,
                dest,
            } => {
                native_call_stack.clear();
                for arg in args {
                    let value = read_register_with_counts(
                        &mut runtime.vm_register_profile,
                        registers,
                        remaining_register_reads,
                        *arg,
                    )?;
                    native_call_stack
                        .push(value)
                        .map_err(VmTrap::into_runtime_error)?;
                }
                let arg_count = u32::try_from(args.len())
                    .map_err(|_| invalid_bytecode("register-ir executor arg_count overflow"))?;
                let caller_depth =
                    depth_offset.saturating_add(frames.len().saturating_sub(1) as u32);
                let frame = frames
                    .current_mut()
                    .ok_or_else(|| VmTrap::CallStackUnderflow.into_runtime_error())?;
                let result = execute_native_call(
                    runtime,
                    module,
                    frame,
                    native_call_stack,
                    caller_depth,
                    budget,
                    *kind,
                    *symbol_idx,
                    arg_count,
                )
                .map_err(VmTrap::into_runtime_error)?;
                write_register(registers, *dest, result)?;
            }
            RegisterInstr::SizeOfType { type_idx, dest } => {
                let size = sizeof_type_from_table(&module.types, *type_idx)
                    .map_err(|err| VmTrap::Runtime(err).into_runtime_error())?;
                let size = i32::try_from(size)
                    .map_err(|_| VmTrap::Runtime(RuntimeError::Overflow).into_runtime_error())?;
                write_register(registers, *dest, Value::DInt(size))?;
            }
            RegisterInstr::SizeOfValue { src, dest } => {
                let value = read_register_with_counts(
                    &mut runtime.vm_register_profile,
                    registers,
                    remaining_register_reads,
                    *src,
                )?;
                let size = size_of_value(runtime.registry(), &value)
                    .map_err(sizeof_error_to_runtime)
                    .map_err(|err| VmTrap::Runtime(err).into_runtime_error())?;
                let size = i32::try_from(size)
                    .map_err(|_| VmTrap::Runtime(RuntimeError::Overflow).into_runtime_error())?;
                write_register(registers, *dest, Value::DInt(size))?;
            }
            RegisterInstr::RefField {
                base,
                field_idx,
                dest,
            } => {
                runtime
                    .vm_register_profile
                    .record_ref_op(RegisterRefOpKind::RefField);
                let field = module
                    .strings
                    .get(*field_idx as usize)
                    .cloned()
                    .ok_or_else(|| {
                        VmTrap::BytecodeDecode(
                            format!("invalid index {field_idx} for string").into(),
                        )
                        .into_runtime_error()
                    })?;
                let base_value = read_register_with_counts(
                    &mut runtime.vm_register_profile,
                    registers,
                    remaining_register_reads,
                    *base,
                )?;
                let next = match base_value {
                    Value::Reference(Some(reference)) => {
                        dynamic_ref_field(runtime, frames, reference, field.clone())
                            .map_err(VmTrap::into_runtime_error)?
                    }
                    Value::Reference(None) => {
                        return Err(VmTrap::NullReference.into_runtime_error());
                    }
                    Value::Instance(instance_id) => {
                        runtime
                            .vm_register_profile
                            .record_ref_op(RegisterRefOpKind::InstanceFieldLookup);
                        runtime
                            .storage
                            .ref_for_instance_recursive(instance_id, field.as_str())
                            .ok_or_else(|| {
                                VmTrap::Runtime(RuntimeError::UndefinedField(field))
                                    .into_runtime_error()
                            })?
                    }
                    _ => {
                        return Err(VmTrap::Runtime(RuntimeError::TypeMismatch).into_runtime_error())
                    }
                };
                write_register(registers, *dest, Value::Reference(Some(next)))?;
            }
            RegisterInstr::RefIndex { base, index, dest } => {
                runtime
                    .vm_register_profile
                    .record_ref_op(RegisterRefOpKind::RefIndex);
                let index_value = read_register_with_counts(
                    &mut runtime.vm_register_profile,
                    registers,
                    remaining_register_reads,
                    *index,
                )?;
                let index = index_to_i64(index_value).map_err(VmTrap::into_runtime_error)?;
                let reference = read_reference_register_with_counts(
                    &mut runtime.vm_register_profile,
                    registers,
                    remaining_register_reads,
                    *base,
                )?;
                let next = dynamic_ref_index(runtime, frames, reference, index)
                    .map_err(VmTrap::into_runtime_error)?;
                write_register(registers, *dest, Value::Reference(Some(next)))?;
            }
            RegisterInstr::LoadDynamic { reference, dest } => {
                runtime
                    .vm_register_profile
                    .record_ref_op(RegisterRefOpKind::LoadDynamic);
                let reference = read_reference_register_with_counts(
                    &mut runtime.vm_register_profile,
                    registers,
                    remaining_register_reads,
                    *reference,
                )?;
                let value = {
                    let value = peek_dynamic_ref(runtime, frames, &reference)
                        .map_err(VmTrap::into_runtime_error)?;
                    let (value, cloned) = materialize_borrowed_value(value);
                    if cloned {
                        runtime
                            .vm_register_profile
                            .record_value_op(RegisterValueOpKind::ReadValueClone);
                    }
                    value
                };
                write_register(registers, *dest, value)?;
            }
            RegisterInstr::StoreDynamic { reference, value } => {
                runtime
                    .vm_register_profile
                    .record_ref_op(RegisterRefOpKind::StoreDynamic);
                let reference = read_reference_register_with_counts(
                    &mut runtime.vm_register_profile,
                    registers,
                    remaining_register_reads,
                    *reference,
                )?;
                let value = read_register_with_counts(
                    &mut runtime.vm_register_profile,
                    registers,
                    remaining_register_reads,
                    *value,
                )?;
                dynamic_store_ref(runtime, frames, &reference, value)
                    .map_err(VmTrap::into_runtime_error)?;
            }
            RegisterInstr::Unary { op, src, dest } => {
                let source = read_register_with_counts(
                    &mut runtime.vm_register_profile,
                    registers,
                    remaining_register_reads,
                    *src,
                )?;
                let result = apply_unary(*op, source)?;
                write_register(registers, *dest, result)?;
            }
            RegisterInstr::Binary {
                op,
                left,
                right,
                dest,
            } => {
                let left = read_register_with_counts(
                    &mut runtime.vm_register_profile,
                    registers,
                    remaining_register_reads,
                    *left,
                )?;
                let right = read_register_with_counts(
                    &mut runtime.vm_register_profile,
                    registers,
                    remaining_register_reads,
                    *right,
                )?;
                let result = apply_binary(*op, left, right, &runtime.profile)?;
                write_register(registers, *dest, result)?;
            }
            RegisterInstr::BinaryRefToRef {
                op,
                left_ref_idx,
                right_ref_idx,
                dest_ref_idx,
            } => {
                let eval = {
                    let left = peek_ref(runtime, module, frames, *left_ref_idx)
                        .map_err(VmTrap::into_runtime_error)?;
                    let right = peek_ref(runtime, module, frames, *right_ref_idx)
                        .map_err(VmTrap::into_runtime_error)?;
                    prepare_borrowed_binary_eval(*op, left, right)?
                };
                let result = match eval {
                    BorrowedBinaryEval::GuardHit(result) => result,
                    BorrowedBinaryEval::Materialized {
                        left,
                        left_cloned,
                        right,
                        right_cloned,
                    } => {
                        if left_cloned {
                            runtime
                                .vm_register_profile
                                .record_value_op(RegisterValueOpKind::ReadValueClone);
                        }
                        if right_cloned {
                            runtime
                                .vm_register_profile
                                .record_value_op(RegisterValueOpKind::ReadValueClone);
                        }
                        apply_binary(*op, left, right, &runtime.profile)?
                    }
                };
                store_ref(runtime, module, frames, *dest_ref_idx, result)
                    .map_err(VmTrap::into_runtime_error)?;
            }
            RegisterInstr::BinaryRefConstToRef {
                op,
                left_ref_idx,
                const_idx,
                dest_ref_idx,
            } => {
                let eval = {
                    let left = peek_ref(runtime, module, frames, *left_ref_idx)
                        .map_err(VmTrap::into_runtime_error)?;
                    let right = module
                        .consts
                        .get(*const_idx as usize)
                        .ok_or(VmTrap::InvalidConstIndex(*const_idx))
                        .map_err(VmTrap::into_runtime_error)?;
                    prepare_borrowed_binary_eval(*op, left, right)?
                };
                let result = match eval {
                    BorrowedBinaryEval::GuardHit(result) => result,
                    BorrowedBinaryEval::Materialized {
                        left,
                        left_cloned,
                        right,
                        right_cloned,
                    } => {
                        if left_cloned {
                            runtime
                                .vm_register_profile
                                .record_value_op(RegisterValueOpKind::ReadValueClone);
                        }
                        if right_cloned {
                            runtime
                                .vm_register_profile
                                .record_value_op(RegisterValueOpKind::ConstLoadClone);
                        }
                        apply_binary(*op, left, right, &runtime.profile)?
                    }
                };
                store_ref(runtime, module, frames, *dest_ref_idx, result)
                    .map_err(VmTrap::into_runtime_error)?;
            }
            RegisterInstr::BinaryConstRefToRef {
                op,
                const_idx,
                right_ref_idx,
                dest_ref_idx,
            } => {
                let eval = {
                    let left = module
                        .consts
                        .get(*const_idx as usize)
                        .ok_or(VmTrap::InvalidConstIndex(*const_idx))
                        .map_err(VmTrap::into_runtime_error)?;
                    let right = peek_ref(runtime, module, frames, *right_ref_idx)
                        .map_err(VmTrap::into_runtime_error)?;
                    prepare_borrowed_binary_eval(*op, left, right)?
                };
                let result = match eval {
                    BorrowedBinaryEval::GuardHit(result) => result,
                    BorrowedBinaryEval::Materialized {
                        left,
                        left_cloned,
                        right,
                        right_cloned,
                    } => {
                        if left_cloned {
                            runtime
                                .vm_register_profile
                                .record_value_op(RegisterValueOpKind::ConstLoadClone);
                        }
                        if right_cloned {
                            runtime
                                .vm_register_profile
                                .record_value_op(RegisterValueOpKind::ReadValueClone);
                        }
                        apply_binary(*op, left, right, &runtime.profile)?
                    }
                };
                store_ref(runtime, module, frames, *dest_ref_idx, result)
                    .map_err(VmTrap::into_runtime_error)?;
            }
            RegisterInstr::CmpRefConstJumpIf {
                op,
                ref_idx,
                const_idx,
                jump_if_true,
                target,
            } => {
                let eval = {
                    let left = peek_ref(runtime, module, frames, *ref_idx)
                        .map_err(VmTrap::into_runtime_error)?;
                    let right = module
                        .consts
                        .get(*const_idx as usize)
                        .ok_or(VmTrap::InvalidConstIndex(*const_idx))
                        .map_err(VmTrap::into_runtime_error)?;
                    prepare_borrowed_binary_eval(*op, left, right)?
                };
                let result = match eval {
                    BorrowedBinaryEval::GuardHit(result) => result,
                    BorrowedBinaryEval::Materialized {
                        left,
                        left_cloned,
                        right,
                        right_cloned,
                    } => {
                        if left_cloned {
                            runtime
                                .vm_register_profile
                                .record_value_op(RegisterValueOpKind::ReadValueClone);
                        }
                        if right_cloned {
                            runtime
                                .vm_register_profile
                                .record_value_op(RegisterValueOpKind::ConstLoadClone);
                        }
                        apply_binary(*op, left, right, &runtime.profile)?
                    }
                };
                let condition = match result {
                    Value::Bool(value) => value,
                    _ => return Err(VmTrap::ConditionNotBool.into_runtime_error()),
                };
                if condition == *jump_if_true {
                    consume_loop_budget_for_block_target(program, block, *target, budget)?;
                    control_target = Some(*target);
                    break;
                }
            }
            RegisterInstr::Jump { target } => {
                consume_loop_budget_for_block_target(program, block, *target, budget)?;
                control_target = Some(*target);
                break;
            }
            RegisterInstr::JumpIf {
                cond,
                jump_if_true,
                target,
            } => {
                let condition = read_bool_register_with_counts(
                    &mut runtime.vm_register_profile,
                    registers,
                    remaining_register_reads,
                    *cond,
                )?;
                if condition == *jump_if_true {
                    consume_loop_budget_for_block_target(program, block, *target, budget)?;
                    control_target = Some(*target);
                    break;
                }
            }
            RegisterInstr::Return => return Ok(RegisterBlockExecutionOutcome::ReturnFromPou),
            RegisterInstr::VmFallback { opcode, .. } => {
                return Err(invalid_bytecode(format!(
                    "register-ir executor encountered fallback opcode 0x{opcode:02X}",
                )));
            }
        }
    }
    Ok(RegisterBlockExecutionOutcome::Continue(control_target))
}

fn read_register_ref(registers: &[Value], register: RegisterId) -> Result<&Value, RuntimeError> {
    registers.get(register.index() as usize).ok_or_else(|| {
        invalid_bytecode(format!(
            "register-ir executor read out-of-bounds register {}",
            register.index()
        ))
    })
}

fn read_register(registers: &[Value], register: RegisterId) -> Result<Value, RuntimeError> {
    read_register_ref(registers, register).cloned()
}

fn register_read_counts_by_block(program: &RegisterProgram) -> Vec<Vec<u32>> {
    program
        .blocks
        .iter()
        .map(|block| register_read_counts_for_block(program.max_registers, block))
        .collect()
}

fn register_read_counts_for_block(max_registers: u32, block: &RegisterBlock) -> Vec<u32> {
    let mut counts = vec![0_u32; max_registers as usize];
    for instruction in &block.instructions {
        match instruction {
            RegisterInstr::CallNative { args, .. } => {
                for arg in args {
                    increment_register_read_count(&mut counts, *arg);
                }
            }
            RegisterInstr::SizeOfValue { src, .. } => {
                increment_register_read_count(&mut counts, *src);
            }
            RegisterInstr::RefField { base, .. } => {
                increment_register_read_count(&mut counts, *base);
            }
            RegisterInstr::RefIndex { base, index, .. } => {
                increment_register_read_count(&mut counts, *base);
                increment_register_read_count(&mut counts, *index);
            }
            RegisterInstr::LoadDynamic { reference, .. } => {
                increment_register_read_count(&mut counts, *reference);
            }
            RegisterInstr::StoreDynamic { reference, value } => {
                increment_register_read_count(&mut counts, *reference);
                increment_register_read_count(&mut counts, *value);
            }
            RegisterInstr::Unary { src, .. } => {
                increment_register_read_count(&mut counts, *src);
            }
            RegisterInstr::Binary { left, right, .. } => {
                increment_register_read_count(&mut counts, *left);
                increment_register_read_count(&mut counts, *right);
            }
            RegisterInstr::StoreRef { src, .. } => {
                increment_register_read_count(&mut counts, *src);
            }
            RegisterInstr::Move { src, .. } => {
                increment_register_read_count(&mut counts, *src);
            }
            RegisterInstr::JumpIf { cond, .. } => {
                increment_register_read_count(&mut counts, *cond);
            }
            RegisterInstr::Nop
            | RegisterInstr::LoadConst { .. }
            | RegisterInstr::LoadRef { .. }
            | RegisterInstr::LoadRefAddr { .. }
            | RegisterInstr::LoadNull { .. }
            | RegisterInstr::LoadSelf { .. }
            | RegisterInstr::LoadSuper { .. }
            | RegisterInstr::SizeOfType { .. }
            | RegisterInstr::BinaryRefToRef { .. }
            | RegisterInstr::BinaryRefConstToRef { .. }
            | RegisterInstr::BinaryConstRefToRef { .. }
            | RegisterInstr::CmpRefConstJumpIf { .. }
            | RegisterInstr::Jump { .. }
            | RegisterInstr::Return
            | RegisterInstr::VmFallback { .. } => {}
        }
    }
    for slot in 0..block.entry_stack_depth {
        if let Some(count) = counts.get_mut(slot as usize) {
            *count = count.saturating_add(1);
        }
    }
    counts
}

fn increment_register_read_count(counts: &mut [u32], register: RegisterId) {
    if let Some(slot) = counts.get_mut(register.index() as usize) {
        *slot = slot.saturating_add(1);
    }
}

fn read_register_with_counts(
    profile: &mut RegisterProfileState,
    registers: &mut [Value],
    remaining_register_reads: &mut [u32],
    register: RegisterId,
) -> Result<Value, RuntimeError> {
    let register_index = register.index() as usize;
    let consume = remaining_register_reads
        .get_mut(register_index)
        .ok_or_else(|| {
            invalid_bytecode(format!(
                "register-ir executor read-count out-of-bounds register {}",
                register.index()
            ))
        })
        .map(|slot| {
            if *slot == 0 {
                false
            } else {
                *slot = slot.saturating_sub(1);
                *slot == 0
            }
        })?;
    let slot = registers.get_mut(register_index).ok_or_else(|| {
        invalid_bytecode(format!(
            "register-ir executor read out-of-bounds register {}",
            register.index()
        ))
    })?;
    if consume {
        profile.record_value_op(RegisterValueOpKind::RegisterReadMove);
        Ok(std::mem::replace(slot, Value::Null))
    } else {
        profile.record_value_op(RegisterValueOpKind::RegisterReadClone);
        Ok(slot.clone())
    }
}

fn read_bool_register_with_counts(
    profile: &mut RegisterProfileState,
    registers: &mut [Value],
    remaining_register_reads: &mut [u32],
    register: RegisterId,
) -> Result<bool, RuntimeError> {
    match read_register_with_counts(profile, registers, remaining_register_reads, register)? {
        Value::Bool(value) => Ok(value),
        _ => Err(VmTrap::ConditionNotBool.into_runtime_error()),
    }
}

fn read_reference_register_with_counts(
    profile: &mut RegisterProfileState,
    registers: &mut [Value],
    remaining_register_reads: &mut [u32],
    register: RegisterId,
) -> Result<crate::value::ValueRef, RuntimeError> {
    match read_register_with_counts(profile, registers, remaining_register_reads, register)? {
        Value::Reference(Some(reference)) => Ok(reference),
        Value::Reference(None) => Err(VmTrap::NullReference.into_runtime_error()),
        _ => Err(VmTrap::Runtime(RuntimeError::TypeMismatch).into_runtime_error()),
    }
}

fn read_bool_register(registers: &[Value], register: RegisterId) -> Result<bool, RuntimeError> {
    match read_register_ref(registers, register)? {
        Value::Bool(value) => Ok(*value),
        _ => Err(VmTrap::ConditionNotBool.into_runtime_error()),
    }
}

fn read_reference_register(
    registers: &[Value],
    register: RegisterId,
) -> Result<crate::value::ValueRef, RuntimeError> {
    match read_register_ref(registers, register)? {
        Value::Reference(Some(reference)) => Ok(reference.clone()),
        Value::Reference(None) => Err(VmTrap::NullReference.into_runtime_error()),
        _ => Err(VmTrap::Runtime(RuntimeError::TypeMismatch).into_runtime_error()),
    }
}

fn write_register(
    registers: &mut [Value],
    register: RegisterId,
    value: Value,
) -> Result<(), RuntimeError> {
    let slot = registers
        .get_mut(register.index() as usize)
        .ok_or_else(|| {
            invalid_bytecode(format!(
                "register-ir executor write out-of-bounds register {}",
                register.index()
            ))
        })?;
    *slot = value;
    Ok(())
}

fn lowered_has_fallback_instructions(program: &RegisterProgram) -> bool {
    program
        .blocks
        .iter()
        .flat_map(|block| block.instructions.iter())
        .any(|instruction| matches!(instruction, RegisterInstr::VmFallback { .. }))
}

fn first_fallback_opcode(program: &RegisterProgram) -> Option<u8> {
    program
        .blocks
        .iter()
        .flat_map(|block| block.instructions.iter())
        .find_map(|instruction| match instruction {
            RegisterInstr::VmFallback { opcode, .. } => Some(*opcode),
            _ => None,
        })
}

fn lowered_uses_complex_local_paths(module: &VmModule, program: &RegisterProgram) -> bool {
    let uses_complex_local_ref = |ref_idx: u32| {
        matches!(
            module.refs.get(ref_idx as usize),
            Some(super::VmRef::Local { path, .. }) if !path.is_empty()
        )
    };

    for instruction in program
        .blocks
        .iter()
        .flat_map(|block| block.instructions.iter())
    {
        match instruction {
            RegisterInstr::LoadRef { ref_idx, .. }
            | RegisterInstr::LoadRefAddr { ref_idx, .. }
            | RegisterInstr::StoreRef { ref_idx, .. }
            | RegisterInstr::CmpRefConstJumpIf { ref_idx, .. } => {
                if uses_complex_local_ref(*ref_idx) {
                    return true;
                }
            }
            RegisterInstr::BinaryRefToRef {
                left_ref_idx,
                right_ref_idx,
                dest_ref_idx,
                ..
            } => {
                if uses_complex_local_ref(*left_ref_idx)
                    || uses_complex_local_ref(*right_ref_idx)
                    || uses_complex_local_ref(*dest_ref_idx)
                {
                    return true;
                }
            }
            RegisterInstr::BinaryRefConstToRef {
                left_ref_idx,
                dest_ref_idx,
                ..
            } => {
                if uses_complex_local_ref(*left_ref_idx) || uses_complex_local_ref(*dest_ref_idx) {
                    return true;
                }
            }
            RegisterInstr::BinaryConstRefToRef {
                right_ref_idx,
                dest_ref_idx,
                ..
            } => {
                if uses_complex_local_ref(*right_ref_idx) || uses_complex_local_ref(*dest_ref_idx) {
                    return true;
                }
            }
            _ => {}
        }
    }
    false
}

fn consume_loop_budget(budget: &mut usize) -> Result<(), RuntimeError> {
    if *budget == 0 {
        return Err(VmTrap::BudgetExceeded.into_runtime_error());
    }
    *budget = budget.saturating_sub(1);
    Ok(())
}

fn consume_loop_budget_for_block_target(
    program: &RegisterProgram,
    source_block: &RegisterBlock,
    target: BlockTarget,
    budget: &mut usize,
) -> Result<(), RuntimeError> {
    let BlockTarget::Block(target_block_id) = target else {
        return Ok(());
    };
    let target_index = block_index_from_id(program, target_block_id)?;
    let target_block = &program.blocks[target_index];
    if target_block.start_pc <= source_block.start_pc {
        consume_loop_budget(budget)?;
    }
    Ok(())
}

fn block_index_from_id(program: &RegisterProgram, block_id: u32) -> Result<usize, RuntimeError> {
    let block_index = usize::try_from(block_id).map_err(|_| {
        invalid_bytecode(format!(
            "register-ir block id {block_id} does not fit target index type"
        ))
    })?;
    let block = program.blocks.get(block_index).ok_or_else(|| {
        invalid_bytecode(format!("register-ir executor missing block id {block_id}"))
    })?;
    if block.id != block_id {
        return Err(invalid_bytecode(format!(
            "register-ir block id/index mismatch for block id {block_id} (found block id {})",
            block.id
        )));
    }
    Ok(block_index)
}

fn register_statement_location(
    runtime: &Runtime,
    module: &VmModule,
    pou_id: u32,
    pc: usize,
) -> Option<crate::debug::SourceLocation> {
    let source = module.debug_map.source_by_pc.get(&(pou_id, pc as u32))?;
    runtime.resolve_vm_debug_location(source.file.as_str(), source.line, source.column)
}

fn deadline_exceeded(deadline: Option<Instant>) -> bool {
    match deadline {
        Some(deadline) => Instant::now() >= deadline,
        None => false,
    }
}

#[inline]
fn should_check_register_deadline(instruction_index: usize) -> bool {
    instruction_index == 0 || instruction_index % REGISTER_DEADLINE_CHECK_STRIDE == 0
}

fn verify_dest(
    dest: &RegisterId,
    max_registers: u32,
    defined: &mut BTreeSet<RegisterId>,
) -> Result<(), RuntimeError> {
    if dest.index() >= max_registers {
        return Err(invalid_bytecode(format!(
            "register-ir destination register {} out of bounds (max={max_registers})",
            dest.index()
        )));
    }
    if !defined.insert(*dest) {
        return Err(invalid_bytecode(format!(
            "register-ir destination register {} redefined in block",
            dest.index()
        )));
    }
    Ok(())
}

fn verify_move_dest(
    dest: &RegisterId,
    max_registers: u32,
    defined: &mut BTreeSet<RegisterId>,
) -> Result<(), RuntimeError> {
    if dest.index() >= max_registers {
        return Err(invalid_bytecode(format!(
            "register-ir destination register {} out of bounds (max={max_registers})",
            dest.index()
        )));
    }
    defined.insert(*dest);
    Ok(())
}

fn verify_src(src: &RegisterId, defined: &BTreeSet<RegisterId>) -> Result<(), RuntimeError> {
    if !defined.contains(src) {
        return Err(invalid_bytecode(format!(
            "register-ir source register {} used before definition",
            src.index()
        )));
    }
    Ok(())
}

fn verify_target(target: &BlockTarget, known_blocks: &HashSet<u32>) -> Result<(), RuntimeError> {
    if let BlockTarget::Block(id) = target {
        if !known_blocks.contains(id) {
            return Err(invalid_bytecode(format!(
                "register-ir unknown block target {id}",
            )));
        }
    }
    Ok(())
}

fn alloc_register(next_register: &mut u32) -> RegisterId {
    let reg = RegisterId(*next_register);
    *next_register = next_register.saturating_add(1);
    reg
}

fn pop_stack(stack: &mut Vec<RegisterId>, opcode: u8) -> Result<RegisterId, RuntimeError> {
    stack.pop().ok_or_else(|| {
        invalid_bytecode(format!(
            "register-ir lowering stack underflow while decoding opcode 0x{opcode:02X}",
        ))
    })
}

fn lower_unary(
    next_register: &mut u32,
    stack: &mut Vec<RegisterId>,
    instructions: &mut Vec<RegisterInstr>,
    op: UnaryOp,
    opcode: u8,
) -> Result<(), RuntimeError> {
    let src = pop_stack(stack, opcode)?;
    let dest = alloc_register(next_register);
    stack.push(dest);
    instructions.push(RegisterInstr::Unary { op, src, dest });
    Ok(())
}

fn lower_binary(
    next_register: &mut u32,
    stack: &mut Vec<RegisterId>,
    instructions: &mut Vec<RegisterInstr>,
    op: BinaryOp,
    opcode: u8,
) -> Result<(), RuntimeError> {
    let right = pop_stack(stack, opcode)?;
    let left = pop_stack(stack, opcode)?;
    let dest = alloc_register(next_register);
    stack.push(dest);
    instructions.push(RegisterInstr::Binary {
        op,
        left,
        right,
        dest,
    });
    Ok(())
}

fn decode_pou(
    module: &VmModule,
    code_start: usize,
    code_end: usize,
) -> Result<Vec<DecodedInstr>, RuntimeError> {
    let mut decoded = Vec::new();
    let mut pc = code_start;
    while pc < code_end {
        let opcode = module.code.get(pc).copied().ok_or_else(|| {
            invalid_bytecode("register-ir decode instruction fetch out of bounds")
        })?;
        let operand_len = opcode_operand_len_for_lowering(opcode).ok_or_else(|| {
            invalid_bytecode(format!("register-ir decode invalid opcode 0x{opcode:02X}"))
        })?;
        let next_pc = pc + 1 + operand_len;
        if next_pc > code_end {
            return Err(invalid_bytecode(
                "register-ir decode unexpected end of input while reading operands",
            ));
        }
        let operands = module.code[(pc + 1)..next_pc].to_vec();
        decoded.push(DecodedInstr {
            pc,
            next_pc,
            opcode,
            operands,
        });
        pc = next_pc;
    }
    Ok(decoded)
}

fn opcode_operand_len_for_lowering(opcode: u8) -> Option<usize> {
    opcode_operand_len(opcode).or(match opcode {
        0x25 => Some(0),
        0x62 | 0x63 => Some(4),
        _ => None,
    })
}

fn collect_block_leaders(
    decoded: &[DecodedInstr],
    code_start: usize,
    code_end: usize,
) -> Result<Vec<usize>, RuntimeError> {
    let mut leaders = BTreeSet::new();
    leaders.insert(code_start);
    for instr in decoded {
        if let 0x02..=0x04 = instr.opcode {
            let offset = operand_i32(instr)?;
            let target = jump_target_pc(instr.next_pc, offset, code_start, code_end)?;
            if target < code_end {
                leaders.insert(target);
            }
            if instr.opcode != 0x02 && instr.next_pc < code_end {
                leaders.insert(instr.next_pc);
            }
        }
    }
    Ok(leaders.into_iter().collect())
}

fn jump_target_pc(
    pc_after_operand: usize,
    offset: i32,
    code_start: usize,
    code_end: usize,
) -> Result<usize, RuntimeError> {
    let base = pc_after_operand as i64;
    let target = base + i64::from(offset);
    if target < code_start as i64 || target > code_end as i64 {
        return Err(invalid_bytecode(format!(
            "register-ir invalid jump target {target}",
        )));
    }
    Ok(target as usize)
}

fn pc_to_block_target(
    target_pc: usize,
    code_end: usize,
    start_to_block: &HashMap<usize, u32>,
) -> Result<BlockTarget, RuntimeError> {
    if target_pc == code_end {
        return Ok(BlockTarget::Exit);
    }
    let id = start_to_block.get(&target_pc).copied().ok_or_else(|| {
        invalid_bytecode(format!(
            "register-ir jump target {target_pc} is not a block leader"
        ))
    })?;
    Ok(BlockTarget::Block(id))
}

fn operand_u32(instr: &DecodedInstr) -> Result<u32, RuntimeError> {
    if instr.operands.len() != 4 {
        return Err(invalid_bytecode(format!(
            "register-ir opcode 0x{:02X} expected 4-byte operand",
            instr.opcode
        )));
    }
    operand_u32_slice(instr, 0)
}

fn operand_native_call(instr: &DecodedInstr) -> Result<(u32, u32, u32), RuntimeError> {
    if instr.operands.len() != 12 {
        return Err(invalid_bytecode(format!(
            "register-ir opcode 0x{:02X} expected 12-byte operand",
            instr.opcode
        )));
    }
    Ok((
        operand_u32_slice(instr, 0)?,
        operand_u32_slice(instr, 4)?,
        operand_u32_slice(instr, 8)?,
    ))
}

fn operand_i32(instr: &DecodedInstr) -> Result<i32, RuntimeError> {
    if instr.operands.len() != 4 {
        return Err(invalid_bytecode(format!(
            "register-ir opcode 0x{:02X} expected 4-byte operand",
            instr.opcode
        )));
    }
    let bytes = [
        instr.operands[0],
        instr.operands[1],
        instr.operands[2],
        instr.operands[3],
    ];
    Ok(i32::from_le_bytes(bytes))
}

fn operand_u32_slice(instr: &DecodedInstr, offset: usize) -> Result<u32, RuntimeError> {
    let end = offset.saturating_add(4);
    if instr.operands.len() < end {
        return Err(invalid_bytecode(format!(
            "register-ir opcode 0x{:02X} missing operand bytes at offset {offset}",
            instr.opcode
        )));
    }
    let bytes = [
        instr.operands[offset],
        instr.operands[offset + 1],
        instr.operands[offset + 2],
        instr.operands[offset + 3],
    ];
    Ok(u32::from_le_bytes(bytes))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DecodedInstr {
    pc: usize,
    next_pc: usize,
    opcode: u8,
    operands: Vec<u8>,
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};
    use std::path::PathBuf;

    use indexmap::IndexMap;
    use smol_str::SmolStr;

    use crate::bundle_builder::collect_project_source_files;
    use crate::bytecode::{SectionData, SectionId, TypeTable};
    use crate::config::RuntimeConfig;
    use crate::error::RuntimeError;
    use crate::eval::ops::{apply_binary, apply_unary, BinaryOp, UnaryOp};
    use crate::execution_backend::ExecutionBackend;
    use crate::harness::{bytecode_module_from_source, CompileSession, TestHarness};
    use crate::value::{DateTimeProfile, RefSegment, StructValue, Value};
    use crate::{RestartMode, Runtime};

    use super::super::{VmPouEntry, VmRef};
    use super::{
        invalid_bytecode, lower_pou_to_register_ir, read_register_with_counts,
        try_execute_pou_with_register_ir, try_execute_pou_with_register_ir_with_locals,
        verify_register_program, BlockTarget, RegisterExecutionOutcome, RegisterId, RegisterInstr,
        RegisterProfileState, RegisterProgram, VmModule,
    };

    fn vm_module_and_main_pou(source: &str) -> (VmModule, u32) {
        let bytecode = bytecode_module_from_source(source).expect("compile bytecode");
        let vm_module = VmModule::from_bytecode(&bytecode).expect("decode vm module");
        let main_key = SmolStr::new("MAIN");
        let pou_id = vm_module
            .program_ids
            .get(&main_key)
            .copied()
            .expect("main pou id");
        (vm_module, pou_id)
    }

    fn manual_vm_module(code: Vec<u8>, consts: Vec<Value>, ref_count: usize) -> (VmModule, u32) {
        let pou_id = 1_u32;
        let mut pou_by_id = HashMap::new();
        pou_by_id.insert(
            pou_id,
            VmPouEntry {
                name: SmolStr::new("MAIN"),
                code_start: 0,
                code_end: code.len(),
                local_ref_start: 0,
                local_ref_count: 0,
                primary_instance_owner: None,
            },
        );
        let mut program_ids = HashMap::new();
        program_ids.insert(SmolStr::new("MAIN"), pou_id);

        let refs = (0..ref_count)
            .map(|offset| VmRef::Global {
                offset,
                path: Vec::new(),
            })
            .collect();

        (
            VmModule {
                code,
                strings: Vec::new(),
                types: TypeTable::default(),
                refs,
                consts,
                pou_by_id,
                program_ids,
                function_ids: HashMap::new(),
                function_block_ids: HashMap::new(),
                class_ids: HashMap::new(),
                native_symbol_specs: Vec::new(),
                pou_params: HashMap::new(),
                pou_has_return_slot: HashSet::new(),
                method_table_by_owner: HashMap::new(),
                debug_map: super::super::debug_map::VmDebugMap::default(),
                instruction_budget: super::super::DEFAULT_INSTRUCTION_BUDGET,
            },
            pou_id,
        )
    }

    fn emit_u32(code: &mut Vec<u8>, value: u32) {
        code.extend_from_slice(&value.to_le_bytes());
    }

    fn emit_i32(code: &mut Vec<u8>, value: i32) {
        code.extend_from_slice(&value.to_le_bytes());
    }

    fn patch_i32(code: &mut [u8], operand_start: usize, value: i32) {
        let bytes = value.to_le_bytes();
        code[operand_start..operand_start + 4].copy_from_slice(&bytes);
    }

    fn read_u32_operand(
        code: &[u8],
        pc: &mut usize,
        code_end: usize,
        opcode: u8,
    ) -> Result<u32, RuntimeError> {
        if *pc + 4 > code_end {
            return Err(invalid_bytecode(format!(
                "parity stack executor operand overflow for opcode 0x{opcode:02X}",
            )));
        }
        let bytes = [code[*pc], code[*pc + 1], code[*pc + 2], code[*pc + 3]];
        *pc += 4;
        Ok(u32::from_le_bytes(bytes))
    }

    fn read_i32_operand(
        code: &[u8],
        pc: &mut usize,
        code_end: usize,
        opcode: u8,
    ) -> Result<i32, RuntimeError> {
        if *pc + 4 > code_end {
            return Err(invalid_bytecode(format!(
                "parity stack executor operand overflow for opcode 0x{opcode:02X}",
            )));
        }
        let bytes = [code[*pc], code[*pc + 1], code[*pc + 2], code[*pc + 3]];
        *pc += 4;
        Ok(i32::from_le_bytes(bytes))
    }

    fn pop_stack_value(stack: &mut Vec<Value>, opcode: u8) -> Result<Value, RuntimeError> {
        stack.pop().ok_or_else(|| {
            invalid_bytecode(format!(
                "parity stack executor stack underflow on opcode 0x{opcode:02X}",
            ))
        })
    }

    fn pop_bool_condition(stack: &mut Vec<Value>) -> Result<bool, RuntimeError> {
        match stack.pop() {
            Some(Value::Bool(value)) => Ok(value),
            Some(_) => Err(RuntimeError::TypeMismatch),
            None => Err(invalid_bytecode(
                "parity stack executor stack underflow on conditional jump",
            )),
        }
    }

    fn jump_target_within(
        pc_after_operand: usize,
        offset: i32,
        code_start: usize,
        code_end: usize,
    ) -> Result<usize, RuntimeError> {
        let target = (pc_after_operand as i64) + i64::from(offset);
        if target < code_start as i64 || target > code_end as i64 {
            return Err(invalid_bytecode(format!(
                "parity stack executor invalid jump target {target}",
            )));
        }
        Ok(target as usize)
    }

    fn execute_stack_subset(
        module: &VmModule,
        pou_id: u32,
        refs: &mut [Value],
    ) -> Result<(), RuntimeError> {
        let pou = module.pou(pou_id).ok_or_else(|| {
            invalid_bytecode(format!(
                "missing pou id {pou_id} for parity stack execution"
            ))
        })?;
        let mut stack = Vec::new();
        let mut pc = pou.code_start;
        let mut budget = 10_000_usize;
        let profile = DateTimeProfile::default();

        while pc < pou.code_end {
            if budget == 0 {
                return Err(invalid_bytecode(
                    "parity stack executor budget exceeded (possible infinite loop)",
                ));
            }
            budget = budget.saturating_sub(1);

            let opcode = module.code[pc];
            pc += 1;
            match opcode {
                0x00 => {}
                0x02 => {
                    let offset = read_i32_operand(&module.code, &mut pc, pou.code_end, opcode)?;
                    pc = jump_target_within(pc, offset, pou.code_start, pou.code_end)?;
                }
                0x03 | 0x04 => {
                    let offset = read_i32_operand(&module.code, &mut pc, pou.code_end, opcode)?;
                    let condition = pop_bool_condition(&mut stack)?;
                    let should_jump =
                        (opcode == 0x03 && condition) || (opcode == 0x04 && !condition);
                    if should_jump {
                        pc = jump_target_within(pc, offset, pou.code_start, pou.code_end)?;
                    }
                }
                0x06 => return Ok(()),
                0x10 => {
                    let const_idx = read_u32_operand(&module.code, &mut pc, pou.code_end, opcode)?;
                    let value =
                        module
                            .consts
                            .get(const_idx as usize)
                            .cloned()
                            .ok_or_else(|| {
                                invalid_bytecode(format!("invalid const index {const_idx}"))
                            })?;
                    stack.push(value);
                }
                0x11 => {
                    let value = stack.last().cloned().ok_or_else(|| {
                        invalid_bytecode("parity stack executor stack underflow on DUP")
                    })?;
                    stack.push(value);
                }
                0x12 => {
                    let _ = pop_stack_value(&mut stack, opcode)?;
                }
                0x13 => {
                    if stack.len() < 2 {
                        return Err(invalid_bytecode(
                            "parity stack executor stack underflow on SWAP",
                        ));
                    }
                    let len = stack.len();
                    stack.swap(len - 1, len - 2);
                }
                0x20 => {
                    let ref_idx = read_u32_operand(&module.code, &mut pc, pou.code_end, opcode)?;
                    let value = refs
                        .get(ref_idx as usize)
                        .cloned()
                        .ok_or_else(|| invalid_bytecode(format!("invalid ref index {ref_idx}")))?;
                    stack.push(value);
                }
                0x21 => {
                    let ref_idx = read_u32_operand(&module.code, &mut pc, pou.code_end, opcode)?;
                    let value = pop_stack_value(&mut stack, opcode)?;
                    let slot = refs
                        .get_mut(ref_idx as usize)
                        .ok_or_else(|| invalid_bytecode(format!("invalid ref index {ref_idx}")))?;
                    *slot = value;
                }
                0x40..=0x55 => {
                    let op = match opcode {
                        0x40 => BinaryOp::Add,
                        0x41 => BinaryOp::Sub,
                        0x42 => BinaryOp::Mul,
                        0x43 => BinaryOp::Div,
                        0x44 => BinaryOp::Mod,
                        0x46 => BinaryOp::And,
                        0x47 => BinaryOp::Or,
                        0x48 => BinaryOp::Xor,
                        0x50 => BinaryOp::Eq,
                        0x51 => BinaryOp::Ne,
                        0x52 => BinaryOp::Lt,
                        0x53 => BinaryOp::Le,
                        0x54 => BinaryOp::Gt,
                        0x55 => BinaryOp::Ge,
                        _ => {
                            let unary = match opcode {
                                0x45 => UnaryOp::Neg,
                                0x49 => UnaryOp::Not,
                                _ => {
                                    return Err(invalid_bytecode(format!(
                                    "unsupported opcode 0x{opcode:02X} in parity stack executor",
                                )))
                                }
                            };
                            let value = pop_stack_value(&mut stack, opcode)?;
                            stack.push(apply_unary(unary, value)?);
                            continue;
                        }
                    };
                    let right = pop_stack_value(&mut stack, opcode)?;
                    let left = pop_stack_value(&mut stack, opcode)?;
                    let result = apply_binary(op, left, right, &profile)?;
                    stack.push(result);
                }
                _ => {
                    return Err(invalid_bytecode(format!(
                        "unsupported opcode 0x{opcode:02X} in parity stack executor",
                    )));
                }
            }
        }

        Ok(())
    }

    fn read_register_value(
        registers: &[Value],
        register: RegisterId,
    ) -> Result<Value, RuntimeError> {
        registers
            .get(register.index() as usize)
            .cloned()
            .ok_or_else(|| {
                invalid_bytecode(format!(
                    "parity register executor read out-of-bounds register {}",
                    register.index()
                ))
            })
    }

    fn write_register_value(
        registers: &mut [Value],
        register: RegisterId,
        value: Value,
    ) -> Result<(), RuntimeError> {
        let slot = registers
            .get_mut(register.index() as usize)
            .ok_or_else(|| {
                invalid_bytecode(format!(
                    "parity register executor write out-of-bounds register {}",
                    register.index()
                ))
            })?;
        *slot = value;
        Ok(())
    }

    fn execute_register_subset(
        module: &VmModule,
        program: &RegisterProgram,
        refs: &mut [Value],
    ) -> Result<(), RuntimeError> {
        let mut registers = vec![Value::Null; program.max_registers as usize];
        let mut current_block = program.entry_block;
        let mut budget = 10_000_usize;
        let mut block_to_index = HashMap::new();
        for (index, block) in program.blocks.iter().enumerate() {
            block_to_index.insert(block.id, index);
        }
        let profile = DateTimeProfile::default();

        loop {
            if budget == 0 {
                return Err(invalid_bytecode(
                    "parity register executor budget exceeded (possible infinite loop)",
                ));
            }
            budget = budget.saturating_sub(1);
            let block_index = block_to_index.get(&current_block).copied().ok_or_else(|| {
                invalid_bytecode(format!(
                    "parity register executor missing block {current_block}"
                ))
            })?;
            let block = &program.blocks[block_index];
            let mut control_target = None;

            for instruction in &block.instructions {
                match instruction {
                    RegisterInstr::Nop => {}
                    RegisterInstr::LoadConst { dest, const_idx } => {
                        let value =
                            module
                                .consts
                                .get(*const_idx as usize)
                                .cloned()
                                .ok_or_else(|| {
                                    invalid_bytecode(format!(
                                        "parity register executor invalid const index {const_idx}",
                                    ))
                                })?;
                        write_register_value(&mut registers, *dest, value)?;
                    }
                    RegisterInstr::LoadNull { dest } => {
                        write_register_value(&mut registers, *dest, Value::Null)?;
                    }
                    RegisterInstr::LoadSelf { .. } => {
                        return Err(invalid_bytecode(
                            "parity register executor does not support LOAD_SELF",
                        ));
                    }
                    RegisterInstr::LoadSuper { .. } => {
                        return Err(invalid_bytecode(
                            "parity register executor does not support LOAD_SUPER",
                        ));
                    }
                    RegisterInstr::Move { src, dest } => {
                        let value = read_register_value(&registers, *src)?;
                        write_register_value(&mut registers, *dest, value)?;
                    }
                    RegisterInstr::LoadRef { dest, ref_idx } => {
                        let value = refs.get(*ref_idx as usize).cloned().ok_or_else(|| {
                            invalid_bytecode(format!(
                                "parity register executor invalid ref index {ref_idx}",
                            ))
                        })?;
                        write_register_value(&mut registers, *dest, value)?;
                    }
                    RegisterInstr::LoadRefAddr { .. } => {
                        return Err(invalid_bytecode(
                            "parity register executor does not support LOAD_REF_ADDR",
                        ));
                    }
                    RegisterInstr::StoreRef { ref_idx, src } => {
                        let value = read_register_value(&registers, *src)?;
                        let slot = refs.get_mut(*ref_idx as usize).ok_or_else(|| {
                            invalid_bytecode(format!(
                                "parity register executor invalid ref index {ref_idx}",
                            ))
                        })?;
                        *slot = value;
                    }
                    RegisterInstr::Unary { op, src, dest } => {
                        let src = read_register_value(&registers, *src)?;
                        let result = apply_unary(*op, src)?;
                        write_register_value(&mut registers, *dest, result)?;
                    }
                    RegisterInstr::Binary {
                        op,
                        left,
                        right,
                        dest,
                    } => {
                        let left = read_register_value(&registers, *left)?;
                        let right = read_register_value(&registers, *right)?;
                        let result = apply_binary(*op, left, right, &profile)?;
                        write_register_value(&mut registers, *dest, result)?;
                    }
                    RegisterInstr::BinaryRefToRef {
                        op,
                        left_ref_idx,
                        right_ref_idx,
                        dest_ref_idx,
                    } => {
                        let left = refs.get(*left_ref_idx as usize).cloned().ok_or_else(|| {
                            invalid_bytecode(format!(
                                "parity register executor invalid ref index {left_ref_idx}",
                            ))
                        })?;
                        let right =
                            refs.get(*right_ref_idx as usize).cloned().ok_or_else(|| {
                                invalid_bytecode(format!(
                                    "parity register executor invalid ref index {right_ref_idx}",
                                ))
                            })?;
                        let result = apply_binary(*op, left, right, &profile)?;
                        let slot = refs.get_mut(*dest_ref_idx as usize).ok_or_else(|| {
                            invalid_bytecode(format!(
                                "parity register executor invalid ref index {dest_ref_idx}",
                            ))
                        })?;
                        *slot = result;
                    }
                    RegisterInstr::BinaryRefConstToRef {
                        op,
                        left_ref_idx,
                        const_idx,
                        dest_ref_idx,
                    } => {
                        let left = refs.get(*left_ref_idx as usize).cloned().ok_or_else(|| {
                            invalid_bytecode(format!(
                                "parity register executor invalid ref index {left_ref_idx}",
                            ))
                        })?;
                        let right =
                            module
                                .consts
                                .get(*const_idx as usize)
                                .cloned()
                                .ok_or_else(|| {
                                    invalid_bytecode(format!(
                                        "parity register executor invalid const index {const_idx}",
                                    ))
                                })?;
                        let result = apply_binary(*op, left, right, &profile)?;
                        let slot = refs.get_mut(*dest_ref_idx as usize).ok_or_else(|| {
                            invalid_bytecode(format!(
                                "parity register executor invalid ref index {dest_ref_idx}",
                            ))
                        })?;
                        *slot = result;
                    }
                    RegisterInstr::BinaryConstRefToRef {
                        op,
                        const_idx,
                        right_ref_idx,
                        dest_ref_idx,
                    } => {
                        let left =
                            module
                                .consts
                                .get(*const_idx as usize)
                                .cloned()
                                .ok_or_else(|| {
                                    invalid_bytecode(format!(
                                        "parity register executor invalid const index {const_idx}",
                                    ))
                                })?;
                        let right =
                            refs.get(*right_ref_idx as usize).cloned().ok_or_else(|| {
                                invalid_bytecode(format!(
                                    "parity register executor invalid ref index {right_ref_idx}",
                                ))
                            })?;
                        let result = apply_binary(*op, left, right, &profile)?;
                        let slot = refs.get_mut(*dest_ref_idx as usize).ok_or_else(|| {
                            invalid_bytecode(format!(
                                "parity register executor invalid ref index {dest_ref_idx}",
                            ))
                        })?;
                        *slot = result;
                    }
                    RegisterInstr::CmpRefConstJumpIf {
                        op,
                        ref_idx,
                        const_idx,
                        jump_if_true,
                        target,
                    } => {
                        let left = refs.get(*ref_idx as usize).cloned().ok_or_else(|| {
                            invalid_bytecode(format!(
                                "parity register executor invalid ref index {ref_idx}",
                            ))
                        })?;
                        let right =
                            module
                                .consts
                                .get(*const_idx as usize)
                                .cloned()
                                .ok_or_else(|| {
                                    invalid_bytecode(format!(
                                        "parity register executor invalid const index {const_idx}",
                                    ))
                                })?;
                        let result = apply_binary(*op, left, right, &profile)?;
                        let condition = match result {
                            Value::Bool(value) => value,
                            _ => return Err(RuntimeError::TypeMismatch),
                        };
                        if condition == *jump_if_true {
                            control_target = Some(*target);
                            break;
                        }
                    }
                    RegisterInstr::CallNative { .. }
                    | RegisterInstr::SizeOfType { .. }
                    | RegisterInstr::SizeOfValue { .. }
                    | RegisterInstr::RefField { .. }
                    | RegisterInstr::RefIndex { .. }
                    | RegisterInstr::LoadDynamic { .. }
                    | RegisterInstr::StoreDynamic { .. } => {
                        return Err(invalid_bytecode(
                            "parity register executor does not support native-call/sizeof/dynamic-ref ops",
                        ));
                    }
                    RegisterInstr::Jump { target } => {
                        control_target = Some(*target);
                        break;
                    }
                    RegisterInstr::JumpIf {
                        cond,
                        jump_if_true,
                        target,
                    } => {
                        let cond = read_register_value(&registers, *cond)?;
                        let cond = match cond {
                            Value::Bool(value) => value,
                            _ => return Err(RuntimeError::TypeMismatch),
                        };
                        if cond == *jump_if_true {
                            control_target = Some(*target);
                            break;
                        }
                    }
                    RegisterInstr::Return => return Ok(()),
                    RegisterInstr::VmFallback { opcode, .. } => {
                        return Err(invalid_bytecode(format!(
                            "parity register executor encountered fallback opcode 0x{opcode:02X}",
                        )));
                    }
                }
            }

            match control_target {
                Some(BlockTarget::Block(next)) => current_block = next,
                Some(BlockTarget::Exit) => return Ok(()),
                None => {
                    if let Some(next_block) = program.blocks.get(block_index + 1) {
                        current_block = next_block.id;
                    } else {
                        return Ok(());
                    }
                }
            }
        }
    }

    fn assert_no_fallback(program: &RegisterProgram) {
        assert!(
            program
                .blocks
                .iter()
                .flat_map(|block| block.instructions.iter())
                .all(|instruction| !matches!(instruction, RegisterInstr::VmFallback { .. })),
            "parity program unexpectedly lowered unsupported opcodes to VmFallback",
        );
    }

    #[test]
    fn register_ir_lowering_handles_linear_arithmetic_main() {
        let source = r#"
            PROGRAM Main
            VAR
                count : DINT := 0;
            END_VAR
            count := count + 1;
            END_PROGRAM
        "#;
        let (vm_module, pou_id) = vm_module_and_main_pou(source);
        let lowered = lower_pou_to_register_ir(&vm_module, pou_id).expect("lower register ir");
        verify_register_program(&lowered).expect("verify register ir");

        assert_eq!(lowered.entry_block, 0);
        assert!(lowered.max_registers > 0);
        assert!(!lowered.blocks.is_empty());
        let all_instr = lowered
            .blocks
            .iter()
            .flat_map(|block| block.instructions.iter())
            .collect::<Vec<_>>();
        assert!(
            all_instr.iter().any(|instr| {
                matches!(
                    instr,
                    RegisterInstr::Binary { .. }
                        | RegisterInstr::BinaryRefToRef { .. }
                        | RegisterInstr::BinaryRefConstToRef { .. }
                        | RegisterInstr::BinaryConstRefToRef { .. }
                )
            }),
            "expected arithmetic lowering to emit binary register instruction",
        );
        assert!(
            all_instr.iter().any(|instr| {
                matches!(
                    instr,
                    RegisterInstr::StoreRef { .. }
                        | RegisterInstr::BinaryRefToRef { .. }
                        | RegisterInstr::BinaryRefConstToRef { .. }
                        | RegisterInstr::BinaryConstRefToRef { .. }
                )
            }),
            "expected store lowering to emit register store instruction",
        );
    }

    #[test]
    fn register_ir_lowering_emits_control_flow_blocks_for_loops() {
        let source = r#"
            PROGRAM Main
            VAR
                i : DINT := 0;
                acc : DINT := 0;
            END_VAR
            WHILE i < 3 DO
                acc := acc + i;
                i := i + 1;
            END_WHILE;
            END_PROGRAM
        "#;
        let (vm_module, pou_id) = vm_module_and_main_pou(source);
        let lowered = lower_pou_to_register_ir(&vm_module, pou_id).expect("lower register ir");
        verify_register_program(&lowered).expect("verify register ir");

        assert!(
            lowered.blocks.len() >= 2,
            "expected loop lowering to produce multiple blocks"
        );
        assert!(
            lowered
                .blocks
                .iter()
                .flat_map(|block| block.instructions.iter())
                .any(|instr| matches!(
                    instr,
                    RegisterInstr::Jump {
                        target: BlockTarget::Block(_)
                    } | RegisterInstr::JumpIf {
                        target: BlockTarget::Block(_),
                        ..
                    }
                )),
            "expected branch instructions targeting lowered blocks"
        );
    }

    #[test]
    fn register_ir_lowering_handles_case_selector_live_across_branch_blocks() {
        let source = r#"
            PROGRAM Main
            VAR
                selector : UINT := UINT#2;
                output : UINT := UINT#0;
            END_VAR

            CASE selector OF
                UINT#1:
                    output := UINT#10;
                UINT#2:
                    output := UINT#20;
                ELSE
                    output := UINT#30;
            END_CASE;
            END_PROGRAM
        "#;

        let (vm_module, pou_id) = vm_module_and_main_pou(source);
        let lowered = lower_pou_to_register_ir(&vm_module, pou_id).expect("lower register ir");
        verify_register_program(&lowered).expect("verify register ir");
        assert_no_fallback(&lowered);
    }

    #[test]
    fn register_executor_runs_case_program_without_fallback() {
        let source = r#"
            VAR_GLOBAL
                g_selector : UINT := UINT#2;
                g_output : UINT := UINT#0;
            END_VAR

            PROGRAM Main
            CASE g_selector OF
                UINT#1:
                    g_output := UINT#10;
                UINT#2:
                    g_output := UINT#20;
                ELSE
                    g_output := UINT#30;
            END_CASE;
            END_PROGRAM
        "#;

        let mut harness = TestHarness::from_source(source).expect("create harness");
        harness
            .runtime_mut()
            .set_execution_backend(ExecutionBackend::BytecodeVm)
            .expect("set backend");
        harness
            .runtime_mut()
            .restart(RestartMode::Cold)
            .expect("restart");
        harness.runtime_mut().set_vm_register_profile_enabled(true);
        harness.runtime_mut().reset_vm_register_profile();

        let result = harness.cycle();
        assert!(
            result.errors.is_empty(),
            "cycle errors: {:?}",
            result.errors
        );
        assert_eq!(harness.get_output("g_output"), Some(Value::UInt(20)));

        let profile = harness.runtime().vm_register_profile_snapshot();
        assert!(profile.register_programs_executed >= 1);
        assert_eq!(
            profile.register_program_fallbacks, 0,
            "expected no register fallbacks, got {:?}",
            profile.fallback_reasons
        );
    }

    #[test]
    fn register_executor_runs_multi_label_case_program_without_fallback() {
        let source = r#"
            VAR_GLOBAL
                g_selector : UINT := UINT#3;
                g_output : UINT := UINT#0;
            END_VAR

            PROGRAM Main
            CASE g_selector OF
                UINT#1:
                    g_output := UINT#10;
                UINT#2:
                    g_output := UINT#20;
                UINT#3:
                    g_output := UINT#30;
                ELSE
                    g_output := UINT#99;
            END_CASE;
            END_PROGRAM
        "#;

        let mut harness = TestHarness::from_source(source).expect("create harness");
        harness
            .runtime_mut()
            .set_execution_backend(ExecutionBackend::BytecodeVm)
            .expect("set backend");
        harness
            .runtime_mut()
            .restart(RestartMode::Cold)
            .expect("restart");
        harness.runtime_mut().set_vm_register_profile_enabled(true);
        harness.runtime_mut().reset_vm_register_profile();

        let result = harness.cycle();
        assert!(
            result.errors.is_empty(),
            "cycle errors: {:?}",
            result.errors
        );
        assert_eq!(harness.get_output("g_output"), Some(Value::UInt(30)));

        let profile = harness.runtime().vm_register_profile_snapshot();
        assert!(profile.register_programs_executed >= 1);
        assert_eq!(
            profile.register_program_fallbacks, 0,
            "expected no register fallbacks, got {:?}",
            profile.fallback_reasons
        );
    }

    #[test]
    fn register_executor_runs_case_branch_with_nested_if_without_fallback() {
        let source = r#"
            VAR_GLOBAL
                g_current_step : UINT := UINT#30;
                g_last_error : UINT := UINT#0;
                g_power_status : BOOL := TRUE;
            END_VAR

            PROGRAM Main
            CASE g_current_step OF
                UINT#10:
                    IF FALSE THEN
                        g_current_step := UINT#20;
                    END_IF;
                UINT#20:
                    IF FALSE THEN
                        g_current_step := UINT#30;
                    END_IF;
                UINT#30:
                    IF g_power_status THEN
                        g_current_step := UINT#40;
                    END_IF;
                ELSE
                    g_last_error := UINT#512;
                    g_current_step := UINT#900;
            END_CASE;

            IF g_last_error <> UINT#0 THEN
                g_current_step := UINT#900;
            END_IF;
            END_PROGRAM
        "#;

        let mut harness = TestHarness::from_source(source).expect("create harness");
        harness
            .runtime_mut()
            .set_execution_backend(ExecutionBackend::BytecodeVm)
            .expect("set backend");
        harness
            .runtime_mut()
            .restart(RestartMode::Cold)
            .expect("restart");
        harness.runtime_mut().set_vm_register_profile_enabled(true);
        harness.runtime_mut().reset_vm_register_profile();

        let result = harness.cycle();
        assert!(
            result.errors.is_empty(),
            "cycle errors: {:?}",
            result.errors
        );
        assert_eq!(harness.get_output("g_current_step"), Some(Value::UInt(40)));
        assert_eq!(harness.get_output("g_last_error"), Some(Value::UInt(0)));

        let profile = harness.runtime().vm_register_profile_snapshot();
        assert!(profile.register_programs_executed >= 1);
        assert_eq!(
            profile.register_program_fallbacks, 0,
            "expected no register fallbacks, got {:?}",
            profile.fallback_reasons
        );
    }

    #[test]
    fn register_executor_progresses_motion_demo_to_step_40_without_error_by_cycle_three() {
        let project = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/plcopen_motion_single_axis_demo");
        let runtime_config =
            RuntimeConfig::load(project.join("runtime.toml")).expect("load runtime config");
        let cycle_budget = runtime_config.cycle_interval;
        let compile_sources =
            collect_project_source_files(&project, None).expect("collect project sources");
        let session = CompileSession::from_sources(compile_sources);
        let mut runtime = session.build_runtime().expect("build runtime");
        runtime
            .set_execution_backend(ExecutionBackend::BytecodeVm)
            .expect("set backend");
        runtime.set_vm_register_profile_enabled(true);
        runtime.reset_vm_register_profile();

        for cycle in 0..3 {
            runtime.execute_cycle().unwrap_or_else(|err| {
                panic!("cycle {} failed: {err}", cycle + 1);
            });
            runtime.advance_time(cycle_budget);
        }

        assert_eq!(
            runtime.storage().get_global("g_motion_demo_current_step"),
            Some(&Value::UInt(40))
        );
        assert_eq!(
            runtime.storage().get_global("g_motion_demo_last_error"),
            Some(&Value::Word(0))
        );

        let profile = runtime.vm_register_profile_snapshot();
        assert!(profile.register_programs_executed >= 1);
        assert_eq!(
            profile.register_program_fallbacks, 0,
            "expected no register fallbacks, got {:?}",
            profile.fallback_reasons
        );
    }

    #[test]
    fn register_ir_verifier_rejects_unknown_block_target() {
        let source = r#"
            PROGRAM Main
            END_PROGRAM
        "#;
        let (vm_module, pou_id) = vm_module_and_main_pou(source);
        let mut lowered = lower_pou_to_register_ir(&vm_module, pou_id).expect("lower register ir");
        lowered.blocks[0].instructions.push(RegisterInstr::Jump {
            target: BlockTarget::Block(9999),
        });
        let err = verify_register_program(&lowered).expect_err("verification should fail");
        let RuntimeError::InvalidBytecode(message) = err else {
            panic!("expected InvalidBytecode verification error");
        };
        assert!(
            message.contains("unknown block target"),
            "unexpected verification message: {message}",
        );
    }

    #[test]
    fn register_ir_lowering_rejects_invalid_jump_target() {
        let source = r#"
            PROGRAM Main
            END_PROGRAM
        "#;
        let mut bytecode = bytecode_module_from_source(source).expect("compile bytecode");
        let main_id = {
            let strings = match bytecode.section(SectionId::StringTable) {
                Some(SectionData::StringTable(strings)) => strings,
                _ => panic!("missing string table"),
            };
            let index = match bytecode.section(SectionId::PouIndex) {
                Some(SectionData::PouIndex(index)) => index,
                _ => panic!("missing pou index"),
            };
            index
                .entries
                .iter()
                .find(|entry| strings.entries[entry.name_idx as usize].eq_ignore_ascii_case("MAIN"))
                .map(|entry| entry.id)
                .expect("main entry id")
        };

        let mut body = Vec::new();
        body.push(0x02);
        body.extend_from_slice(&(4096_i32).to_le_bytes());
        body.push(0x06);

        let new_offset = if let Some(SectionData::PouBodies(code)) =
            bytecode.section_mut(SectionId::PouBodies)
        {
            let offset = code.len() as u32;
            code.extend_from_slice(&body);
            offset
        } else {
            panic!("missing POU_BODIES");
        };
        if let Some(SectionData::PouIndex(index)) = bytecode.section_mut(SectionId::PouIndex) {
            for entry in &mut index.entries {
                if entry.id == main_id {
                    entry.code_offset = new_offset;
                    entry.code_length = body.len() as u32;
                }
            }
        } else {
            panic!("missing POU_INDEX");
        }
        bytecode.sections.retain(|section| {
            section.id != SectionId::DebugMap.as_raw()
                && section.id != SectionId::DebugStringTable.as_raw()
        });

        let vm_module = VmModule::from_bytecode(&bytecode).expect("decode vm module");
        let pou_id = vm_module
            .program_ids
            .get(&SmolStr::new("MAIN"))
            .copied()
            .expect("main pou id");
        let err = lower_pou_to_register_ir(&vm_module, pou_id).expect_err("invalid jump must fail");
        let RuntimeError::InvalidBytecode(message) = err else {
            panic!("expected InvalidBytecode lowering error");
        };
        assert!(
            message.contains("invalid jump target"),
            "unexpected lowering message: {message}",
        );
    }

    #[test]
    fn register_ir_parity_matches_stack_subset_linear_program() {
        let mut code = Vec::new();
        code.push(0x20);
        emit_u32(&mut code, 0);
        code.push(0x10);
        emit_u32(&mut code, 0);
        code.push(0x40);
        code.push(0x21);
        emit_u32(&mut code, 0);
        code.push(0x06);
        let consts = vec![Value::DInt(1)];
        let (module, pou_id) = manual_vm_module(code, consts, 1);
        let lowered = lower_pou_to_register_ir(&module, pou_id).expect("lower register ir");
        verify_register_program(&lowered).expect("verify register ir");
        assert_no_fallback(&lowered);

        let mut stack_refs = vec![Value::DInt(41)];
        execute_stack_subset(&module, pou_id, &mut stack_refs).expect("execute stack subset");
        let mut register_refs = vec![Value::DInt(41)];
        execute_register_subset(&module, &lowered, &mut register_refs)
            .expect("execute register subset");

        assert_eq!(register_refs, stack_refs);
        assert_eq!(register_refs, vec![Value::DInt(42)]);
    }

    #[test]
    fn register_ir_parity_matches_stack_subset_loop_program() {
        let mut code = Vec::new();
        code.push(0x10);
        emit_u32(&mut code, 0);
        code.push(0x21);
        emit_u32(&mut code, 0);
        code.push(0x10);
        emit_u32(&mut code, 0);
        code.push(0x21);
        emit_u32(&mut code, 1);

        let loop_check_pc = code.len();
        code.push(0x20);
        emit_u32(&mut code, 0);
        code.push(0x10);
        emit_u32(&mut code, 2);
        code.push(0x52);

        let jump_false_pc = code.len();
        code.push(0x04);
        emit_i32(&mut code, 0);

        code.push(0x20);
        emit_u32(&mut code, 1);
        code.push(0x20);
        emit_u32(&mut code, 0);
        code.push(0x40);
        code.push(0x21);
        emit_u32(&mut code, 1);
        code.push(0x20);
        emit_u32(&mut code, 0);
        code.push(0x10);
        emit_u32(&mut code, 1);
        code.push(0x40);
        code.push(0x21);
        emit_u32(&mut code, 0);

        let jump_back_pc = code.len();
        code.push(0x02);
        emit_i32(&mut code, 0);

        let loop_end_pc = code.len();
        code.push(0x06);

        let jump_false_offset = loop_end_pc as i32 - (jump_false_pc + 5) as i32;
        patch_i32(&mut code, jump_false_pc + 1, jump_false_offset);
        let jump_back_offset = loop_check_pc as i32 - (jump_back_pc + 5) as i32;
        patch_i32(&mut code, jump_back_pc + 1, jump_back_offset);

        let consts = vec![Value::DInt(0), Value::DInt(1), Value::DInt(3)];
        let (module, pou_id) = manual_vm_module(code, consts, 2);
        let lowered = lower_pou_to_register_ir(&module, pou_id).expect("lower register ir");
        verify_register_program(&lowered).expect("verify register ir");
        assert_no_fallback(&lowered);

        let mut stack_refs = vec![Value::DInt(7), Value::DInt(7)];
        execute_stack_subset(&module, pou_id, &mut stack_refs).expect("execute stack subset");
        let mut register_refs = vec![Value::DInt(7), Value::DInt(7)];
        execute_register_subset(&module, &lowered, &mut register_refs)
            .expect("execute register subset");

        assert_eq!(register_refs, stack_refs);
        assert_eq!(register_refs, vec![Value::DInt(3), Value::DInt(3)]);
    }

    #[test]
    fn register_executor_runs_supported_program() {
        let mut code = Vec::new();
        code.push(0x20);
        emit_u32(&mut code, 0);
        code.push(0x10);
        emit_u32(&mut code, 0);
        code.push(0x40);
        code.push(0x21);
        emit_u32(&mut code, 0);
        code.push(0x06);
        let (module, pou_id) = manual_vm_module(code, vec![Value::DInt(1)], 1);

        let mut runtime = Runtime::new();
        runtime.storage_mut().set_global("g0", Value::DInt(41));

        let outcome = try_execute_pou_with_register_ir(&mut runtime, &module, pou_id, None)
            .expect("execute register program");
        assert_eq!(outcome, RegisterExecutionOutcome::Executed);
        assert_eq!(runtime.storage().get_global("g0"), Some(&Value::DInt(42)));
    }

    #[test]
    fn register_executor_profile_records_hot_blocks_for_supported_program() {
        let mut code = Vec::new();
        code.push(0x20);
        emit_u32(&mut code, 0);
        code.push(0x10);
        emit_u32(&mut code, 0);
        code.push(0x40);
        code.push(0x21);
        emit_u32(&mut code, 0);
        code.push(0x06);
        let (module, pou_id) = manual_vm_module(code, vec![Value::DInt(1)], 1);

        let mut runtime = Runtime::new();
        runtime.storage_mut().set_global("g0", Value::DInt(41));
        runtime.set_vm_register_profile_enabled(true);
        runtime.reset_vm_register_profile();

        let outcome = try_execute_pou_with_register_ir(&mut runtime, &module, pou_id, None)
            .expect("execute register program");
        assert_eq!(outcome, RegisterExecutionOutcome::Executed);

        let profile = runtime.vm_register_profile_snapshot();
        assert!(profile.enabled);
        assert_eq!(profile.register_programs_executed, 1);
        assert_eq!(profile.register_program_fallbacks, 0);
        assert!(
            profile
                .hot_blocks
                .iter()
                .any(|block| block.pou_id == pou_id && block.hits >= 1),
            "expected at least one hot block for executed POU",
        );
    }

    #[test]
    fn register_executor_profile_records_dynamic_ref_and_instance_lookup_counters() {
        let mut code = Vec::new();
        code.push(0x22);
        emit_u32(&mut code, 0);
        code.push(0x30);
        emit_u32(&mut code, 0);
        code.push(0x32);
        code.push(0x21);
        emit_u32(&mut code, 1);
        code.push(0x22);
        emit_u32(&mut code, 0);
        code.push(0x30);
        emit_u32(&mut code, 0);
        code.push(0x10);
        emit_u32(&mut code, 0);
        code.push(0x33);
        code.push(0x23);
        code.push(0x30);
        emit_u32(&mut code, 1);
        code.push(0x32);
        code.push(0x21);
        emit_u32(&mut code, 2);
        code.push(0x23);
        code.push(0x30);
        emit_u32(&mut code, 1);
        code.push(0x10);
        emit_u32(&mut code, 1);
        code.push(0x33);
        code.push(0x06);

        let pou_id = 1_u32;
        let mut pou_by_id = HashMap::new();
        pou_by_id.insert(
            pou_id,
            VmPouEntry {
                name: SmolStr::new("MAIN"),
                code_start: 0,
                code_end: code.len(),
                local_ref_start: 0,
                local_ref_count: 0,
                primary_instance_owner: None,
            },
        );
        let mut program_ids = HashMap::new();
        program_ids.insert(SmolStr::new("MAIN"), pou_id);
        let refs = vec![
            VmRef::Global {
                offset: 0,
                path: Vec::new(),
            },
            VmRef::Global {
                offset: 1,
                path: Vec::new(),
            },
            VmRef::Global {
                offset: 2,
                path: Vec::new(),
            },
        ];
        let module = VmModule {
            code,
            strings: vec![SmolStr::new("VALUE"), SmolStr::new("ACC")],
            types: TypeTable::default(),
            refs,
            consts: vec![Value::DInt(11), Value::DInt(13)],
            pou_by_id,
            program_ids,
            function_ids: HashMap::new(),
            function_block_ids: HashMap::new(),
            class_ids: HashMap::new(),
            native_symbol_specs: Vec::new(),
            pou_params: HashMap::new(),
            pou_has_return_slot: HashSet::new(),
            method_table_by_owner: HashMap::new(),
            debug_map: super::super::debug_map::VmDebugMap::default(),
            instruction_budget: super::super::DEFAULT_INSTRUCTION_BUDGET,
        };

        let mut runtime = Runtime::new();
        runtime.storage_mut().set_global(
            "g0",
            Value::Struct(std::sync::Arc::new(StructValue {
                type_name: SmolStr::new("CELL_T"),
                fields: IndexMap::from([(SmolStr::new("VALUE"), Value::DInt(7))]),
            })),
        );
        runtime.storage_mut().set_global("g1", Value::DInt(0));
        runtime.storage_mut().set_global("g2", Value::DInt(0));
        let instance_id = runtime.storage_mut().create_instance("COUNTER");
        assert!(runtime
            .storage_mut()
            .set_instance_var(instance_id, "ACC", Value::DInt(9)));
        runtime.set_vm_register_profile_enabled(true);
        runtime.reset_vm_register_profile();

        let outcome =
            try_execute_pou_with_register_ir(&mut runtime, &module, pou_id, Some(instance_id))
                .expect("execute register program");
        assert_eq!(outcome, RegisterExecutionOutcome::Executed);
        assert_eq!(runtime.storage().get_global("g1"), Some(&Value::DInt(7)));
        assert_eq!(runtime.storage().get_global("g2"), Some(&Value::DInt(9)));
        assert_eq!(
            runtime.storage().get_global("g0"),
            Some(&Value::Struct(std::sync::Arc::new(StructValue {
                type_name: SmolStr::new("CELL_T"),
                fields: IndexMap::from([(SmolStr::new("VALUE"), Value::DInt(11))]),
            })))
        );
        assert_eq!(
            runtime.storage().get_instance_var(instance_id, "ACC"),
            Some(&Value::DInt(13))
        );

        let profile = runtime.vm_register_profile_snapshot();
        assert_eq!(profile.register_program_fallbacks, 0);
        assert_eq!(profile.ref_ops.load_ref, 0);
        assert_eq!(profile.ref_ops.store_ref, 2);
        assert_eq!(profile.ref_ops.load_ref_addr, 2);
        assert_eq!(profile.ref_ops.ref_field, 4);
        assert_eq!(profile.ref_ops.ref_index, 0);
        assert_eq!(profile.ref_ops.load_dynamic, 2);
        assert_eq!(profile.ref_ops.store_dynamic, 2);
        assert_eq!(profile.ref_ops.instance_field_lookups, 2);
        assert_eq!(profile.value_ops.read_value_clones, 0);
    }

    #[test]
    fn register_executor_profile_records_function_block_call_counters() {
        let source = r#"
            FUNCTION_BLOCK Counter
            VAR_INPUT
                inc : BOOL;
            END_VAR
            VAR_OUTPUT
                value : INT;
            END_VAR

            IF inc THEN
                value := value + INT#1;
            END_IF;
            END_FUNCTION_BLOCK

            PROGRAM Main
            VAR
                fb : Counter;
                out_count : INT := INT#0;
            END_VAR
            fb(inc := TRUE, value => out_count);
            END_PROGRAM
        "#;

        let mut harness = TestHarness::from_source(source).expect("create harness");
        harness
            .runtime_mut()
            .set_execution_backend(ExecutionBackend::BytecodeVm)
            .expect("set backend");
        harness
            .runtime_mut()
            .restart(RestartMode::Cold)
            .expect("restart");
        harness.runtime_mut().set_vm_register_profile_enabled(true);
        harness.runtime_mut().reset_vm_register_profile();

        let result = harness.cycle();
        assert!(
            result.errors.is_empty(),
            "cycle errors: {:?}",
            result.errors
        );
        assert_eq!(harness.get_output("out_count"), Some(Value::Int(1)));

        let profile = harness.runtime().vm_register_profile_snapshot();
        assert_eq!(profile.register_program_fallbacks, 0);
        assert_eq!(profile.call_ops.function_block_call_entries, 1);
        assert_eq!(profile.call_ops.parameter_bindings, 2);
        assert_eq!(profile.call_ops.output_copy_backs, 1);
        assert!(profile.call_ops.frame_pushes >= 2);
        assert!(profile.call_ops.frame_pops >= 2);
        assert_eq!(profile.value_ops.binding_expr_clones, 0);
        assert_eq!(profile.value_ops.output_value_clones, 0);
    }

    #[test]
    fn register_executor_profile_avoids_clone_counters_for_struct_inout_function_block() {
        let source = r#"
            TYPE AXIS_REF :
            STRUCT
                AxisId : UDINT;
                InternalIndex : UINT;
            END_STRUCT
            END_TYPE

            FUNCTION_BLOCK TouchAxis
            VAR_IN_OUT
                Axis : AXIS_REF;
            END_VAR
            VAR_OUTPUT
                Done : BOOL;
            END_VAR

            Axis.InternalIndex := Axis.InternalIndex + UINT#1;
            Done := TRUE;
            END_FUNCTION_BLOCK

            PROGRAM Main
            VAR
                Axis : AXIS_REF;
                Fb : TouchAxis;
            END_VAR

            Axis.AxisId := UDINT#1;
            Axis.InternalIndex := UINT#1;
            Fb(Axis := Axis);
            END_PROGRAM
        "#;

        let mut harness = TestHarness::from_source(source).expect("create harness");
        harness
            .runtime_mut()
            .set_execution_backend(ExecutionBackend::BytecodeVm)
            .expect("set backend");
        harness
            .runtime_mut()
            .restart(RestartMode::Cold)
            .expect("restart");
        harness.runtime_mut().set_vm_register_profile_enabled(true);
        harness.runtime_mut().reset_vm_register_profile();

        let result = harness.cycle();
        assert!(
            result.errors.is_empty(),
            "cycle errors: {:?}",
            result.errors
        );

        let profile = harness.runtime().vm_register_profile_snapshot();
        assert_eq!(profile.register_program_fallbacks, 0);
        assert!(profile.call_ops.function_block_call_entries >= 1);
        assert!(profile.call_ops.parameter_bindings >= 1);
        assert!(profile.call_ops.output_copy_backs >= 1);
        assert_eq!(
            profile.value_ops.read_value_clones, 0,
            "profile: {:?}",
            profile.value_ops
        );
        assert_eq!(
            profile.value_ops.output_value_clones, 0,
            "profile: {:?}",
            profile.value_ops
        );
    }

    #[test]
    fn read_register_with_counts_records_clone_then_move_reads() {
        let mut profile = RegisterProfileState::default();
        profile.set_enabled(true);
        let mut registers = vec![Value::DInt(7)];
        let mut remaining = vec![2_u32];

        let first = read_register_with_counts(
            &mut profile,
            registers.as_mut_slice(),
            remaining.as_mut_slice(),
            RegisterId(0),
        )
        .expect("first read");
        let second = read_register_with_counts(
            &mut profile,
            registers.as_mut_slice(),
            remaining.as_mut_slice(),
            RegisterId(0),
        )
        .expect("second read");

        assert_eq!(first, Value::DInt(7));
        assert_eq!(second, Value::DInt(7));
        assert_eq!(registers[0], Value::Null);
        let snapshot = profile.snapshot();
        assert_eq!(snapshot.value_ops.register_read_clones, 1);
        assert_eq!(snapshot.value_ops.register_read_moves, 1);
    }

    #[test]
    fn register_executor_falls_back_when_lowering_contains_unsupported_opcode() {
        let mut code = Vec::new();
        code.push(0x07);
        emit_u32(&mut code, 0);
        code.push(0x06);
        let (module, pou_id) = manual_vm_module(code, Vec::new(), 0);
        let mut runtime = Runtime::new();

        let outcome = try_execute_pou_with_register_ir(&mut runtime, &module, pou_id, None)
            .expect("fallback decision");
        assert_eq!(outcome, RegisterExecutionOutcome::FallbackToStack);
    }

    #[test]
    fn register_executor_profile_records_ref_op_counters_for_load_ref_store_ref_program() {
        let mut code = Vec::new();
        code.push(0x20);
        emit_u32(&mut code, 0);
        code.push(0x21);
        emit_u32(&mut code, 1);
        code.push(0x06);
        let (module, pou_id) = manual_vm_module(code, Vec::new(), 2);

        let mut runtime = Runtime::new();
        runtime.storage_mut().set_global("g0", Value::DInt(41));
        runtime.storage_mut().set_global("g1", Value::DInt(0));
        runtime.set_vm_register_profile_enabled(true);
        runtime.reset_vm_register_profile();

        let outcome = try_execute_pou_with_register_ir(&mut runtime, &module, pou_id, None)
            .expect("execute register program");
        assert_eq!(outcome, RegisterExecutionOutcome::Executed);
        assert_eq!(runtime.storage().get_global("g1"), Some(&Value::DInt(41)));

        let profile = runtime.vm_register_profile_snapshot();
        assert_eq!(profile.register_program_fallbacks, 0);
        assert_eq!(profile.ref_ops.load_ref, 1);
        assert_eq!(profile.ref_ops.store_ref, 1);
        assert_eq!(profile.ref_ops.load_ref_addr, 0);
        assert_eq!(profile.ref_ops.ref_field, 0);
        assert_eq!(profile.ref_ops.ref_index, 0);
        assert_eq!(profile.ref_ops.load_dynamic, 0);
        assert_eq!(profile.ref_ops.store_dynamic, 0);
        assert_eq!(profile.ref_ops.instance_field_lookups, 0);
        assert_eq!(profile.value_ops.read_value_clones, 0);
        assert_eq!(profile.value_ops.register_read_moves, 1);
        assert_eq!(profile.value_ops.register_read_clones, 0);
    }

    #[test]
    fn register_executor_profile_avoids_clone_counter_for_scalar_load_const() {
        let mut code = Vec::new();
        code.push(0x10);
        emit_u32(&mut code, 0);
        code.push(0x21);
        emit_u32(&mut code, 0);
        code.push(0x06);
        let (module, pou_id) = manual_vm_module(code, vec![Value::DInt(41)], 1);

        let mut runtime = Runtime::new();
        runtime.storage_mut().set_global("g0", Value::DInt(0));
        runtime.set_vm_register_profile_enabled(true);
        runtime.reset_vm_register_profile();

        let outcome = try_execute_pou_with_register_ir(&mut runtime, &module, pou_id, None)
            .expect("execute register program");
        assert_eq!(outcome, RegisterExecutionOutcome::Executed);
        assert_eq!(runtime.storage().get_global("g0"), Some(&Value::DInt(41)));

        let profile = runtime.vm_register_profile_snapshot();
        assert_eq!(profile.register_program_fallbacks, 0);
        assert_eq!(profile.value_ops.const_load_clones, 0);
    }

    #[test]
    fn register_executor_profile_avoids_clone_counters_for_borrowed_ref_const_binary_guard() {
        let mut code = Vec::new();
        code.push(0x20);
        emit_u32(&mut code, 0);
        code.push(0x10);
        emit_u32(&mut code, 0);
        code.push(0x40);
        code.push(0x21);
        emit_u32(&mut code, 1);
        code.push(0x06);
        let (module, pou_id) = manual_vm_module(code, vec![Value::DInt(1)], 2);

        let mut runtime = Runtime::new();
        runtime.storage_mut().set_global("g0", Value::DInt(41));
        runtime.storage_mut().set_global("g1", Value::DInt(0));
        runtime.set_vm_register_profile_enabled(true);
        runtime.reset_vm_register_profile();

        let outcome = try_execute_pou_with_register_ir(&mut runtime, &module, pou_id, None)
            .expect("execute register program");
        assert_eq!(outcome, RegisterExecutionOutcome::Executed);
        assert_eq!(runtime.storage().get_global("g1"), Some(&Value::DInt(42)));

        let profile = runtime.vm_register_profile_snapshot();
        assert_eq!(profile.register_program_fallbacks, 0);
        assert_eq!(profile.value_ops.read_value_clones, 0);
        assert_eq!(profile.value_ops.const_load_clones, 0);
    }

    #[test]
    fn register_executor_profile_avoids_clone_counters_for_borrowed_ref_ref_non_dint_binary() {
        let mut code = Vec::new();
        code.push(0x20);
        emit_u32(&mut code, 0);
        code.push(0x20);
        emit_u32(&mut code, 1);
        code.push(0x47);
        code.push(0x21);
        emit_u32(&mut code, 2);
        code.push(0x06);
        let (module, pou_id) = manual_vm_module(code, Vec::new(), 3);

        let mut runtime = Runtime::new();
        runtime.storage_mut().set_global("g0", Value::Bool(false));
        runtime.storage_mut().set_global("g1", Value::Bool(true));
        runtime.storage_mut().set_global("g2", Value::Bool(false));
        runtime.set_vm_register_profile_enabled(true);
        runtime.reset_vm_register_profile();

        let outcome = try_execute_pou_with_register_ir(&mut runtime, &module, pou_id, None)
            .expect("execute register program");
        assert_eq!(outcome, RegisterExecutionOutcome::Executed);
        assert_eq!(runtime.storage().get_global("g2"), Some(&Value::Bool(true)));

        let profile = runtime.vm_register_profile_snapshot();
        assert_eq!(profile.register_program_fallbacks, 0);
        assert_eq!(profile.value_ops.read_value_clones, 0);
    }

    #[test]
    fn register_executor_profile_avoids_clone_counters_for_borrowed_ref_const_non_dint_binary() {
        let mut code = Vec::new();
        code.push(0x20);
        emit_u32(&mut code, 0);
        code.push(0x10);
        emit_u32(&mut code, 0);
        code.push(0x47);
        code.push(0x21);
        emit_u32(&mut code, 1);
        code.push(0x06);
        let (module, pou_id) = manual_vm_module(code, vec![Value::Bool(true)], 2);

        let mut runtime = Runtime::new();
        runtime.storage_mut().set_global("g0", Value::Bool(false));
        runtime.storage_mut().set_global("g1", Value::Bool(false));
        runtime.set_vm_register_profile_enabled(true);
        runtime.reset_vm_register_profile();

        let outcome = try_execute_pou_with_register_ir(&mut runtime, &module, pou_id, None)
            .expect("execute register program");
        assert_eq!(outcome, RegisterExecutionOutcome::Executed);
        assert_eq!(runtime.storage().get_global("g1"), Some(&Value::Bool(true)));

        let profile = runtime.vm_register_profile_snapshot();
        assert_eq!(profile.register_program_fallbacks, 0);
        assert_eq!(profile.value_ops.read_value_clones, 0);
        assert_eq!(profile.value_ops.const_load_clones, 0);
    }

    #[test]
    fn register_executor_profile_records_fallback_reason() {
        let mut code = Vec::new();
        code.push(0x07);
        emit_u32(&mut code, 0);
        code.push(0x06);
        let (module, pou_id) = manual_vm_module(code, Vec::new(), 0);
        let mut runtime = Runtime::new();
        runtime.set_vm_register_profile_enabled(true);
        runtime.reset_vm_register_profile();

        let outcome = try_execute_pou_with_register_ir(&mut runtime, &module, pou_id, None)
            .expect("fallback decision");
        assert_eq!(outcome, RegisterExecutionOutcome::FallbackToStack);

        let profile = runtime.vm_register_profile_snapshot();
        assert_eq!(profile.register_programs_executed, 0);
        assert_eq!(profile.register_program_fallbacks, 1);
        assert!(
            profile
                .fallback_reasons
                .iter()
                .any(|entry| entry.reason.starts_with("unsupported_opcode") && entry.count == 1),
            "expected unsupported opcode fallback reason in profile snapshot",
        );
    }

    #[test]
    fn register_ir_lowering_handles_function_block_self_fields_without_fallback() {
        let source = r#"
            FUNCTION_BLOCK Counter
            VAR_INPUT
                Enable : BOOL;
            END_VAR
            VAR_OUTPUT
                Value : DINT;
            END_VAR

            IF Enable THEN
                Value := Value + DINT#1;
            END_IF;
            END_FUNCTION_BLOCK

            PROGRAM Main
            VAR
                fb : Counter;
            END_VAR
            fb(Enable := TRUE);
            END_PROGRAM
        "#;

        let bytecode = bytecode_module_from_source(source).expect("compile bytecode");
        let vm_module = VmModule::from_bytecode(&bytecode).expect("decode vm module");
        let fb_pou_id = vm_module
            .function_block_ids
            .get(&SmolStr::new("COUNTER"))
            .copied()
            .expect("counter pou id");
        let lowered = lower_pou_to_register_ir(&vm_module, fb_pou_id).expect("lower register ir");
        verify_register_program(&lowered).expect("verify register ir");
        assert_no_fallback(&lowered);
    }

    #[test]
    fn tier1_compiler_accepts_function_block_self_field_dynamic_ops() {
        let source = r#"
            FUNCTION_BLOCK Counter
            VAR_OUTPUT
                Value : DINT;
            END_VAR

            Value := Value + DINT#1;
            END_FUNCTION_BLOCK

            PROGRAM Main
            VAR
                fb : Counter;
            END_VAR

            fb();
            END_PROGRAM
        "#;

        let bytecode = bytecode_module_from_source(source).expect("compile bytecode");
        let vm_module = VmModule::from_bytecode(&bytecode).expect("decode vm module");
        let fb_pou_id = vm_module
            .function_block_ids
            .get(&SmolStr::new("COUNTER"))
            .copied()
            .expect("counter pou id");
        let lowered = lower_pou_to_register_ir(&vm_module, fb_pou_id).expect("lower register ir");
        verify_register_program(&lowered).expect("verify register ir");
        let block = lowered
            .blocks
            .iter()
            .find(|block| {
                block
                    .instructions
                    .iter()
                    .any(|instruction| matches!(instruction, RegisterInstr::RefField { .. }))
                    && block
                        .instructions
                        .iter()
                        .any(|instruction| matches!(instruction, RegisterInstr::LoadDynamic { .. }))
                    && block.instructions.iter().any(|instruction| {
                        matches!(instruction, RegisterInstr::StoreDynamic { .. })
                    })
            })
            .expect("function block ref/dynamic block");
        let key = super::tier1_block_key(&vm_module, fb_pou_id, block);
        assert!(
            super::compile_tier1_block(&vm_module, block, key).is_ok(),
            "expected tier-1 compiler to accept self-field dynamic block: {:?}",
            block.instructions
        );
    }

    #[test]
    fn tier1_compiler_accepts_function_block_index_dynamic_ops() {
        let source = r#"
            FUNCTION_BLOCK CounterArray
            VAR_OUTPUT
                Data : ARRAY[1..2] OF DINT;
            END_VAR

            Data[1] := Data[1] + DINT#1;
            END_FUNCTION_BLOCK

            PROGRAM Main
            VAR
                fb : CounterArray;
            END_VAR

            fb();
            END_PROGRAM
        "#;

        let bytecode = bytecode_module_from_source(source).expect("compile bytecode");
        let vm_module = VmModule::from_bytecode(&bytecode).expect("decode vm module");
        let fb_pou_id = vm_module
            .function_block_ids
            .get(&SmolStr::new("COUNTERARRAY"))
            .copied()
            .expect("counterarray pou id");
        let lowered = lower_pou_to_register_ir(&vm_module, fb_pou_id).expect("lower register ir");
        verify_register_program(&lowered).expect("verify register ir");
        let block = lowered
            .blocks
            .iter()
            .find(|block| {
                block
                    .instructions
                    .iter()
                    .any(|instruction| matches!(instruction, RegisterInstr::RefIndex { .. }))
                    && block
                        .instructions
                        .iter()
                        .any(|instruction| matches!(instruction, RegisterInstr::LoadDynamic { .. }))
                    && block.instructions.iter().any(|instruction| {
                        matches!(instruction, RegisterInstr::StoreDynamic { .. })
                    })
            })
            .expect("function block ref-index/dynamic block");
        let key = super::tier1_block_key(&vm_module, fb_pou_id, block);
        assert!(
            super::compile_tier1_block(&vm_module, block, key).is_ok(),
            "expected tier-1 compiler to accept index dynamic block: {:?}",
            block.instructions
        );
    }

    #[test]
    fn register_executor_tier1_specialized_executor_executes_array_ref_blocks() {
        let source = r#"
            VAR_GLOBAL
                g_value : DINT;
            END_VAR

            FUNCTION_BLOCK CounterArray
            VAR_OUTPUT
                Data : ARRAY[1..2] OF DINT;
            END_VAR

            Data[1] := Data[1] + DINT#1;
            END_FUNCTION_BLOCK

            PROGRAM Main
            VAR
                fb : CounterArray;
            END_VAR

            fb();
            g_value := fb.Data[1];
            END_PROGRAM
        "#;

        let mut harness = TestHarness::from_source(source).expect("create harness");
        harness
            .runtime_mut()
            .set_execution_backend(ExecutionBackend::BytecodeVm)
            .expect("set backend");
        harness
            .runtime_mut()
            .restart(RestartMode::Cold)
            .expect("restart");
        harness
            .runtime_mut()
            .vm_tier1_specialized_executor
            .set_enabled(true);
        harness
            .runtime_mut()
            .vm_tier1_specialized_executor
            .hot_block_threshold = 1;
        harness.runtime_mut().reset_vm_tier1_specialized_executor();
        harness
            .runtime_mut()
            .vm_tier1_specialized_executor
            .hot_block_threshold = 1;

        for cycle in 0..3 {
            let result = harness.cycle();
            assert!(
                result.errors.is_empty(),
                "cycle {} errors: {:?}",
                cycle + 1,
                result.errors
            );
        }

        assert_eq!(harness.get_output("g_value"), Some(Value::DInt(3)));
        let snapshot = harness.runtime().vm_tier1_specialized_executor_snapshot();
        assert!(
            snapshot.compile_successes >= 1,
            "expected at least one compiled tier-1 block, snapshot={snapshot:?}"
        );
        assert!(
            snapshot.block_executions >= 1,
            "expected at least one executed compiled tier-1 block, snapshot={snapshot:?}"
        );
    }

    #[test]
    fn register_executor_runs_program_with_complex_local_fields_without_fallback() {
        let pou_id = 1_u32;
        let code = vec![
            0x10, 0, 0, 0, 0, // LOAD_CONST 0
            0x21, 0, 0, 0, 0, // STORE_REF local path
            0x20, 0, 0, 0, 0, // LOAD_REF local path
            0x21, 1, 0, 0, 0,    // STORE_REF global
            0x06, // RETURN
        ];
        let mut pou_by_id = HashMap::new();
        pou_by_id.insert(
            pou_id,
            VmPouEntry {
                name: SmolStr::new("MAIN"),
                code_start: 0,
                code_end: code.len(),
                local_ref_start: 0,
                local_ref_count: 1,
                primary_instance_owner: None,
            },
        );
        let mut program_ids = HashMap::new();
        program_ids.insert(SmolStr::new("MAIN"), pou_id);
        let refs = vec![
            VmRef::Local {
                owner_frame_id: 0,
                offset: 0,
                path: vec![
                    RefSegment::Field(SmolStr::new("INNER")),
                    RefSegment::Field(SmolStr::new("VALUE")),
                ],
            },
            VmRef::Global {
                offset: 0,
                path: Vec::new(),
            },
        ];
        let module = VmModule {
            code,
            strings: Vec::new(),
            types: TypeTable::default(),
            refs,
            consts: vec![Value::DInt(7)],
            pou_by_id,
            program_ids,
            function_ids: HashMap::new(),
            function_block_ids: HashMap::new(),
            class_ids: HashMap::new(),
            native_symbol_specs: Vec::new(),
            pou_params: HashMap::new(),
            pou_has_return_slot: HashSet::new(),
            method_table_by_owner: HashMap::new(),
            debug_map: super::super::debug_map::VmDebugMap::default(),
            instruction_budget: super::super::DEFAULT_INSTRUCTION_BUDGET,
        };
        let initial_outer = Value::Struct(std::sync::Arc::new(StructValue {
            type_name: SmolStr::new("OUTER_T"),
            fields: IndexMap::from([(
                SmolStr::new("INNER"),
                Value::Struct(std::sync::Arc::new(StructValue {
                    type_name: SmolStr::new("INNER_T"),
                    fields: IndexMap::from([(SmolStr::new("VALUE"), Value::DInt(0))]),
                })),
            )]),
        }));
        let mut runtime = Runtime::new();
        runtime.storage_mut().set_global("g0", Value::DInt(0));
        runtime.set_vm_register_profile_enabled(true);
        runtime.reset_vm_register_profile();

        let outcome = try_execute_pou_with_register_ir_with_locals(
            &mut runtime,
            &module,
            pou_id,
            None,
            Some(&[initial_outer]),
            false,
            0,
            None,
        )
        .expect("execute register program");
        assert!(
            outcome.is_some(),
            "expected register execution, got stack fallback"
        );
        assert_eq!(runtime.storage().get_global("g0"), Some(&Value::DInt(7)));

        let profile = runtime.vm_register_profile_snapshot();
        assert_eq!(
            profile.register_program_fallbacks, 0,
            "expected no register fallback reasons, got {:?}",
            profile.fallback_reasons
        );
    }

    #[test]
    fn register_lowering_error_fallback_reason_includes_pou_name_and_message() {
        let mut code = Vec::new();
        code.push(0x02);
        emit_i32(&mut code, 4096);
        code.push(0x06);
        let (module, pou_id) = manual_vm_module(code, Vec::new(), 0);

        let mut runtime = Runtime::new();
        runtime.set_vm_register_profile_enabled(true);
        runtime.reset_vm_register_profile();

        let outcome = try_execute_pou_with_register_ir(&mut runtime, &module, pou_id, None)
            .expect("fallback decision");
        assert_eq!(outcome, RegisterExecutionOutcome::FallbackToStack);

        let profile = runtime.vm_register_profile_snapshot();
        assert!(
            profile.fallback_reasons.iter().any(|entry| {
                entry.reason.contains("lowering_error")
                    && entry.reason.contains("MAIN")
                    && entry.reason.contains("invalid jump target")
                    && entry.count == 1
            }),
            "expected lowering_error fallback reason with pou name and message, got {:?}",
            profile.fallback_reasons
        );
    }

    #[test]
    fn register_lowering_cache_hits_after_first_execution() {
        let mut code = Vec::new();
        code.push(0x20);
        emit_u32(&mut code, 0);
        code.push(0x10);
        emit_u32(&mut code, 0);
        code.push(0x40);
        code.push(0x21);
        emit_u32(&mut code, 0);
        code.push(0x06);
        let (module, pou_id) = manual_vm_module(code, vec![Value::DInt(1)], 1);

        let mut runtime = Runtime::new();
        runtime.set_vm_register_lowering_cache_enabled(true);
        runtime.reset_vm_register_lowering_cache();
        runtime.storage_mut().set_global("g0", Value::DInt(1));

        let first = try_execute_pou_with_register_ir(&mut runtime, &module, pou_id, None)
            .expect("first execution");
        let second = try_execute_pou_with_register_ir(&mut runtime, &module, pou_id, None)
            .expect("second execution");
        assert_eq!(first, RegisterExecutionOutcome::Executed);
        assert_eq!(second, RegisterExecutionOutcome::Executed);

        let snapshot = runtime.vm_register_lowering_cache_snapshot();
        assert!(snapshot.enabled);
        assert_eq!(snapshot.cached_entries, 1);
        assert_eq!(snapshot.misses, 1);
        assert_eq!(snapshot.hits, 1);
        assert_eq!(snapshot.build_errors, 0);
    }

    #[test]
    fn register_lowering_cache_caches_lowering_errors() {
        let mut code = Vec::new();
        code.push(0x02);
        emit_i32(&mut code, 4096);
        code.push(0x06);
        let (module, pou_id) = manual_vm_module(code, Vec::new(), 0);

        let mut runtime = Runtime::new();
        runtime.set_vm_register_lowering_cache_enabled(true);
        runtime.reset_vm_register_lowering_cache();

        let first = try_execute_pou_with_register_ir(&mut runtime, &module, pou_id, None)
            .expect("first fallback");
        let second = try_execute_pou_with_register_ir(&mut runtime, &module, pou_id, None)
            .expect("second fallback");
        assert_eq!(first, RegisterExecutionOutcome::FallbackToStack);
        assert_eq!(second, RegisterExecutionOutcome::FallbackToStack);

        let snapshot = runtime.vm_register_lowering_cache_snapshot();
        assert!(snapshot.enabled);
        assert_eq!(snapshot.cached_entries, 1);
        assert_eq!(snapshot.misses, 1);
        assert_eq!(snapshot.hits, 1);
        assert_eq!(snapshot.build_errors, 1);
    }

    #[test]
    fn register_executor_tier1_specialized_executor_keeps_startup_path_cold_until_hot_threshold() {
        let mut code = Vec::new();
        code.push(0x20);
        emit_u32(&mut code, 0);
        code.push(0x10);
        emit_u32(&mut code, 0);
        code.push(0x40);
        code.push(0x21);
        emit_u32(&mut code, 0);
        code.push(0x06);
        let (module, pou_id) = manual_vm_module(code, vec![Value::DInt(1)], 1);

        let mut runtime = Runtime::new();
        runtime.storage_mut().set_global("g0", Value::DInt(41));
        runtime.set_vm_tier1_specialized_executor_enabled(true);
        runtime.reset_vm_tier1_specialized_executor();

        let outcome = try_execute_pou_with_register_ir(&mut runtime, &module, pou_id, None)
            .expect("execute register program");
        assert_eq!(outcome, RegisterExecutionOutcome::Executed);
        let snapshot = runtime.vm_tier1_specialized_executor_snapshot();
        assert_eq!(snapshot.compile_attempts, 0);
        assert_eq!(snapshot.block_executions, 0);
    }

    #[test]
    fn tier1_compiler_accepts_load_ref_addr_dynamic_block() {
        let mut code = Vec::new();
        code.push(0x22);
        emit_u32(&mut code, 0);
        code.push(0x32);
        code.push(0x21);
        emit_u32(&mut code, 1);
        code.push(0x06);
        let (module, pou_id) = manual_vm_module(code, Vec::new(), 2);

        let lowered = lower_pou_to_register_ir(&module, pou_id).expect("lower register ir");
        verify_register_program(&lowered).expect("verify register ir");
        let block = lowered
            .blocks
            .iter()
            .find(|block| {
                block
                    .instructions
                    .iter()
                    .any(|instruction| matches!(instruction, RegisterInstr::LoadRefAddr { .. }))
                    && block
                        .instructions
                        .iter()
                        .any(|instruction| matches!(instruction, RegisterInstr::LoadDynamic { .. }))
            })
            .expect("load-ref-addr block");
        let key = super::tier1_block_key(&module, pou_id, block);
        assert!(
            super::compile_tier1_block(&module, block, key).is_ok(),
            "expected tier-1 compiler to accept LoadRefAddr block: {:?}",
            block.instructions
        );
    }

    #[test]
    fn register_executor_tier1_specialized_executor_executes_load_ref_addr_block() {
        let mut code = Vec::new();
        code.push(0x22);
        emit_u32(&mut code, 0);
        code.push(0x32);
        code.push(0x21);
        emit_u32(&mut code, 1);
        code.push(0x06);
        let (module, pou_id) = manual_vm_module(code, Vec::new(), 2);

        let mut runtime = Runtime::new();
        runtime.storage_mut().set_global("g0", Value::DInt(41));
        runtime.storage_mut().set_global("g1", Value::DInt(0));
        runtime.set_vm_tier1_specialized_executor_enabled(true);
        runtime.reset_vm_tier1_specialized_executor();
        runtime.vm_tier1_specialized_executor.hot_block_threshold = 1;

        let outcome = try_execute_pou_with_register_ir(&mut runtime, &module, pou_id, None)
            .expect("execute register program");
        assert_eq!(outcome, RegisterExecutionOutcome::Executed);
        assert_eq!(runtime.storage().get_global("g1"), Some(&Value::DInt(41)));

        let snapshot = runtime.vm_tier1_specialized_executor_snapshot();
        assert!(snapshot.compile_successes >= 1, "snapshot={snapshot:?}");
        assert!(snapshot.block_executions >= 1, "snapshot={snapshot:?}");
        assert_eq!(snapshot.compile_failures, 0, "snapshot={snapshot:?}");
    }

    #[test]
    fn tier1_compiler_accepts_load_super_dynamic_block() {
        let mut code = Vec::new();
        code.push(0x24);
        code.push(0x30);
        emit_u32(&mut code, 0);
        code.push(0x32);
        code.push(0x21);
        emit_u32(&mut code, 0);
        code.push(0x06);

        let pou_id = 1_u32;
        let mut pou_by_id = HashMap::new();
        pou_by_id.insert(
            pou_id,
            VmPouEntry {
                name: SmolStr::new("MAIN"),
                code_start: 0,
                code_end: code.len(),
                local_ref_start: 0,
                local_ref_count: 0,
                primary_instance_owner: None,
            },
        );
        let mut program_ids = HashMap::new();
        program_ids.insert(SmolStr::new("MAIN"), pou_id);
        let module = VmModule {
            code,
            strings: vec![SmolStr::new("COUNT")],
            types: TypeTable::default(),
            refs: vec![VmRef::Global {
                offset: 0,
                path: Vec::new(),
            }],
            consts: Vec::new(),
            pou_by_id,
            program_ids,
            function_ids: HashMap::new(),
            function_block_ids: HashMap::new(),
            class_ids: HashMap::new(),
            native_symbol_specs: Vec::new(),
            pou_params: HashMap::new(),
            pou_has_return_slot: HashSet::new(),
            method_table_by_owner: HashMap::new(),
            debug_map: super::super::debug_map::VmDebugMap::default(),
            instruction_budget: super::super::DEFAULT_INSTRUCTION_BUDGET,
        };

        let lowered = lower_pou_to_register_ir(&module, pou_id).expect("lower register ir");
        verify_register_program(&lowered).expect("verify register ir");
        let block = lowered
            .blocks
            .iter()
            .find(|block| {
                block
                    .instructions
                    .iter()
                    .any(|instruction| matches!(instruction, RegisterInstr::LoadSuper { .. }))
                    && block
                        .instructions
                        .iter()
                        .any(|instruction| matches!(instruction, RegisterInstr::LoadDynamic { .. }))
            })
            .expect("load-super block");
        let key = super::tier1_block_key(&module, pou_id, block);
        assert!(
            super::compile_tier1_block(&module, block, key).is_ok(),
            "expected tier-1 compiler to accept LoadSuper block: {:?}",
            block.instructions
        );
    }

    #[test]
    fn register_executor_tier1_specialized_executor_executes_load_super_block() {
        let mut code = Vec::new();
        code.push(0x24);
        code.push(0x30);
        emit_u32(&mut code, 0);
        code.push(0x32);
        code.push(0x21);
        emit_u32(&mut code, 0);
        code.push(0x06);

        let pou_id = 1_u32;
        let mut pou_by_id = HashMap::new();
        pou_by_id.insert(
            pou_id,
            VmPouEntry {
                name: SmolStr::new("MAIN"),
                code_start: 0,
                code_end: code.len(),
                local_ref_start: 0,
                local_ref_count: 0,
                primary_instance_owner: None,
            },
        );
        let mut program_ids = HashMap::new();
        program_ids.insert(SmolStr::new("MAIN"), pou_id);
        let module = VmModule {
            code,
            strings: vec![SmolStr::new("COUNT")],
            types: TypeTable::default(),
            refs: vec![VmRef::Global {
                offset: 0,
                path: Vec::new(),
            }],
            consts: Vec::new(),
            pou_by_id,
            program_ids,
            function_ids: HashMap::new(),
            function_block_ids: HashMap::new(),
            class_ids: HashMap::new(),
            native_symbol_specs: Vec::new(),
            pou_params: HashMap::new(),
            pou_has_return_slot: HashSet::new(),
            method_table_by_owner: HashMap::new(),
            debug_map: super::super::debug_map::VmDebugMap::default(),
            instruction_budget: super::super::DEFAULT_INSTRUCTION_BUDGET,
        };

        let mut runtime = Runtime::new();
        runtime.storage_mut().set_global("g0", Value::DInt(0));
        let base = runtime.storage_mut().create_instance("BASE");
        let derived = runtime.storage_mut().create_instance("DERIVED");
        runtime
            .storage_mut()
            .get_instance_mut(derived)
            .expect("derived instance")
            .parent = Some(base);
        assert!(runtime
            .storage_mut()
            .set_instance_var(base, "COUNT", Value::DInt(10)));
        runtime.set_vm_tier1_specialized_executor_enabled(true);
        runtime.reset_vm_tier1_specialized_executor();
        runtime.vm_tier1_specialized_executor.hot_block_threshold = 1;
        runtime.set_vm_register_profile_enabled(true);
        runtime.reset_vm_register_profile();

        let outcome =
            try_execute_pou_with_register_ir(&mut runtime, &module, pou_id, Some(derived))
                .expect("execute register program");
        assert_eq!(outcome, RegisterExecutionOutcome::Executed);
        assert_eq!(runtime.storage().get_global("g0"), Some(&Value::DInt(10)));

        let tier1 = runtime.vm_tier1_specialized_executor_snapshot();
        assert!(tier1.compile_successes >= 1, "snapshot={tier1:?}");
        assert!(tier1.block_executions >= 1, "snapshot={tier1:?}");
        assert_eq!(tier1.compile_failures, 0, "snapshot={tier1:?}");
        assert_eq!(tier1.deopt_count, 0, "snapshot={tier1:?}");

        let profile = runtime.vm_register_profile_snapshot();
        assert_eq!(profile.register_program_fallbacks, 0, "profile={profile:?}");
        assert_eq!(profile.ref_ops.load_dynamic, 1, "profile={profile:?}");
        assert_eq!(
            profile.ref_ops.instance_field_lookups, 1,
            "profile={profile:?}"
        );
    }

    #[test]
    fn register_executor_tier1_specialized_executor_executes_bool_or_without_deopt() {
        let mut code = Vec::new();
        code.push(0x20);
        emit_u32(&mut code, 0);
        code.push(0x10);
        emit_u32(&mut code, 0);
        code.push(0x47);
        code.push(0x21);
        emit_u32(&mut code, 0);
        code.push(0x06);
        let (module, pou_id) = manual_vm_module(code, vec![Value::Bool(true)], 1);

        let mut runtime = Runtime::new();
        runtime.storage_mut().set_global("g0", Value::Bool(false));
        runtime.set_vm_tier1_specialized_executor_enabled(true);
        runtime.reset_vm_tier1_specialized_executor();
        runtime.vm_tier1_specialized_executor.hot_block_threshold = 1;
        runtime.set_vm_register_profile_enabled(true);
        runtime.reset_vm_register_profile();

        let outcome = try_execute_pou_with_register_ir(&mut runtime, &module, pou_id, None)
            .expect("execute register program");
        assert_eq!(outcome, RegisterExecutionOutcome::Executed);
        assert_eq!(runtime.storage().get_global("g0"), Some(&Value::Bool(true)));

        let snapshot = runtime.vm_tier1_specialized_executor_snapshot();
        assert!(snapshot.compile_successes >= 1, "snapshot={snapshot:?}");
        assert!(snapshot.block_executions >= 1, "snapshot={snapshot:?}");
        assert_eq!(snapshot.deopt_count, 0, "snapshot={snapshot:?}");
        let profile = runtime.vm_register_profile_snapshot();
        assert_eq!(
            profile.value_ops.read_value_clones, 0,
            "profile={profile:?}"
        );
        assert_eq!(
            profile.value_ops.const_load_clones, 0,
            "profile={profile:?}"
        );
    }

    #[test]
    fn tier1_compiler_accepts_call_native_function_blocks() {
        let source = r#"
            VAR_GLOBAL
                g_value : DINT;
            END_VAR

            FUNCTION_BLOCK Counter
            VAR_OUTPUT
                Value : DINT;
            END_VAR

            Value := Value + DINT#1;
            END_FUNCTION_BLOCK

            PROGRAM Main
            VAR
                fb : Counter;
            END_VAR

            fb();
            g_value := fb.Value;
            END_PROGRAM
        "#;

        let bytecode = bytecode_module_from_source(source).expect("compile bytecode");
        let vm_module = VmModule::from_bytecode(&bytecode).expect("decode vm module");
        let main_pou_id = vm_module
            .program_ids
            .get(&SmolStr::new("MAIN"))
            .copied()
            .expect("main pou id");
        let lowered = lower_pou_to_register_ir(&vm_module, main_pou_id).expect("lower register ir");
        verify_register_program(&lowered).expect("verify register ir");
        let block = lowered
            .blocks
            .iter()
            .find(|block| {
                block
                    .instructions
                    .iter()
                    .any(|instruction| matches!(instruction, RegisterInstr::CallNative { .. }))
            })
            .expect("call-native block");
        let key = super::tier1_block_key(&vm_module, main_pou_id, block);
        assert!(
            super::compile_tier1_block(&vm_module, block, key).is_ok(),
            "expected tier-1 compiler to accept CallNative block: {:?}",
            block.instructions
        );
    }

    #[test]
    fn register_executor_tier1_specialized_executor_executes_function_call_block() {
        let source = r#"
            VAR_GLOBAL
                g_value : DINT;
            END_VAR

            FUNCTION AddOne : DINT
            VAR_INPUT
                Input : DINT;
            END_VAR

            AddOne := Input + DINT#1;
            END_FUNCTION

            PROGRAM Main
            g_value := AddOne(Input := DINT#41);
            END_PROGRAM
        "#;

        let mut harness = TestHarness::from_source(source).expect("create harness");
        harness
            .runtime_mut()
            .set_execution_backend(ExecutionBackend::BytecodeVm)
            .expect("set backend");
        harness
            .runtime_mut()
            .restart(RestartMode::Cold)
            .expect("restart");
        harness
            .runtime_mut()
            .vm_tier1_specialized_executor
            .set_enabled(true);
        harness
            .runtime_mut()
            .vm_tier1_specialized_executor
            .hot_block_threshold = 1;
        harness.runtime_mut().reset_vm_tier1_specialized_executor();
        harness
            .runtime_mut()
            .vm_tier1_specialized_executor
            .hot_block_threshold = 1;

        let result = harness.cycle();
        assert!(
            result.errors.is_empty(),
            "cycle errors: {:?}",
            result.errors
        );

        assert_eq!(harness.get_output("g_value"), Some(Value::DInt(42)));
        let snapshot = harness.runtime().vm_tier1_specialized_executor_snapshot();
        assert!(snapshot.compile_successes >= 1, "snapshot={snapshot:?}");
        assert!(snapshot.block_executions >= 1, "snapshot={snapshot:?}");
        assert_eq!(snapshot.compile_failures, 0, "snapshot={snapshot:?}");
    }

    #[test]
    fn register_executor_tier1_specialized_executor_executes_function_block_call_block() {
        let source = r#"
            VAR_GLOBAL
                g_value : DINT;
            END_VAR

            FUNCTION_BLOCK Counter
            VAR_OUTPUT
                Value : DINT;
            END_VAR

            Value := Value + DINT#1;
            END_FUNCTION_BLOCK

            PROGRAM Main
            VAR
                fb : Counter;
            END_VAR

            fb();
            g_value := fb.Value;
            END_PROGRAM
        "#;

        let mut harness = TestHarness::from_source(source).expect("create harness");
        harness
            .runtime_mut()
            .set_execution_backend(ExecutionBackend::BytecodeVm)
            .expect("set backend");
        harness
            .runtime_mut()
            .restart(RestartMode::Cold)
            .expect("restart");
        harness
            .runtime_mut()
            .vm_tier1_specialized_executor
            .set_enabled(true);
        harness
            .runtime_mut()
            .vm_tier1_specialized_executor
            .hot_block_threshold = 1;
        harness.runtime_mut().reset_vm_tier1_specialized_executor();
        harness
            .runtime_mut()
            .vm_tier1_specialized_executor
            .hot_block_threshold = 1;

        for cycle in 0..3 {
            let result = harness.cycle();
            assert!(
                result.errors.is_empty(),
                "cycle {} errors: {:?}",
                cycle + 1,
                result.errors
            );
        }

        assert_eq!(harness.get_output("g_value"), Some(Value::DInt(3)));
        let snapshot = harness.runtime().vm_tier1_specialized_executor_snapshot();
        assert!(snapshot.compile_successes >= 1, "snapshot={snapshot:?}");
        assert!(snapshot.block_executions >= 1, "snapshot={snapshot:?}");
        assert_eq!(snapshot.compile_failures, 0, "snapshot={snapshot:?}");
    }

    #[test]
    fn register_executor_tier1_specialized_executor_records_compile_failure_reason_for_unsupported_instruction(
    ) {
        let mut code = Vec::new();
        code.push(0x10);
        emit_u32(&mut code, 0);
        code.push(0x61);
        code.push(0x12);
        code.push(0x06);
        let (module, pou_id) = manual_vm_module(code, vec![Value::DInt(7)], 0);

        let mut runtime = Runtime::new();
        runtime.set_vm_tier1_specialized_executor_enabled(true);
        runtime.reset_vm_tier1_specialized_executor();
        runtime.vm_tier1_specialized_executor.hot_block_threshold = 1;

        let outcome = try_execute_pou_with_register_ir(&mut runtime, &module, pou_id, None)
            .expect("execute register program");
        assert_eq!(outcome, RegisterExecutionOutcome::Executed);

        let snapshot = runtime.vm_tier1_specialized_executor_snapshot();
        assert_eq!(snapshot.compile_attempts, 1);
        assert_eq!(snapshot.compile_failures, 1);
        assert!(
            snapshot.compile_failure_reasons.iter().any(|entry| {
                entry.reason == "unsupported_instr:size_of_value" && entry.count >= 1
            }),
            "expected SizeOfValue compile failure reason in tier-1 specialized executor snapshot, got {snapshot:?}",
        );
    }

    #[test]
    fn register_executor_tier1_specialized_executor_executes_non_dint_binary_without_deopt() {
        let mut code = Vec::new();
        code.push(0x20);
        emit_u32(&mut code, 0);
        code.push(0x10);
        emit_u32(&mut code, 0);
        code.push(0x40);
        code.push(0x21);
        emit_u32(&mut code, 0);
        code.push(0x06);
        let (module, pou_id) = manual_vm_module(code, vec![Value::Int(1)], 1);

        let mut runtime = Runtime::new();
        runtime.storage_mut().set_global("g0", Value::Int(0));
        runtime.set_vm_tier1_specialized_executor_enabled(true);
        runtime.reset_vm_tier1_specialized_executor();

        for _ in 0..80 {
            let outcome = try_execute_pou_with_register_ir(&mut runtime, &module, pou_id, None)
                .expect("execute register program");
            assert_eq!(outcome, RegisterExecutionOutcome::Executed);
        }

        assert_eq!(runtime.storage().get_global("g0"), Some(&Value::Int(80)));
        let snapshot = runtime.vm_tier1_specialized_executor_snapshot();
        assert!(snapshot.compile_attempts >= 1);
        assert!(snapshot.compile_successes >= 1);
        assert!(snapshot.block_executions >= 1);
        assert_eq!(snapshot.deopt_count, 0, "snapshot={snapshot:?}");
        assert!(snapshot.deopt_reasons.is_empty(), "snapshot={snapshot:?}");
    }

    #[test]
    fn register_executor_tier1_specialized_executor_cache_capacity_evicts_old_blocks() {
        let mut code_a = Vec::new();
        code_a.push(0x20);
        emit_u32(&mut code_a, 0);
        code_a.push(0x10);
        emit_u32(&mut code_a, 0);
        code_a.push(0x40);
        code_a.push(0x21);
        emit_u32(&mut code_a, 0);
        code_a.push(0x06);
        let (module_a, pou_a) = manual_vm_module(code_a, vec![Value::DInt(1)], 1);

        let mut code_b = Vec::new();
        code_b.push(0x20);
        emit_u32(&mut code_b, 0);
        code_b.push(0x10);
        emit_u32(&mut code_b, 0);
        code_b.push(0x41);
        code_b.push(0x21);
        emit_u32(&mut code_b, 0);
        code_b.push(0x06);
        let (module_b, pou_b) = manual_vm_module(code_b, vec![Value::DInt(1)], 1);

        let mut runtime = Runtime::new();
        runtime.vm_tier1_specialized_executor.set_enabled(true);
        runtime.vm_tier1_specialized_executor.hot_block_threshold = 1;
        runtime.vm_tier1_specialized_executor.cache_capacity = 1;
        runtime.reset_vm_tier1_specialized_executor();
        runtime.vm_tier1_specialized_executor.hot_block_threshold = 1;
        runtime.vm_tier1_specialized_executor.cache_capacity = 1;

        runtime.storage_mut().set_global("g0", Value::DInt(10));
        try_execute_pou_with_register_ir(&mut runtime, &module_a, pou_a, None)
            .expect("execute module a");
        runtime.storage_mut().set_global("g0", Value::DInt(10));
        try_execute_pou_with_register_ir(&mut runtime, &module_b, pou_b, None)
            .expect("execute module b");

        let snapshot = runtime.vm_tier1_specialized_executor_snapshot();
        assert_eq!(snapshot.cached_blocks, 1);
        assert!(
            snapshot.cache_evictions >= 1,
            "expected at least one cache eviction with cap=1",
        );
    }

    #[test]
    fn register_executor_tier1_specialized_executor_cache_hits_reuse_compiled_block_arc() {
        let key = super::Tier1BlockKey {
            module_ptr: 1,
            pou_id: 2,
            block_id: 3,
            start_pc: 4,
        };
        let compiled = std::sync::Arc::new(super::Tier1CompiledBlock {
            key,
            instructions: vec![super::Tier1CompiledInstr::Return],
        });
        let mut state = super::RegisterTier1SpecializedExecutorState::default();

        state.insert_compiled_block(std::sync::Arc::clone(&compiled));
        let fetched = state.compiled_block(&key).cloned().expect("compiled block");

        assert!(std::sync::Arc::ptr_eq(&compiled, &fetched));
    }

    #[test]
    fn register_deadline_stride_checks_first_and_stride_boundaries() {
        assert!(super::should_check_register_deadline(0));
        assert!(!super::should_check_register_deadline(1));
        assert!(super::should_check_register_deadline(
            super::REGISTER_DEADLINE_CHECK_STRIDE
        ));
        assert!(super::should_check_register_deadline(
            super::REGISTER_DEADLINE_CHECK_STRIDE * 2
        ));
    }

    // ── P2 register-executor corpus diagnostic tests ──

    #[test]
    fn diagnostic_find_fallback_opcodes_in_corpus() {
        let fixtures: &[(&str, &str)] = &[
            (
                "call-binding",
                r#"
                FUNCTION Add : INT
                VAR_INPUT a : INT; b : INT := INT#2; END_VAR
                Add := a + b;
                END_FUNCTION
                FUNCTION Bump : INT
                VAR_IN_OUT x : INT; END_VAR
                VAR_INPUT inc : INT := INT#1; END_VAR
                x := x + inc; Bump := x;
                END_FUNCTION
                PROGRAM Main
                VAR v : INT := INT#10; out_named : INT := INT#0;
                    out_default : INT := INT#0; out_inout : INT := INT#0; END_VAR
                out_named := Add(b := INT#4, a := INT#3);
                out_default := Add(a := INT#3);
                out_inout := Bump(v, INT#5);
                END_PROGRAM
            "#,
            ),
            (
                "string-stdlib",
                r#"
                PROGRAM Main
                VAR out_left : STRING := ''; out_mid : STRING := '';
                    out_find_found : INT := INT#0; out_find_missing : INT := INT#0;
                    out_w_replace : WSTRING := ""; out_w_insert : WSTRING := ""; END_VAR
                out_left := LEFT(IN := 'ABCDE', L := INT#3);
                out_mid := MID(IN := 'ABCDE', L := INT#2, P := INT#2);
                out_find_found := FIND(IN1 := 'ABCDE', IN2 := 'BC');
                out_find_missing := FIND(IN1 := 'BC', IN2 := 'ABCDE');
                out_w_replace := REPLACE(IN1 := "ABCDE", IN2 := "Z", L := INT#2, P := INT#3);
                out_w_insert := INSERT(IN1 := "ABE", IN2 := "CD", P := INT#3);
                END_PROGRAM
            "#,
            ),
            (
                "refs-sizeof",
                r#"
                TYPE
                    Inner : STRUCT arr : ARRAY[0..2] OF INT; END_STRUCT;
                    Outer : STRUCT inner : Inner; END_STRUCT;
                END_TYPE
                PROGRAM Main
                VAR o : Outer; idx : INT := INT#1; value_cell : INT := INT#4;
                    r_value : REF_TO INT; r_outer : REF_TO Outer;
                    out_ref : INT := INT#0; out_after_write : INT := INT#0;
                    out_nested_chain : INT := INT#0; out_size_type_int : DINT := DINT#0; END_VAR
                r_value := REF(value_cell);
                r_outer := REF(o);
                out_ref := r_value^;
                r_value^ := r_value^ + INT#3;
                out_after_write := r_value^;
                out_nested_chain := r_outer^.inner.arr[idx];
                out_size_type_int := SIZEOF(INT);
                END_PROGRAM
            "#,
            ),
        ];

        for (name, source) in fixtures {
            let (vm_module, pou_id) = vm_module_and_main_pou(source);
            let lowered = lower_pou_to_register_ir(&vm_module, pou_id);
            match lowered {
                Err(e) => {
                    panic!("fixture '{name}': lowering error: {e:?}");
                }
                Ok(program) => {
                    let fallbacks: Vec<_> = program
                        .blocks
                        .iter()
                        .flat_map(|b| b.instructions.iter())
                        .filter_map(|i| match i {
                            RegisterInstr::VmFallback { opcode, .. } => Some(*opcode),
                            _ => None,
                        })
                        .collect();
                    if !fallbacks.is_empty() {
                        let opcodes_hex: Vec<_> =
                            fallbacks.iter().map(|o| format!("0x{o:02X}")).collect();
                        panic!(
                            "fixture '{name}': has VmFallback instructions for opcodes: [{}]",
                            opcodes_hex.join(", ")
                        );
                    }
                    let has_complex = super::lowered_uses_complex_local_paths(&vm_module, &program);
                    if has_complex {
                        // Find which ref indices are complex
                        let mut complex_refs = Vec::new();
                        for instr in program.blocks.iter().flat_map(|b| b.instructions.iter()) {
                            let ref_idx = match instr {
                                RegisterInstr::LoadRef { ref_idx, .. }
                                | RegisterInstr::LoadRefAddr { ref_idx, .. }
                                | RegisterInstr::StoreRef { ref_idx, .. } => *ref_idx,
                                _ => continue,
                            };
                            if let Some(VmRef::Local { path, .. }) =
                                vm_module.refs.get(ref_idx as usize)
                            {
                                if !path.is_empty() {
                                    complex_refs.push(ref_idx);
                                }
                            }
                        }
                        panic!(
                            "fixture '{name}': blocked by complex_local_ref_path, ref indices: {complex_refs:?}"
                        );
                    }
                    eprintln!(
                        "fixture '{name}': PASS (no fallback instructions, no complex local refs)"
                    );
                }
            }
        }
    }

    #[test]
    fn diagnostic_execute_corpus_through_register_ir() {
        use crate::execution_backend::ExecutionBackend;
        use crate::harness::{bytecode_bytes_from_source, TestHarness};
        use crate::RestartMode;

        let fixtures: &[(&str, &str)] = &[
            (
                "call-binding",
                r#"
                FUNCTION Add : INT
                VAR_INPUT a : INT; b : INT := INT#2; END_VAR
                Add := a + b;
                END_FUNCTION
                FUNCTION Bump : INT
                VAR_IN_OUT x : INT; END_VAR
                VAR_INPUT inc : INT := INT#1; END_VAR
                x := x + inc; Bump := x;
                END_FUNCTION
                PROGRAM Main
                VAR v : INT := INT#10; out_named : INT := INT#0;
                    out_default : INT := INT#0; out_inout : INT := INT#0; END_VAR
                out_named := Add(b := INT#4, a := INT#3);
                out_default := Add(a := INT#3);
                out_inout := Bump(v, INT#5);
                END_PROGRAM
            "#,
            ),
            (
                "string-stdlib",
                r#"
                PROGRAM Main
                VAR out_left : STRING := ''; out_mid : STRING := '';
                    out_find_found : INT := INT#0; out_find_missing : INT := INT#0;
                    out_w_replace : WSTRING := ""; out_w_insert : WSTRING := ""; END_VAR
                out_left := LEFT(IN := 'ABCDE', L := INT#3);
                out_mid := MID(IN := 'ABCDE', L := INT#2, P := INT#2);
                out_find_found := FIND(IN1 := 'ABCDE', IN2 := 'BC');
                out_find_missing := FIND(IN1 := 'BC', IN2 := 'ABCDE');
                out_w_replace := REPLACE(IN1 := "ABCDE", IN2 := "Z", L := INT#2, P := INT#3);
                out_w_insert := INSERT(IN1 := "ABE", IN2 := "CD", P := INT#3);
                END_PROGRAM
            "#,
            ),
            (
                "refs-sizeof",
                r#"
                TYPE
                    Inner : STRUCT arr : ARRAY[0..2] OF INT; END_STRUCT;
                    Outer : STRUCT inner : Inner; END_STRUCT;
                END_TYPE
                PROGRAM Main
                VAR o : Outer; idx : INT := INT#1; value_cell : INT := INT#4;
                    r_value : REF_TO INT; r_outer : REF_TO Outer;
                    out_ref : INT := INT#0; out_after_write : INT := INT#0;
                    out_nested_chain : INT := INT#0; out_size_type_int : DINT := DINT#0; END_VAR
                r_value := REF(value_cell);
                r_outer := REF(o);
                out_ref := r_value^;
                r_value^ := r_value^ + INT#3;
                out_after_write := r_value^;
                out_nested_chain := r_outer^.inner.arr[idx];
                out_size_type_int := SIZEOF(INT);
                END_PROGRAM
            "#,
            ),
        ];

        for (name, source) in fixtures {
            let mut harness = TestHarness::from_source(source).expect("create harness");
            let bytes = bytecode_bytes_from_source(source).expect("compile bytecode");
            harness
                .runtime_mut()
                .apply_bytecode_bytes(&bytes, None)
                .expect("apply bytecode");
            harness
                .runtime_mut()
                .set_execution_backend(ExecutionBackend::BytecodeVm)
                .expect("set backend");
            harness
                .runtime_mut()
                .restart(RestartMode::Cold)
                .expect("restart");
            harness.runtime_mut().set_vm_register_profile_enabled(true);
            harness.runtime_mut().reset_vm_register_profile();

            let result = harness.cycle();
            if !result.errors.is_empty() {
                panic!("fixture '{name}': cycle errors: {:?}", result.errors);
            }

            let snapshot = harness.runtime().vm_register_profile_snapshot();
            eprintln!(
                "fixture '{name}': executed={}, fallbacks={}, reasons={:?}",
                snapshot.register_programs_executed,
                snapshot.register_program_fallbacks,
                snapshot.fallback_reasons,
            );
            assert!(
                snapshot.register_programs_executed > 0,
                "fixture '{name}': expected register execution, got 0 executed and {} fallbacks, reasons: {:?}",
                snapshot.register_program_fallbacks,
                snapshot.fallback_reasons,
            );
            assert_eq!(
                snapshot.register_program_fallbacks, 0,
                "fixture '{name}': expected zero register fallbacks, reasons: {:?}",
                snapshot.fallback_reasons
            );
        }
    }

    #[test]
    fn diagnostic_register_ir_callee_path_populates_lowering_cache() {
        use crate::execution_backend::ExecutionBackend;
        use crate::harness::{bytecode_bytes_from_source, TestHarness};
        use crate::RestartMode;

        let source = r#"
            FUNCTION Add : INT
            VAR_INPUT
                a : INT;
                b : INT := INT#2;
            END_VAR
            Add := a + b;
            END_FUNCTION

            FUNCTION Bump : INT
            VAR_IN_OUT
                x : INT;
            END_VAR
            VAR_INPUT
                inc : INT := INT#1;
            END_VAR
            x := x + inc;
            Bump := x;
            END_FUNCTION

            PROGRAM Main
            VAR
                v : INT := INT#10;
                out_named : INT := INT#0;
                out_default : INT := INT#0;
                out_inout : INT := INT#0;
            END_VAR

            out_named := Add(b := INT#4, a := INT#3);
            out_default := Add(a := INT#3);
            out_inout := Bump(v, INT#5);
            END_PROGRAM
        "#;

        let mut harness = TestHarness::from_source(source).expect("create harness");
        let bytes = bytecode_bytes_from_source(source).expect("compile bytecode");
        harness
            .runtime_mut()
            .apply_bytecode_bytes(&bytes, None)
            .expect("apply bytecode");
        harness
            .runtime_mut()
            .set_execution_backend(ExecutionBackend::BytecodeVm)
            .expect("set backend");
        harness
            .runtime_mut()
            .restart(RestartMode::Cold)
            .expect("restart");
        harness
            .runtime_mut()
            .set_vm_register_lowering_cache_enabled(true);
        harness.runtime_mut().reset_vm_register_lowering_cache();

        let first = harness.cycle();
        assert!(
            first.errors.is_empty(),
            "first cycle errors: {:?}",
            first.errors
        );
        let second = harness.cycle();
        assert!(
            second.errors.is_empty(),
            "second cycle errors: {:?}",
            second.errors
        );

        let cache = harness.runtime().vm_register_lowering_cache_snapshot();
        assert!(
            cache.cached_entries >= 2,
            "expected main + callee programs cached, got {} entries",
            cache.cached_entries
        );
        assert!(
            cache.hits > 0,
            "expected lowering-cache hits after second cycle, snapshot={cache:?}"
        );
    }
}
