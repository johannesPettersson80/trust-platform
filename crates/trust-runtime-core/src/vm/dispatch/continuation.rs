//! Native user calls suspend into the existing VM frame stack, not Rust recursion.
use super::*;
use crate::vm::call::{
    continuation::{NativeCall, PreparedCall},
    prepare_native_call,
};
use crate::vm::VmFrame;

#[derive(Debug)]
pub(super) struct CallContinuation {
    frame_depth: usize,
    operand_base: usize,
    previous_floor: usize,
    call: PreparedCall,
}

pub(super) enum CallStart {
    Immediate(Value),
    Deferred(usize),
}

fn reserve_continuation(
    runtime: &impl ExecutionContext,
    continuations: &mut Vec<CallContinuation>,
) -> Result<(), RuntimeError> {
    if continuations.len() == continuations.capacity() {
        let capacity = continuations
            .capacity()
            .checked_mul(2)
            .ok_or(RuntimeError::Overflow)?
            .max(1)
            .min(runtime.call_depth_limit());
        if capacity <= continuations.len() {
            return Err(VmTrap::CallStackOverflow.into_runtime_error());
        }
        let bytes = capacity
            .checked_mul(core::mem::size_of::<CallContinuation>())
            .ok_or(RuntimeError::Overflow)?;
        runtime.charge_call_storage(bytes, continuations.len())?;
        continuations
            .try_reserve_exact(capacity - continuations.len())
            .map_err(|_| RuntimeError::Overflow)?;
    }
    Ok(())
}

// Keep user-call binding temporaries out of initializer dispatcher frames.
#[inline(never)]
#[allow(clippy::too_many_arguments)]
pub(super) fn start_call(
    runtime: &mut impl ExecutionContext,
    module: &VmModule,
    frames: &mut FrameStack,
    stack: &mut OperandStack,
    continuations: &mut Vec<CallContinuation>,
    depth_offset: u32,
    return_pc: usize,
    kind: u32,
    symbol_idx: u32,
    arg_count: u32,
) -> Result<CallStart, RuntimeError> {
    let caller = frames
        .current_mut()
        .ok_or_else(|| VmTrap::CallStackUnderflow.into_runtime_error())?;
    let mut call =
        match prepare_native_call(runtime, module, caller, stack, kind, symbol_idx, arg_count)
            .map_err(VmTrap::into_runtime_error)?
        {
            NativeCall::Immediate(value) => {
                runtime.check_execution_deadline()?;
                return Ok(CallStart::Immediate(value));
            }
            NativeCall::User(call) => call,
        };
    // Binding and edge qualification keep their original order. Once they have
    // succeeded, every failed entry must restore the edge transaction it owns.
    let reserved = ensure_global_call_depth(depth_offset, frames.len().saturating_add(1))
        .map_err(VmTrap::into_runtime_error)
        .and_then(|()| {
            if (depth_offset as usize).saturating_add(frames.len()) >= runtime.call_depth_limit() {
                Err(VmTrap::CallStackOverflow.into_runtime_error())
            } else {
                Ok(())
            }
        });
    if let Err(error) = reserved {
        call.restore_edges(runtime);
        return Err(error);
    }
    let depth = depth_offset.saturating_add(frames.len() as u32);
    let caller = frames
        .current_mut()
        .ok_or_else(|| VmTrap::CallStackUnderflow.into_runtime_error())?;
    if let Err(error) = runtime.suspend_frame(caller) {
        call.restore_edges(runtime);
        return Err(error);
    }
    let optimized = runtime.try_execute_optimized(
        module,
        call.pou_id,
        call.instance,
        call.locals.as_deref(),
        call.capture_return,
        depth,
        super::super::budget::ExecutionEntry::Nested,
    );
    match optimized {
        Ok(None) => {}
        other => {
            let resumed = runtime.resume_frame(caller);
            let result = resumed.and_then(|()| {
                other.and_then(|value| value.ok_or(RuntimeError::InvalidExecutionState))
            });
            let value = call
                .complete(runtime, module, caller, result.map_err(VmTrap::Runtime))
                .map_err(VmTrap::into_runtime_error)?;
            runtime.check_execution_deadline()?;
            return Ok(CallStart::Immediate(value));
        }
    }
    let entered = reserve_continuation(runtime, continuations)
        .and_then(|()| {
            let (bytes, work) = frames.growth_demand().map_err(VmTrap::into_runtime_error)?;
            runtime.charge_call_storage(bytes, work)
        })
        .and_then(|()| {
            runtime.begin_execution(
                super::super::budget::ExecutionEntry::Nested,
                module.instruction_budget,
            )
        })
        .and_then(|()| {
            push_call_frame(frames, module, call.pou_id, return_pc, call.instance)
                .map_err(VmTrap::into_runtime_error)
        });
    let entry_pc = match entered {
        Ok(pc) => pc,
        Err(error) => {
            let resumed = runtime.resume_frame(
                frames
                    .current_mut()
                    .ok_or(RuntimeError::InvalidExecutionState)?,
            );
            call.restore_edges(runtime);
            return Err(resumed.err().unwrap_or(error));
        }
    };
    let (operand_base, previous_floor) = stack.enter_call();
    continuations.push(CallContinuation {
        frame_depth: frames.len(),
        operand_base,
        previous_floor,
        call,
    });
    runtime.record_call_op(RegisterCallOpKind::FramePush);
    // Ownership is already on the explicit continuation stack if initialization
    // fails. The outer unwind will retire this frame, resume its caller and restore edges.
    let call = &continuations
        .last()
        .ok_or(RuntimeError::InvalidExecutionState)?
        .call;
    let frame = frames
        .current_mut()
        .ok_or(RuntimeError::InvalidExecutionState)?;
    copy_call_inputs(
        runtime,
        frame,
        call.locals.as_deref(),
        call.present.as_deref(),
    )?;
    runtime.initialize_frame(module, frame, depth)?;
    Ok(CallStart::Deferred(entry_pc))
}

