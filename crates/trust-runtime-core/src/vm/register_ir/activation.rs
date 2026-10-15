//! Resumable register POU state used by the shared dispatch pump.
use super::interpreter::execute_register_block_interpreted_slice;
use super::tier1::execute_tier1_compiled_block_slice;
use super::*;
use crate::vm::call::continuation::PreparedCall;

#[derive(Debug)]
enum BlockExecutor {
    Select,
    Interpreted,
    /// Runs the block's compiled form in `plan.tier1_blocks`, which the
    /// activation's plan keeps alive; selecting it takes no reference count.
    Tier1,
}

/// Owned, suspended register execution state. Its prepared scratch lease stays
/// unavailable until the activation completes or is unwound.
#[doc(hidden)]
pub struct RegisterActivation {
    pou_id: u32,
    depth: u32,
    /// Admitted plan; its lowered program, read counts and compiled blocks are
    /// borrowed from it rather than reference-counted per activation.
    plan: Arc<PreparedRegisterProgram>,
    buffers: RegisterExecutionBuffers,
    current_block: u32,
    block_started: bool,
    instruction_index: usize,
    executor: BlockExecutor,
    profile_enabled: bool,
    capture_return: bool,
    capture_result_locals: bool,
}

/// A user POU call that suspended a register caller at its native-call
/// instruction. The caller cursor already points at the following instruction.
#[doc(hidden)]
pub struct DeferredRegisterCall {
    call: PreparedCall,
    destination: RegisterId,
}

impl DeferredRegisterCall {
    pub(in crate::vm) fn into_parts(self) -> (PreparedCall, RegisterId) {
        (self.call, self.destination)
    }
}

/// Result of advancing a register activation until it yields or completes.
#[doc(hidden)]
pub enum RegisterActivationStep {
    /// The activation suspended at a user POU call.
    Call(DeferredRegisterCall),
    /// The activation completed and owns its optional result plus local payload.
    Complete(RegisterPouExecutionResult),
}

impl RegisterActivation {
    /// Caller-relative POU depth, used for frame and statement contracts.
    pub(crate) const fn depth(&self) -> u32 {
        self.depth
    }

    pub(crate) fn current_frame_mut(
        &mut self,
    ) -> Result<&mut super::super::frames::VmFrame, RuntimeError> {
        self.buffers
            .buffers_mut()
            .0
            .current_mut()
            .ok_or_else(|| RuntimeError::InvalidExecutionState)
    }

    pub(crate) fn set_call_result(
        &mut self,
        destination: RegisterId,
        value: Value,
    ) -> Result<(), RuntimeError> {
        let (_, registers, _, _) = self.buffers.buffers_mut();
        write_register(registers, destination, value)
    }
}

/// The admitted lowered program; preparation refuses a plan without one.
fn ready_program(plan: &PreparedRegisterProgram) -> Result<&CachedRegisterProgram, RuntimeError> {
    match &plan.lowered {
        RegisterLoweringCacheEntry::Ready(lowered) => Ok(lowered),
        RegisterLoweringCacheEntry::LoweringError { .. } => {
            Err(RuntimeError::InvalidExecutionState)
        }
    }
}

