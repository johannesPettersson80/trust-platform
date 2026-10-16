//! Native user-call preparation and direct stack-frame cleanup.
use super::*;
use crate::vm::call::{continuation::NativeCall, prepare_native_call};
use crate::vm::VmFrame;

pub(super) enum CallStart {
    Immediate(Value),
    User(crate::vm::call::continuation::PreparedCall),
}

#[allow(clippy::too_many_arguments)]
pub(super) fn start_call(
    runtime: &mut impl ExecutionContext,
    module: &VmModule,
    frames: &mut FrameStack,
    stack: &mut OperandStack,
    kind: u32,
    symbol_idx: u32,
    arg_count: u32,
) -> Result<CallStart, RuntimeError> {
    let caller = frames
        .current_mut()
        .ok_or_else(|| VmTrap::CallStackUnderflow.into_runtime_error())?;
    match prepare_native_call(runtime, module, caller, stack, kind, symbol_idx, arg_count)
        .map_err(VmTrap::into_runtime_error)?
    {
        NativeCall::Immediate(value) => {
            runtime.check_execution_deadline()?;
            Ok(CallStart::Immediate(value))
        }
        NativeCall::User(call) => Ok(CallStart::User(call)),
    }
}

/// Finish a direct bytecode call frame (opcode `CALL`), which has no native
/// output binding continuation of its own.
pub(super) fn finish_call(
    runtime: &mut impl ExecutionContext,
    frames: &mut FrameStack,
    finished: VmFrame,
) -> Result<usize, RuntimeError> {
    let return_pc = finished.return_pc;
    runtime.retire_frame(&finished)?;
    runtime.resume_frame(
        frames
            .current_mut()
            .ok_or_else(|| VmTrap::CallStackUnderflow.into_runtime_error())?,
    )?;
    Ok(return_pc)
}

/// Retire partially entered stack frames after a non-pump initializer fault.
pub(super) fn unwind_stack_frames(
    runtime: &mut impl ExecutionContext,
    buffers: &mut ExecutionBuffers,
    result: &mut Result<VmPouStackResult, RuntimeError>,
) {
    while let Ok(frame) = buffers.frames.pop() {
        if let Err(error) = runtime.retire_frame(&frame)
            && result.is_ok()
        {
            *result = Err(error);
        }
        if let Some(parent) = buffers.frames.current_mut()
            && let Err(error) = runtime.resume_frame(parent)
        {
            *result = Err(error);
        }
    }
}
