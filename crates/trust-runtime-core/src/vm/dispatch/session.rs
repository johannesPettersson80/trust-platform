//! Compile-time selected orchestration for deferred stack and register calls.
use super::*;
use crate::vm::budget::ExecutionEntry;
use crate::vm::call::continuation::PreparedCall;

#[derive(Clone, Copy, Debug)]
pub(super) struct StackCursor {
    pub(super) pc: usize,
    pub(super) base_frames: usize,
    pub(super) depth: u32,
    pub(super) capture_return: bool,
    pub(super) is_initializer: bool,
}

#[cfg(feature = "register-ir")]
impl core::fmt::Debug for Activation {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Stack(_) => formatter.write_str("Stack"),
            #[cfg(feature = "register-ir")]
            Self::Register(_) => formatter.write_str("Register"),
        }
    }
}

pub(super) enum StackProgress {
    #[cfg(feature = "register-ir")]
    Call {
        cursor: StackCursor,
        call: PreparedCall,
    },
    Complete(VmPouStackResult),
}

// Register continuations carry their admitted scratch lease inline. The no-register
// composition stores only the stack cursor. Both layouts are charged using the
// selected `size_of::<PumpContinuation>()` before growth.
#[allow(clippy::large_enum_variant)]
#[cfg(feature = "register-ir")]
pub(super) enum Activation {
    Stack(StackCursor),
    Register(crate::vm::register_ir::RegisterActivation),
}

#[cfg(feature = "register-ir")]
pub(super) type ParentActivation = Activation;
#[cfg(not(feature = "register-ir"))]
pub(super) type ParentActivation = StackCursor;

pub(super) struct PumpContinuation {
    parent: ParentActivation,
    call: PreparedCall,
    #[cfg(feature = "register-ir")]
    register_destination: Option<crate::vm::register_ir::RegisterId>,
    operand_scope: (usize, usize),
    pub(super) stack_frame_depth: usize,
}

impl core::fmt::Debug for PumpContinuation {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("PumpContinuation")
            .field("parent", &self.parent)
            .field("call_pou_id", &self.call.pou_id)
            .field("operand_scope", &self.operand_scope)
            .field("stack_frame_depth", &self.stack_frame_depth)
            .finish()
    }
}

impl PumpContinuation {
    fn new(
        parent: ParentActivation,
        call: PreparedCall,
        #[cfg(feature = "register-ir")] register_destination: Option<
            crate::vm::register_ir::RegisterId,
        >,
        operand_scope: (usize, usize),
        stack_frame_depth: usize,
    ) -> Self {
        Self {
            parent,
            call,
            #[cfg(feature = "register-ir")]
            register_destination,
            operand_scope,
            stack_frame_depth,
        }
    }
}

#[cfg(feature = "register-ir")]
pub(super) fn drive_stack_progress(
    runtime: &mut impl ExecutionContext,
    module: &VmModule,
    buffers: &mut ExecutionBuffers,
    progress: StackProgress,
) -> Result<VmPouStackResult, RuntimeError> {
    match progress {
        StackProgress::Complete(result) => Ok(result),
        StackProgress::Call { cursor, call } => {
            let parent = Activation::Stack(cursor);
            match start_child(
                runtime,
                module,
                buffers,
                parent,
                call,
                #[cfg(feature = "register-ir")]
                None,
            ) {
                Ok(active) => drive(runtime, module, buffers, active),
                Err((parent, error)) => Err(unwind(runtime, buffers, parent, error)),
            }
        }
    }
}

#[cfg(not(feature = "register-ir"))]
pub(super) fn unwind_stack_failure(
    runtime: &mut impl ExecutionContext,
    buffers: &mut ExecutionBuffers,
    result: &mut Result<VmPouStackResult, RuntimeError>,
) {
    if !buffers.continuations.is_empty() {
        let original = core::mem::replace(result, Err(RuntimeError::InvalidExecutionState));
        let error = match original {
            Err(error) => error,
            Ok(value) => {
                *result = Ok(value);
                RuntimeError::InvalidExecutionState
            }
        };
        let base_frames = buffers
            .continuations
            .last()
            .map(|continuation| continuation.stack_frame_depth)
            .unwrap_or_default();
        let active = StackCursor {
            pc: 0,
            base_frames,
            depth: 0,
            capture_return: false,
            is_initializer: false,
        };
        *result = Err(unwind(runtime, buffers, Some(active), error));
    }
    super::continuation::unwind_stack_frames(runtime, buffers, result);
}

