use super::*;
use crate::value::{ArrayValue, EnumValue};

/// Immutable optimization plans owned by one prepared module.
#[derive(Debug, Clone, Default)]
pub struct PreparedRegisterPlans {
    pub(super) entries: Vec<(u32, Arc<PreparedRegisterProgram>)>,
    max_registers: usize,
    max_native_arguments: usize,
    summary: VmRegisterPreparationSnapshot,
    /// The calls of built-in text functions the module's native symbols name, by string index
    /// (C1-20): a register call finds its text function here, not by name at each call.
    text_calls: Vec<(u32, crate::vm::dispatch_text::TextCall)>,
}

/// Eagerly lowered POU plus all optional specialized blocks.
#[derive(Debug, Clone)]
pub struct PreparedRegisterProgram {
    pub(super) lowered: RegisterLoweringCacheEntry,
    pub(super) tier1_blocks: Vec<Option<Arc<Tier1CompiledBlock>>>,
    pub(super) tier1_selection_indices: Vec<Option<usize>>,
    pub(super) tier1_failures: Vec<Option<String>>,
    fallback_reason: Option<String>,
    /// The program makes no user call, so its activation cannot suspend.
    leaf: bool,
    /// The most bytes the body holds in the temporary area at once: the room of the register
    /// file's texts and copies (C1-20).
    temporaries: usize,
}

impl PreparedRegisterProgram {
    /// Stable fallback reason prepared before entering RUN.
    pub fn fallback_reason(&self) -> Option<&str> {
        self.fallback_reason.as_deref()
    }
    /// Source-free immutable register body, if its lowering succeeded.
    pub fn program(&self) -> Option<&RegisterProgram> {
        match &self.lowered {
            RegisterLoweringCacheEntry::Ready(entry) => Some(&entry.program),
            RegisterLoweringCacheEntry::LoweringError { .. } => None,
        }
    }
}

impl PreparedRegisterPlans {
    /// Prepare the plans of `module`, as [`Self::prepare`] does, and keep the text call of each of
    /// its native symbols that names a built-in text function of `stdlib` (C1-20), all charged as
    /// preparation.
    pub(crate) fn prepare_for(
        module: &VmModule,
        stdlib: &crate::stdlib::StandardLibrary,
        mut charge: impl FnMut(usize, usize) -> Result<(), RuntimeError>,
    ) -> Result<Self, RuntimeError> {
        let mut plans = Self::prepare(module, &mut charge)?;
        plans.keep_text_calls(module, stdlib, &mut charge)?;
        Ok(plans)
    }

    /// Find the text call of each native symbol of `module` that names a built-in text function of
    /// `stdlib`, charged as preparation (C1-20).
    fn keep_text_calls(
        &mut self,
        module: &VmModule,
        stdlib: &crate::stdlib::StandardLibrary,
        mut charge: impl FnMut(usize, usize) -> Result<(), RuntimeError>,
    ) -> Result<(), RuntimeError> {
        let calls = || {
            module
                .native_symbol_specs
                .indexed()
                .filter_map(|(index, spec)| {
                    let crate::vm::module::VmNativeSymbolSpec::Parsed { arg_specs, .. } = spec
                    else {
                        return None;
                    };
                    crate::vm::dispatch_text::text_call_of(stdlib, spec, arg_specs.len())
                        .map(|call| (index, call))
                })
        };
        let count = calls().count();
        charge(
            count
                .checked_mul(mem::size_of::<(u32, crate::vm::dispatch_text::TextCall)>())
                .ok_or(RuntimeError::Overflow)?,
            module.native_symbol_specs.indexed().count(),
        )?;
        self.text_calls
            .try_reserve_exact(count)
            .map_err(|_| RuntimeError::PreparationLimit)?;
        self.text_calls.extend(calls());
        Ok(())
    }

    /// The text call of a standard-function call of the native symbol `symbol_idx` with `count`
    /// arguments, when [`Self::keep_text_calls`] found one.
    pub(crate) fn text_call(
        &self,
        symbol_idx: u32,
        count: usize,
    ) -> Option<crate::vm::dispatch_text::TextCall> {
        let at = self
            .text_calls
            .binary_search_by_key(&symbol_idx, |(index, _)| *index)
            .ok()?;
        let call = self.text_calls[at].1;
        (call.count() == count).then_some(call)
    }

    /// Keep `peak`, the most bytes the body of `pou` holds in the temporary area at once, in its
    /// plan. Preparation sets it before the plans are shared.
    pub(crate) fn set_temporaries(&mut self, pou: u32, peak: usize) {
        if let Some((_, plan)) = self.entries.iter_mut().find(|(id, _)| *id == pou)
            && let Some(plan) = Arc::get_mut(plan)
        {
            plan.temporaries = peak;
        }
    }