/// Start a prepared register activation without recursively executing it.
#[allow(clippy::too_many_arguments)]
pub(crate) fn start_register_activation(
    runtime: &mut impl RegisterContext,
    module: &VmModule,
    pou_id: u32,
    entry_instance: Option<InstanceId>,
    initial_locals: Option<&[Value]>,
    owned_initial_locals: Option<&mut Option<Vec<Value>>>,
    parameter_values_present: Option<&[bool]>,
    capture_return: bool,
    depth: u32,
    entry: ExecutionEntry,
    capture_result_locals: bool,
) -> Result<Option<RegisterActivation>, RuntimeError> {
    if runtime.debug_active() {
        runtime
            .register_profile_mut()
            .record_prepared_fallback("debug_mode");
        return Ok(None);
    }

    let plan = runtime.prepared_register_plan(pou_id);
    runtime.record_register_plan_lookup(plan.is_some());
    let Some(plan) = plan else {
        runtime
            .register_profile_mut()
            .record_prepared_fallback("unprepared_program");
        return Ok(None);
    };
    if let Some(reason) = plan.fallback_reason() {
        runtime
            .register_profile_mut()
            .record_prepared_fallback(reason);
        return Ok(None);
    }
    if !runtime.register_buffers_available(pou_id) {
        runtime
            .register_profile_mut()
            .record_prepared_fallback("nested_scratch");
        return Ok(None);
    }
    let lowered = ready_program(&plan)?;
    let max_registers = lowered.program.max_registers as usize;
    let entry_block = lowered.program.entry_block;

    runtime.begin_execution(entry, module.instruction_budget())?;
    if (depth as usize).saturating_add(1) > runtime.call_depth_limit() {
        return Err(VmTrap::CallStackOverflow.into_runtime_error());
    }
    let mut buffers = runtime.take_register_buffers(pou_id, max_registers)?;
    let profile_enabled = {
        let (frames, _, _, _) = buffers.buffers_mut();
        prepare_register_entry_frame(
            runtime,
            module,
            frames,
            pou_id,
            entry_instance,
            initial_locals,
            owned_initial_locals,
            parameter_values_present,
            depth,
        )
    };
    let profile_enabled = match profile_enabled {
        Ok(enabled) => enabled,
        Err(error) => {
            while let Ok(frame) = buffers.frames.pop() {
                let _ = runtime.retire_frame(&frame);
                buffers.frames.recycle_frame_payload(frame);
            }
            buffers.clear();
            let _ = runtime.recycle_register_buffers(pou_id, buffers);
            return Err(error);
        }
    };
    Ok(Some(RegisterActivation {
        pou_id,
        depth,
        plan,
        buffers,
        current_block: entry_block,
        block_started: false,
        instruction_index: 0,
        executor: BlockExecutor::Select,
        profile_enabled,
        capture_return,
        capture_result_locals,
    }))
}

/// Run one block slice, suspending on the first user POU call.
pub(crate) fn step_register_activation(
    runtime: &mut impl RegisterContext,
    module: &VmModule,
    activation: &mut RegisterActivation,
) -> Result<RegisterActivationStep, RuntimeError> {
    let lowered = ready_program(&activation.plan)?;
    let program = &lowered.program;
    loop {
        let (frames, registers, remaining_reads, native_call_stack) =
            activation.buffers.buffers_mut();
        let block_index = block_index_from_id(program, activation.current_block)?;
        let block = program
            .blocks
            .get(block_index)
            .ok_or_else(|| RuntimeError::InvalidExecutionState)?;
        if block.id != activation.current_block {
            return Err(invalid_bytecode(format!(
                "register-ir executor resolved block id {} to index {block_index} containing block id {}",
                activation.current_block, block.id
            )));
        }

        if !activation.block_started {
            if *lowered
                .block_has_register_reads
                .get(block_index)
                .unwrap_or(&false)
            {
                for (index, count) in lowered
                    .register_read_counts_by_block
                    .get(block_index)
                    .ok_or_else(|| RuntimeError::InvalidExecutionState)?
                {
                    let Some(slot) = remaining_reads.get_mut(*index as usize) else {
                        return Err(RuntimeError::InvalidExecutionState);
                    };
                    *slot = *count;
                }
            }
            if activation.profile_enabled {
                runtime.register_profile_mut().record_block_hit(
                    program.pou_id,
                    block.id,
                    block.start_pc.try_into().unwrap_or(u32::MAX),
                );
            }
            let frame = frames
                .current()
                .ok_or_else(|| RuntimeError::InvalidExecutionState)?;
            runtime.on_statement_with_frame_checked(
                module,
                program.pou_id,
                block.start_pc,
                activation.depth,
                super::super::context::StatementFrame::from_frame(frame),
            )?;
            activation.executor = select_block_executor(runtime, activation.plan.as_ref(), block);
            if activation.profile_enabled
                && matches!(activation.executor, BlockExecutor::Interpreted)
            {
                runtime
                    .register_profile_mut()
                    .record_interpreted_block_entry();
            }
            activation.block_started = true;
            activation.instruction_index = 0;
        }

        let outcome = match &activation.executor {
            BlockExecutor::Select => return Err(RuntimeError::InvalidExecutionState),
            BlockExecutor::Interpreted => execute_register_block_interpreted_slice(
                runtime,
                module,
                program,
                frames,
                registers,
                remaining_reads,
                native_call_stack,
                block,
                activation.depth,
                activation.instruction_index,
            )?,
            BlockExecutor::Tier1 => {
                let Some(Some(compiled)) = activation.plan.tier1_blocks.get(block.id as usize)
                else {
                    return Err(RuntimeError::InvalidExecutionState);
                };
                execute_tier1_compiled_block_slice(
                    runtime,
                    module,
                    program,
                    block,
                    frames,
                    registers,
                    remaining_reads,
                    native_call_stack,
                    compiled,
                    activation.depth,
                    activation.instruction_index,
                )?
            }
        };
        match outcome {
            RegisterBlockStep::Call {
                call,
                destination,
                next_instruction,
            } => {
                activation.instruction_index = next_instruction;
                return Ok(RegisterActivationStep::Call(DeferredRegisterCall {
                    call,
                    destination,
                }));
            }
            RegisterBlockStep::ReturnFromPou => {
                if matches!(&activation.executor, BlockExecutor::Tier1) {
                    runtime.tier1_state_mut().record_block_execution();
                }
                let finished = frames.pop().map_err(VmTrap::into_runtime_error)?;
                runtime
                    .register_profile_mut()
                    .record_call_op(RegisterCallOpKind::FramePop);
                if !frames.is_empty() {
                    return Err(invalid_bytecode(format!(
                        "register-ir executor unsupported nested call return_pc={}",
                        finished.return_pc
                    )));
                }
                let result = finish_register_result(
                    runtime,
                    frames,
                    finished,
                    activation.capture_return,
                    activation.capture_result_locals,
                )?;
                if let Err(error) = runtime.check_execution_deadline() {
                    if activation.capture_result_locals {
                        let _ = frames.recycle_returned_locals(result.locals);
                    }
                    return Err(error);
                }
                runtime.register_profile_mut().record_executed();
                return Ok(RegisterActivationStep::Complete(result));
            }
            RegisterBlockStep::Continue(control_target) => {
                if matches!(&activation.executor, BlockExecutor::Tier1) {
                    runtime.tier1_state_mut().record_block_execution();
                }
                let next_target = match control_target {
                    Some(target) => target,
                    None => next_linear_block_target(program, block_index),
                };
                match next_target {
                    BlockTarget::Block(next_block) => {
                        activation.current_block = next_block;
                        activation.block_started = false;
                        activation.executor = BlockExecutor::Select;
                        activation.instruction_index = 0;
                    }
                    BlockTarget::Exit => {
                        let finished = frames.pop().map_err(VmTrap::into_runtime_error)?;
                        runtime
                            .register_profile_mut()
                            .record_call_op(RegisterCallOpKind::FramePop);
                        let result = finish_register_result(
                            runtime,
                            frames,
                            finished,
                            activation.capture_return,
                            activation.capture_result_locals,
                        )?;
                        if let Err(error) = runtime.check_execution_deadline() {
                            if activation.capture_result_locals {
                                let _ = frames.recycle_returned_locals(result.locals);
                            }
                            return Err(error);
                        }
                        runtime.register_profile_mut().record_executed();
                        return Ok(RegisterActivationStep::Complete(result));
                    }
                }
            }
        }
    }
}