#[cfg(feature = "register-ir")]
pub(super) fn drive_register_root(
    runtime: &mut impl ExecutionContext,
    module: &VmModule,
    buffers: &mut ExecutionBuffers,
    activation: crate::vm::register_ir::RegisterActivation,
) -> Result<VmPouStackResult, RuntimeError> {
    drive(runtime, module, buffers, Activation::Register(activation))
}

#[cfg(feature = "register-ir")]
enum RegisterProgress {
    Call(crate::vm::register_ir::DeferredRegisterCall),
    Complete(crate::vm::register_ir::RegisterPouExecutionResult),
}

#[cfg(feature = "register-ir")]
enum DispatchProgress {
    Stack(StackProgress),
    Register(RegisterProgress),
}

#[cfg(feature = "register-ir")]
fn drive(
    runtime: &mut impl ExecutionContext,
    module: &VmModule,
    buffers: &mut ExecutionBuffers,
    mut active: Activation,
) -> Result<VmPouStackResult, RuntimeError> {
    loop {
        let progress = match &mut active {
            Activation::Stack(cursor) => execute_with_buffers(
                runtime,
                module,
                None,
                None,
                None,
                cursor.capture_return,
                cursor.depth,
                ExecutionEntry::Nested,
                buffers,
                &mut None,
                None,
                Some(*cursor),
                None,
            )
            .map(DispatchProgress::Stack),
            #[cfg(feature = "register-ir")]
            Activation::Register(activation) => runtime
                .step_register_activation(module, activation)
                .map(|step| {
                    DispatchProgress::Register(match step {
                        crate::vm::register_ir::RegisterActivationStep::Call(call) => {
                            RegisterProgress::Call(call)
                        }
                        crate::vm::register_ir::RegisterActivationStep::Complete(result) => {
                            RegisterProgress::Complete(result)
                        }
                    })
                }),
        };
        let progress = match progress {
            Ok(progress) => progress,
            Err(error) => return Err(unwind(runtime, buffers, Some(active), error)),
        };

        match progress {
            DispatchProgress::Stack(StackProgress::Call { cursor, call }) => {
                let previous = core::mem::replace(&mut active, Activation::Stack(cursor));
                let parent = match previous {
                    Activation::Stack(_) => Activation::Stack(cursor),
                    #[cfg(feature = "register-ir")]
                    Activation::Register(activation) => {
                        active = Activation::Register(activation);
                        return Err(unwind(
                            runtime,
                            buffers,
                            Some(active),
                            RuntimeError::InvalidExecutionState,
                        ));
                    }
                };
                match start_child(
                    runtime,
                    module,
                    buffers,
                    parent,
                    call,
                    #[cfg(feature = "register-ir")]
                    None,
                ) {
                    Ok(child) => active = child,
                    Err((parent, error)) => {
                        return Err(unwind(runtime, buffers, parent, error));
                    }
                }
            }
            #[cfg(feature = "register-ir")]
            DispatchProgress::Register(RegisterProgress::Call(call)) => {
                let (call, destination) = call.into_parts();
                let parent = core::mem::replace(
                    &mut active,
                    Activation::Stack(StackCursor {
                        pc: 0,
                        base_frames: buffers.frames.len(),
                        depth: 0,
                        capture_return: false,
                        is_initializer: false,
                    }),
                );
                match start_child(runtime, module, buffers, parent, call, Some(destination)) {
                    Ok(child) => active = child,
                    Err((parent, error)) => {
                        return Err(unwind(runtime, buffers, parent, error));
                    }
                }
            }
            DispatchProgress::Stack(StackProgress::Complete(result)) => {
                if buffers.continuations.is_empty() {
                    return Ok(result);
                }
                match complete_call(
                    runtime,
                    module,
                    &mut buffers.frames,
                    &mut buffers.operand_stack,
                    &mut buffers.continuations,
                    result,
                ) {
                    Ok(parent) => active = parent,
                    Err((parent, error)) => {
                        return Err(unwind(runtime, buffers, Some(parent), error));
                    }
                }
            }
            #[cfg(feature = "register-ir")]
            DispatchProgress::Register(RegisterProgress::Complete(result)) => {
                let Activation::Register(activation) = core::mem::replace(
                    &mut active,
                    Activation::Stack(StackCursor {
                        pc: 0,
                        base_frames: buffers.frames.len(),
                        depth: 0,
                        capture_return: false,
                        is_initializer: false,
                    }),
                ) else {
                    return Err(unwind(
                        runtime,
                        buffers,
                        Some(active),
                        RuntimeError::InvalidExecutionState,
                    ));
                };
                if let Err(error) = runtime.recycle_register_activation(activation) {
                    return Err(unwind(runtime, buffers, None, error));
                }
                let result = VmPouStackResult {
                    return_value: result.return_value,
                    locals: result.locals,
                };
                if buffers.continuations.is_empty() {
                    return Ok(result);
                }
                match complete_call(
                    runtime,
                    module,
                    &mut buffers.frames,
                    &mut buffers.operand_stack,
                    &mut buffers.continuations,
                    result,
                ) {
                    Ok(parent) => active = parent,
                    Err((parent, error)) => {
                        return Err(unwind(runtime, buffers, Some(parent), error));
                    }
                }
            }
        }
    }
}

