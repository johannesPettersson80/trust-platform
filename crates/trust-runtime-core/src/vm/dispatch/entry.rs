//! Public stack-dispatch entry points and initializer execution.
use super::session;
use super::{execute_with_buffers, ExecutionBuffers, StackProgress, VmPouStackResult};
use crate::error::RuntimeError;
use crate::memory::InstanceId;
use crate::value::Value;
use crate::vm::context::ExecutionContext;
use crate::vm::module::VmModule;
use alloc::vec::Vec;

/// Execute one POU through the shared stack dispatcher with optional initial locals.
#[cfg(any(feature = "hir", all(feature = "register-ir", test)))]
#[allow(clippy::too_many_arguments)]
pub fn execute_pou_stack_with_locals(
    runtime: &mut impl ExecutionContext,
    module: &VmModule,
    pou_id: u32,
    entry_instance: Option<InstanceId>,
    initial_locals: Option<&[Value]>,
    capture_return: bool,
    depth_offset: u32,
    entry: crate::vm::budget::ExecutionEntry,
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
#[cfg(any(feature = "hir", all(feature = "register-ir", test)))]
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
    entry: crate::vm::budget::ExecutionEntry,
) -> Result<VmPouStackResult, RuntimeError> {
    execute_pou_with_pump(
        runtime,
        module,
        pou_id,
        entry_instance,
        initial_locals,
        parameter_values_present,
        capture_return,
        depth_offset,
        entry,
        None,
        true,
        false,
    )
}

// The legacy optimized hook has already selected/declined this private call.
// Explicit stack fallback must not probe it again.
#[cfg(any(feature = "hir", feature = "register-ir"))]
#[allow(clippy::too_many_arguments)]
pub(in crate::vm) fn execute_pou_stack_with_owned_locals(
    runtime: &mut impl ExecutionContext,
    module: &VmModule,
    pou_id: u32,
    entry_instance: Option<InstanceId>,
    initial_locals: &mut Option<Vec<Value>>,
    parameter_values_present: Option<&[bool]>,
    capture_return: bool,
    depth_offset: u32,
    entry: crate::vm::budget::ExecutionEntry,
) -> Result<VmPouStackResult, RuntimeError> {
    execute_pou_with_pump(
        runtime,
        module,
        pou_id,
        entry_instance,
        None,
        parameter_values_present,
        capture_return,
        depth_offset,
        entry,
        Some(initial_locals),
        true,
        false,
    )
}

/// Execute an engine-owned entry through the same activation pump without
/// retaining a local snapshot that the caller cannot observe.
pub(crate) fn execute_pou_stack_for_engine_entry(
    runtime: &mut impl ExecutionContext,
    module: &VmModule,
    pou_id: u32,
    entry_instance: Option<InstanceId>,
    depth_offset: u32,
    entry: crate::vm::budget::ExecutionEntry,
) -> Result<(), RuntimeError> {
    execute_pou_with_pump(
        runtime,
        module,
        pou_id,
        entry_instance,
        None,
        None,
        false,
        depth_offset,
        entry,
        None,
        false,
        true,
    )
    .map(|_| ())
}

