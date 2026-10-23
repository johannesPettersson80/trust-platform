use super::*;

mod compile;
mod execute;
mod state;

pub use self::compile::compile_tier1_block;
pub use self::execute::execute_tier1_compiled_block;
pub(super) use self::execute::execute_tier1_compiled_block_slice;
pub use self::state::RegisterTier1SpecializedExecutorState;
pub use self::state::{Tier1CompiledBlock, Tier1CompiledInstr};

#[allow(clippy::too_many_arguments)]
pub fn maybe_execute_tier1_block(
    runtime: &mut impl RegisterContext,
    module: &VmModule,
    plan: &PreparedRegisterProgram,
    program: &RegisterProgram,
    block: &RegisterBlock,
    frames: &mut FrameStack,
    registers: &mut [Value],
    remaining_register_reads: &mut [u32],
    native_call_stack: &mut OperandStack,
    depth_offset: u32,
) -> Result<Option<RegisterBlockExecutionOutcome>, RuntimeError> {
    if !runtime.tier1_state_mut().enabled() {
        return Ok(None);
    }

    let Some(Some(compiled)) = plan.tier1_blocks.get(block.id as usize) else {
        return Ok(None);
    };
    let Some(Some(index)) = plan.tier1_selection_indices.get(block.id as usize) else {
        return Ok(None);
    };
    if !runtime.tier1_state_mut().select_prepared_block(*index) {
        return Ok(None);
    }
    let outcome = execute_tier1_compiled_block(
        runtime,
        module,
        program,
        block,
        frames,
        registers,
        remaining_register_reads,
        native_call_stack,
        compiled.as_ref(),
        depth_offset,
    )?;
    let Tier1BlockExecutionOutcome::Executed(outcome) = outcome;
    runtime.tier1_state_mut().record_block_execution();
    Ok(Some(outcome))
}

pub fn apply_dint_binary_guard_borrowed(
    op: BinaryOp,
    left: &Value,
    right: &Value,
) -> Result<Option<Value>, RuntimeError> {
    let (left, right) = match (left, right) {
        (Value::DInt(left), Value::DInt(right)) => (*left, *right),
        _ => return Ok(None),
    };

    let value = match op {
        BinaryOp::Add => Value::DInt(
            left.checked_add(right)
                .ok_or_else(|| RuntimeError::Overflow)?,
        ),
        BinaryOp::Sub => Value::DInt(
            left.checked_sub(right)
                .ok_or_else(|| RuntimeError::Overflow)?,
        ),
        BinaryOp::Mul => Value::DInt(
            left.checked_mul(right)
                .ok_or_else(|| RuntimeError::Overflow)?,
        ),
        BinaryOp::Div => {
            if right == 0 {
                return Err(RuntimeError::DivisionByZero);
            }
            Value::DInt(
                left.checked_div(right)
                    .ok_or_else(|| RuntimeError::Overflow)?,
            )
        }
        BinaryOp::Mod => {
            let remainder = crate::program_model::ops::checked_signed_remainder(
                i64::from(left),
                i64::from(right),
            )?;
            Value::DInt(i32::try_from(remainder).map_err(|_| RuntimeError::Overflow)?)
        }
        BinaryOp::Eq => Value::Bool(left == right),
        BinaryOp::Ne => Value::Bool(left != right),
        BinaryOp::Lt => Value::Bool(left < right),
        BinaryOp::Le => Value::Bool(left <= right),
        BinaryOp::Gt => Value::Bool(left > right),
        BinaryOp::Ge => Value::Bool(left >= right),
        _ => return Ok(None),
    };
    Ok(Some(value))
}

pub fn prepare_borrowed_binary_eval(
    _runtime: &impl RegisterContext,
    op: BinaryOp,
    left: &Value,
    right: &Value,
) -> Result<BorrowedBinaryEval, RuntimeError> {
    if let Some(result) = apply_dint_binary_guard_borrowed(op, left, right)? {
        return Ok(BorrowedBinaryEval::GuardHit(result));
    }
    let (left, left_cloned) = materialize_borrowed_value(left);
    let (right, right_cloned) = materialize_borrowed_value(right);
    Ok(BorrowedBinaryEval::Materialized {
        left,
        left_cloned,
        right,
        right_cloned,
    })
}

pub fn charge_borrowed_binary_operand<'a>(
    runtime: &impl RegisterContext,
    operand: Cow<'a, Value>,
) -> Result<Cow<'a, Value>, RuntimeError> {
    match operand {
        Cow::Borrowed(value) => {
            runtime.before_value_clone(value)?;
            Ok(Cow::Borrowed(value))
        }
        owned @ Cow::Owned(_) => Ok(owned),
    }
}