// Error returns carry the exact suspended parent back to centralized unwind.
// Boxing it would add an unmetered per-call allocation; pump storage is charged
// at the continuation vector's measured element size.
#[allow(clippy::result_large_err)]
#[cfg(feature = "register-ir")]
fn start_child(
    runtime: &mut impl ExecutionContext,
    module: &VmModule,
    buffers: &mut ExecutionBuffers,
    mut parent: ParentActivation,
    mut call: PreparedCall,
    #[cfg(feature = "register-ir")] register_destination: Option<
        crate::vm::register_ir::RegisterId,
    >,
) -> Result<Activation, (Option<Activation>, RuntimeError)> {
    let parent_depth = match &parent {
        Activation::Stack(cursor) => cursor.depth.saturating_add(
            buffers
                .frames
                .len()
                .saturating_sub(cursor.base_frames)
                .saturating_sub(1) as u32,
        ),
        #[cfg(feature = "register-ir")]
        Activation::Register(activation) => activation.depth(),
    };
    let child_depth = parent_depth.saturating_add(1);
    if (child_depth as usize).saturating_add(1) > runtime.call_depth_limit() {
        call.restore_edges(runtime);
        return Err((Some(parent), VmTrap::CallStackOverflow.into_runtime_error()));
    }
    if let Err(error) =
        ensure_global_call_depth(parent_depth, 2).map_err(VmTrap::into_runtime_error)
    {
        call.restore_edges(runtime);
        return Err((Some(parent), error));
    }

    let has_pump = {
        #[cfg(feature = "register-ir")]
        {
            runtime.has_register_activation_pump()
        }
        #[cfg(not(feature = "register-ir"))]
        {
            false
        }
    };
    if !has_pump {
        let (frames, operand_stack, continuations) = (
            &mut buffers.frames,
            &mut buffers.operand_stack,
            &mut buffers.continuations,
        );
        return start_legacy_child(
            runtime,
            module,
            frames,
            operand_stack,
            continuations,
            parent,
            call,
            child_depth,
        );
    }

    if let Err(error) = reserve_continuation(runtime, &mut buffers.continuations) {
        call.restore_edges(runtime);
        return Err((Some(parent), error));
    }

    let (stack_frame_depth, suspend_result) = match &mut parent {
        Activation::Stack(_) => {
            let depth = buffers.frames.len();
            let result = buffers
                .frames
                .current_mut()
                .ok_or_else(|| RuntimeError::InvalidExecutionState)
                .and_then(|frame| runtime.suspend_frame(frame));
            (depth, result)
        }
        #[cfg(feature = "register-ir")]
        Activation::Register(activation) => {
            let result = activation
                .current_frame_mut()
                .and_then(|frame| runtime.suspend_frame(frame));
            (buffers.frames.len(), result)
        }
    };
    if let Err(error) = suspend_result {
        call.restore_edges(runtime);
        return Err((Some(parent), error));
    }

    let scope = buffers.operand_stack.enter_call();
    buffers.continuations.push(PumpContinuation::new(
        parent,
        call,
        #[cfg(feature = "register-ir")]
        register_destination,
        scope,
        stack_frame_depth,
    ));

    #[cfg(feature = "register-ir")]
    if has_pump {
        let start = {
            let Some(continuation) = buffers.continuations.last_mut() else {
                return Err((None, RuntimeError::InvalidExecutionState));
            };
            runtime.start_register_activation(
                module,
                continuation.call.pou_id,
                continuation.call.instance,
                None,
                Some(&mut continuation.call.locals),
                continuation.call.present.as_deref(),
                continuation.call.capture_return,
                child_depth,
                ExecutionEntry::Nested,
                true,
            )
        };
        match start {
            Ok(Some(activation)) => {
                return Ok(Activation::Register(activation));
            }
            Ok(None) => {}
            Err(error) => {
                let (frames, operand_stack, continuations) = (
                    &mut buffers.frames,
                    &mut buffers.operand_stack,
                    &mut buffers.continuations,
                );
                return Err(restore_failed_child_start(
                    runtime,
                    frames,
                    operand_stack,
                    continuations,
                    error,
                ));
            }
        }
    }

    match start_stack_child(
        runtime,
        module,
        &mut buffers.frames,
        &mut buffers.continuations,
        child_depth,
    ) {
        Ok(cursor) => Ok(Activation::Stack(cursor)),
        Err(error) => {
            let (frames, operand_stack, continuations) = (
                &mut buffers.frames,
                &mut buffers.operand_stack,
                &mut buffers.continuations,
            );
            Err(restore_failed_child_start(
                runtime,
                frames,
                operand_stack,
                continuations,
                error,
            ))
        }
    }
}

