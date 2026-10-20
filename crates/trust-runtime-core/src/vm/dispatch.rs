//! Shared stack-bytecode dispatcher. Platform services enter through ExecutionContext.
use super::call::context::RegisterCallOpKind;
use super::call::push_call_frame;
mod continuation;
mod continuation_stack;
mod entry;
#[cfg(feature = "register-ir")]
mod leaf_call;
mod session;
use super::context::ExecutionContext;
use super::dispatch_ops::{
    apply_jump, execute_binary, execute_unary, read_i32, read_u32, reference_equality,
};
use super::dispatch_refs::{
    dynamic_load_operand, dynamic_ref_field, dynamic_ref_index, dynamic_store_copy,
    dynamic_store_enum, dynamic_store_place, dynamic_store_ref, dynamic_store_text, index_to_i64,
    load_ref_addr, load_ref_operand, materialize_place, pop_reference, store_ref, store_ref_copy,
    store_ref_enum, store_ref_place, store_ref_text,
};
use super::dispatch_sizeof::sizeof_type_from_table_with_abi;
use super::module::VmModule;
use super::reference_attempt::apply_reference_attempt;
use super::{ensure_global_call_depth, FrameStack, Operand, OperandStack, VmTrap};
use crate::error::RuntimeError;
use crate::fault::FaultSite;
use crate::memory::view::StoreAccess;
use crate::memory::InstanceId;
use crate::program_model::{apply_binary, BinaryOp, UnaryOp};
use crate::value::{
    read_partial_access, write_partial_access, PartialAccess, PartialAccessError, Value,
};
use alloc::vec::Vec;
use continuation::CallStart;
use continuation_stack::ContinuationStack;
pub use entry::execute_initializer_body;
pub(crate) use entry::execute_pou_stack_for_engine_entry;
#[cfg(any(feature = "hir", all(feature = "register-ir", test)))]
pub use entry::execute_pou_stack_with_locals;
#[cfg(any(feature = "hir", feature = "register-ir"))]
pub(super) use entry::execute_pou_stack_with_owned_locals;
#[cfg(any(feature = "hir", all(feature = "register-ir", test)))]
pub use entry::execute_pou_stack_with_parameter_presence;
#[cfg(feature = "register-ir")]
pub(in crate::vm) use leaf_call::{execute_leaf_call, leaf_call_ready};
use session::{PumpContinuation, StackCursor, StackProgress};

/// Returned value and final local slots from one stack invocation.
#[derive(Debug, Clone)]
pub struct VmPouStackResult {
    /// Optional function return value.
    pub return_value: Option<Value>,
    /// Final or initially bound local values in slot order.
    pub locals: Vec<Value>,
    /// The finished frame's bytes on the activation stack, whose slots its outputs are read from
    /// until the next frame takes them (C1-11).
    pub(crate) block: Option<crate::memory::regions::FrameBlock>,
    /// Whether the return value lies in slot 0 of `block` instead of `return_value`, for the
    /// caller to copy onto its operand stack (C1-19c4).
    pub(crate) return_in_block: bool,
}

impl VmPouStackResult {
    /// The result of an activation that kept every local as a value, as the register tier and the
    /// hosted runtime run one: no frame of it lies on the activation stack (C1-11).
    #[cfg(any(feature = "hir", feature = "register-ir", test))]
    #[must_use]
    pub fn of_values(return_value: Option<Value>, locals: Vec<Value>) -> Self {
        Self {
            return_value,
            locals,
            block: None,
            return_in_block: false,
        }
    }
}

/// Reusable operand and frame buffers owned by an execution context.
#[derive(Debug, Default)]
pub struct ExecutionBuffers {
    /// Reusable operand storage, cleared between root entries.
    pub operand_stack: OperandStack,
    /// Reusable stack-frame storage, cleared between root entries.
    pub frames: FrameStack,
    continuations: ContinuationStack,
    /// Site of the instruction the innermost activation is handling. The dispatcher records
    /// it at every instruction boundary, so the failing instruction's site survives the
    /// unwinding that follows its error (specification 34 MEM-05). A cleared buffer holds no
    /// site, so a recycled buffer never reports another entry's failure.
    fault_site: Option<FaultSite>,
}

impl ExecutionBuffers {
    /// Clear already-unwound execution state while retaining all scratch capacities.
    pub fn clear(&mut self) {
        self.operand_stack.clear();
        self.frames.clear();
        self.continuations.clear();
        self.fault_site = None;
    }

    /// Bytes that `reserve` takes for `operands` operand values and `levels`
    /// call levels: a frame per level and a pending call per level below the first.
    pub(crate) fn reservation_bytes(operands: usize, levels: usize) -> Option<usize> {
        let operand_bytes = operands.checked_mul(core::mem::size_of::<Value>())?;
        let frame_bytes = levels.checked_mul(core::mem::size_of::<super::VmFrame>())?;
        let continuation_bytes = levels
            .saturating_sub(1)
            .checked_mul(core::mem::size_of::<PumpContinuation>())?;
        operand_bytes
            .checked_add(frame_bytes)?
            .checked_add(continuation_bytes)
    }

    /// Reserve the operand, frame and pending-call storage of `levels` call levels at
    /// construction (spec 34 MEM-03). Every charge and limit of a later activation is
    /// unchanged; only the allocation goes.
    pub(crate) fn reserve(&mut self, operands: usize, levels: usize) -> Result<(), VmTrap> {
        self.operand_stack.reserve_storage(operands)?;
        self.frames.reserve_storage(levels)?;
        self.continuations.reserve_storage(levels.saturating_sub(1))
    }

