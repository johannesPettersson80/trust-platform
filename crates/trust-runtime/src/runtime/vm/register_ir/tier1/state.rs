use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(in crate::runtime::vm::register_ir) struct Tier1BlockKey {
    pub(in crate::runtime::vm::register_ir) module_ptr: usize,
    pub(in crate::runtime::vm::register_ir) pou_id: u32,
    pub(in crate::runtime::vm::register_ir) block_id: u32,
    pub(in crate::runtime::vm::register_ir) start_pc: u32,
}

#[derive(Debug, Clone)]
pub(in crate::runtime::vm::register_ir) struct Tier1CompiledBlock {
    pub(in crate::runtime::vm::register_ir) key: Tier1BlockKey,
    pub(in crate::runtime::vm::register_ir) instructions: Vec<Tier1CompiledInstr>,
}

#[derive(Debug, Clone)]
pub(in crate::runtime::vm::register_ir) enum Tier1CompiledInstr {
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
    LoadSelfFieldDynamic {
        field: smol_str::SmolStr,
        dest: RegisterId,
    },
    StoreSelfFieldDynamic {
        field: smol_str::SmolStr,
        value: RegisterId,
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
    pub(in crate::runtime::vm::register_ir) hot_block_threshold: u64,
    pub(in crate::runtime::vm::register_ir) cache_capacity: usize,
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

    pub(in crate::runtime::vm::register_ir) fn enabled(&self) -> bool {
        self.enabled
    }

    pub(super) fn track_block_hit(&mut self, key: Tier1BlockKey) -> u64 {
        let entry = self.block_hits.entry(key).or_insert(0);
        *entry = entry.saturating_add(1);
        *entry
    }

    pub(super) fn can_attempt_compile(&self, hits: u64, key: &Tier1BlockKey) -> bool {
        hits >= self.hot_block_threshold && !self.compiled_blocks.contains_key(key)
    }

    pub(in crate::runtime::vm::register_ir) fn compiled_block(
        &self,
        key: &Tier1BlockKey,
    ) -> Option<&Arc<Tier1CompiledBlock>> {
        self.compiled_blocks.get(key)
    }

    pub(super) fn record_compile_attempt(&mut self) {
        self.compile_attempts = self.compile_attempts.saturating_add(1);
    }

    pub(super) fn record_compile_success(&mut self) {
        self.compile_successes = self.compile_successes.saturating_add(1);
    }

    pub(super) fn record_compile_failure(&mut self, reason: impl Into<String>) {
        self.compile_failures = self.compile_failures.saturating_add(1);
        let entry = self
            .compile_failure_reasons
            .entry(reason.into())
            .or_insert(0);
        *entry = entry.saturating_add(1);
    }

    pub(in crate::runtime::vm::register_ir) fn insert_compiled_block(
        &mut self,
        block: Arc<Tier1CompiledBlock>,
    ) {
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

    pub(super) fn record_block_execution(&mut self) {
        self.block_executions = self.block_executions.saturating_add(1);
    }
}

pub(in crate::runtime::vm::register_ir) fn parse_env_bool(name: &str, default: bool) -> bool {
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

pub(in crate::runtime::vm::register_ir) fn parse_env_usize(name: &str, default: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
        .unwrap_or(default)
}