// Same inline-parent ownership as `start_child`; preserve compatibility without
// allocating a second heap wrapper for immediate legacy adapters. Kept out of the
// dispatch loop: child setup returns before the child runs, so its locals must not
// enlarge the loop frame that stays live under every nested execution (F401 stack).
#[inline(never)]
#[allow(clippy::result_large_err)]
#[allow(clippy::too_many_arguments)]
pub(super) fn start_legacy_child(
    runtime: &mut impl ExecutionContext,
    module: &VmModule,
    frames: &mut FrameStack,
    operand_stack: &mut OperandStack,
    continuations: &mut Vec<PumpContinuation>,
    mut parent: ParentActivation,
    mut call: PreparedCall,
    child_depth: u32,
) -> Result<ParentActivation, (Option<ParentActivation>, RuntimeError)> {
    let caller = parent_frame_mut(frames, &mut parent);
    let caller = match caller {
        Ok(caller) => caller,
        Err(error) => {
            call.restore_edges(runtime);
            return Err((Some(parent), error));
        }
    };
    if let Err(error) = runtime.suspend_frame(caller) {
        call.restore_edges(runtime);
        return Err((Some(parent), error));
    }

    let optimized = runtime.try_execute_optimized_with_parameter_presence_owned(
        module,
        call.pou_id,
        call.instance,
        &mut call.locals,
        call.present.as_deref(),
        call.capture_return,
        child_depth,
        ExecutionEntry::Nested,
    );
    match optimized {
        Ok(Some(result)) => {
            let resume =
                parent_frame_mut(frames, &mut parent).and_then(|frame| runtime.resume_frame(frame));
            if let Err(error) = resume {
                let _ = runtime.recycle_optimized_result_locals(call.pou_id, result.locals);
                call.restore_edges(runtime);
                return Err((Some(parent), error));
            }
            let caller_frame = parent_frame_mut(frames, &mut parent);
            let caller_frame = match caller_frame {
                Ok(frame) => frame,
                Err(error) => {
                    call.restore_edges(runtime);
                    return Err((Some(parent), error));
                }
            };
            let value = match call.complete(runtime, module, caller_frame, Ok(result)) {
                Ok(value) => value,
                Err(error) => return Err((Some(parent), error.into_runtime_error())),
            };
            if let Err(error) = runtime.check_execution_deadline() {
                return Err((Some(parent), error));
            }
            if let Err(error) = operand_stack.push(value) {
                return Err((Some(parent), error.into_runtime_error()));
            }
            Ok(parent)
        }
        Err(error) => {
            let error = resume_legacy_parent(runtime, frames, &mut parent, error);
            call.restore_edges(runtime);
            Err((Some(parent), error))
        }
        Ok(None) => {
            let stack_frame_depth = frames.len();
            if let Err(error) = reserve_continuation(runtime, continuations) {
                let error = resume_legacy_parent(runtime, frames, &mut parent, error);
                call.restore_edges(runtime);
                return Err((Some(parent), error));
            }
            let scope = operand_stack.enter_call();
            continuations.push(PumpContinuation::new(
                parent,
                call,
                #[cfg(feature = "register-ir")]
                None,
                scope,
                stack_frame_depth,
            ));
            match start_stack_child(runtime, module, frames, continuations, child_depth) {
                #[cfg(feature = "register-ir")]
                Ok(cursor) => Ok(Activation::Stack(cursor)),
                #[cfg(not(feature = "register-ir"))]
                Ok(cursor) => Ok(cursor),
                Err(error) => Err(restore_failed_child_start(
                    runtime,
                    frames,
                    operand_stack,
                    continuations,
                    error,
                )),
            }
        }
    }
}

