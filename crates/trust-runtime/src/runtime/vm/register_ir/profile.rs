use super::*;

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

    pub(in crate::runtime::vm) fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub(in crate::runtime::vm) fn record_executed(&mut self) {
        if !self.enabled {
            return;
        }
        self.register_programs_executed = self.register_programs_executed.saturating_add(1);
    }

    pub(in crate::runtime::vm) fn record_fallback(&mut self, reason: impl Into<String>) {
        if !self.enabled {
            return;
        }
        self.register_program_fallbacks = self.register_program_fallbacks.saturating_add(1);
        let reason = reason.into();
        let entry = self.fallback_reasons.entry(reason).or_insert(0);
        *entry = entry.saturating_add(1);
    }

    pub(in crate::runtime::vm) fn record_block_hit(
        &mut self,
        pou_id: u32,
        block_id: u32,
        start_pc: u32,
    ) {
        if !self.enabled {
            return;
        }
        let entry = self
            .block_hits
            .entry((pou_id, block_id, start_pc))
            .or_insert(0);
        *entry = entry.saturating_add(1);
    }

    pub(in crate::runtime::vm) fn record_ref_op(&mut self, kind: RegisterRefOpKind) {
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

    pub(in crate::runtime::vm) fn record_call_op(&mut self, kind: RegisterCallOpKind) {
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

    pub(in crate::runtime::vm) fn record_value_op(&mut self, kind: RegisterValueOpKind) {
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
pub(in crate::runtime::vm) enum RegisterRefOpKind {
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
pub(in crate::runtime::vm) enum RegisterCallOpKind {
    FramePush,
    FramePop,
    FunctionBlockCallEntry,
    ParameterBinding,
    OutputCopyBack,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::runtime::vm) enum RegisterValueOpKind {
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
pub(super) struct CachedRegisterProgram {
    pub(super) program: Arc<RegisterProgram>,
    pub(super) register_read_counts_by_block: Arc<Vec<Vec<u32>>>,
    pub(super) block_has_register_reads: Arc<Vec<bool>>,
    pub(super) fallback_opcode: Option<u8>,
}

#[derive(Debug, Clone)]
pub(super) enum RegisterLoweringCacheEntry {
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

    pub(super) fn get_or_build(
        &mut self,
        module: &VmModule,
        pou_id: u32,
    ) -> Arc<RegisterLoweringCacheEntry> {
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
