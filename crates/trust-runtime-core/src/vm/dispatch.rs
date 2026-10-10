//! Shared stack-bytecode dispatcher. Platform services enter through ExecutionContext.
use super::call::context::RegisterCallOpKind;
use super::call::{execute_native_call, push_call_frame};
use super::context::ExecutionContext;
use super::dispatch_ops::{apply_jump, execute_binary, execute_unary, read_i32, read_u32};
use super::dispatch_refs::{
    dynamic_load_ref, dynamic_ref_field, dynamic_ref_index, dynamic_store_ref, index_to_i64,
    load_ref, load_ref_addr, pop_reference, store_ref,
};
use super::dispatch_sizeof::sizeof_type_from_table_with;
use super::module::VmModule;
use super::reference_attempt::apply_reference_attempt;
use super::{ensure_global_call_depth, FrameStack, OperandStack, VmTrap};
use crate::error::RuntimeError;
use crate::memory::InstanceId;
use crate::program_model::{BinaryOp, UnaryOp};
use crate::value::{
    read_partial_access, write_partial_access, PartialAccess, PartialAccessError, Value,
};
use alloc::{format, vec::Vec};

/// Returned value and final local slots from one stack invocation.
#[derive(Debug, Clone)]
pub struct VmPouStackResult {
    /// Optional function return value.
    pub return_value: Option<Value>,
    /// Final or initially bound local values in slot order.
    pub locals: Vec<Value>,
}

/// Reusable operand and frame buffers owned by an execution context.
#[derive(Debug, Default)]
pub struct ExecutionBuffers {
    /// Reusable operand storage, cleared between root entries.
    pub operand_stack: OperandStack,
    /// Reusable stack-frame storage, cleared between root entries.
    pub frames: FrameStack,
}