#[cfg(feature = "register-ir")]
fn parent_frame_mut<'a>(
    frames: &'a mut FrameStack,
    parent: &'a mut ParentActivation,
) -> Result<&'a mut super::super::VmFrame, RuntimeError> {
    match parent {
        Activation::Stack(_) => frames
            .current_mut()
            .ok_or_else(|| RuntimeError::InvalidExecutionState),
        Activation::Register(activation) => activation.current_frame_mut(),
    }
}

#[cfg(not(feature = "register-ir"))]
fn parent_frame_mut<'a>(
    frames: &'a mut FrameStack,
    _parent: &'a mut ParentActivation,
) -> Result<&'a mut super::super::VmFrame, RuntimeError> {
    frames
        .current_mut()
        .ok_or_else(|| RuntimeError::InvalidExecutionState)
}

fn resume_legacy_parent(
    runtime: &mut impl ExecutionContext,
    frames: &mut FrameStack,
    parent: &mut ParentActivation,
    error: RuntimeError,
) -> RuntimeError {
    let resume = parent_frame_mut(frames, parent).and_then(|frame| runtime.resume_frame(frame));
    resume.err().unwrap_or(error)
}

fn restore_failed_child_start(
    runtime: &mut impl ExecutionContext,
    frames: &mut FrameStack,
    operand_stack: &mut OperandStack,
    continuations: &mut Vec<PumpContinuation>,
    mut error: RuntimeError,
) -> (Option<ParentActivation>, RuntimeError) {
    let Some(mut continuation) = continuations.pop() else {
        return (None, RuntimeError::InvalidExecutionState);
    };
    operand_stack.leave_call(continuation.operand_scope.0, continuation.operand_scope.1);
    let resume = parent_frame_mut(frames, &mut continuation.parent)
        .and_then(|frame| runtime.resume_frame(frame));
    if let Err(resume_error) = resume {
        error = resume_error;
    }
    continuation.call.restore_edges(runtime);
    (Some(continuation.parent), error)
}

fn reserve_continuation(
    runtime: &impl ExecutionContext,
    continuations: &mut Vec<PumpContinuation>,
) -> Result<(), RuntimeError> {
    if continuations.len() < continuations.capacity() {
        return Ok(());
    }
    let capacity = continuations
        .capacity()
        .checked_mul(2)
        .ok_or_else(|| RuntimeError::Overflow)?
        .max(1)
        .min(runtime.call_depth_limit());
    if capacity <= continuations.len() {
        return Err(VmTrap::CallStackOverflow.into_runtime_error());
    }
    let bytes = capacity
        .checked_mul(core::mem::size_of::<PumpContinuation>())
        .ok_or_else(|| RuntimeError::Overflow)?;
    runtime.charge_call_storage(bytes, continuations.len())?;
    continuations
        .try_reserve_exact(capacity - continuations.len())
        .map_err(|_| RuntimeError::Overflow)
}

