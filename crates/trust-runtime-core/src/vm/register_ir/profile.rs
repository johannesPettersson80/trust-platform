use super::*;

#[derive(Debug, Clone, Default)]
pub struct RegisterProfileState {
    enabled: bool,
    register_programs_executed: u64,
    register_program_fallbacks: u64,
    fallback_reasons: BTreeMap<String, u64>,
    block_hits: BTreeMap<(u32, u32, u32), u64>,
    interpreted_block_entries: u64,
    ref_ops: VmRegisterRefOpCounters,
    call_ops: VmRegisterCallOpCounters,
    value_ops: VmRegisterValueOpCounters,
}

impl RegisterProfileState {
    /// Preallocate telemetry keys before enabling a bounded controller.
    pub fn prepare(&mut self, plans: &PreparedRegisterPlans) {
        self.fallback_reasons
            .entry("debug_mode".into())
            .or_insert(0);
        self.fallback_reasons
            .entry("unprepared_program".into())
            .or_insert(0);
        self.fallback_reasons
            .entry("nested_scratch".into())
            .or_insert(0);
        for (_, plan) in &plans.entries {
            if let Some(reason) = plan.fallback_reason() {
                self.fallback_reasons.entry(reason.into()).or_insert(0);
            }
            if let Some(program) = plan.program() {
                for block in &program.blocks {
                    self.block_hits
                        .entry((
                            program.pou_id,
                            block.id,
                            block.start_pc.try_into().unwrap_or(u32::MAX),
                        ))
                        .or_insert(0);
                }
            }
        }
    }
    pub fn record_prepared_fallback(&mut self, reason: &str) {
        if !self.enabled {
            return;
        }
        self.register_program_fallbacks = self.register_program_fallbacks.saturating_add(1);
        if let Some(count) = self.fallback_reasons.get_mut(reason) {
            *count = count.saturating_add(1);
        }
    }
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn reset(&mut self) {
        self.register_programs_executed = 0;
        self.register_program_fallbacks = 0;
        self.fallback_reasons
            .values_mut()
            .for_each(|count| *count = 0);
        self.block_hits.values_mut().for_each(|count| *count = 0);
        self.interpreted_block_entries = 0;
        self.ref_ops = VmRegisterRefOpCounters::default();
        self.call_ops = VmRegisterCallOpCounters::default();
        self.value_ops = VmRegisterValueOpCounters::default();
    }