pub(super) fn finish_call(
    runtime: &mut impl ExecutionContext,
    module: &VmModule,
    frames: &mut FrameStack,
    stack: &mut OperandStack,
    continuations: &mut Vec<CallContinuation>,
    finished: VmFrame,
) -> Result<usize, RuntimeError> {
    let pc = finished.return_pc;
    if continuations
        .last()
        .is_some_and(|call| call.frame_depth == frames.len() + 1)
    {
        let continuation = continuations
            .pop()
            .ok_or(RuntimeError::InvalidExecutionState)?;
        let mut result =
            finish_stack_result(runtime, finished, continuation.call.capture_return, true);
        stack.leave_call(continuation.operand_base, continuation.previous_floor);
        if result.is_ok() {
            result = runtime.check_execution_deadline().and(result);
        }
        let caller = frames
            .current_mut()
            .ok_or(RuntimeError::InvalidExecutionState)?;
        // As in execute_vm_target, failure to resume replaces a callee error.
        result = runtime.resume_frame(caller).and(result);
        let value = continuation
            .call
            .complete(runtime, module, caller, result.map_err(VmTrap::Runtime))
            .map_err(VmTrap::into_runtime_error)?;
        runtime.check_execution_deadline()?;
        stack.push(value).map_err(VmTrap::into_runtime_error)?;
    } else {
        runtime.retire_frame(&finished)?;
        runtime.resume_frame(
            frames
                .current_mut()
                .ok_or_else(|| VmTrap::CallStackUnderflow.into_runtime_error())?,
        )?;
    }
    Ok(pc)
}

pub(super) fn unwind(
    runtime: &mut impl ExecutionContext,
    buffers: &mut ExecutionBuffers,
    result: &mut Result<VmPouStackResult, RuntimeError>,
) {
    while let Ok(frame) = buffers.frames.pop() {
        let cleanup = runtime.retire_frame(&frame);
        if result.is_ok() {
            if let Err(error) = cleanup {
                *result = Err(error);
            }
        }
        if buffers
            .continuations
            .last()
            .is_some_and(|call| call.frame_depth == buffers.frames.len() + 1)
        {
            if let Some(mut continuation) = buffers.continuations.pop() {
                buffers
                    .operand_stack
                    .leave_call(continuation.operand_base, continuation.previous_floor);
                if let Some(caller) = buffers.frames.current_mut() {
                    if let Err(error) = runtime.resume_frame(caller) {
                        *result = Err(error);
                    }
                }
                continuation.call.restore_edges(runtime);
            }
        }
    }
}