fn start_stack_child(
    runtime: &mut impl ExecutionContext,
    module: &VmModule,
    frames: &mut FrameStack,
    continuations: &mut [PumpContinuation],
    depth: u32,
) -> Result<StackCursor, RuntimeError> {
    let base_frames = frames.len();
    let result = (|| {
        let (frame_bytes, frame_work) =
            frames.growth_demand().map_err(VmTrap::into_runtime_error)?;
        runtime.charge_call_storage(frame_bytes, frame_work)?;
        runtime.begin_execution(ExecutionEntry::Nested, module.instruction_budget)?;
        let continuation = continuations
            .last()
            .ok_or_else(|| RuntimeError::InvalidExecutionState)?;
        let pou_id = continuation.call.pou_id;
        let instance = continuation.call.instance;
        let capture_return = continuation.call.capture_return;
        let pc = push_call_frame(frames, module, pou_id, 0, instance)
            .map_err(VmTrap::into_runtime_error)?;
        runtime.record_call_op(RegisterCallOpKind::FramePush);
        let cursor = StackCursor {
            pc,
            base_frames,
            depth,
            capture_return,
            is_initializer: false,
        };
        let continuation = continuations
            .last_mut()
            .ok_or_else(|| RuntimeError::InvalidExecutionState)?;
        let frame = frames
            .current_mut()
            .ok_or_else(|| RuntimeError::InvalidExecutionState)?;
        super::install_owned_call_inputs(
            runtime,
            frame,
            &mut continuation.call.locals,
            continuation.call.present.as_deref(),
        )?;
        runtime.initialize_frame(module, frame, depth)?;
        Ok(cursor)
    })();
    if result.is_err() {
        while frames.len() > base_frames {
            let frame = frames.pop().map_err(VmTrap::into_runtime_error)?;
            let _ = runtime.retire_frame(&frame);
            #[cfg(feature = "register-ir")]
            frames.recycle_frame_payload(frame);
        }
    }
    result
}

// Returning the parent with a completion error lets the single unwind path
// restore its frame and edges without a box or a lost activation lease.
#[allow(clippy::result_large_err)]
pub(super) fn complete_call(
    runtime: &mut impl ExecutionContext,
    module: &VmModule,
    frames: &mut FrameStack,
    operand_stack: &mut OperandStack,
    continuations: &mut Vec<PumpContinuation>,
    result: VmPouStackResult,
) -> Result<ParentActivation, (ParentActivation, RuntimeError)> {
    let Some(mut continuation) = continuations.pop() else {
        return Err((invalid_parent(frames), RuntimeError::InvalidExecutionState));
    };
    if parent_is_stack(&continuation.parent) && frames.len() != continuation.stack_frame_depth {
        let _ = runtime.recycle_optimized_result_locals(continuation.call.pou_id, result.locals);
        operand_stack.leave_call(continuation.operand_scope.0, continuation.operand_scope.1);
        if let Some(caller) = frames.current_mut()
            && let Err(error) = runtime.resume_frame(caller)
        {
            continuation.call.restore_edges(runtime);
            return Err((continuation.parent, error));
        }
        continuation.call.restore_edges(runtime);
        return Err((continuation.parent, RuntimeError::InvalidExecutionState));
    }
    let mut parent = continuation.parent;
    let caller = parent_frame_mut(frames, &mut parent);
    let value = finish_child_call(runtime, module, caller, continuation.call, result);
    operand_stack.leave_call(continuation.operand_scope.0, continuation.operand_scope.1);
    let value = match value {
        Ok(value) => value,
        Err(error) => return Err((parent, error)),
    };

    #[cfg(feature = "register-ir")]
    let placed = match &mut parent {
        Activation::Stack(_) => operand_stack
            .push(value)
            .map_err(VmTrap::into_runtime_error),
        Activation::Register(activation) => {
            let Some(destination) = continuation.register_destination else {
                return Err((parent, RuntimeError::InvalidExecutionState));
            };
            activation.set_call_result(destination, value)
        }
    };
    #[cfg(not(feature = "register-ir"))]
    let placed = operand_stack
        .push(value)
        .map_err(VmTrap::into_runtime_error);
    if let Err(error) = placed {
        return Err((parent, error));
    }
    Ok(parent)
}

