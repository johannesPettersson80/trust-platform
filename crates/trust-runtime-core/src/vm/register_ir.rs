//! Prepared optional register execution over the shared portable VM services.
//! Plans are compiled before installation; execution never lowers bytecode.
#![cfg_attr(any(test, feature = "test-support"), allow(missing_docs))]
use super::call::continuation::NativeCall;
use super::call::{prepare_native_call, push_call_frame_with_reserved_payload};
use super::dispatch_refs::{
    dynamic_load_ref, dynamic_ref_field, dynamic_ref_field_borrowed, dynamic_ref_index,
    dynamic_store_ref, index_to_i64, load_ref, load_ref_addr, peek_ref, store_ref,
};
use super::helpers::{materialize_borrowed_value, opcode_operand_len};
use super::module::invalid_bytecode;
use super::module::VmModule;
use super::{FrameStack, OperandStack, VmTrap};
use crate::error::RuntimeError;
use crate::memory::InstanceId;
use crate::program_model::{apply_binary, apply_unary, BinaryOp, UnaryOp};
use crate::value::Value;
use crate::vm::{budget::ExecutionEntry, context::ExecutionContext};
use alloc::{
    borrow::Cow,
    boxed::Box,
    collections::{BTreeMap, BTreeSet, VecDeque},
    format,
    string::{String, ToString},
    sync::Arc,
    vec,
    vec::Vec,
};
use core::mem;
mod snapshots;
pub use snapshots::*;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RegisterId(pub u32);