    /// Prepare every decoded POU before controller installation. Unsupported
    /// bodies retain a prepared stack fallback; no execution path compiles them.
    pub fn prepare(
        module: &VmModule,
        mut charge: impl FnMut(usize, usize) -> Result<(), RuntimeError>,
    ) -> Result<Self, RuntimeError> {
        let count = module.pou_by_id().len();
        charge(
            count
                .checked_mul(
                    mem::size_of::<u32>()
                        + mem::size_of::<(u32, Arc<PreparedRegisterProgram>)>()
                        + mem::size_of::<PreparedRegisterProgram>()
                        + 128,
                )
                .ok_or(RuntimeError::Overflow)?,
            count,
        )?;
        let mut ids = Vec::new();
        ids.try_reserve_exact(count)
            .map_err(|_| RuntimeError::PreparationLimit)?;
        ids.extend(module.pou_by_id().keys().copied());
        ids.sort_unstable();
        let mut result = Self {
            summary: VmRegisterPreparationSnapshot {
                pou_count: count,
                ..Default::default()
            },
            ..Self::default()
        };
        let mut failure_reasons = BTreeMap::<String, u64>::new();
        result
            .entries
            .try_reserve_exact(count)
            .map_err(|_| RuntimeError::PreparationLimit)?;
        for id in ids {
            preflight_pou(module, id, &mut charge)?;
            let lowered = match build_cached_register_program(module, id) {
                Ok(program) => {
                    result.summary.lowered_pous += 1;
                    RegisterLoweringCacheEntry::Ready(program)
                }
                Err(error) => {
                    result.summary.lowering_failures += 1;
                    RegisterLoweringCacheEntry::LoweringError {
                        message: error.to_string(),
                    }
                }
            };
            let mut tier1_blocks = Vec::new();
            let mut tier1_selection_indices = Vec::new();
            let mut tier1_failures = Vec::new();
            let mut leaf = true;
            let fallback_reason = match &lowered {
                RegisterLoweringCacheEntry::LoweringError { message } => Some(format!(
                    "lowering_error:{}: {message}",
                    module.pou_name(id).unwrap_or("<unknown>")
                )),
                RegisterLoweringCacheEntry::Ready(cached) => {
                    result.max_registers = result
                        .max_registers
                        .max(cached.program.max_registers as usize);
                    for block in &cached.program.blocks {
                        for instruction in &block.instructions {
                            if let RegisterInstr::CallNative { kind, args, .. } = instruction {
                                result.max_native_arguments =
                                    result.max_native_arguments.max(args.len());
                                leaf &= !crate::vm::call::native_call_may_yield(*kind);
                            }
                        }
                        match compile_tier1_block(module, block) {
                            Ok(compiled) => {
                                tier1_selection_indices
                                    .push(Some(result.summary.specialized_blocks));
                                result.summary.specialized_blocks = result
                                    .summary
                                    .specialized_blocks
                                    .checked_add(1)
                                    .ok_or(RuntimeError::Overflow)?;
                                tier1_blocks.push(Some(Arc::new(compiled)));
                                tier1_failures.push(None);
                            }
                            Err(reason) => {
                                result.summary.specialization_failures = result
                                    .summary
                                    .specialization_failures
                                    .checked_add(1)
                                    .ok_or(RuntimeError::Overflow)?;
                                let comparisons = 24usize
                                    .checked_mul(failure_reasons.len().max(1).ilog2() as usize + 1)
                                    .and_then(|units| units.checked_mul(reason.len().max(1)))
                                    .ok_or(RuntimeError::Overflow)?;
                                charge(0, comparisons)?;
                                if let Some(count) = failure_reasons.get_mut(&reason) {
                                    *count = count.checked_add(1).ok_or(RuntimeError::Overflow)?;
                                } else {
                                    let bytes = reason
                                        .len()
                                        .checked_add(
                                            mem::size_of::<(String, u64)>()
                                                + 4 * mem::size_of::<usize>(),
                                        )
                                        .ok_or(RuntimeError::Overflow)?;
                                    charge(bytes, reason.len())?;
                                    failure_reasons.insert(reason.clone(), 1);
                                }
                                tier1_blocks.push(None);
                                tier1_selection_indices.push(None);
                                tier1_failures.push(Some(reason));
                            }
                        }
                    }
                    cached
                        .fallback_opcode
                        .map(|opcode| format!("unsupported_opcode_0x{opcode:02X}"))
                }
            };
            result.entries.push((
                id,
                Arc::new(PreparedRegisterProgram {
                    lowered,
                    tier1_blocks,
                    tier1_selection_indices,
                    tier1_failures,
                    fallback_reason,
                    leaf,
                    // Preparation sets it from the walk of the body (C1-20).
                    temporaries: 0,
                }),
            ));
        }
        charge(
            failure_reasons
                .len()
                .checked_mul(mem::size_of::<VmTier1SpecializedExecutorCompileFailureReason>())
                .ok_or(RuntimeError::Overflow)?,
            failure_reasons.len(),
        )?;
        result
            .summary
            .specialization_failure_reasons
            .try_reserve_exact(failure_reasons.len())
            .map_err(|_| RuntimeError::PreparationLimit)?;
        result
            .summary
            .specialization_failure_reasons
            .extend(failure_reasons.into_iter().map(|(reason, count)| {
                VmTier1SpecializedExecutorCompileFailureReason { reason, count }
            }));
        Ok(result)
    }
    /// Preparation counts belong to the immutable module, never resettable RUN telemetry.
    pub(crate) fn preparation_snapshot(&self) -> VmRegisterPreparationSnapshot {
        self.summary.clone()
    }
    /// Lookup by artifact POU identity without mutable cache state.
    pub fn get(&self, pou_id: u32) -> Option<Arc<PreparedRegisterProgram>> {
        self.entries
            .binary_search_by_key(&pou_id, |(id, _)| *id)
            .ok()
            .map(|index| Arc::clone(&self.entries[index].1))
    }
    /// Whether `pou_id` has a prepared, fallback-free plan that makes no user call.
    pub(crate) fn is_leaf(&self, pou_id: u32) -> bool {
        self.entries
            .binary_search_by_key(&pou_id, |(id, _)| *id)
            .ok()
            .is_some_and(|index| {
                let plan = &self.entries[index].1;
                plan.leaf && plan.fallback_reason.is_none()
            })
    }
    /// Maximum register scratch needed by any prepared body.
    #[cfg(any(test, feature = "test-support"))]
    pub fn max_registers(&self) -> usize {
        self.max_registers
    }
    /// Maximum native argument scratch needed by any prepared body.
    #[cfg(any(test, feature = "test-support"))]
    pub fn max_native_arguments(&self) -> usize {
        self.max_native_arguments
    }

