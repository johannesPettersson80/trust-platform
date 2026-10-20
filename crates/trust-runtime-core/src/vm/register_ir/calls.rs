//! The register tier's `CALL_NATIVE` (specification 34 ARCH-05; checklist C1-20). A call's arguments
//! are taken out of the registers as the stack's operands and pushed above them, and the call then
//! runs as the stack's runs: a built-in text function reads its arguments where they lie and leaves a
//! text result in the temporary area (C1-12), and any other call takes its arguments as values, the
//! places left in the registers keeping the bytes their loads read (C1-19a, C1-19b). Every result,
//! charge and refusal is the one the call had when it took values (TIME-08).

use super::operands::settle_register_places;
use super::registers::{checked_read_operand_with_counts, write_operand, write_register};
use super::*;
use crate::memory::regions::FrameBlock;
use crate::vm::call::continuation::PreparedCall;
use crate::vm::call::prepare_native_call_planned;
use crate::vm::dispatch::CallResult;
use crate::vm::dispatch_refs::materialize_place;
use crate::vm::dispatch_text::{run_text_call, text_call_at};

/// What a native call leaves to the activation that runs it.
pub(crate) enum CallStep {
    /// The call ran; its result is in its destination register.
    Done,
    /// A user call, which the session runs before the activation goes on.
    User(PreparedCall),
}

/// The register form of one `CALL_NATIVE`: what it calls and where its operands are.
pub(crate) struct NativeCallSite<'a> {
    /// The call's kind (`NATIVE_CALL_KIND_*`).
    pub(crate) kind: u32,
    /// The native symbol it calls.
    pub(crate) symbol_idx: u32,
    /// The registers of its arguments, in order.
    pub(crate) args: &'a [RegisterId],
    /// The register its result goes to.
    pub(crate) dest: RegisterId,
    /// The offset of its `CALL_NATIVE` in the module's code.
    pub(crate) pc: u32,
}

/// Run the native call `site` of an activation entered at `depth_offset`.
pub(crate) fn call_native(
    runtime: &mut impl RegisterContext,
    module: &VmModule,
    frames: &mut FrameStack,
    registers: &mut OperandStack,
    reads: &mut [u32],
    site: &NativeCallSite<'_>,
    depth_offset: u32,
) -> Result<CallStep, RuntimeError> {
    registers.truncate_to_registers();
    for arg in site.args {
        let operand = checked_read_operand_with_counts(runtime, registers, reads, *arg)?;
        registers
            .push_any(operand)
            .map_err(VmTrap::into_runtime_error)?;
    }
    let arg_count = u32::try_from(site.args.len())
        .map_err(|_| invalid_bytecode("register-ir executor arg_count overflow"))?;
    // A built-in text function reads its arguments where they lie, its enumerations and copies read
    // into their values first, as the stack's dispatcher reads them (C1-12, C1-19a, C1-19b).
    let operands = (site.pc as usize).saturating_add(1);
    if let Some(call) = text_call_at(&*runtime, module, operands) {
        read_call_operands(&*runtime, registers, false)?;
        let caller = frames
            .current()
            .ok_or_else(|| VmTrap::CallStackUnderflow.into_runtime_error())?;
        if run_text_call(runtime, registers, caller, &call)? {
            let result = registers
                .pop_operand()
                .map_err(VmTrap::into_runtime_error)?;
            write_operand(registers, site.dest, result)?;
            return Ok(CallStep::Done);
        }
    }
    // Any other call may write what a place in a register stands for, and takes its arguments as
    // values.
    settle_register_places(runtime, registers, None)?;
    read_call_operands(&*runtime, registers, true)?;
    let plan = runtime.call_plan(site.pc);
    let frame = frames
        .current_mut()
        .ok_or_else(|| VmTrap::CallStackUnderflow.into_runtime_error())?;
    let prepared = prepare_native_call_planned(
        runtime,
        module,
        frame,
        registers,
        site.kind,
        site.symbol_idx,
        arg_count,
        plan.as_deref(),
    )
    .map_err(VmTrap::into_runtime_error)?;
    match prepared {
        NativeCall::Immediate(value) => {
            runtime.check_execution_deadline()?;
            write_register(registers, site.dest, value)?;
            Ok(CallStep::Done)
        }
        NativeCall::User(call) => {
            if !crate::vm::dispatch::leaf_call_ready(runtime, call.pou_id) {
                return Ok(CallStep::User(call));
            }
            match crate::vm::dispatch::execute_leaf_call(
                runtime,
                module,
                frames,
                depth_offset,
                call,
            )? {
                CallResult::Value(value) => write_register(registers, site.dest, value)?,
                CallResult::InBlock(block) => {
                    take_result_copy(&*runtime, registers, site.dest, block)?;
                }
            }
            Ok(CallStep::Done)
        }
    }
}

/// Copy the result a finished call left in its frame's bytes `block` into `dest`: a text into the
/// temporary area as a text, any other value as a copy of its bytes, as the stack takes it
/// (C1-19c4, C1-20).
pub(crate) fn take_result_copy(
    runtime: &impl ExecutionContext,
    registers: &mut OperandStack,
    dest: RegisterId,
    block: FrameBlock,
) -> Result<(), RuntimeError> {
    registers.truncate_to_registers();
    runtime.push_return_copy(block, registers)?;
    let result = registers
        .pop_operand()
        .map_err(VmTrap::into_runtime_error)?;
    write_operand(registers, dest, result)
}

/// Read the call operands above the registers into their values: the enumerations and the copies,
/// and the places too when `places`.
fn read_call_operands(
    runtime: &impl RegisterContext,
    registers: &mut OperandStack,
    places: bool,
) -> Result<(), RuntimeError> {
    registers.read_call_operands(
        places,
        |place| materialize_place(runtime, place),
        |temp| runtime.enum_value(temp),
        |bytes, type_id| runtime.copy_value(bytes, type_id),
    )
}

/// Drop register `register`'s operand, as the stack's `POP` drops its top operand: a text's or a
/// copy's room is given back (C1-20).
pub(crate) fn drop_register(
    registers: &mut OperandStack,
    register: RegisterId,
) -> Result<(), RuntimeError> {
    registers
        .take_register(register.index() as usize)
        .map_err(VmTrap::into_runtime_error)?;
    registers.release_temps();
    Ok(())
}