/// Execute one POU through the shared stack dispatcher with optional initial locals.
#[allow(clippy::too_many_arguments)]
pub fn execute_pou_stack_with_locals(
    runtime: &mut impl ExecutionContext,
    module: &VmModule,
    pou_id: u32,
    entry_instance: Option<InstanceId>,
    initial_locals: Option<&[Value]>,
    capture_return: bool,
    depth_offset: u32,
    entry: super::budget::ExecutionEntry,
) -> Result<VmPouStackResult, RuntimeError> {
    execute_pou_stack_with_parameter_presence(
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

/// Execute with explicit parameter presence, distinguishing omitted defaults from supplied NULL.
#[allow(clippy::too_many_arguments)]
pub fn execute_pou_stack_with_parameter_presence(
    runtime: &mut impl ExecutionContext,
    module: &VmModule,
    pou_id: u32,
    entry_instance: Option<InstanceId>,
    initial_locals: Option<&[Value]>,
    parameter_values_present: Option<&[bool]>,
    capture_return: bool,
    depth_offset: u32,
    entry: super::budget::ExecutionEntry,
) -> Result<VmPouStackResult, RuntimeError> {
    let mut buffers = runtime.take_execution_buffers();
    let result = execute_with_buffers(
        runtime,
        module,
        Some(pou_id),
        entry_instance,
        initial_locals,
        capture_return,
        depth_offset,
        entry,
        &mut buffers,
        None,
        parameter_values_present,
    );
    buffers.operand_stack.clear();
    while let Ok(frame) = buffers.frames.pop() {
        runtime.retire_frame(&frame);
    }
    runtime.recycle_execution_buffers(buffers);
    if result.is_ok() {
        runtime.check_execution_deadline()?;
    }
    result
}

/// Execute an initializer range with the caller's lexical locals through the same loop.
/// The caller retains activation ownership and restores returned locals on completion.
pub fn execute_initializer_body(
    runtime: &mut impl ExecutionContext,
    module: &VmModule,
    frame: &mut super::VmFrame,
    depth: u32,
) -> Result<(), RuntimeError> {
    let mut buffers = runtime.take_execution_buffers();
    let locals = core::mem::take(&mut frame.locals);
    let mut staged_frame = frame.clone();
    staged_frame.locals = locals;
    let result = execute_with_buffers(
        runtime,
        module,
        frame.pou_id,
        frame.runtime_instance,
        None,
        false,
        depth,
        super::budget::ExecutionEntry::Nested,
        &mut buffers,
        Some(staged_frame),
        None,
    );
    let outcome = match result {
        Ok(result) => {
            frame.locals = result.locals;
            Ok(())
        }
        Err(error) => {
            // Validated initializer bodies cannot issue direct/user calls. Preserve
            // lexical locals on traps, including a failing coercion callback.
            if let Ok(active) = buffers.frames.pop() {
                frame.locals = active.locals;
            }
            Err(error)
        }
    };
    buffers.operand_stack.clear();
    buffers.frames.clear();
    runtime.recycle_execution_buffers(buffers);
    if outcome.is_ok() {
        runtime.check_execution_deadline()?;
    }
    outcome
}

#[allow(clippy::too_many_arguments)]
fn execute_with_buffers(
    runtime: &mut impl ExecutionContext,
    module: &VmModule,
    pou_id: Option<u32>,
    entry_instance: Option<InstanceId>,
    initial_locals: Option<&[Value]>,
    capture_return: bool,
    depth_offset: u32,
    entry: super::budget::ExecutionEntry,
    buffers: &mut ExecutionBuffers,
    initializer_frame: Option<super::VmFrame>,
    parameter_values_present: Option<&[bool]>,
) -> Result<VmPouStackResult, RuntimeError> {
    runtime.begin_execution(entry, module.instruction_budget)?;
    ensure_global_call_depth(depth_offset, 1).map_err(VmTrap::into_runtime_error)?;
    let operand_stack = &mut buffers.operand_stack;
    let frames = &mut buffers.frames;
    let is_initializer = initializer_frame.is_some();
    let mut pc = if let Some(frame) = initializer_frame {
        let start = frame.code_start;
        frames.push(frame).map_err(VmTrap::into_runtime_error)?;
        start
    } else {
        push_call_frame(
            frames,
            module,
            pou_id.ok_or_else(|| super::module::invalid_bytecode("POU entry has no owner"))?,
            usize::MAX,
            entry_instance,
        )
        .map_err(VmTrap::into_runtime_error)?
    };
    runtime.record_call_op(RegisterCallOpKind::FramePush);
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
        for (index, value) in initial_locals.iter().enumerate() {
            runtime.before_value_clone(value)?;
            frame.locals[index] = value.clone();
        }
    }
    if !is_initializer {
        let frame = frames
            .current_mut()
            .ok_or_else(|| VmTrap::CallStackUnderflow.into_runtime_error())?;
        if let Some(present) = parameter_values_present {
            if present.len() > frame.locals.len() {
                return Err(super::module::invalid_bytecode(
                    "parameter presence exceeds frame capacity",
                ));
            }
            frame.parameter_values_present = present.to_vec();
        }
        runtime.initialize_frame(module, frame, depth_offset)?;
    }

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
            runtime.check_execution_deadline()?;
            let finished = frames.pop().map_err(VmTrap::into_runtime_error)?;
            runtime.record_call_op(RegisterCallOpKind::FramePop);
            if frames.is_empty() {
                return finish_stack_result(runtime, finished, capture_return, !is_initializer);
            }
            runtime.retire_frame(&finished);
            runtime.resume_frame(
                frames
                    .current_mut()
                    .ok_or_else(|| VmTrap::CallStackUnderflow.into_runtime_error())?,
            )?;
            pc = finished.return_pc;
            continue;
        }

        if pc < frame_start || pc > frame_end {
            return Err(VmTrap::InvalidJumpTarget(pc as i64).into_runtime_error());
        }

        runtime.charge_execution_work(1)?;

        let call_depth = depth_offset.saturating_add(frames.len().saturating_sub(1) as u32);
        if let Some(pou_id) = frame_pou_id {
            runtime.on_statement(module, pou_id, pc, call_depth);
        }

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
                let frame = frames
                    .current()
                    .ok_or_else(|| VmTrap::CallStackUnderflow.into_runtime_error())?;
                apply_jump(&mut pc, offset, frame).map_err(VmTrap::into_runtime_error)?;
            }
            0x03 | 0x04 => {
                let offset = read_i32(&module.code, &mut pc).map_err(VmTrap::into_runtime_error)?;
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
                }
            }
            0x05 => {
                runtime.check_execution_deadline()?;
                let callee = read_u32(&module.code, &mut pc).map_err(VmTrap::into_runtime_error)?;
                let inherited_instance = frames.current().and_then(|frame| frame.runtime_instance);
                let return_pc = pc;
                ensure_global_call_depth(depth_offset, frames.len().saturating_add(1))
                    .map_err(VmTrap::into_runtime_error)?;
                runtime.suspend_frame(
                    frames
                        .current_mut()
                        .ok_or_else(|| VmTrap::CallStackUnderflow.into_runtime_error())?,
                )?;
                pc = push_call_frame(frames, module, callee, return_pc, inherited_instance)
                    .map_err(VmTrap::into_runtime_error)?;
                runtime.record_call_op(RegisterCallOpKind::FramePush);
                let depth = depth_offset.saturating_add(frames.len().saturating_sub(1) as u32);
                let frame = frames
                    .current_mut()
                    .ok_or_else(|| VmTrap::CallStackUnderflow.into_runtime_error())?;
                runtime.initialize_frame(module, frame, depth)?;
            }
            0x06 => {
                runtime.check_execution_deadline()?;
                let finished = frames.pop().map_err(VmTrap::into_runtime_error)?;
                runtime.record_call_op(RegisterCallOpKind::FramePop);
                if frames.is_empty() {
                    return finish_stack_result(runtime, finished, capture_return, !is_initializer);
                }
                runtime.retire_frame(&finished);
                runtime.resume_frame(
                    frames
                        .current_mut()
                        .ok_or_else(|| VmTrap::CallStackUnderflow.into_runtime_error())?,
                )?;
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
                    .ok_or(VmTrap::InvalidConstIndex(const_idx))
                    .map_err(VmTrap::into_runtime_error)?;
                runtime.before_value_clone(value)?;
                operand_stack
                    .push(value.clone())
                    .map_err(VmTrap::into_runtime_error)?;
            }
            0x11 => {
                let value = operand_stack.peek().map_err(VmTrap::into_runtime_error)?;
                runtime.before_value_clone(value)?;
                operand_stack
                    .push(value.clone())
                    .map_err(VmTrap::into_runtime_error)?;
            }
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
                let value_ref = load_ref_addr(runtime, module, frames, ref_idx)
                    .map_err(VmTrap::into_runtime_error)?;
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
                    .storage()
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
                        .storage()
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
                dynamic_store_ref(runtime, module, frames, &reference, value)
                    .map_err(VmTrap::into_runtime_error)?;
            }
            0x40 => execute_binary(runtime.profile(), operand_stack, BinaryOp::Add)
                .map_err(VmTrap::into_runtime_error)?,
            0x41 => execute_binary(runtime.profile(), operand_stack, BinaryOp::Sub)
                .map_err(VmTrap::into_runtime_error)?,
            0x42 => execute_binary(runtime.profile(), operand_stack, BinaryOp::Mul)
                .map_err(VmTrap::into_runtime_error)?,
            0x43 => execute_binary(runtime.profile(), operand_stack, BinaryOp::Div)
                .map_err(VmTrap::into_runtime_error)?,
            0x44 => execute_binary(runtime.profile(), operand_stack, BinaryOp::Mod)
                .map_err(VmTrap::into_runtime_error)?,
            0x45 => {
                execute_unary(operand_stack, UnaryOp::Neg).map_err(VmTrap::into_runtime_error)?
            }
            0x46 => execute_binary(runtime.profile(), operand_stack, BinaryOp::And)
                .map_err(VmTrap::into_runtime_error)?,
            0x47 => execute_binary(runtime.profile(), operand_stack, BinaryOp::Or)
                .map_err(VmTrap::into_runtime_error)?,
            0x48 => execute_binary(runtime.profile(), operand_stack, BinaryOp::Xor)
                .map_err(VmTrap::into_runtime_error)?,
            0x49 => {
                execute_unary(operand_stack, UnaryOp::Not).map_err(VmTrap::into_runtime_error)?
            }
            0x4A => return Err(VmTrap::UnsupportedOpcode("SHL").into_runtime_error()),
            0x4B => return Err(VmTrap::UnsupportedOpcode("SHR").into_runtime_error()),
            0x4C => execute_binary(runtime.profile(), operand_stack, BinaryOp::Pow)
                .map_err(VmTrap::into_runtime_error)?,
            0x4D => return Err(VmTrap::UnsupportedOpcode("ROL").into_runtime_error()),
            0x4E => return Err(VmTrap::UnsupportedOpcode("ROR").into_runtime_error()),
            0x50 => execute_binary(runtime.profile(), operand_stack, BinaryOp::Eq)
                .map_err(VmTrap::into_runtime_error)?,
            0x51 => execute_binary(runtime.profile(), operand_stack, BinaryOp::Ne)
                .map_err(VmTrap::into_runtime_error)?,
            0x52 => execute_binary(runtime.profile(), operand_stack, BinaryOp::Lt)
                .map_err(VmTrap::into_runtime_error)?,
            0x53 => execute_binary(runtime.profile(), operand_stack, BinaryOp::Le)
                .map_err(VmTrap::into_runtime_error)?,
            0x54 => execute_binary(runtime.profile(), operand_stack, BinaryOp::Gt)
                .map_err(VmTrap::into_runtime_error)?,
            0x55 => execute_binary(runtime.profile(), operand_stack, BinaryOp::Ge)
                .map_err(VmTrap::into_runtime_error)?,
            0x60 => {
                let type_idx =
                    read_u32(&module.code, &mut pc).map_err(VmTrap::into_runtime_error)?;
                let size = sizeof_type_from_table_with(&module.types, type_idx, &mut |units| {
                    runtime.charge_execution_work(units)
                })
                .map_err(|err| VmTrap::Runtime(err).into_runtime_error())?;
                let size = i32::try_from(size)
                    .map_err(|_| VmTrap::Runtime(RuntimeError::Overflow).into_runtime_error())?;
                operand_stack
                    .push(Value::DInt(size))
                    .map_err(VmTrap::into_runtime_error)?;
            }
            0x61 => {
                let value = operand_stack.pop().map_err(VmTrap::into_runtime_error)?;
                let size = runtime
                    .sizeof_value(&value)
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
            0x64 => {
                let target_type_idx =
                    read_u32(&module.code, &mut pc).map_err(VmTrap::into_runtime_error)?;
                let value = operand_stack.pop().map_err(VmTrap::into_runtime_error)?;
                let frame = frames
                    .current()
                    .ok_or_else(|| VmTrap::CallStackUnderflow.into_runtime_error())?;
                let value = apply_reference_attempt(runtime, module, frame, value, target_type_idx)
                    .map_err(VmTrap::into_runtime_error)?;
                operand_stack
                    .push(value)
                    .map_err(VmTrap::into_runtime_error)?;
            }
            0x65..=0x6C => {
                let operand =
                    read_u32(&module.code, &mut pc).map_err(VmTrap::into_runtime_error)?;
                let depth = depth_offset.saturating_add(frames.len().saturating_sub(1) as u32);
                let frame = frames
                    .current_mut()
                    .ok_or_else(|| VmTrap::CallStackUnderflow.into_runtime_error())?;
                runtime.execute_initialization_opcode(
                    module,
                    frame,
                    opcode,
                    operand,
                    operand_stack,
                    depth,
                )?;
            }
            0x70 => {
                let _debug_idx =
                    read_u32(&module.code, &mut pc).map_err(VmTrap::into_runtime_error)?;
            }
            _ => return Err(VmTrap::InvalidOpcode(opcode).into_runtime_error()),
        }
    }
}

fn finish_stack_result(
    runtime: &mut impl ExecutionContext,
    frame: super::VmFrame,
    capture_return: bool,
    retire: bool,
) -> Result<VmPouStackResult, RuntimeError> {
    let checked_copy = if capture_return {
        frame
            .locals
            .first()
            .map(|value| runtime.before_value_clone(value))
            .transpose()
    } else {
        Ok(None)
    };
    if let Err(error) = checked_copy {
        if retire {
            runtime.retire_frame(&frame);
        }
        return Err(error);
    }
    let return_value = if capture_return {
        frame.locals.first().cloned()
    } else {
        None
    };
    let checked = if let Some(value) = &return_value {
        runtime.check_frame_return(&frame, value)
    } else {
        Ok(())
    };
    if retire {
        runtime.retire_frame(&frame);
    }
    checked?;
    Ok(VmPouStackResult {
        return_value,
        locals: frame.locals,
    })
}

/// Decode an instruction partial-access selector, rejecting invalid widths.
pub fn decode_partial_access(operand: u32) -> Result<PartialAccess, RuntimeError> {
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

/// Preserve partial-access failure categories at the runtime boundary.
pub fn partial_access_error_to_runtime(err: PartialAccessError) -> RuntimeError {
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
