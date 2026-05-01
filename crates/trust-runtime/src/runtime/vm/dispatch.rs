use std::cell::RefCell;
use std::time::Instant;

use smol_str::SmolStr;

use crate::debug::DebugHook;
use crate::error::RuntimeError;
use crate::memory::InstanceId;
use crate::program_model::{BinaryOp, UnaryOp};
use crate::task::ProgramDef;
use crate::value::{
    read_partial_access, size_of_value, write_partial_access, PartialAccess, PartialAccessError,
    Value, ValueRef,
};

use super::super::core::Runtime;
use super::call::{execute_native_call, push_call_frame};
use super::dispatch_ops::{apply_jump, execute_binary, execute_unary, read_i32, read_u32};
use super::dispatch_refs::{
    dynamic_load_ref, dynamic_ref_field, dynamic_ref_index, dynamic_store_ref, index_to_i64,
    load_ref, load_ref_addr, pop_reference, store_ref,
};
use super::dispatch_sizeof::{sizeof_error_to_runtime, sizeof_type_from_table};
use super::errors::VmTrap;
use super::frames::{ensure_global_call_depth, FrameStack};
use super::register_ir::{try_execute_pou_with_register_ir, RegisterExecutionOutcome};
use super::stack::OperandStack;
use super::VmModule;

#[derive(Debug, Clone)]
pub(super) struct VmPouStackResult {
    pub(super) return_value: Option<Value>,
    pub(super) locals: Vec<Value>,
}

const STACK_DEADLINE_CHECK_STRIDE: usize = 32;
const VM_EXECUTION_POOL_LIMIT: usize = 64;

thread_local! {
    static VM_OPERAND_STACK_POOL: RefCell<Vec<OperandStack>> = const { RefCell::new(Vec::new()) };
    static VM_FRAME_STACK_POOL: RefCell<Vec<FrameStack>> = const { RefCell::new(Vec::new()) };
}

#[derive(Debug)]
struct VmExecutionBuffers {
    operand_stack: Option<OperandStack>,
    frames: Option<FrameStack>,
}

impl VmExecutionBuffers {
    fn acquire() -> Self {
        let operand_stack = VM_OPERAND_STACK_POOL
            .with(|pool| pool.borrow_mut().pop())
            .unwrap_or_default();
        let frames = VM_FRAME_STACK_POOL
            .with(|pool| pool.borrow_mut().pop())
            .unwrap_or_default();
        Self {
            operand_stack: Some(operand_stack),
            frames: Some(frames),
        }
    }

    fn stacks_mut(&mut self) -> (&mut OperandStack, &mut FrameStack) {
        let operand_stack = self
            .operand_stack
            .as_mut()
            .expect("vm execution buffers missing operand stack");
        let frames = self
            .frames
            .as_mut()
            .expect("vm execution buffers missing frame stack");
        (operand_stack, frames)
    }
}

impl Drop for VmExecutionBuffers {
    fn drop(&mut self) {
        if let Some(mut operand_stack) = self.operand_stack.take() {
            operand_stack.clear();
            VM_OPERAND_STACK_POOL.with(|pool| {
                let mut pool = pool.borrow_mut();
                if pool.len() < VM_EXECUTION_POOL_LIMIT {
                    pool.push(operand_stack);
                }
            });
        }
        if let Some(mut frames) = self.frames.take() {
            frames.clear();
            VM_FRAME_STACK_POOL.with(|pool| {
                let mut pool = pool.borrow_mut();
                if pool.len() < VM_EXECUTION_POOL_LIMIT {
                    pool.push(frames);
                }
            });
        }
    }
}

pub(super) fn execute_program(
    runtime: &mut Runtime,
    program: &ProgramDef,
) -> Result<(), RuntimeError> {
    execute_program_by_name(runtime, &program.name)
}