    /// Empty buffers charged as these, with storage only for their charged capacities, as
    /// buffers that grew on demand hold.
    #[cfg(test)]
    pub(crate) fn grown_on_demand(&self) -> Self {
        Self {
            operand_stack: OperandStack::default(),
            frames: self.frames.grown_on_demand(),
            continuations: self.continuations.grown_on_demand(),
            fault_site: None,
        }
    }
}

/// Push the frame of a new activation of `pou_id`, with its locals taken from the
/// execution context's reserved payloads. A call whose `owned` payload (with its presence bits)
/// will replace them (`install_owned_call_inputs`) takes none: each activation then holds one
/// payload, so the payloads reserved per call depth never run out (spec 34 ARCH-05).
/// Errors keep the order of `push_call_frame`.
fn push_activation_frame(
    runtime: &mut impl ExecutionContext,
    frames: &mut FrameStack,
    module: &VmModule,
    pou_id: u32,
    return_pc: usize,
    runtime_instance: Option<InstanceId>,
    owned: Option<(&[Value], Option<&[bool]>)>,
) -> Result<usize, VmTrap> {
    let pou = module
        .pou(pou_id)
        .ok_or_else(|| VmTrap::MissingPou(pou_id))?;
    let local_count = super::materialization_limits::checked_local_count(pou.local_ref_count)?;
    // The push below cannot fail once its room exists, so the payload is never lost.
    frames.reserve_for_push()?;
    let locals = match owned {
        Some((owned, present)) if transfers_owned_locals(Some(owned), local_count, present) => {
            Vec::new()
        }
        _ => runtime.take_frame_locals(local_count),
    };
    let frame = super::VmFrame {
        pou_id: Some(pou_id),
        activation: None,
        return_pc,
        code_start: pou.code_start,
        code_end: pou.code_end,
        local_ref_start: pou.local_ref_start,
        local_ref_count: pou.local_ref_count,
        locals,
        parameter_values_present: runtime.take_frame_presence(),
        runtime_instance,
        instance_owner: pou.primary_instance_owner,
    };
    let entry_pc = frame.code_start;
    frames.push(frame)?;
    Ok(entry_pc)
}