#[allow(clippy::too_many_arguments)]
fn execute_pou_with_pump(
    runtime: &mut impl ExecutionContext,
    module: &VmModule,
    pou_id: u32,
    entry_instance: Option<InstanceId>,
    initial_locals: Option<&[Value]>,
    parameter_values_present: Option<&[bool]>,
    capture_return: bool,
    depth_offset: u32,
    entry: crate::vm::budget::ExecutionEntry,
    owned_initial_locals: Option<&mut Option<Vec<Value>>>,
    capture_result_locals: bool,
    optimize_root: bool,
) -> Result<VmPouStackResult, RuntimeError> {
    let mut buffers = runtime.take_execution_buffers();
    let result = {
        #[cfg(feature = "register-ir")]
        if optimize_root && runtime.has_register_activation_pump() {
            match runtime.start_register_activation(
                module,
                pou_id,
                entry_instance,
                initial_locals,
                None,
                parameter_values_present,
                capture_return,
                depth_offset,
                entry,
                capture_result_locals,
            ) {
                Ok(Some(activation)) => {
                    session::drive_register_root(runtime, module, &mut buffers, activation)
                }
                Ok(None) => execute_stack_root(
                    runtime,
                    module,
                    pou_id,
                    entry_instance,
                    initial_locals,
                    parameter_values_present,
                    capture_return,
                    depth_offset,
                    entry,
                    &mut buffers,
                    owned_initial_locals,
                ),
                Err(error) => Err(error),
            }
        } else {
            execute_stack_root(
                runtime,
                module,
                pou_id,
                entry_instance,
                initial_locals,
                parameter_values_present,
                capture_return,
                depth_offset,
                entry,
                &mut buffers,
                owned_initial_locals,
            )
        }
        #[cfg(not(feature = "register-ir"))]
        {
            let _ = optimize_root;
            let _ = capture_result_locals;
            execute_stack_root(
                runtime,
                module,
                pou_id,
                entry_instance,
                initial_locals,
                parameter_values_present,
                capture_return,
                depth_offset,
                entry,
                &mut buffers,
                owned_initial_locals,
            )
        }
    };
    let mut result = result;
    if result.is_err() {
        #[cfg(feature = "register-ir")]
        super::continuation::unwind_stack_frames(runtime, &mut buffers, &mut result);
        #[cfg(not(feature = "register-ir"))]
        session::unwind_stack_failure(runtime, &mut buffers, &mut result);
    }
    buffers.clear();
    runtime.recycle_execution_buffers(buffers);
    if result.is_ok() {
        runtime.check_execution_deadline()?;
    }
    result
}

#[allow(clippy::too_many_arguments)]
fn execute_stack_root(
    runtime: &mut impl ExecutionContext,
    module: &VmModule,
    pou_id: u32,
    entry_instance: Option<InstanceId>,
    initial_locals: Option<&[Value]>,
    parameter_values_present: Option<&[bool]>,
    capture_return: bool,
    depth_offset: u32,
    entry: crate::vm::budget::ExecutionEntry,
    buffers: &mut ExecutionBuffers,
    owned_initial_locals: Option<&mut Option<Vec<Value>>>,
) -> Result<VmPouStackResult, RuntimeError> {
    let progress = execute_with_buffers(
        runtime,
        module,
        Some(pou_id),
        entry_instance,
        initial_locals,
        capture_return,
        depth_offset,
        entry,
        buffers,
        &mut None,
        parameter_values_present,
        None,
        owned_initial_locals,
    )?;
    #[cfg(feature = "register-ir")]
    {
        session::drive_stack_progress(runtime, module, buffers, progress)
    }
    #[cfg(not(feature = "register-ir"))]
    {
        match progress {
            StackProgress::Complete(result) => Ok(result),
        }
    }
}

/// Execute an initializer range with the caller's lexical locals through the same loop.
/// The caller retains activation ownership and restores returned locals on completion.
pub fn execute_initializer_body(
    runtime: &mut impl ExecutionContext,
    module: &VmModule,
    frame: &mut crate::vm::VmFrame,
    depth: u32,
) -> Result<(), RuntimeError> {
    let mut buffers = runtime.take_execution_buffers();
    let locals = core::mem::take(&mut frame.locals);
    let mut staged_frame = frame.clone();
    staged_frame.locals = locals;
    let mut staged_frame = Some(staged_frame);
    let result = execute_with_buffers(
        runtime,
        module,
        frame.pou_id,
        frame.runtime_instance,
        None,
        false,
        depth,
        crate::vm::budget::ExecutionEntry::Nested,
        &mut buffers,
        &mut staged_frame,
        None,
        None,
        None,
    );
    let outcome = match result {
        Ok(StackProgress::Complete(result)) => {
            frame.locals = result.locals;
            Ok(())
        }
        #[cfg(feature = "register-ir")]
        Ok(StackProgress::Call { mut call, .. }) => {
            call.restore_edges(runtime);
            Err(RuntimeError::StagingViolation)
        }
        Err(error) => {
            // Validated initializer bodies cannot issue direct/user calls. Preserve
            // lexical locals on traps, including a failing coercion callback.
            if let Some(active) = buffers.frames.pop().ok().or_else(|| staged_frame.take()) {
                frame.locals = active.locals;
            }
            Err(error)
        }
    };
    buffers.clear();
    runtime.recycle_execution_buffers(buffers);
    if outcome.is_ok() {
        runtime.check_execution_deadline()?;
    }
    outcome
}