    /// Reserve independent scratch for each eligible POU. Calls into another
    /// POU retain register execution rather than forcing a stack
    /// fallback; recursive re-entry into an occupied body still can.
    pub fn reserve_execution_buffers(
        &self,
        module: &VmModule,
        mut charge: impl FnMut(usize, usize) -> Result<(), RuntimeError>,
    ) -> Result<Vec<(u32, Option<RegisterExecutionBuffers>)>, RuntimeError> {
        charge(
            0,
            self.entries
                .len()
                .checked_mul(2)
                .ok_or(RuntimeError::Overflow)?,
        )?;
        let count = self
            .entries
            .iter()
            .filter(|(_, plan)| plan.fallback_reason().is_none())
            .count();
        charge(
            count
                .checked_mul(mem::size_of::<(u32, Option<RegisterExecutionBuffers>)>())
                .ok_or(RuntimeError::Overflow)?,
            count,
        )?;
        let mut buffers = Vec::new();
        buffers
            .try_reserve_exact(count)
            .map_err(|_| RuntimeError::PreparationLimit)?;
        for (id, plan) in &self.entries {
            if plan.fallback_reason().is_some() {
                continue;
            }
            let program = plan.program().ok_or(RuntimeError::InvalidExecutionState)?;
            let pou = module.pou(*id).ok_or(RuntimeError::InvalidExecutionState)?;
            let local_count =
                super::super::materialization_limits::checked_local_count(pou.local_ref_count)
                    .map_err(VmTrap::into_runtime_error)?;
            let mut arguments = 0;
            for block in &program.blocks {
                charge(
                    0,
                    block
                        .instructions
                        .len()
                        .checked_add(1)
                        .ok_or(RuntimeError::Overflow)?,
                )?;
                for instruction in &block.instructions {
                    arguments = arguments.max(operands_above_registers(instruction));
                }
            }
            let (bytes, work) = RegisterExecutionBuffers::reservation_demand_with_temporaries(
                program.max_registers as usize,
                arguments,
                local_count,
                plan.temporaries,
            )?;
            charge(bytes, work)?;
            buffers.push((
                *id,
                Some(RegisterExecutionBuffers::reserve_with_temporaries(
                    program.max_registers as usize,
                    arguments,
                    local_count,
                    plan.temporaries,
                )?),
            ));
        }
        Ok(buffers)
    }
    /// Preparation demand for all optional profile maps and compatibility
    /// observations, charged before installing the engine's telemetry keys.
    pub fn telemetry_demand(&self) -> Result<(usize, usize), RuntimeError> {
        let mut bytes = 3usize
            .checked_mul(
                mem::size_of::<String>() + mem::size_of::<u64>() + 4 * mem::size_of::<usize>(),
            )
            .and_then(|bytes| bytes.checked_add(64))
            .ok_or(RuntimeError::Overflow)?;
        let mut work = 3usize;
        for (_, plan) in &self.entries {
            if let Some(reason) = plan.fallback_reason() {
                bytes = bytes
                    .checked_add(
                        reason.len()
                            + mem::size_of::<String>()
                            + mem::size_of::<u64>()
                            + 4 * mem::size_of::<usize>(),
                    )
                    .ok_or(RuntimeError::Overflow)?;
                work = work.checked_add(1).ok_or(RuntimeError::Overflow)?;
            }
            if let Some(program) = plan.program() {
                let width = mem::size_of::<(u32, u32, u32)>()
                    + mem::size_of::<u64>()
                    + 4 * mem::size_of::<usize>()
                    + mem::size_of::<Arc<Tier1CompiledBlock>>()
                    + 6 * mem::size_of::<usize>()
                    + mem::size_of::<String>()
                    + 64;
                bytes = bytes
                    .checked_add(
                        program
                            .blocks
                            .len()
                            .checked_mul(width)
                            .ok_or(RuntimeError::Overflow)?,
                    )
                    .ok_or(RuntimeError::Overflow)?;
                work = work
                    .checked_add(
                        program
                            .blocks
                            .len()
                            .checked_mul(3)
                            .ok_or(RuntimeError::Overflow)?,
                    )
                    .ok_or(RuntimeError::Overflow)?;
            }
        }
        Ok((bytes, work))
    }
    pub fn specialized_blocks(&self) -> usize {
        self.summary.specialized_blocks
    }
    /// Number of eagerly lowered bodies, including explicit stack fallbacks.
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    #[cfg(feature = "test-support")]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    /// Bytes held by plan records, instruction/operand buffers and read counts.
    #[cfg(feature = "test-support")]
    pub fn storage_bytes(&self) -> usize {
        let summary_bytes = self.summary.specialization_failure_reasons.iter().fold(
            self.summary
                .specialization_failure_reasons
                .capacity()
                .saturating_mul(mem::size_of::<VmTier1SpecializedExecutorCompileFailureReason>()),
            |bytes, reason| bytes.saturating_add(reason.reason.capacity()),
        );
        self.entries.iter().fold(
            self.entries
                .capacity()
                .saturating_mul(mem::size_of::<(u32, Arc<PreparedRegisterProgram>)>())
                .saturating_add(summary_bytes),
            |total, (_, plan)| {
                let mut bytes = mem::size_of::<PreparedRegisterProgram>()
                    .saturating_add(
                        plan.tier1_blocks
                            .capacity()
                            .saturating_mul(mem::size_of::<Option<Arc<Tier1CompiledBlock>>>()),
                    )
                    .saturating_add(
                        plan.tier1_selection_indices
                            .capacity()
                            .saturating_mul(mem::size_of::<Option<usize>>()),
                    )
                    .saturating_add(
                        plan.tier1_failures
                            .capacity()
                            .saturating_mul(mem::size_of::<Option<String>>()),
                    );
                for reason in plan.tier1_failures.iter().flatten() {
                    bytes = bytes.saturating_add(reason.capacity());
                }
                if let Some(reason) = &plan.fallback_reason {
                    bytes = bytes.saturating_add(reason.capacity());
                }
                if let RegisterLoweringCacheEntry::Ready(entry) = &plan.lowered {
                    bytes = bytes
                        .saturating_add(mem::size_of::<RegisterProgram>())
                        .saturating_add(
                            entry
                                .program
                                .blocks
                                .capacity()
                                .saturating_mul(mem::size_of::<RegisterBlock>()),
                        )
                        .saturating_add(
                            entry
                                .register_read_counts_by_block
                                .capacity()
                                .saturating_mul(mem::size_of::<Vec<(u32, u32)>>()),
                        )
                        .saturating_add(entry.block_has_register_reads.capacity());
                    for block in &entry.program.blocks {
                        bytes = bytes
                            .saturating_add(
                                block
                                    .instructions
                                    .capacity()
                                    .saturating_mul(mem::size_of::<RegisterInstr>()),
                            )
                            .saturating_add(
                                block
                                    .instruction_costs
                                    .capacity()
                                    .saturating_mul(mem::size_of::<usize>()),
                            )
                            .saturating_add(
                                block
                                    .instruction_cost_steps
                                    .capacity()
                                    .saturating_mul(mem::size_of::<Vec<usize>>()),
                            );
                        for steps in &block.instruction_cost_steps {
                            bytes = bytes.saturating_add(
                                steps.capacity().saturating_mul(mem::size_of::<usize>()),
                            );
                        }
                        for instruction in &block.instructions {
                            match instruction {
                                RegisterInstr::CallNative { args, .. } => {
                                    bytes = bytes.saturating_add(
                                        args.capacity()
                                            .saturating_mul(mem::size_of::<RegisterId>()),
                                    )
                                }
                                RegisterInstr::VmFallback { operands, .. } => {
                                    bytes = bytes.saturating_add(operands.capacity())
                                }
                                _ => {}
                            }
                        }
                    }
                    for counts in entry.register_read_counts_by_block.iter() {
                        bytes = bytes.saturating_add(
                            counts
                                .capacity()
                                .saturating_mul(mem::size_of::<(u32, u32)>()),
                        );
                    }
                    for block in plan.tier1_blocks.iter().flatten() {
                        bytes = bytes
                            .saturating_add(mem::size_of::<Tier1CompiledBlock>())
                            .saturating_add(
                                block
                                    .instructions
                                    .capacity()
                                    .saturating_mul(mem::size_of::<Tier1CompiledInstr>()),
                            );
                    }
                }
                total.saturating_add(bytes)
            },
        )
    }
}

