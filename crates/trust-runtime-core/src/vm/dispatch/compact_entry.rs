//! The entry of a program whose body compact code runs without its frame (checklist P1-9;
//! specification 34 PERF-05).
//!
//! A program's body that keeps no variable in a frame and initializes nothing in one
//! (`CompactCode::frameless`) runs on the typed loop without the frame the dispatcher's entry
//! pushes. The entry makes the checks and the charges of the dispatcher's entry, records the
//! frame's push and starts the execution entry as the frame's initialization does; the frame itself
//! is pushed and initialized only when an operation needs it. At the body's end the deadline is
//! checked and the frame's pop recorded, as the dispatcher ends a frame; a frame that an operation
//! needed is ended by the dispatcher itself. A program therefore runs as it runs from the
//! dispatcher's entry: with the same result, charges, deadline samples and fault record.

use super::super::call::context::RegisterCallOpKind;
use super::super::compact::CompactCode;
use super::super::context::ExecutionContext;
use super::super::frames::ensure_global_call_depth;
use super::super::module::VmModule;
use super::super::{FrameStack, VmTrap};
use super::session::StackCursor;
use super::typed_loop::{self, Frame};
use super::{execute_with_buffers, push_activation_frame, ExecutionBuffers};
use crate::error::RuntimeError;
use crate::memory::InstanceId;

/// The frameless compact code of program `pou_id` and the end of its body, when this composition
/// runs it as the dispatcher's root would (`entry::execute_stack_root`) for an engine entry, which
/// keeps no result: the caller then runs it with [`enter`], takes and gives back the buffers,
/// unwinds a failure and checks the deadline after success, as for any root. `None` otherwise.
pub(super) fn frameless<'m>(
    runtime: &impl ExecutionContext,
    module: &'m VmModule,
    pou_id: u32,
) -> Option<(&'m CompactCode, usize)> {
    let code = module.compact_code(pou_id).filter(|code| code.frameless)?;
    let pou = module.pou(pou_id)?;
    // The dispatcher's conditions for compact code at a frame's first instruction.
    if code.start != pou.code_start || !runtime.runs_compact() {
        return None;
    }
    Some((code, pou.code_end))
}

/// The entry of `code`, the body of `pou_id` that ends at `end`, in `buffers`, of `instance` at call
/// depth `depth_offset`, joining allowance `entry`. No activation result is built: the engine entry
/// keeps none (checklist P1-9).
#[allow(clippy::too_many_arguments)]
pub(super) fn enter(
    runtime: &mut impl ExecutionContext,
    module: &VmModule,
    code: &CompactCode,
    pou_id: u32,
    end: usize,
    instance: Option<InstanceId>,
    depth_offset: u32,
    entry: crate::vm::budget::ExecutionEntry,
    buffers: &mut ExecutionBuffers,
) -> Result<(), RuntimeError> {
    // The dispatcher's entry before its frame (`execute_with_buffers`).
    runtime.begin_execution(entry, module.instruction_budget)?;
    ensure_global_call_depth(depth_offset, 1).map_err(VmTrap::into_runtime_error)?;
    if (depth_offset as usize)
        .saturating_add(buffers.frames.len())
        .saturating_add(1)
        > runtime.call_depth_limit()
    {
        return Err(VmTrap::CallStackOverflow.into_runtime_error());
    }
    let (frame_bytes, frame_work) = buffers
        .frames
        .growth_demand()
        .map_err(VmTrap::into_runtime_error)?;
    runtime.charge_call_storage(frame_bytes, frame_work)?;
    buffers
        .frames
        .reserve_for_push()
        .map_err(VmTrap::into_runtime_error)?;
    runtime.record_call_op(RegisterCallOpKind::FramePush);
    runtime.begin_frameless_invocation(depth_offset)?;
    let mut pc = code.start;
    let pushed = typed_loop::run(
        runtime,
        module,
        code,
        &mut buffers.operand_stack,
        &mut buffers.frames,
        &mut buffers.fault_site,
        pou_id,
        &mut pc,
        end,
        Frame::Deferred {
            instance,
            depth: depth_offset,
        },
    )?;
    if !pushed {
        // The dispatcher's end of a frame: the deadline is checked and the pop recorded.
        runtime.check_execution_deadline()?;
        runtime.record_call_op(RegisterCallOpKind::FramePop);
        return Ok(());
    }
    // The frame an operation needed ends at the body's end on the dispatcher, whose engine entry
    // gives the activation's locals back (`entry::execute_stack_root`).
    let cursor = StackCursor {
        pc: end,
        base_frames: 0,
        depth: depth_offset,
        capture_return: false,
        is_initializer: false,
    };
    let progress = execute_with_buffers(
        runtime,
        module,
        Some(pou_id),
        instance,
        None,
        false,
        depth_offset,
        entry,
        buffers,
        &mut None,
        None,
        Some(cursor),
        None,
    )?;
    #[cfg(feature = "register-ir")]
    let result = super::session::drive_stack_progress(runtime, module, buffers, progress)?;
    #[cfg(not(feature = "register-ir"))]
    let super::StackProgress::Complete(result) = progress;
    runtime.recycle_frame_locals(result.locals);
    Ok(())
}

/// Push and initialize the frame of program `pou_id` of `instance` at call depth `depth`, as the
/// dispatcher's entry does before the body, for an operation of a frameless body that needs it.
pub(super) fn push_frame(
    runtime: &mut impl ExecutionContext,
    frames: &mut FrameStack,
    module: &VmModule,
    pou_id: u32,
    instance: Option<InstanceId>,
    depth: u32,
) -> Result<(), RuntimeError> {
    push_activation_frame(runtime, frames, module, pou_id, usize::MAX, instance, None)
        .map_err(VmTrap::into_runtime_error)?;
    let frame = frames
        .current_mut()
        .ok_or_else(|| VmTrap::CallStackUnderflow.into_runtime_error())?;
    runtime.initialize_frame(module, frame, depth)
}