/// Completes a finished child for its caller frame: the deadline check, the caller's
/// resumption and the call's copy-back, then the deadline again. Shared by the pump
/// and in-place leaf calls; a failure has already restored the call's edge inputs.
pub(super) fn finish_child_call(
    runtime: &mut impl ExecutionContext,
    module: &VmModule,
    caller: Result<&mut super::super::VmFrame, RuntimeError>,
    mut call: PreparedCall,
    result: VmPouStackResult,
) -> Result<Value, RuntimeError> {
    let call_pou_id = call.pou_id;
    let mut call_result = match runtime.check_execution_deadline() {
        Ok(()) => Ok(result),
        Err(error) => {
            let _ = runtime.recycle_optimized_result_locals(call_pou_id, result.locals);
            Err(error)
        }
    };
    match caller {
        Ok(caller_frame) => {
            call_result = match runtime.resume_frame(caller_frame) {
                Ok(()) => call_result,
                Err(error) => {
                    if let Ok(result) = call_result {
                        let _ = runtime.recycle_optimized_result_locals(call_pou_id, result.locals);
                    }
                    Err(error)
                }
            };
            let value = call
                .complete(
                    runtime,
                    module,
                    caller_frame,
                    call_result.map_err(VmTrap::Runtime),
                )
                .map_err(VmTrap::into_runtime_error)?;
            runtime.check_execution_deadline()?;
            Ok(value)
        }
        Err(error) => {
            if let Ok(result) = call_result {
                let _ = runtime.recycle_optimized_result_locals(call_pou_id, result.locals);
            }
            call.restore_edges(runtime);
            Err(error)
        }
    }
}

#[cfg(feature = "register-ir")]
fn parent_is_stack(parent: &ParentActivation) -> bool {
    matches!(parent, Activation::Stack(_))
}

#[cfg(not(feature = "register-ir"))]
fn parent_is_stack(_parent: &ParentActivation) -> bool {
    true
}

#[cfg(feature = "register-ir")]
fn invalid_parent(frames: &FrameStack) -> ParentActivation {
    Activation::Stack(StackCursor {
        pc: 0,
        base_frames: frames.len(),
        depth: 0,
        capture_return: false,
        is_initializer: false,
    })
}

#[cfg(not(feature = "register-ir"))]
fn invalid_parent(frames: &FrameStack) -> ParentActivation {
    StackCursor {
        pc: 0,
        base_frames: frames.len(),
        depth: 0,
        capture_return: false,
        is_initializer: false,
    }
}

fn unwind(
    runtime: &mut impl ExecutionContext,
    buffers: &mut ExecutionBuffers,
    mut active: Option<ParentActivation>,
    mut error: RuntimeError,
) -> RuntimeError {
    if let Some(activation) = active.take()
        && let Some(resume_error) = abort_activation(runtime, buffers, activation)
    {
        error = resume_error;
    }
    while let Some(mut continuation) = buffers.continuations.pop() {
        buffers
            .operand_stack
            .leave_call(continuation.operand_scope.0, continuation.operand_scope.1);
        let resume = parent_frame_mut(&mut buffers.frames, &mut continuation.parent)
            .and_then(|frame| runtime.resume_frame(frame));
        if let Err(resume_error) = resume {
            error = resume_error;
        }
        continuation.call.restore_edges(runtime);
        if let Some(resume_error) = abort_activation(runtime, buffers, continuation.parent) {
            error = resume_error;
        }
    }
    error
}

fn abort_activation(
    runtime: &mut impl ExecutionContext,
    buffers: &mut ExecutionBuffers,
    activation: ParentActivation,
) -> Option<RuntimeError> {
    #[cfg(feature = "register-ir")]
    match activation {
        Activation::Stack(cursor) => {
            let mut resume_error = None;
            while buffers.frames.len() > cursor.base_frames {
                let Ok(frame) = buffers.frames.pop() else {
                    break;
                };
                let _ = runtime.retire_frame(&frame);
                #[cfg(feature = "register-ir")]
                buffers.frames.recycle_frame_payload(frame);
                if buffers.frames.len() > cursor.base_frames
                    && let Some(parent) = buffers.frames.current_mut()
                    && let Err(error) = runtime.resume_frame(parent)
                {
                    // The old synchronous dispatcher let a parent resume
                    // failure supersede the child execution fault.
                    resume_error = Some(error);
                }
            }
            resume_error
        }
        #[cfg(feature = "register-ir")]
        Activation::Register(activation) => {
            let _ = runtime.abort_register_activation(activation);
            None
        }
    }
    #[cfg(not(feature = "register-ir"))]
    {
        let cursor = activation;
        let mut resume_error = None;
        while buffers.frames.len() > cursor.base_frames {
            let Ok(frame) = buffers.frames.pop() else {
                break;
            };
            let _ = runtime.retire_frame(&frame);
            if buffers.frames.len() > cursor.base_frames
                && let Some(parent) = buffers.frames.current_mut()
                && let Err(error) = runtime.resume_frame(parent)
            {
                resume_error = Some(error);
            }
        }
        resume_error
    }
}