/// Per-controller scratch. It is explicitly reserved while the controller is
/// constructed and recycled by its owner; it has no global or thread-local pool.
#[derive(Debug, Default)]
pub struct RegisterExecutionBuffers {
    pub(super) frames: FrameStack,
    /// The register file and, above it, the operands of a call the activation makes, whose texts
    /// and copies lie in its temporary area (C1-20).
    registers: OperandStack,
    remaining_register_reads: Vec<u32>,
    needs_clear: bool,
}

impl RegisterExecutionBuffers {
    /// Conservative owned scratch payload; allocator bucket overhead is not RSS.
    #[cfg(any(test, feature = "test-support"))]
    pub fn reservation_demand(
        max_registers: usize,
        max_native_arguments: usize,
    ) -> Result<(usize, usize), RuntimeError> {
        Self::reservation_demand_with_frame(max_registers, max_native_arguments, 0)
    }
    /// Reservation demand including the eligible POU's reusable local frame.
    #[cfg(any(test, feature = "test-support"))]
    pub fn reservation_demand_with_frame(
        max_registers: usize,
        max_native_arguments: usize,
        local_count: usize,
    ) -> Result<(usize, usize), RuntimeError> {
        Self::reservation_demand_with_temporaries(
            max_registers,
            max_native_arguments,
            local_count,
            0,
        )
    }
    /// Reservation demand including the local frame and `temporaries` bytes of texts and copies
    /// the register file holds at once (C1-20).
    pub fn reservation_demand_with_temporaries(
        max_registers: usize,
        max_native_arguments: usize,
        local_count: usize,
        temporaries: usize,
    ) -> Result<(usize, usize), RuntimeError> {
        let registers = max_registers
            .checked_mul(mem::size_of::<Value>() + mem::size_of::<u32>())
            .ok_or(RuntimeError::Overflow)?;
        let operands = max_native_arguments
            .checked_mul(mem::size_of::<Value>())
            .ok_or(RuntimeError::Overflow)?;
        let frame_payload = local_count
            .checked_mul(mem::size_of::<Value>() + mem::size_of::<bool>())
            .ok_or(RuntimeError::Overflow)?;
        let bytes = registers
            .checked_add(operands)
            .and_then(|bytes| bytes.checked_add(frame_payload))
            .and_then(|bytes| bytes.checked_add(temporaries))
            .and_then(|bytes| bytes.checked_add(mem::size_of::<super::super::VmFrame>()))
            .and_then(|bytes| bytes.checked_add(mem::size_of::<Self>()))
            .ok_or(RuntimeError::Overflow)?;
        let work = max_registers
            .checked_add(max_native_arguments)
            .and_then(|work| work.checked_add(local_count))
            .and_then(|work| work.checked_add(1))
            .ok_or(RuntimeError::Overflow)?;
        Ok((bytes, work))
    }
    /// Fallibly reserve the admitted scratch demand before entering RUN.
    #[cfg(any(test, feature = "test-support"))]
    pub fn reserve(
        max_registers: usize,
        max_native_arguments: usize,
    ) -> Result<Self, RuntimeError> {
        Self::reserve_with_frame(max_registers, max_native_arguments, 0)
    }
    /// Reserve execution buffers and one reusable local/presence frame payload.
    #[cfg(any(test, feature = "test-support"))]
    pub fn reserve_with_frame(
        max_registers: usize,
        max_native_arguments: usize,
        local_count: usize,
    ) -> Result<Self, RuntimeError> {
        Self::reserve_with_temporaries(max_registers, max_native_arguments, local_count, 0)
    }
    /// Reserve execution buffers, the local frame and room for `temporaries` bytes of the texts and
    /// copies the register file holds at once (C1-20).
    pub fn reserve_with_temporaries(
        max_registers: usize,
        max_native_arguments: usize,
        local_count: usize,
        temporaries: usize,
    ) -> Result<Self, RuntimeError> {
        let mut result = Self::default();
        let operands = max_registers
            .checked_add(max_native_arguments)
            .ok_or(RuntimeError::Overflow)?;
        result
            .registers
            .reserve_storage(operands)
            .map_err(VmTrap::into_runtime_error)?;
        result
            .registers
            .reserve_temporaries(temporaries)
            .map_err(VmTrap::into_runtime_error)?;
        result
            .remaining_register_reads
            .try_reserve_exact(max_registers)
            .map_err(|_| RuntimeError::PreparationLimit)?;
        result
            .frames
            .try_reserve_frames(1)
            .map_err(VmTrap::into_runtime_error)?;
        result
            .frames
            .try_reserve_frame_payload(local_count)
            .map_err(VmTrap::into_runtime_error)?;
        result
            .registers
            .reset_registers(max_registers)
            .map_err(VmTrap::into_runtime_error)?;
        result.remaining_register_reads.resize(max_registers, 0);
        Ok(result)
    }
    /// Select a body without reallocating its pre-reserved scratch.
    pub fn select(&mut self, registers: usize) -> Result<(), RuntimeError> {
        if registers > self.registers.register_capacity()
            || registers > self.remaining_register_reads.capacity()
        {
            return Err(RuntimeError::PreparationLimit);
        }
        self.clear();
        self.registers
            .reset_registers(registers)
            .map_err(VmTrap::into_runtime_error)?;
        self.remaining_register_reads.resize(registers, 0);
        Ok(())
    }
    pub fn clear(&mut self) {
        if !self.needs_clear {
            return;
        }
        self.frames.clear();
        self.registers.clear_registers();
        self.remaining_register_reads.fill(0);
        self.needs_clear = false;
    }

