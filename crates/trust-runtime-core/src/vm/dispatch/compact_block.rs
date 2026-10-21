//! The call of a standard function block in compact code (checklist P1-5; `compact::BlockCall`).

use super::super::compact::{BlockCall, CompactCode, Slot};
use super::super::context::ExecutionContext;
use super::super::type_policy::{primitive_cell_matches, primitive_value_matches};
use super::super::{Operand, OperandStack, VmTrap};
use crate::error::RuntimeError;
use crate::memory::cells::Cell;
use crate::memory::layout::ByteRange;
use crate::memory::InstanceId;
use crate::value::Value;

/// Run `block`, a call of `code`, on the operands on top of `operand_stack`: the receiver and the
/// arguments. It runs as the general call does: the deadline is checked, each bound input's
/// argument is bound as an expression and written into its member where it lies, the block's native
/// code runs at the logical time, the deadline is checked again, and the call's value is pushed.
/// Answers `false`, having done nothing, when the receiver is not a persistent instance of the
/// block or an argument is not a value of its member's primitive; the general call runs it then.
pub(super) fn call_block(
    runtime: &mut impl ExecutionContext,
    code: &CompactCode,
    operand_stack: &mut OperandStack,
    block: BlockCall,
) -> Result<bool, RuntimeError> {
    let inputs = inputs(code, block)?;
    let operands = usize::from(block.operands);
    let Some(operands) = operand_stack.top_operands(operands) else {
        return Ok(false);
    };
    let Some((instance, base)) = receiver(runtime, block, inputs, operands) else {
        return Ok(false);
    };
    bind(runtime, inputs, operands, base)?;
    operand_stack
        .discard_top(usize::from(block.operands))
        .map_err(VmTrap::into_runtime_error)?;
    run(runtime, block, instance, base)?;
    operand_stack
        .push(Value::Null)
        .map_err(VmTrap::into_runtime_error)?;
    Ok(true)
}

/// The bound inputs of `block`: the members its arguments are written into.
pub(super) fn inputs(code: &CompactCode, block: BlockCall) -> Result<&[Slot], RuntimeError> {
    let first = block.first as usize;
    code.slots
        .get(first..first + usize::from(block.bound))
        .ok_or(RuntimeError::InvalidExecutionState)
}

/// The receiver of `block` among `operands` (the receiver, then the arguments) and its bytes, when
/// it is a persistent instance of the block and each argument of `inputs` is a value of its
/// member's primitive; `None` leaves the call to the general call. Nothing is charged. The
/// instance's POU and bytes are found at once, and [`run`] runs the block in those bytes.
pub(super) fn receiver(
    runtime: &impl ExecutionContext,
    block: BlockCall,
    inputs: &[Slot],
    operands: &[impl Argument],
) -> Option<(InstanceId, ByteRange)> {
    let (receiver, arguments) = operands.split_first()?;
    let instance = receiver.instance()?;
    let (pou, base) = runtime.instance_pou_and_bytes(instance)?;
    if pou != block.pou {
        return None;
    }
    inputs
        .iter()
        .all(|member| {
            arguments
                .get(member.name as usize)
                .is_some_and(|argument| argument.fits(member.prim))
        })
        .then_some((instance, base))
}

/// Check the deadline and bind each input of `inputs` from `operands` (the receiver, then the
/// arguments): its argument is admitted as a copy and written into its member, which lies in the
/// instance's bytes `base`.
pub(super) fn bind(
    runtime: &mut impl ExecutionContext,
    inputs: &[Slot],
    operands: &[impl Argument],
    base: ByteRange,
) -> Result<(), RuntimeError> {
    if runtime.deadline_exceeded() {
        return Err(VmTrap::DeadlineExceeded.into_runtime_error());
    }
    let arguments = operands.get(1..).unwrap_or_default();
    for member in inputs {
        let Some(argument) = arguments.get(member.name as usize) else {
            return Err(RuntimeError::InvalidExecutionState);
        };
        argument.admit(runtime)?;
        let leaf = ByteRange {
            region: base.region,
            start: base
                .start
                .checked_add(member.offset)
                .ok_or(RuntimeError::Overflow)?,
            bytes: u32::from(member.bytes),
        };
        if !argument.store(runtime, leaf, member.prim) {
            return Err(RuntimeError::NullReference);
        }
    }
    Ok(())
}

/// Run the native code of `block` on `instance`, whose own variables are `base` ([`receiver`]),
/// at the logical time, once its inputs are bound, and check the deadline after it.
pub(super) fn run(
    runtime: &mut impl ExecutionContext,
    block: BlockCall,
    instance: InstanceId,
    base: ByteRange,
) -> Result<(), RuntimeError> {
    runtime.check_builtin_call(instance)?;
    let now = runtime.current_time();
    runtime.execute_builtin_fb_at(now, instance, block.kind, block.pou, base)?;
    runtime.check_execution_deadline()
}

/// An operand of a block call as compact code holds it: a value on the operand stack, or a cell
/// in the typed loop (checklist P1-9).
pub(super) trait Argument {
    /// The instance it names, as a receiver.
    fn instance(&self) -> Option<InstanceId>;
    /// Whether it is a value of primitive `prim`.
    fn fits(&self, prim: u16) -> bool;
    /// Admit its copy as the argument's binding admits it.
    fn admit(&self, runtime: &impl ExecutionContext) -> Result<(), RuntimeError>;
    /// Write it into the scalar leaf of primitive `prim` at `leaf`; `false` when there is no such
    /// leaf.
    fn store(&self, runtime: &mut impl ExecutionContext, leaf: ByteRange, prim: u16) -> bool;
}

impl Argument for Operand {
    fn instance(&self) -> Option<InstanceId> {
        match self {
            Operand::Value(Value::Instance(instance)) => Some(*instance),
            _ => None,
        }
    }
    fn fits(&self, prim: u16) -> bool {
        matches!(self, Operand::Value(value) if primitive_value_matches(prim, value))
    }
    fn admit(&self, runtime: &impl ExecutionContext) -> Result<(), RuntimeError> {
        match self {
            Operand::Value(value) => runtime.before_value_clone(value),
            _ => Err(RuntimeError::InvalidExecutionState),
        }
    }
    fn store(&self, runtime: &mut impl ExecutionContext, leaf: ByteRange, prim: u16) -> bool {
        matches!(self, Operand::Value(value) if runtime.store_scalar_at(leaf, prim, value))
    }
}

impl Argument for Cell {
    fn instance(&self) -> Option<InstanceId> {
        match self {
            Cell::Instance(instance) => Some(*instance),
            _ => None,
        }
    }
    fn fits(&self, prim: u16) -> bool {
        primitive_cell_matches(prim, self)
    }
    fn admit(&self, runtime: &impl ExecutionContext) -> Result<(), RuntimeError> {
        runtime.before_scalar_clone()
    }
    fn store(&self, runtime: &mut impl ExecutionContext, leaf: ByteRange, prim: u16) -> bool {
        runtime.store_cell_at(leaf, prim, *self)
    }
}
