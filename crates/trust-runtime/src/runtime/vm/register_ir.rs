use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};
use std::sync::Arc;
use std::time::Instant;

use crate::debug::DebugHook;
use crate::error::RuntimeError;
use crate::execution_backend::{
    VmRegisterCallOpCounters, VmRegisterFallbackReason, VmRegisterHotBlock,
    VmRegisterLoweringCacheSnapshot, VmRegisterProfileSnapshot, VmRegisterRefOpCounters,
    VmRegisterValueOpCounters, VmTier1SpecializedExecutorCompileFailureReason,
    VmTier1SpecializedExecutorDeoptReason, VmTier1SpecializedExecutorSnapshot,
};
use crate::memory::InstanceId;
use crate::program_model::{apply_binary, apply_unary, BinaryOp, UnaryOp};
use crate::value::{size_of_value, Value};

use super::super::core::Runtime;
use super::call::{execute_native_call, push_call_frame};
use super::dispatch_refs::{
    dynamic_load_ref, dynamic_ref_field, dynamic_ref_field_borrowed, dynamic_ref_index,
    dynamic_store_ref, index_to_i64, load_ref_addr, peek_ref, store_ref,
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
    static VM_REGISTER_FRAME_STACK_POOL: RefCell<Vec<FrameStack>> = const { RefCell::new(Vec::new()) };
}

#[derive(Debug)]
struct RegisterExecutionBuffers {
    frames: Option<FrameStack>,
    registers: Option<Vec<Value>>,
    remaining_register_reads: Option<Vec<u32>>,
    native_call_stack: Option<OperandStack>,
}

impl RegisterExecutionBuffers {
    fn acquire(max_registers: usize) -> Self {
        let frames = VM_REGISTER_FRAME_STACK_POOL
            .with(|pool| pool.borrow_mut().pop())
            .unwrap_or_default();
        let mut registers = VM_REGISTER_FILE_POOL
            .with(|pool| pool.borrow_mut().pop())
            .unwrap_or_default();
        prepare_register_file(&mut registers, max_registers);
        let mut remaining_register_reads = VM_REGISTER_READ_COUNTS_POOL
            .with(|pool| pool.borrow_mut().pop())
            .unwrap_or_default();
        remaining_register_reads.resize(max_registers, 0);
        let native_call_stack = VM_REGISTER_NATIVE_CALL_STACK_POOL
            .with(|pool| pool.borrow_mut().pop())
            .unwrap_or_default();
        Self {
            frames: Some(frames),
            registers: Some(registers),
            remaining_register_reads: Some(remaining_register_reads),
            native_call_stack: Some(native_call_stack),
        }
    }

    fn buffers_mut(&mut self) -> (&mut FrameStack, &mut [Value], &mut [u32], &mut OperandStack) {
        let frames = self
            .frames
            .as_mut()
            .expect("register execution buffers missing frame stack");
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
            frames,
            registers.as_mut_slice(),
            remaining_register_reads.as_mut_slice(),
            native_call_stack,
        )
    }
}

impl Drop for RegisterExecutionBuffers {
    fn drop(&mut self) {
        if let Some(mut frames) = self.frames.take() {
            frames.clear();
            VM_REGISTER_FRAME_STACK_POOL.with(|pool| {
                let mut pool = pool.borrow_mut();
                if pool.len() < REGISTER_EXECUTION_POOL_LIMIT {
                    pool.push(frames);
                }
            });
        }
        if let Some(mut registers) = self.registers.take() {
            reset_register_file(&mut registers);
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

fn prepare_register_file(registers: &mut Vec<Value>, max_registers: usize) {
    registers.resize(max_registers, Value::Null);
}

fn reset_register_file(registers: &mut [Value]) {
    for slot in registers {
        if !matches!(slot, Value::Null) {
            *slot = Value::Null;
        }
    }
}

mod interpreter;
mod lower;
mod profile;
mod registers;
#[cfg(test)]
mod tests;
mod tier1;
use self::interpreter::{
    build_register_pou_result, execute_register_block_interpreted, BorrowedBinaryEval,
    RegisterBlockExecutionOutcome, Tier1BlockExecutionOutcome,
};

use self::lower::is_cmp_binary_op;
use self::lower::lower_pou_to_register_ir;
#[cfg(test)]
use self::lower::{
    collect_block_leaders, compute_block_entry_stack_depths, decode_pou,
    fuse_register_block_instructions, instruction_reads_register, normalize_stack_for_block_exit,
    verify_register_program,
};
use self::profile::{CachedRegisterProgram, RegisterLoweringCacheEntry};
pub(in crate::runtime::vm) use self::profile::{
    RegisterCallOpKind, RegisterRefOpKind, RegisterValueOpKind,
};
pub(in crate::runtime) use self::profile::{RegisterLoweringCacheState, RegisterProfileState};
use self::registers::*;
pub(in crate::runtime) use self::tier1::RegisterTier1SpecializedExecutorState;
#[cfg(test)]
use self::tier1::{
    apply_dint_binary_guard_borrowed, compile_tier1_block, execute_tier1_compiled_block,
    parse_tier1_env_bool, parse_tier1_env_usize, tier1_block_key, Tier1CompiledBlock,
    Tier1CompiledInstr,
};
use self::tier1::{maybe_execute_tier1_block, prepare_borrowed_binary_eval};

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
    let mut register_execution_buffers =
        RegisterExecutionBuffers::acquire(program.max_registers as usize);
    let (frames, registers, remaining_register_reads, native_call_stack) =
        register_execution_buffers.buffers_mut();
    let _ = push_call_frame(frames, module, program.pou_id, usize::MAX, entry_instance)
        .map_err(VmTrap::into_runtime_error)?;
    runtime
        .vm_register_profile
        .record_call_op(RegisterCallOpKind::FramePush);
    let profile_enabled = runtime.vm_register_profile.is_enabled();
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
    {
        let frame = frames
            .current_mut()
            .ok_or_else(|| VmTrap::CallStackUnderflow.into_runtime_error())?;
        super::local_init::initialize_declared_locals(runtime, module, frame)?;
    }
    let mut current_block = program.entry_block;
    let mut local_budget = module.instruction_budget;
    let budget = shared_budget.unwrap_or(&mut local_budget);
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
        if block.id != current_block {
            return Err(invalid_bytecode(format!(
                "register-ir executor resolved block id {current_block} to index {block_index} containing block id {}",
                block.id
            )));
        }
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
                let storage = &runtime.storage;
                let current_time = runtime.current_time;
                if let Some(debug) = runtime.debug.as_mut() {
                    let call_depth =
                        depth_offset.saturating_add(frames.len().saturating_sub(1) as u32);
                    debug.refresh_snapshot_from_storage(storage, current_time);
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
                frames,
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
                    frames,
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
                frames,
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
                    None => next_linear_block_target(program, block_index),
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

fn next_linear_block_target(program: &RegisterProgram, block_index: usize) -> BlockTarget {
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

#[cfg(test)]
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
    instruction_index == 0 || instruction_index.is_multiple_of(REGISTER_DEADLINE_CHECK_STRIDE)
}