    /// Reclaim locals returned to a call continuation after copyback. A
    /// recursive fallback can leave the pre-reserved locals in place, in
    /// which case the returned stack locals are simply dropped.
    pub(crate) fn recycle_result_locals(&mut self, locals: Vec<Value>) -> Result<(), RuntimeError> {
        if self.frames.has_reserved_frame_payload() {
            return Ok(());
        }
        self.frames.recycle_returned_locals(locals)
    }

    pub(crate) fn can_enter_with_frame_payload(&self) -> bool {
        self.frames.has_reserved_frame_payload()
    }
    pub fn buffers_mut(&mut self) -> (&mut FrameStack, &mut OperandStack, &mut [u32]) {
        // Every mutable lease may dirty scratch; cleanup is idempotent across
        // selection, executor finalization and owner recycling.
        self.needs_clear = true;
        (
            &mut self.frames,
            &mut self.registers,
            &mut self.remaining_register_reads,
        )
    }
    #[cfg(any(test, feature = "test-support"))]
    pub fn storage_bytes(&self) -> usize {
        self.registers
            .storage_bytes()
            .saturating_add(
                self.remaining_register_reads
                    .capacity()
                    .saturating_mul(mem::size_of::<u32>()),
            )
            .saturating_add(self.frames.storage_bytes())
    }
}