pub(super) fn execute_program_by_name(
    runtime: &mut Runtime,
    program_name: &SmolStr,
) -> Result<(), RuntimeError> {
    let module = runtime.vm_module.clone().ok_or_else(|| {
        RuntimeError::InvalidConfig(
            "runtime.execution_backend='vm' requires loaded bytecode module".into(),
        )
    })?;

    let key = SmolStr::new(program_name.to_ascii_uppercase());
    let pou_id = module
        .program_ids
        .get(&key)
        .copied()
        .ok_or_else(|| VmTrap::MissingProgram(program_name.clone()).into_runtime_error())?;

    let instance_id = match runtime.storage.get_global(program_name.as_ref()) {
        Some(Value::Instance(id)) => Some(*id),
        _ => None,
    };

    execute_pou(runtime, module.as_ref(), pou_id, instance_id)
}

pub(super) fn execute_function_block_ref(
    runtime: &mut Runtime,
    reference: &ValueRef,
) -> Result<(), RuntimeError> {
    let module = runtime.vm_module.clone().ok_or_else(|| {
        RuntimeError::InvalidConfig(
            "runtime.execution_backend='vm' requires loaded bytecode module".into(),
        )
    })?;

    let instance_id = match runtime.storage.read_by_ref_ref(reference) {
        Some(Value::Instance(id)) => *id,
        Some(_) => return Err(RuntimeError::TypeMismatch),
        None => return Err(RuntimeError::NullReference),
    };

    let instance = runtime
        .storage
        .get_instance(instance_id)
        .ok_or(RuntimeError::NullReference)?;
    let key = SmolStr::new(instance.type_name.to_ascii_uppercase());
    let pou_id = module
        .function_block_ids
        .get(&key)
        .copied()
        .ok_or_else(|| {
            VmTrap::MissingFunctionBlock(instance.type_name.clone()).into_runtime_error()
        })?;

    execute_pou(runtime, module.as_ref(), pou_id, Some(instance_id))
}

fn execute_pou(
    runtime: &mut Runtime,
    module: &VmModule,
    pou_id: u32,
    entry_instance: Option<InstanceId>,
) -> Result<(), RuntimeError> {
    match try_execute_pou_with_register_ir(runtime, module, pou_id, entry_instance)? {
        RegisterExecutionOutcome::Executed => Ok(()),
        RegisterExecutionOutcome::FallbackToStack => {
            execute_pou_stack(runtime, module, pou_id, entry_instance)
        }
    }
}

