//! In-place execution of calls to prepared leaf functions.
//!
//! A register activation whose program makes no user call cannot suspend, so
//! its caller may run it to completion inside the call instruction instead of
//! yielding to the pump. The native stack grows by one bounded activation. The
//! checks, charges, frame suspension, child steps, completion and failure
//! cleanup are the pump's for the same call (`session::start_child`, `drive`
//! and `complete_call`), so results, faults and accounting are unchanged, apart
//! from the continuation storage that an in-place call does not allocate.
//! Calls that can nest further, and every call while debugging, use the pump.
use super::session::finish_child_call;
use super::*;
use crate::vm::budget::ExecutionEntry;
use crate::vm::call::continuation::PreparedCall;
use crate::vm::register_ir::{
    abort_register_activation, recycle_register_activation, start_register_activation,
    step_register_activation, RegisterActivationStep, RegisterContext,
};

/// Whether a call to `pou_id` may run in place: the context drives register
/// activations, no debugger is attached, the POU has a prepared fallback-free
/// leaf plan, and its scratch is free. These are exactly the conditions under
/// which `start_register_activation` does not decline; the start can still fail.
/// Only pump-driven contexts report activations, so the synchronous executors'
/// `execute_sync` route is never replaced in a context that uses it.
pub(in crate::vm) fn leaf_call_ready(runtime: &impl RegisterContext, pou_id: u32) -> bool {
    runtime.has_register_activation_pump()
        && !runtime.debug_active()
        && runtime.register_leaf_plan(pou_id)
        && runtime.register_buffers_available(pou_id)
}

/// Runs `call` for the caller frame on top of `frames`, the register frames of
/// an activation entered at `depth_offset`, and returns the call's value.
pub(in crate::vm) fn execute_leaf_call(
    runtime: &mut impl RegisterContext,
    module: &VmModule,
    frames: &mut FrameStack,
    depth_offset: u32,
    mut call: PreparedCall,
) -> Result<Value, RuntimeError> {
    #[cfg(feature = "test-support")]
    runtime.record_in_place_call();
    let caller_depth = depth_offset.saturating_add(frames.len().saturating_sub(1) as u32);
    let child_depth = caller_depth.saturating_add(1);
    if (child_depth as usize).saturating_add(1) > runtime.call_depth_limit() {
        call.restore_edges(runtime);
        return Err(VmTrap::CallStackOverflow.into_runtime_error());
    }
    if let Err(error) =
        ensure_global_call_depth(caller_depth, 2).map_err(VmTrap::into_runtime_error)
    {
        call.restore_edges(runtime);
        return Err(error);
    }
    let suspended = frames
        .current_mut()
        .ok_or_else(|| RuntimeError::InvalidExecutionState)
        .and_then(|frame| runtime.suspend_frame(frame));
    if let Err(error) = suspended {
        call.restore_edges(runtime);
        return Err(error);
    }
    let started = start_register_activation(
        runtime,
        module,
        call.pou_id,
        call.instance,
        None,
        Some(&mut call.locals),
        call.present.as_deref(),
        call.capture_return,
        child_depth,
        ExecutionEntry::Nested,
        true,
    );
    let mut child = match started {
        Ok(Some(child)) => child,
        // `leaf_call_ready` admitted every condition that declines a start.
        Ok(None) => {
            let error = RuntimeError::InvalidExecutionState;
            return Err(resume_after_failure(runtime, frames, &mut call, error));
        }
        Err(error) => return Err(resume_after_failure(runtime, frames, &mut call, error)),
    };
    // One step runs the activation's blocks until it calls or completes; a leaf
    // plan makes no user call, so the step completes it.
    let result = match step_register_activation(runtime, module, &mut child) {
        Ok(RegisterActivationStep::Complete(result)) => result,
        Ok(RegisterActivationStep::Call(nested)) => {
            let (mut nested, _) = nested.into_parts();
            nested.restore_edges(runtime);
            let _ = abort_register_activation(runtime, child);
            let error = RuntimeError::InvalidExecutionState;
            return Err(resume_after_failure(runtime, frames, &mut call, error));
        }
        Err(error) => {
            let _ = abort_register_activation(runtime, child);
            return Err(resume_after_failure(runtime, frames, &mut call, error));
        }
    };
    if let Err(error) = recycle_register_activation(runtime, child) {
        return Err(resume_after_failure(runtime, frames, &mut call, error));
    }
    let result = VmPouStackResult {
        return_value: result.return_value,
        locals: result.locals,
    };
    let caller = frames
        .current_mut()
        .ok_or_else(|| RuntimeError::InvalidExecutionState);
    finish_child_call(runtime, module, caller, call, result)
}

/// The pump's cleanup for a child that failed after its caller was suspended:
/// the caller frame resumes, and its failure supersedes the child's; the call's
/// edge inputs are restored.
fn resume_after_failure(
    runtime: &mut impl RegisterContext,
    frames: &mut FrameStack,
    call: &mut PreparedCall,
    mut error: RuntimeError,
) -> RuntimeError {
    let resumed = frames
        .current_mut()
        .ok_or_else(|| RuntimeError::InvalidExecutionState)
        .and_then(|frame| runtime.resume_frame(frame));
    if let Err(resume_error) = resumed {
        error = resume_error;
    }
    call.restore_edges(runtime);
    error
}