/// Optional execution-plan and telemetry access provided by the engine context.
/// This trait is implemented only by internal adapters, never public state facades.
pub trait RegisterContext: ExecutionContext {
    fn register_profile(&self) -> &RegisterProfileState;
    fn register_profile_mut(&mut self) -> &mut RegisterProfileState;
    fn tier1_state(&self) -> &RegisterTier1SpecializedExecutorState;
    fn tier1_state_mut(&mut self) -> &mut RegisterTier1SpecializedExecutorState;
    fn prepared_register_plan(&self, pou_id: u32) -> Option<Arc<PreparedRegisterProgram>>;
    /// The text call of a standard-function call of the native symbol `symbol_idx` with `count`
    /// arguments (C1-20). The default finds it by name; a composition with prepared plans keeps it.
    fn register_text_call(
        &self,
        module: &VmModule,
        symbol_idx: u32,
        count: usize,
    ) -> Option<crate::vm::dispatch_text::TextCall> {
        crate::vm::dispatch_text::text_call_of(
            self.stdlib(),
            module.native_symbol_spec(symbol_idx).ok()?,
            count,
        )
    }
    fn record_register_plan_lookup(&mut self, _found: bool) {}
    fn register_buffers_available(&self, _pou_id: u32) -> bool {
        true
    }
    fn debug_active(&self) -> bool {
        false
    }
    /// Whether `pou_id` has a prepared, fallback-free plan that makes no user call.
    fn register_leaf_plan(&self, _pou_id: u32) -> bool {
        false
    }
    /// Counts a call run in place, for parity tests.
    #[cfg(feature = "test-support")]
    fn record_in_place_call(&mut self) {}
    fn take_register_buffers(
        &mut self,
        pou_id: u32,
        registers: usize,
    ) -> Result<RegisterExecutionBuffers, RuntimeError>;
    fn recycle_register_buffers(
        &mut self,
        pou_id: u32,
        buffers: RegisterExecutionBuffers,
    ) -> Result<(), RuntimeError>;
}