fn select_block_executor(
    runtime: &mut impl RegisterContext,
    plan: &PreparedRegisterProgram,
    block: &RegisterBlock,
) -> BlockExecutor {
    if !runtime.tier1_state().enabled() {
        return BlockExecutor::Interpreted;
    }
    let Some(Some(_)) = plan.tier1_blocks.get(block.id as usize) else {
        return BlockExecutor::Interpreted;
    };
    let Some(Some(selection_index)) = plan.tier1_selection_indices.get(block.id as usize) else {
        return BlockExecutor::Interpreted;
    };
    if runtime
        .tier1_state_mut()
        .select_prepared_block(*selection_index)
    {
        BlockExecutor::Tier1
    } else {
        BlockExecutor::Interpreted
    }
}

/// Return a completed activation's reserved scratch to its owner.
pub(crate) fn recycle_register_activation(
    runtime: &mut impl RegisterContext,
    activation: RegisterActivation,
) -> Result<(), RuntimeError> {
    if !activation.buffers.frames.is_empty() {
        return Err(RuntimeError::InvalidExecutionState);
    }
    let mut buffers = activation.buffers;
    buffers.clear();
    runtime.recycle_register_buffers(activation.pou_id, buffers)
}

/// Retire an interrupted activation and return its cleared scratch exactly once.
pub(crate) fn abort_register_activation(
    runtime: &mut impl RegisterContext,
    mut activation: RegisterActivation,
) -> Result<(), RuntimeError> {
    let mut cleanup_error = None;
    while let Ok(frame) = activation.buffers.frames.pop() {
        if let Err(error) = runtime.retire_frame(&frame) {
            cleanup_error.get_or_insert(error);
        }
        activation.buffers.frames.recycle_frame_payload(frame);
    }
    activation.buffers.clear();
    let recycle = runtime.recycle_register_buffers(activation.pou_id, activation.buffers);
    cleanup_error.or_else(|| recycle.err()).map_or(Ok(()), Err)
}