// Root and deferred entries share one input-copy body. Keep its temporaries out
// of the dispatcher frame that remains live during synchronous initialization.
#[inline(never)]
fn copy_call_inputs(
    runtime: &impl ExecutionContext,
    frame: &mut super::VmFrame,
    initial_locals: Option<&[Value]>,
    parameter_values_present: Option<&[bool]>,
) -> Result<(), RuntimeError> {
    if let Some(initial_locals) = initial_locals {
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
    if let Some(present) = parameter_values_present {
        if present.len() > frame.locals.len() {
            return Err(super::module::invalid_bytecode(
                "parameter presence exceeds frame capacity",
            ));
        }
        // Write into the frame's own presence storage rather than replacing it, so an
        // activation that took a reserved payload makes no allocation (spec 34 MEM-03;
        // census A4). The bits are the same either way.
        frame.parameter_values_present.clear();
        frame.parameter_values_present.extend_from_slice(present);
    }
    Ok(())
}

// A private call owns its entire local payload. Only complete, well-shaped
// vectors transfer: legacy hooks may decline after changing the supplied shape,
// and those payloads must retain the borrowed copy/error ordering.
fn transfers_owned_locals(
    owned: Option<&[Value]>,
    local_count: usize,
    parameter_values_present: Option<&[bool]>,
) -> bool {
    owned.is_some_and(|locals| {
        !locals.is_empty()
            && locals.len() == local_count
            && parameter_values_present.is_none_or(|present| present.len() <= locals.len())
    })
}

fn install_owned_call_inputs(
    runtime: &mut impl ExecutionContext,
    frame: &mut super::VmFrame,
    initial_locals: &mut Option<Vec<Value>>,
    parameter_values_present: Option<&[bool]>,
) -> Result<(), RuntimeError> {
    let local_count = frame.local_ref_count as usize;
    if transfers_owned_locals(
        initial_locals.as_deref(),
        local_count,
        parameter_values_present,
    ) {
        // Admit the actual constant ownership transfer before consuming input.
        runtime.charge_call_storage(0, 1)?;
        let owned = initial_locals
            .take()
            .ok_or(RuntimeError::InvalidExecutionState)?;
        // The frame was pushed without locals for this payload (`push_activation_frame`).
        let pushed = core::mem::replace(&mut frame.locals, owned);
        runtime.recycle_frame_locals(pushed);
        copy_call_inputs(runtime, frame, None, parameter_values_present)
    } else {
        copy_call_inputs(
            runtime,
            frame,
            initial_locals.as_deref(),
            parameter_values_present,
        )
    }
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
    initializer_frame: &mut Option<super::VmFrame>,
    parameter_values_present: Option<&[bool]>,
    resume: Option<StackCursor>,
    owned_initial_locals: Option<&mut Option<Vec<Value>>>,
) -> Result<StackProgress, RuntimeError> {
    #[cfg(all(feature = "std", feature = "test-support"))]
    let _timing =
        crate::vm::diagnostic_timing::begin(crate::vm::diagnostic_timing::Phase::StackExecute);
    let ExecutionBuffers {
        operand_stack,
        frames,
        continuations: _continuations,
        fault_site,
    } = buffers;
    #[cfg(feature = "register-ir")]
    let _ = _continuations;
    let mut cursor = if let Some(cursor) = resume {
        cursor
    } else {
        runtime.begin_execution(entry, module.instruction_budget)?;
        ensure_global_call_depth(depth_offset, 1).map_err(VmTrap::into_runtime_error)?;
        if (depth_offset as usize)
            .saturating_add(frames.len())
            .saturating_add(1)
            > runtime.call_depth_limit()
        {
            return Err(VmTrap::CallStackOverflow.into_runtime_error());
        }
        let (frame_bytes, frame_work) =
            frames.growth_demand().map_err(VmTrap::into_runtime_error)?;
        runtime.charge_call_storage(frame_bytes, frame_work)?;
        frames
            .reserve_for_push()
            .map_err(VmTrap::into_runtime_error)?;
        let base_frames = frames.len();
        let is_initializer = initializer_frame.is_some();
        let pc = if let Some(frame) = initializer_frame.take() {
            let start = frame.code_start;
            frames.push(frame).map_err(VmTrap::into_runtime_error)?;
            start
        } else {
            let owned = owned_initial_locals
                .as_ref()
                .and_then(|owned| owned.as_deref())
                .map(|owned| (owned, parameter_values_present));
            push_activation_frame(
                runtime,
                frames,
                module,
                pou_id.ok_or_else(|| super::module::invalid_bytecode("POU entry has no owner"))?,
                usize::MAX,
                entry_instance,
                owned,
            )
            .map_err(VmTrap::into_runtime_error)?
        };
        runtime.record_call_op(RegisterCallOpKind::FramePush);
        if initial_locals.is_some() || !is_initializer {
            let frame = frames
                .current_mut()
                .ok_or_else(|| VmTrap::CallStackUnderflow.into_runtime_error())?;
            if let Some(owned) = owned_initial_locals {
                install_owned_call_inputs(runtime, frame, owned, parameter_values_present)?;
            } else {
                copy_call_inputs(
                    runtime,
                    frame,
                    initial_locals,
                    if is_initializer {
                        None
                    } else {
                        parameter_values_present
                    },
                )?;
            }
            if !is_initializer {
                runtime.initialize_frame(module, frame, depth_offset)?;
            }
        }
        StackCursor {
            pc,
            base_frames,
            depth: depth_offset,
            capture_return,
            is_initializer,
        }
    };
    let is_initializer = cursor.is_initializer;

    loop {
        if frames.is_empty() {
            return Ok(StackProgress::Complete(VmPouStackResult {
                return_value: None,
                locals: Vec::new(),
                block: None,
                return_in_block: false,
            }));
        }

        let (frame_pou_id, frame_start, frame_end) = {
            let frame = frames
                .current()
                .ok_or_else(|| VmTrap::CallStackUnderflow.into_runtime_error())?;
            (frame.pou_id, frame.code_start, frame.code_end)
        };

        if cursor.pc == frame_end {
            runtime.check_execution_deadline()?;
            let finished = frames.pop().map_err(VmTrap::into_runtime_error)?;
            runtime.record_call_op(RegisterCallOpKind::FramePop);
            if frames.len() > cursor.base_frames {
                cursor.pc = continuation::finish_call(runtime, frames, finished)?;
                continue;
            }
            let result = finish_stack_result(
                runtime,
                finished,
                cursor.capture_return,
                !is_initializer,
                !_continuations.is_empty(),
            )?;
            #[cfg(not(feature = "register-ir"))]
            if let Some(continuation) = _continuations.last() {
                if frames.len() != continuation.stack_frame_depth {
                    return Err(RuntimeError::InvalidExecutionState);
                }
                match session::complete_call(
                    runtime,
                    module,
                    frames,
                    operand_stack,
                    _continuations,
                    result,
                ) {
                    Ok(parent) => {
                        cursor = parent;
                        continue;
                    }
                    Err((_, error)) => return Err(error),
                }
            }
            return Ok(StackProgress::Complete(result));
        }

        if cursor.pc < frame_start || cursor.pc > frame_end {
            return Err(VmTrap::InvalidJumpTarget(cursor.pc as i64).into_runtime_error());
        }

        // The instruction this activation is about to handle. Recording its site here, before
        // any handler can fail, leaves the innermost failing instruction's site in place when
        // its error unwinds the activation (specification 34 MEM-05).
        *fault_site = frame_pou_id.map(|pou| FaultSite {
            pou,
            pc: u32::try_from(cursor.pc).unwrap_or(u32::MAX),
        });

        runtime.charge_execution_work(1)?;

        let call_depth = cursor.depth.saturating_add(
            frames
                .len()
                .saturating_sub(cursor.base_frames)
                .saturating_sub(1) as u32,
        );
        if let Some(pou_id) = frame_pou_id {
            let frame = frames
                .current()
                .ok_or_else(|| VmTrap::CallStackUnderflow.into_runtime_error())?;
            runtime.on_statement_with_frame_checked(
                module,
                pou_id,
                cursor.pc,
                call_depth,
                super::context::StatementFrame::from_frame(frame),
            )?;
        }

        let opcode = module
            .code
            .get(cursor.pc)
            .copied()
            .ok_or_else(|| VmTrap::BytecodeDecode("vm instruction fetch out of bounds".into()))
            .map_err(VmTrap::into_runtime_error)?;
        cursor.pc += 1;

        // A built-in text function reads its arguments where they lie (C1-12).
        let text_call = if opcode == 0x09 {
            super::dispatch_text::text_call_at(&*runtime, module, cursor.pc)
        } else {
            None
        };
        let call_operands = if opcode == 0x09 {
            call_operands(module, cursor.pc)
        } else {
            None
        };
        if operand_stack.places() != 0
            && text_call.is_none()
            && !keeps_places(opcode, operand_stack)
            // `=` and `<>` compare two texts where they lie (C1-19b).
            && !(matches!(opcode, 0x50 | 0x51)
                && super::dispatch_text::texts_on_top(&*runtime, operand_stack))
        {
            // A place a call leaves on the stack keeps the bytes its load read as a copy, whatever
            // the call writes (C1-19b).
            if let Some(own) = call_operands {
                let reader = &*runtime;
                operand_stack
                    .detach_places(own, move |place| reader.place_copy(place))
                    .map_err(VmTrap::into_runtime_error)?;
            }
            operand_stack.materialize_places(|place| materialize_place(&*runtime, place))?;
        }
        if operand_stack.enums() != 0 && !keeps_enums(opcode, operand_stack) {
            operand_stack.materialize_enums(|temp| runtime.enum_value(temp))?;
        }
        if operand_stack.copies() != 0 && !keeps_copies(opcode, operand_stack, call_operands) {
            operand_stack
                .materialize_copies(|bytes, type_id| runtime.copy_value(bytes, type_id))?;
        }

        match opcode {
            0x00 => {}
            0x01 => return Err(VmTrap::ForStepZero.into_runtime_error()),
            0x02 => {
                let offset =
                    read_i32(&module.code, &mut cursor.pc).map_err(VmTrap::into_runtime_error)?;
                let frame = frames
                    .current()
                    .ok_or_else(|| VmTrap::CallStackUnderflow.into_runtime_error())?;
                apply_jump(&mut cursor.pc, offset, frame).map_err(VmTrap::into_runtime_error)?;
            }
            0x03 | 0x04 => {
                let offset =
                    read_i32(&module.code, &mut cursor.pc).map_err(VmTrap::into_runtime_error)?;
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
                    apply_jump(&mut cursor.pc, offset, frame)
                        .map_err(VmTrap::into_runtime_error)?;
                }
            }
            0x05 => {
                if is_initializer {
                    return Err(RuntimeError::StagingViolation);
                }
                runtime.check_execution_deadline()?;
                let callee =
                    read_u32(&module.code, &mut cursor.pc).map_err(VmTrap::into_runtime_error)?;
                let inherited_instance = frames.current().and_then(|frame| frame.runtime_instance);
                let return_pc = cursor.pc;
                let relative_frames = frames.len().saturating_sub(cursor.base_frames);
                ensure_global_call_depth(cursor.depth, relative_frames.saturating_add(1))
                    .map_err(VmTrap::into_runtime_error)?;
                if (cursor.depth as usize)
                    .saturating_add(relative_frames)
                    .saturating_add(1)
                    > runtime.call_depth_limit()
                {
                    return Err(VmTrap::CallStackOverflow.into_runtime_error());
                }
                let (frame_bytes, frame_work) =
                    frames.growth_demand().map_err(VmTrap::into_runtime_error)?;
                runtime.charge_call_storage(frame_bytes, frame_work)?;
                runtime.suspend_frame(
                    frames
                        .current_mut()
                        .ok_or_else(|| VmTrap::CallStackUnderflow.into_runtime_error())?,
                )?;
                cursor.pc = push_call_frame(frames, module, callee, return_pc, inherited_instance)
                    .map_err(VmTrap::into_runtime_error)?;
                runtime.record_call_op(RegisterCallOpKind::FramePush);
                let depth = cursor.depth.saturating_add(
                    frames
                        .len()
                        .saturating_sub(cursor.base_frames)
                        .saturating_sub(1) as u32,
                );
                let frame = frames
                    .current_mut()
                    .ok_or_else(|| VmTrap::CallStackUnderflow.into_runtime_error())?;
                runtime.initialize_frame(module, frame, depth)?;
            }
            0x06 => {
                runtime.check_execution_deadline()?;
                let finished = frames.pop().map_err(VmTrap::into_runtime_error)?;
                runtime.record_call_op(RegisterCallOpKind::FramePop);
                if frames.len() == cursor.base_frames {
                    let result = finish_stack_result(
                        runtime,
                        finished,
                        cursor.capture_return,
                        !is_initializer,
                        !_continuations.is_empty(),
                    )?;
                    #[cfg(not(feature = "register-ir"))]
                    if let Some(continuation) = _continuations.last() {
                        if frames.len() != continuation.stack_frame_depth {
                            return Err(RuntimeError::InvalidExecutionState);
                        }
                        match session::complete_call(
                            runtime,
                            module,
                            frames,
                            operand_stack,
                            _continuations,
                            result,
                        ) {
                            Ok(parent) => {
                                cursor = parent;
                                continue;
                            }
                            Err((_, error)) => return Err(error),
                        }
                    }
                    return Ok(StackProgress::Complete(result));
                }
                cursor.pc = continuation::finish_call(runtime, frames, finished)?;
            }
            0x07 => return Err(VmTrap::UnsupportedOpcode("CALL_METHOD").into_runtime_error()),
            0x08 => return Err(VmTrap::UnsupportedOpcode("CALL_VIRTUAL").into_runtime_error()),
            0x09 => {
                let instruction_offset = u32::try_from(cursor.pc.saturating_sub(1))
                    .map_err(|_| RuntimeError::Overflow)?;
                let kind =
                    read_u32(&module.code, &mut cursor.pc).map_err(VmTrap::into_runtime_error)?;
                if is_initializer && kind != crate::bytecode::NATIVE_CALL_KIND_STDLIB {
                    return Err(RuntimeError::StagingViolation);
                }
                let symbol_idx =
                    read_u32(&module.code, &mut cursor.pc).map_err(VmTrap::into_runtime_error)?;
                let arg_count =
                    read_u32(&module.code, &mut cursor.pc).map_err(VmTrap::into_runtime_error)?;
                if let Some(call) = text_call {
                    let caller = frames
                        .current()
                        .ok_or_else(|| VmTrap::CallStackUnderflow.into_runtime_error())?;
                    if super::dispatch_text::run_text_call(runtime, operand_stack, caller, &call)? {
                        continue;
                    }
                    // Any other argument is given its value, as for any call.
                    operand_stack
                        .materialize_places(|place| materialize_place(&*runtime, place))?;
                }
                let plan = runtime.call_plan(instruction_offset);
                match continuation::start_call(
                    runtime,
                    module,
                    frames,
                    operand_stack,
                    kind,
                    symbol_idx,
                    arg_count,
                    plan.as_deref(),
                )? {
                    CallStart::Immediate(value) => operand_stack
                        .push(value)
                        .map_err(VmTrap::into_runtime_error)?,
                    #[cfg(feature = "register-ir")]
                    CallStart::User(call) => {
                        return Ok(StackProgress::Call { cursor, call });
                    }
                    #[cfg(not(feature = "register-ir"))]
                    CallStart::User(mut call) => {
                        let parent_depth = cursor.depth.saturating_add(
                            frames
                                .len()
                                .saturating_sub(cursor.base_frames)
                                .saturating_sub(1) as u32,
                        );
                        let child_depth = parent_depth.saturating_add(1);
                        if (child_depth as usize).saturating_add(1) > runtime.call_depth_limit() {
                            call.restore_edges(runtime);
                            return Err(VmTrap::CallStackOverflow.into_runtime_error());
                        }
                        if let Err(error) = ensure_global_call_depth(parent_depth, 2)
                            .map_err(VmTrap::into_runtime_error)
                        {
                            call.restore_edges(runtime);
                            return Err(error);
                        }
                        match session::start_legacy_child(
                            runtime,
                            module,
                            frames,
                            operand_stack,
                            _continuations,
                            cursor,
                            call,
                            child_depth,
                        ) {
                            Ok(parent_or_child) => cursor = parent_or_child,
                            Err((_, error)) => return Err(error),
                        }
                    }
                }
            }
            0x10 => {
                let const_idx =
                    read_u32(&module.code, &mut cursor.pc).map_err(VmTrap::into_runtime_error)?;
                let value = module
                    .consts
                    .get(const_idx as usize)
                    .ok_or(VmTrap::InvalidConstIndex(const_idx))
                    .map_err(VmTrap::into_runtime_error)?;
                runtime.before_value_clone(value)?;
                let pushed = match runtime.enum_constant(value) {
                    // An enumeration constant is pushed as its type and number (C1-19a).
                    Some(temp) => operand_stack.push_enum(temp),
                    None => match value {
                        // A WSTRING constant goes into the text area, so its load allocates
                        // nothing (C1-12); a STRING constant's copy shares its characters.
                        Value::WString(text) if runtime.keeps_texts() => {
                            operand_stack.push_text(text, true)
                        }
                        _ => operand_stack.push(value.clone()),
                    },
                };
                pushed.map_err(VmTrap::into_runtime_error)?;
            }
            0x11 => {
                if let Some(place) = operand_stack.top_place() {
                    // A second place copies the same bytes, so it is charged as one more copy.
                    let view = runtime
                        .place_view(place)
                        .ok_or(RuntimeError::InvalidExecutionState)?;
                    runtime.before_view_clone(&view)?;
                    operand_stack
                        .push_place(place)
                        .map_err(VmTrap::into_runtime_error)?;
                } else if let Some(temp) = operand_stack.top_enum() {
                    // A second enumeration is charged as one more copy of its value (C1-19a).
                    runtime.before_enum_clone(temp)?;
                    operand_stack
                        .push_enum(temp)
                        .map_err(VmTrap::into_runtime_error)?;
                } else if let Some(temp) = operand_stack.top_text() {
                    // A second text copies its characters, charged as one more copy (C1-12).
                    let text = operand_stack
                        .text(temp)
                        .map_err(VmTrap::into_runtime_error)?;
                    runtime.before_text_clone(text, temp.wide())?;
                    operand_stack
                        .duplicate_text(temp)
                        .map_err(VmTrap::into_runtime_error)?;
                } else {
                    let value = operand_stack.peek().map_err(VmTrap::into_runtime_error)?;
                    runtime.before_value_clone(value)?;
                    operand_stack
                        .push(value.clone())
                        .map_err(VmTrap::into_runtime_error)?;
                }
            }
            0x12 => match operand_stack
                .pop_operand()
                .map_err(VmTrap::into_runtime_error)?
            {
                Operand::Value(Value::Reference(Some(reference))) => {
                    runtime.recycle_reference_path(reference.path);
                }
                Operand::Text(temp) => operand_stack.release_text(temp),
                Operand::Copy(temp) => operand_stack.release_copy(temp),
                _ => {}
            },
            0x13 => operand_stack
                .swap_top()
                .map_err(VmTrap::into_runtime_error)?,
            0x14 => return Err(VmTrap::UnsupportedOpcode("ROT3").into_runtime_error()),
            0x15 => return Err(VmTrap::UnsupportedOpcode("ROT4").into_runtime_error()),
            0x16 => return Err(VmTrap::UnsupportedOpcode("CAST_IMPLICIT").into_runtime_error()),
            0x20 => {
                let ref_idx =
                    read_u32(&module.code, &mut cursor.pc).map_err(VmTrap::into_runtime_error)?;
                let operand = load_ref_operand(runtime, module, frames, ref_idx)
                    .map_err(VmTrap::into_runtime_error)?;
                push_loaded(operand_stack, operand).map_err(VmTrap::into_runtime_error)?;
            }
            0x21 => {
                let ref_idx =
                    read_u32(&module.code, &mut cursor.pc).map_err(VmTrap::into_runtime_error)?;
                match operand_stack
                    .pop_operand()
                    .map_err(VmTrap::into_runtime_error)?
                {
                    Operand::Value(value) => store_ref(runtime, module, frames, ref_idx, value),
                    Operand::Place(place) => {
                        store_ref_place(runtime, module, frames, ref_idx, place)
                    }
                    Operand::Enum(temp) => store_ref_enum(runtime, module, frames, ref_idx, temp),
                    Operand::Text(temp) => {
                        let stored = operand_stack.text(temp).and_then(|text| {
                            store_ref_text(runtime, module, frames, ref_idx, text, temp.wide())
                        });
                        operand_stack.release_text(temp);
                        stored
                    }
                    Operand::Copy(temp) => {
                        let stored = operand_stack.copy_bytes(temp).and_then(|bytes| {
                            store_ref_copy(runtime, module, frames, ref_idx, bytes, temp.place())
                        });
                        operand_stack.release_copy(temp);
                        stored
                    }
                }
                .map_err(VmTrap::into_runtime_error)?;
            }
            0x22 => {
                let ref_idx =
                    read_u32(&module.code, &mut cursor.pc).map_err(VmTrap::into_runtime_error)?;
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
                let super_instance = runtime
                    .storage()
                    .instance_parent(self_instance)
                    .map_err(VmTrap::into_runtime_error)?
                    .ok_or_else(|| {
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
                    read_u32(&module.code, &mut cursor.pc).map_err(VmTrap::into_runtime_error)?;
                let field = module
                    .strings
                    .name(field_idx as usize)
                    .ok_or_else(|| super::errors::invalid_field_string_index(field_idx))?;
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
                let operand = dynamic_load_operand(runtime, frames, &reference)
                    .map_err(VmTrap::into_runtime_error)?;
                runtime.recycle_reference_path(reference.path);
                push_loaded(operand_stack, operand).map_err(VmTrap::into_runtime_error)?;
            }
            0x33 => {
                let operand = operand_stack
                    .pop_operand()
                    .map_err(VmTrap::into_runtime_error)?;
                let reference = pop_reference(operand_stack).map_err(VmTrap::into_runtime_error)?;
                match operand {
                    Operand::Value(value) => {
                        dynamic_store_ref(runtime, module, frames, &reference, value)
                    }
                    Operand::Place(place) => {
                        dynamic_store_place(runtime, module, frames, &reference, place)
                    }
                    Operand::Enum(temp) => {
                        dynamic_store_enum(runtime, module, frames, &reference, temp)
                    }
                    Operand::Text(temp) => {
                        let stored = operand_stack.text(temp).and_then(|text| {
                            dynamic_store_text(
                                runtime,
                                module,
                                frames,
                                &reference,
                                text,
                                temp.wide(),
                            )
                        });
                        operand_stack.release_text(temp);
                        stored
                    }
                    Operand::Copy(temp) => {
                        let stored = operand_stack.copy_bytes(temp).and_then(|bytes| {
                            dynamic_store_copy(
                                runtime,
                                module,
                                frames,
                                &reference,
                                bytes,
                                temp.place(),
                            )
                        });
                        operand_stack.release_copy(temp);
                        stored
                    }
                }
                .map_err(VmTrap::into_runtime_error)?;
                runtime.recycle_reference_path(reference.path);
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
            0x50 | 0x51 => compare_equality(runtime, operand_stack, opcode == 0x50)
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
                    read_u32(&module.code, &mut cursor.pc).map_err(VmTrap::into_runtime_error)?;
                let size = sizeof_type_from_table_with_abi(
                    &module.types,
                    type_idx,
                    module.storage_abi(),
                    &mut |units| runtime.charge_execution_work(units),
                )
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
                    read_u32(&module.code, &mut cursor.pc).map_err(VmTrap::into_runtime_error)?;
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
                    read_u32(&module.code, &mut cursor.pc).map_err(VmTrap::into_runtime_error)?;
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
                    read_u32(&module.code, &mut cursor.pc).map_err(VmTrap::into_runtime_error)?;
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
                    read_u32(&module.code, &mut cursor.pc).map_err(VmTrap::into_runtime_error)?;
                let depth = depth_offset.saturating_add(frames.len().saturating_sub(1) as u32);
                let frame = frames
                    .current_mut()
                    .ok_or_else(|| VmTrap::CallStackUnderflow.into_runtime_error())?;
                // A typed value that the next instruction stores as the initializer's result is
                // built where that result lies (C1-13).
                let in_place = match stored_initializer_result(module, cursor.pc) {
                    Some(initializer) if (0x66..=0x68).contains(&opcode) => runtime
                        .construct_result_in_place(
                            frame,
                            opcode,
                            operand,
                            operand_stack,
                            initializer,
                        )?,
                    _ => false,
                };
                if !in_place {
                    runtime.execute_initialization_opcode(
                        module,
                        frame,
                        opcode,
                        operand,
                        operand_stack,
                        depth,
                    )?;
                }
            }
            0x70 => {
                let _debug_idx =
                    read_u32(&module.code, &mut cursor.pc).map_err(VmTrap::into_runtime_error)?;
            }
            _ => return Err(VmTrap::InvalidOpcode(opcode).into_runtime_error()),
        }
    }
}

/// Push the operand a load gave: a value, a place or an enumeration. A load gives no temporary text
/// and no copy.
fn push_loaded(stack: &mut OperandStack, operand: Operand) -> Result<(), VmTrap> {
    match operand {
        Operand::Value(value) => stack.push(value),
        Operand::Place(place) => stack.push_place(place),
        Operand::Enum(temp) => stack.push_enum(temp),
        Operand::Text(_) | Operand::Copy(_) => {
            Err(VmTrap::Runtime(RuntimeError::InvalidExecutionState))
        }
    }
}

/// The result of the finished `frame`. When `defer`, a caller of this execution takes the result,
/// and a result whose bytes a store copies stays in the finished frame's bytes for the caller to
/// copy onto its operand stack, charged and checked here as reading its value is (C1-19c4).
fn finish_stack_result(
    runtime: &mut impl ExecutionContext,
    mut frame: super::VmFrame,
    capture_return: bool,
    retire: bool,
    defer: bool,
) -> Result<VmPouStackResult, RuntimeError> {
    // The presence bits are read only while the frame initializes; they go back for the next
    // activation (census A4).
    runtime.recycle_frame_presence(core::mem::take(&mut frame.parameter_values_present));
    if capture_return
        && defer
        && let Some(checked) = runtime.check_return_place(&frame)
    {
        let block = runtime.frame_block(&frame);
        let cleanup = if retire {
            runtime.retire_frame(&frame)
        } else {
            Ok(())
        };
        checked?;
        cleanup?;
        return Ok(VmPouStackResult {
            return_value: None,
            locals: frame.locals,
            block,
            return_in_block: true,
        });
    }
    // A return slot on the activation stack is read from the frame's bytes (C1-11); the copy is
    // charged from the value it gives, as a copy of the frame's value is.
    let read_return = if capture_return {
        runtime.read_frame_slot(&frame, 0).transpose()
    } else {
        Ok(None)
    };
    let from_block = match read_return {
        Ok(value) => value,
        Err(error) => {
            if retire {
                let _ = runtime.retire_frame(&frame);
            }
            return Err(error);
        }
    };
    let checked_copy = if capture_return {
        from_block
            .as_ref()
            .or_else(|| frame.locals.first())
            .map(|value| runtime.before_value_clone(value))
            .transpose()
    } else {
        Ok(None)
    };
    if let Err(error) = checked_copy {
        if retire {
            let _ = runtime.retire_frame(&frame);
        }
        return Err(error);
    }
    let return_value = if capture_return {
        from_block.or_else(|| frame.locals.first().cloned())
    } else {
        None
    };
    let block = runtime.frame_block(&frame);
    let checked = if let Some(value) = &return_value {
        runtime.check_frame_return(&frame, value)
    } else {
        Ok(())
    };
    let cleanup = if retire {
        runtime.retire_frame(&frame)
    } else {
        Ok(())
    };
    checked?;
    cleanup?;
    Ok(VmPouStackResult {
        return_value,
        locals: frame.locals,
        block,
        return_in_block: false,
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
    err.into()
}

/// The initializer whose staged result the instruction at `pc` stores whole: a store through an
/// initializer-result reference without a path (C1-13).
fn stored_initializer_result(module: &VmModule, pc: usize) -> Option<u32> {
    if module.code.get(pc) != Some(&0x21) {
        return None;
    }
    let operand = module.code.get(pc.checked_add(1)?..pc.checked_add(5)?)?;
    let ref_idx = u32::from_le_bytes(operand.try_into().ok()?);
    match module.refs.get(ref_idx as usize)? {
        super::module::VmRef::InitializerResult {
            initializer_id,
            path,
        } if path.is_empty() => Some(*initializer_id),
        _ => None,
    }
}

/// Whether `opcode` leaves the places on `stack` standing for unchanged bytes (C1-10c): it writes
/// no block and calls nothing, and it pops a place only as the one place a store copies. Before any
/// other instruction the dispatcher replaces every place with its value, so a place never shows a
/// write that came after its load.
fn keeps_places(opcode: u8, stack: &OperandStack) -> bool {
    match opcode {
        // NOP, JMP, CONST, LOAD, REF, SELF, SUPER and NULL push without writing.
        0x00 | 0x02 | 0x10 | 0x20 | 0x22..=0x25 => true,
        // DUP, POP and SWAP move a place as an operand.
        0x11..=0x13 => true,
        // The conditional jumps, REF_FIELD, LOAD_DYNAMIC, NEG and NOT pop one value.
        0x03 | 0x04 | 0x30 | 0x32 | 0x45 | 0x49 => !stack.top_holds_place(1),
        // REF_INDEX and the binary operators pop two values.
        0x31 | 0x40..=0x44 | 0x46..=0x48 | 0x4C | 0x50..=0x55 => !stack.top_holds_place(2),
        // A store of the only place copies it; the bytes it writes are the only ones it changes.
        0x21 | 0x33 => stack.places() == 1 && stack.top_place().is_some(),
        _ => false,
    }
}

/// Whether `opcode` takes the enumerations on `stack` as they are (C1-19a): it pops none of them,
/// moves one as an operand, stores one, or compares two with `=` or `<>`. Before any other
/// instruction the dispatcher replaces every enumeration with the value it stands for.
fn keeps_enums(opcode: u8, stack: &OperandStack) -> bool {
    match opcode {
        // NOP, JMP, CONST, LOAD, REF, SELF, SUPER and NULL push without popping.
        0x00 | 0x02 | 0x10 | 0x20 | 0x22..=0x25 => true,
        // DUP, POP and SWAP move an operand; STORE and STORE_DYNAMIC store one.
        0x11..=0x13 | 0x21 | 0x33 => true,
        // `=` and `<>` compare two enumerations as they are, and an enumeration beside an operand of
        // another kind as its value.
        0x50 | 0x51 => match stack.top_operands(2) {
            Some([Operand::Enum(_), Operand::Enum(_)]) => true,
            Some(top) => !top
                .iter()
                .any(|operand| matches!(operand, Operand::Enum(_))),
            None => true,
        },
        // The conditional jumps, REF_FIELD, LOAD_DYNAMIC, NEG and NOT pop one value, and REF_INDEX
        // and the other binary operators two, so the enumerations below them stay as they are.
        0x03 | 0x04 | 0x30 | 0x32 | 0x45 | 0x49 => !stack.top_holds_enum(1),
        0x31 | 0x40..=0x44 | 0x46..=0x48 | 0x4C | 0x52..=0x55 => !stack.top_holds_enum(2),
        _ => false,
    }
}

/// Whether `opcode` takes the copies on `stack` as they are (C1-19b): it pops none of them as a
/// value, or it moves or stores one. A copy's bytes are its own, so no write changes them. A call
/// pops its top `own` operands as values. Before any other instruction the dispatcher replaces every
/// copy with the value it stands for.
fn keeps_copies(opcode: u8, stack: &OperandStack, own: Option<usize>) -> bool {
    match opcode {
        // NOP, JMP, CONST, LOAD, REF, SELF, SUPER and NULL push without popping.
        0x00 | 0x02 | 0x10 | 0x20 | 0x22..=0x25 => true,
        // POP and SWAP move an operand; STORE and STORE_DYNAMIC store one.
        0x12 | 0x13 | 0x21 | 0x33 => true,
        0x09 => own.is_some_and(|own| !stack.top_holds_copy(own)),
        // The conditional jumps, DUP, REF_FIELD, LOAD_DYNAMIC, NEG and NOT take one value, and
        // REF_INDEX and the binary operators two.
        0x03 | 0x04 | 0x11 | 0x30 | 0x32 | 0x45 | 0x49 => !stack.top_holds_copy(1),
        0x31 | 0x40..=0x44 | 0x46..=0x48 | 0x4C | 0x50..=0x55 => !stack.top_holds_copy(2),
        _ => false,
    }
}

/// The operands the call whose operands start at `pc` pops: its `arg_count`, the third of them
/// (C1-19b).
fn call_operands(module: &VmModule, pc: usize) -> Option<usize> {
    let operand = module.code.get(pc.checked_add(8)?..pc.checked_add(12)?)?;
    usize::try_from(u32::from_le_bytes(operand.try_into().ok()?)).ok()
}

/// `=` (`equal`) or `<>` of the top two operands. When one of them is a reference they compare as
/// `apply_binary` compares them, and the path buffers of the references go back to the engine
/// (spec 34 ARCH-05), so a test such as `r <> NULL` allocates nothing once the engine is warm.
pub(crate) fn compare_equality(
    runtime: &impl ExecutionContext,
    stack: &mut OperandStack,
    equal: bool,
) -> Result<(), VmTrap> {
    // Two enumerations are equal when they have one type and one number, as their values are
    // (C1-19a).
    if let Some([Operand::Enum(left), Operand::Enum(right)]) = stack.top_operands(2) {
        let same = runtime.enums_equal(*left, *right);
        stack.discard_top(2)?;
        return stack.push(Value::Bool(same == equal));
    }
    // Two texts are compared where they lie, as their values are (C1-19b).
    if let Some(same) = super::dispatch_text::equal_texts(runtime, stack) {
        for _ in 0..2 {
            if let Operand::Text(temp) = stack.pop_operand()? {
                stack.release_text(temp);
            }
        }
        return stack.push(Value::Bool(same == equal));
    }
    let (left, right) = stack.pop_pair()?;
    // Two references to one target compare equal whichever form they were taken in (C1-11d).
    let canonical = |operand: Value| match operand {
        Value::Reference(Some(reference)) => {
            Value::Reference(Some(runtime.flatten_reference(reference)))
        }
        other => other,
    };
    let (left, right) = (canonical(left), canonical(right));
    let Some(same) = reference_equality(&left, &right) else {
        let op = if equal { BinaryOp::Eq } else { BinaryOp::Ne };
        let result = apply_binary(op, left, right, runtime.profile())?;
        return stack.push(result);
    };
    for operand in [left, right] {
        if let Value::Reference(Some(reference)) = operand {
            runtime.recycle_reference_path(reference.path);
        }
    }
    stack.push(Value::Bool(same == equal))
}