/// Charge conservative scratch and retained plan demand before lowering allocates.
/// Entry stack depths bound normalization expansion, rather than a quadratic
/// max-registers by block-count reservation. Sparse read counts stay linear in
/// the actual register uses represented by the lowered body.
fn preflight_pou(
    module: &VmModule,
    id: u32,
    charge: &mut impl FnMut(usize, usize) -> Result<(), RuntimeError>,
) -> Result<(), RuntimeError> {
    let pou = module.pou(id).ok_or(RuntimeError::InvalidExecutionState)?;
    charge(
        pou.name
            .len()
            .checked_add(512)
            .ok_or(RuntimeError::Overflow)?,
        pou.name
            .len()
            .checked_add(1)
            .ok_or(RuntimeError::Overflow)?,
    )?;
    let code = module
        .code()
        .get(pou.code_start..pou.code_end)
        .ok_or(RuntimeError::InvalidExecutionState)?;
    let mut count = 0usize;
    let mut pc = 0usize;
    let mut native_arguments = 0usize;
    while pc < code.len() {
        let opcode = code[pc];
        let Some(width) = opcode_operand_len(opcode) else {
            break;
        };
        let end = pc
            .checked_add(1)
            .and_then(|start| start.checked_add(width))
            .ok_or(RuntimeError::Overflow)?;
        let Some(operands) = code.get(pc + 1..end) else {
            break;
        };
        if opcode == 0x09 && operands.len() == 12 {
            let arguments = u32::from_le_bytes(
                operands[8..12]
                    .try_into()
                    .map_err(|_| RuntimeError::InvalidExecutionState)?,
            ) as usize;
            native_arguments = native_arguments
                .checked_add(arguments)
                .ok_or(RuntimeError::Overflow)?;
        }
        if opcode == 0x10 && operands.len() == 4 {
            let constant = u32::from_le_bytes(
                operands
                    .try_into()
                    .map_err(|_| RuntimeError::InvalidExecutionState)?,
            ) as usize;
            if let Some(value) = module.consts().get(constant) {
                charge(value_payload_bytes(value, 0)?, 1)?;
            }
        }
        count = count.checked_add(1).ok_or(RuntimeError::Overflow)?;
        pc = end;
    }
    // Decoding and leader/depth maps are created twice (preflight and lowering).
    let scratch_width = mem::size_of::<super::lower::decode::DecodedInstr>()
        .checked_mul(4)
        .and_then(|v| v.checked_add(512))
        .ok_or(RuntimeError::Overflow)?;
    charge(
        count
            .checked_mul(scratch_width)
            .ok_or(RuntimeError::Overflow)?,
        count.checked_mul(16).ok_or(RuntimeError::Overflow)?,
    )?;
    let decoded = match decode_pou(module, pou.code_start, pou.code_end) {
        Ok(decoded) => decoded,
        Err(_) => return Ok(()),
    };
    let leaders = match collect_block_leaders(&decoded, pou.code_start, pou.code_end) {
        Ok(leaders) => leaders,
        Err(_) => return Ok(()),
    };
    let depths =
        match compute_block_entry_stack_depths(&decoded, &leaders, pou.code_start, pou.code_end) {
            Ok(depths) => depths,
            Err(_) => return Ok(()),
        };
    let entry_slots = depths.values().try_fold(0usize, |total, depth| {
        total
            .checked_add(*depth as usize)
            .ok_or(RuntimeError::Overflow)
    })?;
    let expanded = count
        .checked_mul(5)
        .and_then(|v| {
            entry_slots
                .checked_mul(3)
                .and_then(|depths| v.checked_add(depths))
        })
        .and_then(|v| v.checked_add(leaders.len()))
        .ok_or(RuntimeError::Overflow)?;
    let width = mem::size_of::<RegisterInstr>()
        .checked_mul(4)
        .and_then(|v| {
            mem::size_of::<Tier1CompiledInstr>()
                .checked_mul(2)
                .and_then(|tier| v.checked_add(tier))
        })
        .and_then(|v| v.checked_add(256))
        .ok_or(RuntimeError::Overflow)?;
    let bytes = expanded
        .checked_mul(width)
        .and_then(|v| {
            native_arguments
                .checked_mul(mem::size_of::<RegisterId>() * 8)
                .and_then(|arguments| v.checked_add(arguments))
        })
        .and_then(|v| {
            leaders
                .len()
                .checked_mul(mem::size_of::<RegisterBlock>() * 4)
                .and_then(|blocks| v.checked_add(blocks))
        })
        .ok_or(RuntimeError::Overflow)?;
    let lookup_depth = (usize::BITS - expanded.max(1).leading_zeros()) as usize;
    charge(
        bytes,
        expanded
            .checked_mul(lookup_depth.checked_mul(16).ok_or(RuntimeError::Overflow)?)
            .ok_or(RuntimeError::Overflow)?,
    )
}

