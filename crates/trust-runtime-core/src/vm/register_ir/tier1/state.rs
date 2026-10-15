use super::*;

#[derive(Debug, Clone)]
pub struct Tier1CompiledBlock {
    pub instructions: Vec<Tier1CompiledInstr>,
}

#[derive(Debug, Clone)]
pub enum Tier1CompiledInstr {
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

#[derive(Debug, Clone, Default)]
struct SelectionBlock {
    hits: u64,
    active: bool,
    previous: Option<usize>,
    next: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct RegisterTier1SpecializedExecutorState {
    enabled: bool,
    pub hot_block_threshold: u64,
    pub cache_capacity: usize,
    selection: Vec<SelectionBlock>,
    selection_head: Option<usize>,
    selection_tail: Option<usize>,
    selected_blocks: usize,
    compile_attempts: u64,
    compile_successes: u64,
    compile_failures: u64,
    compile_failure_reasons: BTreeMap<String, u64>,
    cache_evictions: u64,
    block_executions: u64,
}

impl Default for RegisterTier1SpecializedExecutorState {
    fn default() -> Self {
        Self {
            enabled: false,
            hot_block_threshold: 64,
            cache_capacity: 128,
            selection: Vec::new(),
            selection_head: None,
            selection_tail: None,
            selected_blocks: 0,
            compile_attempts: 0,
            compile_successes: 0,
            compile_failures: 0,
            compile_failure_reasons: BTreeMap::new(),
            cache_evictions: 0,
            block_executions: 0,
        }
    }
}

impl RegisterTier1SpecializedExecutorState {
    /// Install immutable specialized plans and observe compilation performed
    /// during preparation. No hot-block counter can trigger RUN compilation.
    pub fn prepare(&mut self, plans: &PreparedRegisterPlans) -> Result<(), RuntimeError> {
        self.invalidate_all();
        self.selection
            .try_reserve_exact(plans.specialized_blocks())
            .map_err(|_| RuntimeError::PreparationLimit)?;
        self.selection
            .resize(plans.specialized_blocks(), SelectionBlock::default());
        for (_, plan) in &plans.entries {
            if let RegisterLoweringCacheEntry::Ready(_) = &plan.lowered {
                for (index, block) in plan.tier1_blocks.iter().enumerate() {
                    self.record_compile_attempt();
                    if let Some(block) = block {
                        self.record_compile_success();
                        let _ = block;
                    } else {
                        let reason = plan
                            .tier1_failures
                            .get(index)
                            .and_then(Option::as_ref)
                            .cloned()
                            .unwrap_or_else(|| "invalid_prepared_block".into());
                        self.record_compile_failure(reason);
                    }
                }
            }
        }
        Ok(())
    }
    /// Select by a dense index assigned during preparation. Hits and fixed LRU
    /// links require constant work and never shift, search, allocate or compile.
    pub fn select_prepared_block(&mut self, index: usize) -> bool {
        let Some(block) = self.selection.get_mut(index) else {
            return false;
        };
        block.hits = block.hits.saturating_add(1);
        if self.cache_capacity == 0 || (!block.active && block.hits < self.hot_block_threshold) {
            return false;
        }
        if block.active {
            self.unlink_selected(index);
        } else {
            if self.selected_blocks >= self.cache_capacity {
                let Some(oldest) = self.selection_head else {
                    return false;
                };
                self.unlink_selected(oldest);
                self.selection[oldest].active = false;
                self.selected_blocks -= 1;
                self.cache_evictions = self.cache_evictions.saturating_add(1);
            }
            self.selection[index].active = true;
            self.selected_blocks += 1;
        }
        let previous = self.selection_tail;
        self.selection[index].previous = previous;
        self.selection[index].next = None;
        if let Some(previous) = previous {
            self.selection[previous].next = Some(index);
        } else {
            self.selection_head = Some(index);
        }
        self.selection_tail = Some(index);
        true
    }
    fn unlink_selected(&mut self, index: usize) {
        let previous = self.selection[index].previous;
        let next = self.selection[index].next;
        if let Some(previous) = previous {
            self.selection[previous].next = next;
        } else {
            self.selection_head = next;
        }
        if let Some(next) = next {
            self.selection[next].previous = previous;
        } else {
            self.selection_tail = previous;
        }
        self.selection[index].previous = None;
        self.selection[index].next = None;
    }
    pub fn from_environment(mut read: impl FnMut(&str) -> Option<String>) -> Self {
        use super::super::environment::{parse_bool, parse_number};

        let defaults = Self::default();
        Self {
            enabled: parse_bool(
                read("TRUST_VM_TIER1_SPECIALIZED_EXECUTOR").as_deref(),
                false,
            ),
            hot_block_threshold: parse_number(
                read("TRUST_VM_TIER1_SPECIALIZED_EXECUTOR_HOT_THRESHOLD").as_deref(),
                defaults.hot_block_threshold,
            ),
            cache_capacity: parse_number(
                read("TRUST_VM_TIER1_SPECIALIZED_EXECUTOR_CACHE_CAP").as_deref(),
                defaults.cache_capacity,
            )
            .max(1),
            ..defaults
        }
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn reset(&mut self) {
        // Reset explicit execution selection, preserving all reserved records and
        // immutable plans. Rewarming selects existing code; it never compiles.
        self.selection.fill(SelectionBlock::default());
        self.selection_head = None;
        self.selection_tail = None;
        self.selected_blocks = 0;
        self.compile_attempts = 0;
        self.compile_successes = 0;
        self.compile_failures = 0;
        self.compile_failure_reasons
            .values_mut()
            .for_each(|count| *count = 0);
        self.cache_evictions = 0;
        self.block_executions = 0;
    }

    pub fn invalidate_all(&mut self) {
        self.selection.clear();
        self.selection_head = None;
        self.selection_tail = None;
        self.selected_blocks = 0;
    }

    pub fn snapshot(&self) -> VmTier1SpecializedExecutorSnapshot {
        let compile_failure_reasons = self
            .compile_failure_reasons
            .iter()
            .filter(|(_, count)| **count != 0)
            .map(
                |(reason, count)| VmTier1SpecializedExecutorCompileFailureReason {
                    reason: reason.clone(),
                    count: *count,
                },
            )
            .collect::<Vec<_>>();
        VmTier1SpecializedExecutorSnapshot {
            enabled: self.enabled,
            hot_block_threshold: self.hot_block_threshold,
            cache_capacity: self.cache_capacity,
            cached_blocks: self.selected_blocks,
            compile_attempts: self.compile_attempts,
            compile_successes: self.compile_successes,
            compile_failures: self.compile_failures,
            compile_failure_reasons,
            cache_evictions: self.cache_evictions,
            block_executions: self.block_executions,
        }
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn record_compile_attempt(&mut self) {
        self.compile_attempts = self.compile_attempts.saturating_add(1);
    }

    pub fn record_compile_success(&mut self) {
        self.compile_successes = self.compile_successes.saturating_add(1);
    }

    pub fn record_compile_failure(&mut self, reason: impl Into<String>) {
        self.compile_failures = self.compile_failures.saturating_add(1);
        let entry = self
            .compile_failure_reasons
            .entry(reason.into())
            .or_insert(0);
        *entry = entry.saturating_add(1);
    }

    pub fn record_block_execution(&mut self) {
        self.block_executions = self.block_executions.saturating_add(1);
    }
}