    pub fn snapshot(&self) -> VmRegisterProfileSnapshot {
        let fallback_reasons = self
            .fallback_reasons
            .iter()
            .filter(|(_, count)| **count != 0)
            .map(|(reason, count)| VmRegisterFallbackReason {
                reason: reason.clone(),
                count: *count,
            })
            .collect();
        let hot_blocks = self
            .block_hits
            .iter()
            .filter(|(_, count)| **count != 0)
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
            interpreted_block_entries: self.interpreted_block_entries,
            ref_ops: self.ref_ops.clone(),
            call_ops: self.call_ops.clone(),
            value_ops: self.value_ops.clone(),
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn record_executed(&mut self) {
        if !self.enabled {
            return;
        }
        self.register_programs_executed = self.register_programs_executed.saturating_add(1);
    }

    pub fn record_block_hit(&mut self, pou_id: u32, block_id: u32, start_pc: u32) {
        if !self.enabled {
            return;
        }
        let entry = self
            .block_hits
            .entry((pou_id, block_id, start_pc))
            .or_insert(0);
        *entry = entry.saturating_add(1);
    }

    /// Count a block entry that the interpreter runs; Tier-1 entries are counted separately.
    pub fn record_interpreted_block_entry(&mut self) {
        if !self.enabled {
            return;
        }
        self.interpreted_block_entries = self.interpreted_block_entries.saturating_add(1);
    }

    pub fn record_ref_op(&mut self, kind: RegisterRefOpKind) {
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

    pub fn record_call_op(&mut self, kind: RegisterCallOpKind) {
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

    pub fn record_value_op(&mut self, kind: RegisterValueOpKind) {
        if !self.enabled {
            return;
        }
        let counter = match kind {
            RegisterValueOpKind::LoadConstant => &mut self.value_ops.const_load_clones,
            RegisterValueOpKind::RegisterReadClone => &mut self.value_ops.register_read_clones,
            RegisterValueOpKind::RegisterReadMove => &mut self.value_ops.register_read_moves,
            RegisterValueOpKind::ReadReference => &mut self.value_ops.read_value_clones,
            RegisterValueOpKind::BindExpression => &mut self.value_ops.binding_expr_clones,
            RegisterValueOpKind::CopyOutput => &mut self.value_ops.output_value_clones,
        };
        *counter = counter.saturating_add(1);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegisterRefOpKind {
    LoadRef,
    StoreRef,
    LoadRefAddr,
    RefField,
    RefIndex,
    LoadDynamic,
    StoreDynamic,
    InstanceFieldLookup,
}

pub use crate::vm::call::context::{RegisterCallOpKind, RegisterValueOpKind};

#[derive(Debug, Clone)]
pub struct CachedRegisterProgram {
    pub program: Arc<RegisterProgram>,
    pub register_read_counts_by_block: Arc<Vec<Vec<(u32, u32)>>>,
    pub block_has_register_reads: Arc<Vec<bool>>,
    pub fallback_opcode: Option<u8>,
}

#[derive(Debug, Clone)]
pub enum RegisterLoweringCacheEntry {
    Ready(CachedRegisterProgram),
    LoweringError { message: String },
}

/// Compatibility telemetry for eagerly prepared lowering. It never owns or
/// rebuilds executable plans and cannot turn a RUN cache miss into preparation.
#[derive(Debug, Clone)]
pub struct RegisterLoweringCacheState {
    enabled: bool,
    cache_capacity: usize,
    prepared_entries: usize,
    hits: u64,
    misses: u64,
    build_errors: u64,
    invalidations: u64,
}
impl Default for RegisterLoweringCacheState {
    fn default() -> Self {
        Self {
            enabled: true,
            cache_capacity: 256,
            prepared_entries: 0,
            hits: 0,
            misses: 0,
            build_errors: 0,
            invalidations: 0,
        }
    }
}
impl RegisterLoweringCacheState {
    pub(crate) fn with_configuration(enabled: bool, capacity: usize) -> Self {
        Self {
            enabled,
            cache_capacity: capacity.max(1),
            ..Self::default()
        }
    }

    pub fn from_environment(mut read: impl FnMut(&str) -> Option<String>) -> Self {
        let mut result = Self::default();
        result.enabled = environment::parse_bool(
            read("TRUST_VM_REGISTER_LOWERING_CACHE").as_deref(),
            result.enabled,
        );
        result.cache_capacity = environment::parse_number(
            read("TRUST_VM_REGISTER_LOWERING_CACHE_CAP").as_deref(),
            result.cache_capacity,
        )
        .max(1);
        result
    }
    /// Observe installation, with all preparation completed outside execution.
    pub fn prepare(&mut self, plans: &PreparedRegisterPlans) {
        self.invalidate_all();
        self.prepared_entries = plans.len();
        self.misses = self.misses.saturating_add(plans.len() as u64);
        self.build_errors = self.build_errors.saturating_add(
            plans
                .entries
                .iter()
                .filter(|(_, plan)| {
                    matches!(
                        plan.lowered,
                        RegisterLoweringCacheEntry::LoweringError { .. }
                    )
                })
                .count() as u64,
        );
    }
    pub fn record_lookup(&mut self, found: bool) {
        if found {
            self.hits = self.hits.saturating_add(1);
        }
    }
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }
    pub fn reset(&mut self) {
        self.hits = 0;
        self.misses = 0;
        self.build_errors = 0;
        self.invalidations = 0;
    }
    pub fn invalidate_all(&mut self) {
        self.invalidations = self
            .invalidations
            .saturating_add(self.prepared_entries as u64);
        self.prepared_entries = 0;
    }
    pub fn snapshot(&self) -> VmRegisterLoweringCacheSnapshot {
        VmRegisterLoweringCacheSnapshot {
            enabled: self.enabled,
            cache_capacity: self.cache_capacity,
            cached_entries: self.prepared_entries,
            hits: self.hits,
            misses: self.misses,
            build_errors: self.build_errors,
            cache_evictions: 0,
            invalidations: self.invalidations,
        }
    }
}
