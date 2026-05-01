use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(super) struct Tier1BlockKey {
    pub(super) module_ptr: usize,
    pub(super) pou_id: u32,
    pub(super) block_id: u32,
    pub(super) start_pc: u32,
}

#[derive(Debug, Clone)]
pub(super) struct Tier1CompiledBlock {
    pub(super) key: Tier1BlockKey,
    pub(super) instructions: Vec<Tier1CompiledInstr>,
}

#[derive(Debug, Clone)]
pub(super) enum Tier1CompiledInstr {
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
    pub(super) hot_block_threshold: u64,
    pub(super) cache_capacity: usize,
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

    pub(super) fn enabled(&self) -> bool {
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

    pub(super) fn compiled_block(&self, key: &Tier1BlockKey) -> Option<&Arc<Tier1CompiledBlock>> {
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

    pub(super) fn insert_compiled_block(&mut self, block: Arc<Tier1CompiledBlock>) {
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

    fn record_block_execution(&mut self) {
        self.block_executions = self.block_executions.saturating_add(1);
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

#[allow(clippy::too_many_arguments)]
pub(super) fn maybe_execute_tier1_block(
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

    let outcome = execute_tier1_compiled_block(
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
    )?;
    let Tier1BlockExecutionOutcome::Executed(outcome) = outcome;
    runtime
        .vm_tier1_specialized_executor
        .record_block_execution();
    Ok(Some(outcome))
}

pub(super) fn tier1_block_key(
    module: &VmModule,
    pou_id: u32,
    block: &RegisterBlock,
) -> Tier1BlockKey {
    Tier1BlockKey {
        module_ptr: module as *const VmModule as usize,
        pou_id,
        block_id: block.id,
        start_pc: block.start_pc.try_into().unwrap_or(u32::MAX),
    }
}

pub(super) fn compile_tier1_block(
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
                        dynamic_ref_field_borrowed(runtime, frames, reference, field.clone())
                            .map_err(VmTrap::into_runtime_error)?
                    }
                    Value::Reference(None) => {
                        return Err(VmTrap::NullReference.into_runtime_error());
                    }
                    Value::Instance(instance_id) => {
                        runtime
                            .vm_register_profile
                            .record_ref_op(RegisterRefOpKind::InstanceFieldLookup);
                        let next = runtime
                            .storage
                            .resolved_instance_field_ref(*instance_id, field.as_str())
                            .ok_or_else(|| {
                                VmTrap::Runtime(RuntimeError::UndefinedField(field.clone()))
                                    .into_runtime_error()
                            })?;
                        next
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
                let value = dynamic_load_ref(runtime, frames, &reference)
                    .map_err(VmTrap::into_runtime_error)?;
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

pub(super) fn apply_dint_binary_guard_borrowed(
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
                return Err(RuntimeError::ModuloByZero);
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

pub(super) fn prepare_borrowed_binary_eval(
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