impl RegisterId {
    pub fn index(self) -> u32 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockTarget {
    Block(u32),
    Exit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegisterInstr {
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
    LoadSelfFieldDynamic {
        field_idx: u32,
        dest: RegisterId,
    },
    StoreSelfFieldDynamic {
        field_idx: u32,
        value: RegisterId,
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
    /// The `HALT` (opcode 0x01) of the FOR zero-step guard. It faults with
    /// `ForStepZero` exactly as the stack path does and ends the block.
    ForStepZero,
    VmFallback {
        opcode: u8,
        operands: Vec<u8>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisterBlock {
    pub id: u32,
    pub start_pc: usize,
    pub end_pc: usize,
    pub entry_stack_depth: u32,
    pub bytecode_instruction_count: usize,
    pub instruction_costs: Vec<usize>,
    /// Original per-bytecode charges for each lowered instruction. Fused
    /// entries retain one charge per source operation to preserve fault order.
    pub instruction_cost_steps: Vec<Vec<usize>>,
    pub instructions: Vec<RegisterInstr>,
}

#[inline(always)]
fn charge_instruction_cost_step(
    runtime: &impl RegisterContext,
    costs: &[usize],
    step: usize,
) -> Result<(), RuntimeError> {
    let cost = costs
        .get(step)
        .copied()
        .ok_or_else(|| invalid_bytecode("register-ir fused instruction cost missing"))?;
    charge_instruction_cost(runtime, cost)
}

/// Charge a step that stands for `units` original instructions of one unit each. This runs on
/// every executed step, so it is inlined into the executors and a refusal is handled out of
/// line (measured: an out-of-line charge cost about 11 instructions per step).
#[inline(always)]
#[cfg_attr(all(feature = "std", feature = "test-support"), track_caller)]
fn charge_instruction_cost(
    runtime: &impl RegisterContext,
    units: usize,
) -> Result<(), RuntimeError> {
    runtime
        .charge_execution_work(units)
        .or_else(|error| refuse_instruction_cost(runtime, error))
}

/// A step that does not fit spends the rest of the allowance on the originals before the
/// refused one, then refuses that original as the stack executor does: `used` is the limit,
/// `requested` is one unit. Any other error passes through unchanged.
#[cold]
#[inline(never)]
fn refuse_instruction_cost(
    runtime: &impl RegisterContext,
    error: RuntimeError,
) -> Result<(), RuntimeError> {
    let RuntimeError::ExecutionLimit {
        kind: crate::error::ExecutionLimitKind::Work,
        used,
        limit,
        ..
    } = error
    else {
        return Err(error);
    };
    runtime.charge_execution_work(limit.saturating_sub(used))?;
    Err(RuntimeError::ExecutionLimit {
        kind: crate::error::ExecutionLimitKind::Work,
        used: limit,
        requested: 1,
        limit,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisterProgram {
    pub pou_id: u32,
    pub entry_block: u32,
    pub max_registers: u32,
    pub blocks: Vec<RegisterBlock>,
}

#[cfg(any(test, feature = "test-support"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegisterExecutionOutcome {
    Executed,
    FallbackToStack,
}

/// Result of one register POU entry. When `locals` is nonempty, it owns the
/// prepared local buffer until the caller finishes copyback and returns it via
/// `CallContext::recycle_optimized_result_locals`; that POU uses stack fallback
/// for recursive re-entry while the buffer is in flight.
#[derive(Debug, Clone)]
pub struct RegisterPouExecutionResult {
    /// Optional value returned by a function POU.
    pub return_value: Option<Value>,
    /// Owned local payload retained until caller copyback and scratch recycle.
    pub locals: Vec<Value>,
}

#[cfg(feature = "test-support")]
pub fn prepare_register_file(registers: &mut Vec<Value>, max_registers: usize) {
    registers.resize(max_registers, Value::Null);
}

pub fn reset_register_file(registers: &mut [Value]) {
    for slot in registers {
        if !matches!(slot, Value::Null) {
            *slot = Value::Null;
        }
    }
}

mod activation;
mod configuration;
pub mod environment;
mod interpreter;
mod lower;
mod profile;
pub use configuration::RegisterExecutionConfiguration;
mod registers;

pub mod tier1;
#[cfg(feature = "test-support")]
pub use self::interpreter::build_register_pou_result;
pub(in crate::vm) use self::interpreter::RegisterBlockStep;
pub use self::interpreter::{
    execute_register_block_interpreted, BorrowedBinaryEval, RegisterBlockExecutionOutcome,
    Tier1BlockExecutionOutcome,
};

pub(crate) use self::activation::{
    abort_register_activation, recycle_register_activation, start_register_activation,
    step_register_activation,
};
pub use self::activation::{DeferredRegisterCall, RegisterActivation, RegisterActivationStep};
use self::lower::is_cmp_binary_op;
pub use self::lower::lower_pou_to_register_ir;
pub use self::lower::{collect_block_leaders, compute_block_entry_stack_depths, decode_pou};
#[cfg(feature = "test-support")]
pub use self::lower::{
    fuse_register_block_instructions, fuse_register_block_instructions_with_costs,
    instruction_reads_register, normalize_stack_for_block_exit, verify_register_program,
};
pub use self::profile::{CachedRegisterProgram, RegisterLoweringCacheEntry};
pub use self::profile::{RegisterCallOpKind, RegisterRefOpKind, RegisterValueOpKind};
pub use self::profile::{RegisterLoweringCacheState, RegisterProfileState};
pub use self::registers::*;
#[cfg(any(test, feature = "test-support"))]
pub use self::tier1::apply_dint_binary_guard_borrowed;
#[cfg(feature = "test-support")]
pub use self::tier1::execute_tier1_compiled_block;
pub use self::tier1::RegisterTier1SpecializedExecutorState;
use self::tier1::{
    charge_borrowed_binary_operand, maybe_execute_tier1_block, prepare_borrowed_binary_eval,
};
pub use self::tier1::{compile_tier1_block, Tier1CompiledBlock, Tier1CompiledInstr};

pub fn build_cached_register_program(
    module: &VmModule,
    pou_id: u32,
) -> Result<CachedRegisterProgram, RuntimeError> {
    let program = lower_pou_to_register_ir(module, pou_id)?;
    let register_read_counts_by_block = sparse_register_read_counts_by_block(&program);
    let block_has_register_reads = register_read_counts_by_block
        .iter()
        .map(|counts| counts.iter().any(|(_, count)| *count != 0))
        .collect::<Vec<_>>();
    let fallback_opcode = first_fallback_opcode(&program);
    Ok(CachedRegisterProgram {
        program: Arc::new(program),
        register_read_counts_by_block: Arc::new(register_read_counts_by_block),
        block_has_register_reads: Arc::new(block_has_register_reads),
        fallback_opcode,
    })
}

#[cfg(any(test, feature = "test-support"))]
pub fn try_execute_pou_with_register_ir(
    runtime: &mut impl RegisterContext,
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
        ExecutionEntry::Root,
    )?;
    if result.is_some() {
        Ok(RegisterExecutionOutcome::Executed)
    } else {
        Ok(RegisterExecutionOutcome::FallbackToStack)
    }
}

#[cfg(any(test, feature = "test-support"))]
#[allow(clippy::too_many_arguments)]
pub fn try_execute_pou_with_register_ir_with_locals(
    runtime: &mut impl RegisterContext,
    module: &VmModule,
    pou_id: u32,
    entry_instance: Option<InstanceId>,
    initial_locals: Option<&[Value]>,
    capture_return: bool,
    depth_offset: u32,
    entry: ExecutionEntry,
) -> Result<Option<RegisterPouExecutionResult>, RuntimeError> {
    try_execute_pou_with_register_ir_with_parameter_presence(
        runtime,
        module,
        pou_id,
        entry_instance,
        initial_locals,
        None,
        capture_return,
        depth_offset,
        entry,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn try_execute_pou_with_register_ir_with_parameter_presence(
    runtime: &mut impl RegisterContext,
    module: &VmModule,
    pou_id: u32,
    entry_instance: Option<InstanceId>,
    initial_locals: Option<&[Value]>,
    parameter_values_present: Option<&[bool]>,
    capture_return: bool,
    depth_offset: u32,
    entry: ExecutionEntry,
) -> Result<Option<RegisterPouExecutionResult>, RuntimeError> {
    try_execute_pou_with_result_policy(
        runtime,
        module,
        pou_id,
        entry_instance,
        initial_locals,
        None,
        parameter_values_present,
        capture_return,
        depth_offset,
        entry,
        true,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn try_execute_pou_with_register_ir_with_parameter_presence_owned(
    runtime: &mut impl RegisterContext,
    module: &VmModule,
    pou_id: u32,
    entry_instance: Option<InstanceId>,
    initial_locals: &mut Option<Vec<Value>>,
    parameter_values_present: Option<&[bool]>,
    capture_return: bool,
    depth_offset: u32,
    entry: ExecutionEntry,
) -> Result<Option<RegisterPouExecutionResult>, RuntimeError> {
    try_execute_pou_with_result_policy(
        runtime,
        module,
        pou_id,
        entry_instance,
        None,
        Some(initial_locals),
        parameter_values_present,
        capture_return,
        depth_offset,
        entry,
        true,
    )
}

#[allow(clippy::too_many_arguments)]
fn try_execute_pou_with_result_policy(
    runtime: &mut impl RegisterContext,
    module: &VmModule,
    pou_id: u32,
    entry_instance: Option<InstanceId>,
    initial_locals: Option<&[Value]>,
    owned_initial_locals: Option<&mut Option<Vec<Value>>>,
    parameter_values_present: Option<&[bool]>,
    capture_return: bool,
    depth_offset: u32,
    entry: ExecutionEntry,
    capture_result_locals: bool,
) -> Result<Option<RegisterPouExecutionResult>, RuntimeError> {
    // Keep stack execution as the single source of truth while debug stepping is active.
    if runtime.debug_active() {
        runtime
            .register_profile_mut()
            .record_prepared_fallback("debug_mode");
        return Ok(None);
    }

    let plan = runtime.prepared_register_plan(pou_id);
    runtime.record_register_plan_lookup(plan.is_some());
    let Some(plan) = plan else {
        runtime
            .register_profile_mut()
            .record_prepared_fallback("unprepared_program");
        return Ok(None);
    };
    if let Some(reason) = plan.fallback_reason() {
        runtime
            .register_profile_mut()
            .record_prepared_fallback(reason);
        return Ok(None);
    }
    // Only recursive re-entry into an already occupied POU needs the inherited
    // stack fallback. Independent callees have their own prepared scratch.
    if !runtime.register_buffers_available(pou_id) {
        runtime
            .register_profile_mut()
            .record_prepared_fallback("nested_scratch");
        return Ok(None);
    }
    let RegisterLoweringCacheEntry::Ready(lowered) = &plan.lowered else {
        return Err(RuntimeError::InvalidExecutionState);
    };
    let result = execute_register_program(
        runtime,
        module,
        plan.as_ref(),
        lowered.program.as_ref(),
        lowered.register_read_counts_by_block.as_ref(),
        lowered.block_has_register_reads.as_ref(),
        entry_instance,
        initial_locals,
        owned_initial_locals,
        parameter_values_present,
        capture_return,
        capture_result_locals,
        depth_offset,
        entry,
    )?;
    if let Err(error) = runtime.check_execution_deadline() {
        if capture_result_locals {
            let _ = runtime.recycle_optimized_result_locals(pou_id, result.locals);
        }
        return Err(error);
    }
    runtime.register_profile_mut().record_executed();
    Ok(Some(result))
}

#[allow(clippy::too_many_arguments)]
fn execute_register_program(
    runtime: &mut impl RegisterContext,
    module: &VmModule,
    plan: &PreparedRegisterProgram,
    program: &RegisterProgram,
    register_read_counts_by_block: &[Vec<(u32, u32)>],
    block_has_register_reads: &[bool],
    entry_instance: Option<InstanceId>,
    initial_locals: Option<&[Value]>,
    mut owned_initial_locals: Option<&mut Option<Vec<Value>>>,
    parameter_values_present: Option<&[bool]>,
    capture_return: bool,
    capture_result_locals: bool,
    depth_offset: u32,
    entry: ExecutionEntry,
) -> Result<RegisterPouExecutionResult, RuntimeError> {
    #[cfg(all(feature = "std", feature = "test-support"))]
    let _timing =
        crate::vm::diagnostic_timing::begin(crate::vm::diagnostic_timing::Phase::RegisterExecute);
    runtime.begin_execution(entry, module.instruction_budget())?;
    if (depth_offset as usize).saturating_add(1) > runtime.call_depth_limit() {
        return Err(VmTrap::CallStackOverflow.into_runtime_error());
    }
    let mut register_execution_buffers =
        runtime.take_register_buffers(program.pou_id, program.max_registers as usize)?;
    let result = (|| {
        let (frames, registers, remaining_register_reads, native_call_stack) =
            register_execution_buffers.buffers_mut();
        let profile_enabled = prepare_register_entry_frame(
            runtime,
            module,
            frames,
            program.pou_id,
            entry_instance,
            initial_locals,
            owned_initial_locals.as_deref_mut(),
            parameter_values_present,
            depth_offset,
        )?;
        let mut current_block = program.entry_block;

        let tier1_enabled = runtime.tier1_state().enabled();
        loop {
            if frames.is_empty() {
                return Ok(RegisterPouExecutionResult {
                    return_value: None,
                    locals: Vec::new(),
                });
            }
            let block_index = block_index_from_id(program, current_block)?;
            let block = &program.blocks[block_index];
            if block.id != current_block {
                return Err(invalid_bytecode(format!(
                "register-ir executor resolved block id {current_block} to index {block_index} containing block id {}",
                block.id
            )));
            }
            if *block_has_register_reads.get(block_index).unwrap_or(&false) {
                for (index, count) in &register_read_counts_by_block[block_index] {
                    let Some(slot) = remaining_register_reads.get_mut(*index as usize) else {
                        return Err(RuntimeError::InvalidExecutionState);
                    };
                    *slot = *count;
                }
            }
            if profile_enabled {
                runtime.register_profile_mut().record_block_hit(
                    program.pou_id,
                    block.id,
                    block.start_pc.try_into().unwrap_or(u32::MAX),
                );
            }
            let Some(frame) = frames.current() else {
                return Err(RuntimeError::InvalidExecutionState);
            };
            let depth = depth_offset.saturating_add(frames.len().saturating_sub(1) as u32);
            runtime.on_statement_with_frame_checked(
                module,
                program.pou_id,
                block.start_pc,
                depth,
                super::context::StatementFrame::from_frame(frame),
            )?;

            let outcome = if tier1_enabled {
                match maybe_execute_tier1_block(
                    runtime,
                    module,
                    plan,
                    program,
                    block,
                    frames,
                    registers,
                    remaining_register_reads,
                    native_call_stack,
                    depth_offset,
                )? {
                    Some(outcome) => outcome,
                    None => execute_register_block_interpreted(
                        runtime,
                        module,
                        program,
                        frames,
                        registers,
                        remaining_register_reads,
                        native_call_stack,
                        block,
                        depth_offset,
                    )?,
                }
            } else {
                execute_register_block_interpreted(
                    runtime,
                    module,
                    program,
                    frames,
                    registers,
                    remaining_register_reads,
                    native_call_stack,
                    block,
                    depth_offset,
                )?
            };

            match outcome {
                RegisterBlockExecutionOutcome::ReturnFromPou => {
                    let finished = frames.pop().map_err(VmTrap::into_runtime_error)?;
                    runtime
                        .register_profile_mut()
                        .record_call_op(RegisterCallOpKind::FramePop);
                    if frames.is_empty() {
                        return finish_register_result(
                            runtime,
                            frames,
                            finished,
                            capture_return,
                            capture_result_locals,
                        );
                    }
                    let _ = runtime.retire_frame(&finished);
                    return Err(invalid_bytecode(format!(
                        "register-ir executor unsupported nested call return_pc={}",
                        finished.return_pc
                    )));
                }
                RegisterBlockExecutionOutcome::Continue(control_target) => {
                    let next_target = match control_target {
                        Some(target) => target,
                        None => next_linear_block_target(program, block_index),
                    };
                    match next_target {
                        BlockTarget::Block(next_block) => current_block = next_block,
                        BlockTarget::Exit => {
                            let finished = frames.pop().map_err(VmTrap::into_runtime_error)?;
                            runtime
                                .register_profile_mut()
                                .record_call_op(RegisterCallOpKind::FramePop);
                            return finish_register_result(
                                runtime,
                                frames,
                                finished,
                                capture_return,
                                capture_result_locals,
                            );
                        }
                    }
                }
            }
        }
    })();
    while let Ok(frame) = register_execution_buffers.frames.pop() {
        let _ = runtime.retire_frame(&frame);
        register_execution_buffers
            .frames
            .recycle_frame_payload(frame);
    }
    register_execution_buffers.clear();
    let recycled = runtime.recycle_register_buffers(program.pou_id, register_execution_buffers);
    match result {
        Err(error) => Err(error),
        Ok(value) => recycled.map(|()| value),
    }
}

#[allow(clippy::too_many_arguments)]
fn prepare_register_entry_frame(
    runtime: &mut impl RegisterContext,
    module: &VmModule,
    frames: &mut FrameStack,
    pou_id: u32,
    entry_instance: Option<InstanceId>,
    initial_locals: Option<&[Value]>,
    owned_initial_locals: Option<&mut Option<Vec<Value>>>,
    parameter_values_present: Option<&[bool]>,
    depth_offset: u32,
) -> Result<bool, RuntimeError> {
    let (frame_bytes, frame_work) = frames.growth_demand().map_err(VmTrap::into_runtime_error)?;
    runtime.charge_call_storage(frame_bytes, frame_work)?;
    let _ =
        push_call_frame_with_reserved_payload(frames, module, pou_id, usize::MAX, entry_instance)
            .map_err(VmTrap::into_runtime_error)?;
    runtime
        .register_profile_mut()
        .record_call_op(RegisterCallOpKind::FramePush);
    let profile_enabled = runtime.register_profile().is_enabled();
    if let Some(initial_locals) = owned_initial_locals.and_then(|payload| payload.as_mut()) {
        frames
            .move_current_locals_from(initial_locals)
            .map_err(VmTrap::into_runtime_error)?;
    } else if let Some(initial_locals) = initial_locals {
        let frame = frames
            .current_mut()
            .ok_or_else(|| VmTrap::CallStackUnderflow.into_runtime_error())?;
        if initial_locals.len() > frame.locals.len() {
            return Err(VmTrap::BytecodeDecode(
                "register-ir call initial local payload exceeds frame local capacity".into(),
            )
            .into_runtime_error());
        }
        for (index, value) in initial_locals.iter().enumerate() {
            runtime.before_value_clone(value)?;
            frame.locals[index] = value.clone();
        }
    }
    if let Some(present) = parameter_values_present {
        let frame = frames
            .current_mut()
            .ok_or_else(|| RuntimeError::InvalidExecutionState)?;
        if present.len() > frame.locals.len() {
            return Err(RuntimeError::InvalidExecutionState);
        }
        frame.parameter_values_present[..present.len()].copy_from_slice(present);
    }
    let frame = frames
        .current_mut()
        .ok_or_else(|| VmTrap::CallStackUnderflow.into_runtime_error())?;
    runtime.initialize_frame(module, frame, depth_offset)?;
    Ok(profile_enabled)
}

pub fn next_linear_block_target(program: &RegisterProgram, block_index: usize) -> BlockTarget {
    program
        .blocks
        .get(block_index..)
        .and_then(|blocks| blocks.get(1))
        .map_or(BlockTarget::Exit, |next| BlockTarget::Block(next.id))
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

#[cfg(any(test, feature = "test-support"))]
#[cfg(feature = "test-support")]
pub fn lowered_uses_complex_local_paths(module: &VmModule, program: &RegisterProgram) -> bool {
    let uses_complex_local_ref = |ref_idx: u32| {
        matches!(
            module.refs().get(ref_idx as usize),
            Some(super::module::VmRef::Local { path, .. }) if !path.is_empty()
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
            | RegisterInstr::CmpRefConstJumpIf { ref_idx, .. }
                if uses_complex_local_ref(*ref_idx) =>
            {
                return true;
            }
            RegisterInstr::BinaryRefToRef {
                left_ref_idx,
                right_ref_idx,
                dest_ref_idx,
                ..
            } if uses_complex_local_ref(*left_ref_idx)
                || uses_complex_local_ref(*right_ref_idx)
                || uses_complex_local_ref(*dest_ref_idx) =>
            {
                return true;
            }
            RegisterInstr::BinaryRefConstToRef {
                left_ref_idx,
                dest_ref_idx,
                ..
            } if uses_complex_local_ref(*left_ref_idx) || uses_complex_local_ref(*dest_ref_idx) => {
                return true;
            }
            RegisterInstr::BinaryConstRefToRef {
                right_ref_idx,
                dest_ref_idx,
                ..
            } if uses_complex_local_ref(*right_ref_idx)
                || uses_complex_local_ref(*dest_ref_idx) =>
            {
                return true;
            }
            _ => {}
        }
    }
    false
}

pub fn block_index_from_id(
    program: &RegisterProgram,
    block_id: u32,
) -> Result<usize, RuntimeError> {
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

mod prepared;
pub use prepared::*;

fn finish_register_result(
    runtime: &mut impl RegisterContext,
    frames: &mut FrameStack,
    mut frame: super::VmFrame,
    capture_return: bool,
    capture_result_locals: bool,
) -> Result<RegisterPouExecutionResult, RuntimeError> {
    let checked = if capture_return {
        frame
            .locals
            .first()
            .map(|value| {
                runtime
                    .before_value_clone(value)
                    .and_then(|_| runtime.check_frame_return(&frame, value))
            })
            .transpose()
            .map(|_| ())
    } else {
        Ok(())
    };
    let output = checked.map(|()| {
        // The return clone was admitted and lifetime-checked above. Clone only
        // after both checks, matching stack return error precedence.
        let return_value = if capture_return {
            frame.locals.first().cloned()
        } else {
            None
        };
        let locals = if capture_result_locals {
            frames.recycle_frame_presence(&mut frame);
            core::mem::take(&mut frame.locals)
        } else {
            Vec::new()
        };
        RegisterPouExecutionResult {
            return_value,
            locals,
        }
    });
    let cleanup = runtime.retire_frame(&frame);
    match (output, cleanup) {
        (Ok(output), Ok(())) => {
            if !capture_result_locals {
                frames.recycle_frame_payload(frame);
            }
            Ok(output)
        }
        (Err(error), _) => {
            frames.recycle_frame_payload(frame);
            Err(error)
        }
        (Ok(output), Err(error)) => {
            if capture_result_locals {
                frames.recycle_frame_presence(&mut frame);
                let _ = frames.recycle_returned_locals(output.locals);
            } else {
                frames.recycle_frame_payload(frame);
            }
            Err(error)
        }
    }
}

#[cfg(test)]
mod tests;