fn value_payload_bytes(value: &Value, depth: usize) -> Result<usize, RuntimeError> {
    if depth >= 128 {
        return Err(RuntimeError::PreparationLimit);
    }
    match value {
        Value::WString(text) => Ok(text.len()),
        Value::Array(array) => {
            let mut bytes = mem::size_of::<ArrayValue>()
                .checked_add(
                    array
                        .elements()
                        .len()
                        .checked_mul(mem::size_of::<Value>())
                        .ok_or(RuntimeError::Overflow)?,
                )
                .and_then(|v| {
                    array
                        .dimensions()
                        .len()
                        .checked_mul(mem::size_of::<(i64, i64)>())
                        .and_then(|dimensions| v.checked_add(dimensions))
                })
                .ok_or(RuntimeError::Overflow)?;
            for child in array.elements() {
                bytes = bytes
                    .checked_add(value_payload_bytes(child, depth + 1)?)
                    .ok_or(RuntimeError::Overflow)?;
            }
            Ok(bytes)
        }
        Value::Enum(_) => Ok(mem::size_of::<EnumValue>()),
        _ => Ok(0), // strings and structures share immutable backing; scalars are inline
    }
}

impl crate::memory::figure::HeapBytes for PreparedRegisterPlans {
    fn add_heap(&self, count: &mut crate::memory::figure::HeapCount) {
        count.of(&self.entries);
        count.of(&self.summary);
        count.of(&self.text_calls);
    }
}

impl crate::memory::figure::HeapBytes for PreparedRegisterProgram {
    fn add_heap(&self, count: &mut crate::memory::figure::HeapCount) {
        count.of(&self.lowered);
        count.of(&self.tier1_blocks);
        count.of(&self.tier1_selection_indices);
        count.of(&self.tier1_failures);
        count.of(&self.fallback_reason);
    }
}

#[cfg(test)]
mod allocation_model_tests {
    use super::*;

    #[test]
    fn aggregate_and_enum_constant_estimates_include_box_payloads() {
        let array = Value::Array(alloc::boxed::Box::new(ArrayValue::from_canonical_parts(
            vec![Value::DInt(1), Value::DInt(2)],
            vec![(0, 1)],
        )));
        let enum_value = Value::Enum(alloc::boxed::Box::new(EnumValue::from_canonical_parts(
            "Mode".into(),
            "Active".into(),
            1,
        )));

        assert!(value_payload_bytes(&array, 0).unwrap() >= mem::size_of::<ArrayValue>());
        assert!(value_payload_bytes(&enum_value, 0).unwrap() >= mem::size_of::<EnumValue>());
    }
}

#[cfg(test)]
mod leaf_tests;

/// The most operands `instruction` pushes above the registers (C1-20): a call's arguments, and its
/// result when the call hands it back as bytes; the two operands `=` and `<>` compare as the stack
/// compares them.
fn operands_above_registers(instruction: &RegisterInstr) -> usize {
    let equality = |op: &BinaryOp| matches!(op, BinaryOp::Eq | BinaryOp::Ne);
    match instruction {
        RegisterInstr::CallNative { args, .. } => args.len().max(1),
        RegisterInstr::Binary { op, .. }
        | RegisterInstr::BinaryRefToRef { op, .. }
        | RegisterInstr::BinaryRefConstToRef { op, .. }
        | RegisterInstr::BinaryConstRefToRef { op, .. }
        | RegisterInstr::CmpRefConstJumpIf { op, .. }
            if equality(op) =>
        {
            2
        }
        _ => 0,
    }
}
