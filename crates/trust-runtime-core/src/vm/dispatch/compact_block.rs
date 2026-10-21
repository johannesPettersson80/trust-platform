//! The call of a standard function block in compact code (checklist P1-5; `compact::BlockCall`).

use super::super::compact::{BlockCall, CompactCode};
use super::super::context::ExecutionContext;
use super::super::type_policy::primitive_value_matches;
use super::super::{Operand, OperandStack, VmTrap};
use crate::error::RuntimeError;
use crate::memory::layout::ByteRange;
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
    let first = block.first as usize;
    let Some(inputs) = code.slots.get(first..first + usize::from(block.bound)) else {
        return Err(RuntimeError::InvalidExecutionState);
    };
    let operands = usize::from(block.operands);
    let Some([Operand::Value(Value::Instance(instance)), arguments @ ..]) =
        operand_stack.top_operands(operands)
    else {
        return Ok(false);
    };
    let instance = *instance;
    if runtime.instance_pou(instance) != Some(block.pou) {
        return Ok(false);
    }
    let Some(base) = runtime.instance_bytes(instance) else {
        return Ok(false);
    };
    let fits = inputs.iter().all(|member| {
        matches!(arguments.get(member.name as usize),
            Some(Operand::Value(value)) if primitive_value_matches(member.prim, value))
    });
    if !fits {
        return Ok(false);
    }
    if runtime.deadline_exceeded() {
        return Err(VmTrap::DeadlineExceeded.into_runtime_error());
    }
    for member in inputs {
        let Some(Operand::Value(value)) = arguments.get(member.name as usize) else {
            return Err(RuntimeError::InvalidExecutionState);
        };
        runtime.before_value_clone(value)?;
        let leaf = ByteRange {
            region: base.region,
            start: base
                .start
                .checked_add(member.offset)
                .ok_or(RuntimeError::Overflow)?,
            bytes: u32::from(member.bytes),
        };
        if !runtime.store_scalar_at(leaf, member.prim, value) {
            return Err(RuntimeError::NullReference);
        }
    }
    operand_stack
        .discard_top(operands)
        .map_err(VmTrap::into_runtime_error)?;
    runtime.check_builtin_call(instance)?;
    let now = runtime.current_time();
    runtime.execute_builtin_fb(now, instance, block.kind)?;
    runtime.check_execution_deadline()?;
    operand_stack
        .push(Value::Null)
        .map_err(VmTrap::into_runtime_error)?;
    Ok(true)
}
