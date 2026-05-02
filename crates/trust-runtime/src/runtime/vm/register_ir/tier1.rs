use super::*;

mod compile;
mod execute;
mod state;

pub(super) use self::compile::compile_tier1_block;
pub(super) use self::execute::execute_tier1_compiled_block;
pub(in crate::runtime) use self::state::RegisterTier1SpecializedExecutorState;
#[cfg(test)]
pub(super) use self::state::{
    parse_env_bool as parse_tier1_env_bool, parse_env_usize as parse_tier1_env_usize,
};
pub(super) use self::state::{Tier1BlockKey, Tier1CompiledBlock, Tier1CompiledInstr};

#[allow(clippy::too_many_arguments)]
pub(super) fn maybe_execute_tier1_block(
    runtime: &mut Runtime,
    module: &VmModule,
    program: &RegisterProgram,
    block: &RegisterBlock,
    frames: &mut FrameStack,
    registers: &mut [Value],
    native_call_stack: &mut OperandStack,
    budget: &mut usize,
    depth_offset: u32,
) -> Result<Option<RegisterBlockExecutionOutcome>, RuntimeError> {
    if !runtime.vm_tier1_specialized_executor.enabled() {
        return Ok(None);
    }

    let key = tier1_block_key(module, program.pou_id, block);
    let mut compiled = runtime
        .vm_tier1_specialized_executor
        .compiled_block(&key)
        .map(Arc::clone);
    let hits = runtime.vm_tier1_specialized_executor.track_block_hit(key);
    if compiled.is_none()
        && runtime
            .vm_tier1_specialized_executor
            .can_attempt_compile(hits, &key)
    {
        runtime
            .vm_tier1_specialized_executor
            .record_compile_attempt();
        match compile_tier1_block(module, block, key) {
            Ok(compiled_block) => {
                let compiled_block = Arc::new(compiled_block);
                runtime
                    .vm_tier1_specialized_executor
                    .record_compile_success();
                runtime
                    .vm_tier1_specialized_executor
                    .insert_compiled_block(Arc::clone(&compiled_block));
                compiled = Some(compiled_block);
            }
            Err(reason) => {
                runtime
                    .vm_tier1_specialized_executor
                    .record_compile_failure(reason);
            }
        }
    }

    let Some(compiled) = compiled else {
        return Ok(None);
    };

    let outcome = execute_tier1_compiled_block(
        runtime,
        module,
        program,
        block,
        frames,
        registers,
        native_call_stack,
        compiled.as_ref(),
        budget,
        depth_offset,
    )?;
    let Tier1BlockExecutionOutcome::Executed(outcome) = outcome;
    runtime
        .vm_tier1_specialized_executor
        .record_block_execution();
    Ok(Some(outcome))
}

pub(super) fn tier1_block_key(
    module: &VmModule,
    pou_id: u32,
    block: &RegisterBlock,
) -> Tier1BlockKey {
    Tier1BlockKey {
        module_ptr: module as *const VmModule as usize,
        pou_id,
        block_id: block.id,
        start_pc: block.start_pc.try_into().unwrap_or(u32::MAX),
    }
}

pub(super) fn apply_dint_binary_guard_borrowed(
    op: BinaryOp,
    left: &Value,
    right: &Value,
) -> Result<Option<Value>, RuntimeError> {
    let (left, right) = match (left, right) {
        (Value::DInt(left), Value::DInt(right)) => (*left, *right),
        _ => return Ok(None),
    };

    let value = match op {
        BinaryOp::Add => Value::DInt(left.checked_add(right).ok_or(RuntimeError::Overflow)?),
        BinaryOp::Sub => Value::DInt(left.checked_sub(right).ok_or(RuntimeError::Overflow)?),
        BinaryOp::Mul => Value::DInt(left.checked_mul(right).ok_or(RuntimeError::Overflow)?),
        BinaryOp::Div => {
            if right == 0 {
                return Err(RuntimeError::DivisionByZero);
            }
            Value::DInt(left.checked_div(right).ok_or(RuntimeError::Overflow)?)
        }
        BinaryOp::Mod => {
            if right == 0 {
                return Err(RuntimeError::ModuloByZero);
            }
            Value::DInt(left.checked_rem(right).ok_or(RuntimeError::Overflow)?)
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

pub(super) fn prepare_borrowed_binary_eval(
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