fn execute_pou_stack(
    runtime: &mut Runtime,
    module: &VmModule,
    pou_id: u32,
    entry_instance: Option<InstanceId>,
) -> Result<(), RuntimeError> {
    let _ = execute_pou_stack_with_locals(
        runtime,
        module,
        pou_id,
        entry_instance,
        None,
        false,
        0,
        None,
    )?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn execute_pou_stack_with_locals(
    runtime: &mut Runtime,
    module: &VmModule,
    pou_id: u32,
    entry_instance: Option<InstanceId>,
    initial_locals: Option<&[Value]>,
    capture_return: bool,
    depth_offset: u32,
    shared_budget: Option<&mut usize>,
) -> Result<VmPouStackResult, RuntimeError> {
    ensure_global_call_depth(depth_offset, 1).map_err(VmTrap::into_runtime_error)?;
    let mut execution_buffers = VmExecutionBuffers::acquire();
    let (operand_stack, frames) = execution_buffers.stacks_mut();
    let mut pc = push_call_frame(frames, module, pou_id, usize::MAX, entry_instance)
        .map_err(VmTrap::into_runtime_error)?;
    runtime
        .vm_register_profile
        .record_call_op(super::register_ir::RegisterCallOpKind::FramePush);
    if let Some(initial_locals) = initial_locals {
        let frame = frames
            .current_mut()
            .ok_or_else(|| VmTrap::CallStackUnderflow.into_runtime_error())?;
        if initial_locals.len() > frame.locals.len() {
            return Err(VmTrap::BytecodeDecode(
                "vm call initial local payload exceeds frame local capacity".into(),
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
    let mut local_budget = module.instruction_budget;
    let budget = shared_budget.unwrap_or(&mut local_budget);
    let mut instruction_count = 0usize;

    loop {
        if frames.is_empty() {
            return Ok(VmPouStackResult {
                return_value: None,
                locals: Vec::new(),
            });
        }

        let (frame_pou_id, frame_start, frame_end) = {
            let frame = frames
                .current()
                .ok_or_else(|| VmTrap::CallStackUnderflow.into_runtime_error())?;
            (frame.pou_id, frame.code_start, frame.code_end)
        };

        if pc == frame_end {
            let finished = frames.pop().map_err(VmTrap::into_runtime_error)?;
            runtime
                .vm_register_profile
                .record_call_op(super::register_ir::RegisterCallOpKind::FramePop);
            if frames.is_empty() {
                return Ok(build_stack_result(finished, capture_return));
            }
            pc = finished.return_pc;
            continue;
        }

        if pc < frame_start || pc > frame_end {
            return Err(VmTrap::InvalidJumpTarget(pc as i64).into_runtime_error());
        }

        if should_check_stack_deadline(instruction_count)
            && deadline_exceeded(runtime.execution_deadline)
        {
            return Err(VmTrap::DeadlineExceeded.into_runtime_error());
        }

        if let Some(location) = vm_statement_location(runtime, module, frame_pou_id, pc) {
            let storage = &runtime.storage;
            let current_time = runtime.current_time;
            if let Some(debug) = runtime.debug.as_mut() {
                let call_depth = depth_offset.saturating_add(frames.len().saturating_sub(1) as u32);
                debug.refresh_snapshot_from_storage(storage, current_time);
                debug.on_statement(Some(&location), call_depth);
            }
        }
        instruction_count = instruction_count.saturating_add(1);

        let opcode = module
            .code
            .get(pc)
            .copied()
            .ok_or_else(|| VmTrap::BytecodeDecode("vm instruction fetch out of bounds".into()))
            .map_err(VmTrap::into_runtime_error)?;
        pc += 1;

        match opcode {
            0x00 => {}
            0x01 => return Err(VmTrap::ForStepZero.into_runtime_error()),
            0x02 => {
                let offset = read_i32(&module.code, &mut pc).map_err(VmTrap::into_runtime_error)?;
                let jump_origin = pc;
                let frame = frames
                    .current()
                    .ok_or_else(|| VmTrap::CallStackUnderflow.into_runtime_error())?;
                apply_jump(&mut pc, offset, frame).map_err(VmTrap::into_runtime_error)?;
                if pc < jump_origin {
                    consume_loop_budget(budget)?;
                }
            }
            0x03 | 0x04 => {
                let offset = read_i32(&module.code, &mut pc).map_err(VmTrap::into_runtime_error)?;
                let jump_origin = pc;
                let condition = operand_stack.pop().map_err(VmTrap::into_runtime_error)?;
                let condition = match condition {
                    Value::Bool(value) => value,
                    _ => return Err(VmTrap::ConditionNotBool.into_runtime_error()),
                };
                let should_jump = (opcode == 0x03 && condition) || (opcode == 0x04 && !condition);
                if should_jump {
                    let frame = frames
                        .current()
                        .ok_or_else(|| VmTrap::CallStackUnderflow.into_runtime_error())?;
                    apply_jump(&mut pc, offset, frame).map_err(VmTrap::into_runtime_error)?;
                    if pc < jump_origin {
                        consume_loop_budget(budget)?;
                    }
                }
            }
            0x05 => {
                let callee = read_u32(&module.code, &mut pc).map_err(VmTrap::into_runtime_error)?;
                let inherited_instance = frames.current().and_then(|frame| frame.runtime_instance);
                let return_pc = pc;
                ensure_global_call_depth(depth_offset, frames.len().saturating_add(1))
                    .map_err(VmTrap::into_runtime_error)?;
                pc = push_call_frame(frames, module, callee, return_pc, inherited_instance)
                    .map_err(VmTrap::into_runtime_error)?;
                runtime
                    .vm_register_profile
                    .record_call_op(super::register_ir::RegisterCallOpKind::FramePush);
            }
            0x06 => {
                let finished = frames.pop().map_err(VmTrap::into_runtime_error)?;
                runtime
                    .vm_register_profile
                    .record_call_op(super::register_ir::RegisterCallOpKind::FramePop);
                if frames.is_empty() {
                    return Ok(build_stack_result(finished, capture_return));
                }
                pc = finished.return_pc;
            }
            0x07 => return Err(VmTrap::UnsupportedOpcode("CALL_METHOD").into_runtime_error()),
            0x08 => return Err(VmTrap::UnsupportedOpcode("CALL_VIRTUAL").into_runtime_error()),
            0x09 => {
                let kind = read_u32(&module.code, &mut pc).map_err(VmTrap::into_runtime_error)?;
                let symbol_idx =
                    read_u32(&module.code, &mut pc).map_err(VmTrap::into_runtime_error)?;
                let arg_count =
                    read_u32(&module.code, &mut pc).map_err(VmTrap::into_runtime_error)?;
                let caller_depth =
                    depth_offset.saturating_add(frames.len().saturating_sub(1) as u32);
                let frame = frames
                    .current_mut()
                    .ok_or_else(|| VmTrap::CallStackUnderflow.into_runtime_error())?;
                let result = execute_native_call(
                    runtime,
                    module,
                    frame,
                    operand_stack,
                    caller_depth,
                    budget,
                    kind,
                    symbol_idx,
                    arg_count,
                )
                .map_err(VmTrap::into_runtime_error)?;
                operand_stack
                    .push(result)
                    .map_err(VmTrap::into_runtime_error)?;
            }
            0x10 => {
                let const_idx =
                    read_u32(&module.code, &mut pc).map_err(VmTrap::into_runtime_error)?;
                let value = module
                    .consts
                    .get(const_idx as usize)
                    .cloned()
                    .ok_or(VmTrap::InvalidConstIndex(const_idx))
                    .map_err(VmTrap::into_runtime_error)?;
                operand_stack
                    .push(value)
                    .map_err(VmTrap::into_runtime_error)?;
            }
            0x11 => operand_stack
                .duplicate_top()
                .map_err(VmTrap::into_runtime_error)?,
            0x12 => {
                let _ = operand_stack.pop().map_err(VmTrap::into_runtime_error)?;
            }
            0x13 => operand_stack
                .swap_top()
                .map_err(VmTrap::into_runtime_error)?,
            0x14 => return Err(VmTrap::UnsupportedOpcode("ROT3").into_runtime_error()),
            0x15 => return Err(VmTrap::UnsupportedOpcode("ROT4").into_runtime_error()),
            0x16 => return Err(VmTrap::UnsupportedOpcode("CAST_IMPLICIT").into_runtime_error()),
            0x20 => {
                let ref_idx =
                    read_u32(&module.code, &mut pc).map_err(VmTrap::into_runtime_error)?;
                let value = load_ref(runtime, module, frames, ref_idx)
                    .map_err(VmTrap::into_runtime_error)?;
                operand_stack
                    .push(value)
                    .map_err(VmTrap::into_runtime_error)?;
            }
            0x21 => {
                let ref_idx =
                    read_u32(&module.code, &mut pc).map_err(VmTrap::into_runtime_error)?;
                let value = operand_stack.pop().map_err(VmTrap::into_runtime_error)?;
                store_ref(runtime, module, frames, ref_idx, value)
                    .map_err(VmTrap::into_runtime_error)?;
            }
            0x22 => {
                let ref_idx =
                    read_u32(&module.code, &mut pc).map_err(VmTrap::into_runtime_error)?;
                let value_ref =
                    load_ref_addr(module, frames, ref_idx).map_err(VmTrap::into_runtime_error)?;
                operand_stack
                    .push(Value::Reference(Some(value_ref)))
                    .map_err(VmTrap::into_runtime_error)?;
            }
            0x23 => {
                let frame = frames
                    .current()
                    .ok_or_else(|| VmTrap::CallStackUnderflow.into_runtime_error())?;
                let self_instance = frame.runtime_instance.ok_or_else(|| {
                    VmTrap::Runtime(RuntimeError::TypeMismatch).into_runtime_error()
                })?;
                operand_stack
                    .push(Value::Instance(self_instance))
                    .map_err(VmTrap::into_runtime_error)?;
            }
            0x24 => {
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
                operand_stack
                    .push(Value::Instance(super_instance))
                    .map_err(VmTrap::into_runtime_error)?;
            }
            0x25 => {
                operand_stack
                    .push(Value::Null)
                    .map_err(VmTrap::into_runtime_error)?;
            }
            0x30 => {
                let field_idx =
                    read_u32(&module.code, &mut pc).map_err(VmTrap::into_runtime_error)?;
                let field = module
                    .strings
                    .get(field_idx as usize)
                    .cloned()
                    .ok_or_else(|| {
                        VmTrap::BytecodeDecode(
                            format!("invalid index {field_idx} for string").into(),
                        )
                        .into_runtime_error()
                    })?;
                let base = operand_stack.pop().map_err(VmTrap::into_runtime_error)?;
                let next = match base {
                    Value::Reference(Some(reference)) => {
                        dynamic_ref_field(runtime, frames, reference, field.clone())
                            .map_err(VmTrap::into_runtime_error)?
                    }
                    Value::Reference(None) => {
                        return Err(VmTrap::NullReference.into_runtime_error());
                    }
                    Value::Instance(instance_id) => runtime
                        .storage
                        .ref_for_instance_recursive(instance_id, field.as_str())
                        .ok_or_else(|| {
                            VmTrap::Runtime(RuntimeError::UndefinedField(field))
                                .into_runtime_error()
                        })?,
                    _ => {
                        return Err(VmTrap::Runtime(RuntimeError::TypeMismatch).into_runtime_error())
                    }
                };
                operand_stack
                    .push(Value::Reference(Some(next)))
                    .map_err(VmTrap::into_runtime_error)?;
            }
            0x31 => {
                let index = operand_stack.pop().map_err(VmTrap::into_runtime_error)?;
                let index = index_to_i64(index).map_err(VmTrap::into_runtime_error)?;
                let reference = pop_reference(operand_stack).map_err(VmTrap::into_runtime_error)?;
                let next = dynamic_ref_index(runtime, frames, reference, index)
                    .map_err(VmTrap::into_runtime_error)?;
                operand_stack
                    .push(Value::Reference(Some(next)))
                    .map_err(VmTrap::into_runtime_error)?;
            }
            0x32 => {
                let reference = pop_reference(operand_stack).map_err(VmTrap::into_runtime_error)?;
                let value = dynamic_load_ref(runtime, frames, &reference)
                    .map_err(VmTrap::into_runtime_error)?;
                operand_stack
                    .push(value)
                    .map_err(VmTrap::into_runtime_error)?;
            }
            0x33 => {
                let value = operand_stack.pop().map_err(VmTrap::into_runtime_error)?;
                let reference = pop_reference(operand_stack).map_err(VmTrap::into_runtime_error)?;
                dynamic_store_ref(runtime, frames, &reference, value)
                    .map_err(VmTrap::into_runtime_error)?;
            }
            0x40 => execute_binary(&runtime.profile, operand_stack, BinaryOp::Add)
                .map_err(VmTrap::into_runtime_error)?,
            0x41 => execute_binary(&runtime.profile, operand_stack, BinaryOp::Sub)
                .map_err(VmTrap::into_runtime_error)?,
            0x42 => execute_binary(&runtime.profile, operand_stack, BinaryOp::Mul)
                .map_err(VmTrap::into_runtime_error)?,
            0x43 => execute_binary(&runtime.profile, operand_stack, BinaryOp::Div)
                .map_err(VmTrap::into_runtime_error)?,
            0x44 => execute_binary(&runtime.profile, operand_stack, BinaryOp::Mod)
                .map_err(VmTrap::into_runtime_error)?,
            0x45 => {
                execute_unary(operand_stack, UnaryOp::Neg).map_err(VmTrap::into_runtime_error)?
            }
            0x46 => execute_binary(&runtime.profile, operand_stack, BinaryOp::And)
                .map_err(VmTrap::into_runtime_error)?,
            0x47 => execute_binary(&runtime.profile, operand_stack, BinaryOp::Or)
                .map_err(VmTrap::into_runtime_error)?,
            0x48 => execute_binary(&runtime.profile, operand_stack, BinaryOp::Xor)
                .map_err(VmTrap::into_runtime_error)?,
            0x49 => {
                execute_unary(operand_stack, UnaryOp::Not).map_err(VmTrap::into_runtime_error)?
            }
            0x4A => return Err(VmTrap::UnsupportedOpcode("SHL").into_runtime_error()),
            0x4B => return Err(VmTrap::UnsupportedOpcode("SHR").into_runtime_error()),
            0x4C => execute_binary(&runtime.profile, operand_stack, BinaryOp::Pow)
                .map_err(VmTrap::into_runtime_error)?,
            0x4D => return Err(VmTrap::UnsupportedOpcode("ROL").into_runtime_error()),
            0x4E => return Err(VmTrap::UnsupportedOpcode("ROR").into_runtime_error()),
            0x50 => execute_binary(&runtime.profile, operand_stack, BinaryOp::Eq)
                .map_err(VmTrap::into_runtime_error)?,
            0x51 => execute_binary(&runtime.profile, operand_stack, BinaryOp::Ne)
                .map_err(VmTrap::into_runtime_error)?,
            0x52 => execute_binary(&runtime.profile, operand_stack, BinaryOp::Lt)
                .map_err(VmTrap::into_runtime_error)?,
            0x53 => execute_binary(&runtime.profile, operand_stack, BinaryOp::Le)
                .map_err(VmTrap::into_runtime_error)?,
            0x54 => execute_binary(&runtime.profile, operand_stack, BinaryOp::Gt)
                .map_err(VmTrap::into_runtime_error)?,
            0x55 => execute_binary(&runtime.profile, operand_stack, BinaryOp::Ge)
                .map_err(VmTrap::into_runtime_error)?,
            0x60 => {
                let type_idx =
                    read_u32(&module.code, &mut pc).map_err(VmTrap::into_runtime_error)?;
                let size = sizeof_type_from_table(&module.types, type_idx)
                    .map_err(|err| VmTrap::Runtime(err).into_runtime_error())?;
                let size = i32::try_from(size)
                    .map_err(|_| VmTrap::Runtime(RuntimeError::Overflow).into_runtime_error())?;
                operand_stack
                    .push(Value::DInt(size))
                    .map_err(VmTrap::into_runtime_error)?;
            }
            0x61 => {
                let value = operand_stack.pop().map_err(VmTrap::into_runtime_error)?;
                let size = size_of_value(runtime.registry(), &value)
                    .map_err(sizeof_error_to_runtime)
                    .map_err(|err| VmTrap::Runtime(err).into_runtime_error())?;
                let size = i32::try_from(size)
                    .map_err(|_| VmTrap::Runtime(RuntimeError::Overflow).into_runtime_error())?;
                operand_stack
                    .push(Value::DInt(size))
                    .map_err(VmTrap::into_runtime_error)?;
            }
            0x62 => {
                let operand =
                    read_u32(&module.code, &mut pc).map_err(VmTrap::into_runtime_error)?;
                let access = decode_partial_access(operand)
                    .map_err(|err| VmTrap::Runtime(err).into_runtime_error())?;
                let target = operand_stack.pop().map_err(VmTrap::into_runtime_error)?;
                let result = read_partial_access(&target, access)
                    .map_err(partial_access_error_to_runtime)
                    .map_err(|err| VmTrap::Runtime(err).into_runtime_error())?;
                operand_stack
                    .push(result)
                    .map_err(VmTrap::into_runtime_error)?;
            }
            0x63 => {
                let operand =
                    read_u32(&module.code, &mut pc).map_err(VmTrap::into_runtime_error)?;
                let access = decode_partial_access(operand)
                    .map_err(|err| VmTrap::Runtime(err).into_runtime_error())?;
                let value = operand_stack.pop().map_err(VmTrap::into_runtime_error)?;
                let target = operand_stack.pop().map_err(VmTrap::into_runtime_error)?;
                let updated = write_partial_access(target, access, value)
                    .map_err(partial_access_error_to_runtime)
                    .map_err(|err| VmTrap::Runtime(err).into_runtime_error())?;
                operand_stack
                    .push(updated)
                    .map_err(VmTrap::into_runtime_error)?;
            }
            0x70 => {
                let _debug_idx =
                    read_u32(&module.code, &mut pc).map_err(VmTrap::into_runtime_error)?;
            }
            _ => return Err(VmTrap::InvalidOpcode(opcode).into_runtime_error()),
        }
    }
}

fn build_stack_result(frame: super::frames::VmFrame, capture_return: bool) -> VmPouStackResult {
    let return_value = if capture_return {
        frame.locals.first().cloned()
    } else {
        None
    };
    VmPouStackResult {
        return_value,
        locals: frame.locals,
    }
}

fn consume_loop_budget(budget: &mut usize) -> Result<(), RuntimeError> {
    if *budget == 0 {
        return Err(VmTrap::BudgetExceeded.into_runtime_error());
    }
    *budget = budget.saturating_sub(1);
    Ok(())
}

fn deadline_exceeded(deadline: Option<Instant>) -> bool {
    match deadline {
        Some(deadline) => Instant::now() >= deadline,
        None => false,
    }
}

fn should_check_stack_deadline(instruction_count: usize) -> bool {
    instruction_count == 0 || instruction_count.is_multiple_of(STACK_DEADLINE_CHECK_STRIDE)
}

fn vm_statement_location(
    runtime: &Runtime,
    module: &VmModule,
    pou_id: u32,
    pc: usize,
) -> Option<crate::debug::SourceLocation> {
    let source = module.debug_map.source_by_pc.get(&(pou_id, pc as u32))?;
    runtime.resolve_vm_debug_location(source.file.as_str(), source.line, source.column)
}

fn decode_partial_access(operand: u32) -> Result<PartialAccess, RuntimeError> {
    if (operand & !0x3FF) != 0 {
        return Err(RuntimeError::TypeMismatch);
    }
    let kind = (operand >> 8) & 0x03;
    let index = (operand & 0xFF) as u8;
    match kind {
        0 => Ok(PartialAccess::Bit(index)),
        1 => Ok(PartialAccess::Byte(index)),
        2 => Ok(PartialAccess::Word(index)),
        3 => Ok(PartialAccess::DWord(index)),
        _ => Err(RuntimeError::TypeMismatch),
    }
}

fn partial_access_error_to_runtime(err: PartialAccessError) -> RuntimeError {
    match err {
        PartialAccessError::IndexOutOfBounds {
            index,
            lower,
            upper,
        } => RuntimeError::IndexOutOfBounds {
            index,
            lower,
            upper,
        },
        PartialAccessError::TypeMismatch => RuntimeError::TypeMismatch,
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn stack_deadline_stride_checks_first_and_stride_boundaries() {
        assert!(super::should_check_stack_deadline(0));
        assert!(!super::should_check_stack_deadline(1));
        assert!(super::should_check_stack_deadline(
            super::STACK_DEADLINE_CHECK_STRIDE
        ));
        assert!(super::should_check_stack_deadline(
            super::STACK_DEADLINE_CHECK_STRIDE * 2
        ));
    }
}
