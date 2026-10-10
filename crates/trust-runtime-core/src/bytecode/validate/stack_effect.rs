use super::*;

/// Stack movement shared by conservative linear analyses. Typed CFG checks remain separate.
enum StackEffect {
    Move { pops: usize, pushes: usize },
    Duplicate,
    Swap,
}

fn effect(instruction: &Instruction) -> Result<StackEffect, BytecodeError> {
    let (pops, pushes) = match instruction.opcode {
        0x00..=0x02 | 0x05 | 0x06 | 0x70 => (0, 0),
        0x03 | 0x04 | 0x12 | 0x21 => (1, 0),
        0x09 => (instruction.operand(2) as usize, 1),
        0x10 | 0x20 | 0x22..=0x25 | 0x60 => (0, 1),
        0x11 => return Ok(StackEffect::Duplicate),
        0x13 => return Ok(StackEffect::Swap),
        0x30 | 0x32 | 0x45 | 0x49 | 0x61 | 0x62 | 0x64 => (1, 1),
        0x31 | 0x40..=0x44 | 0x46..=0x48 | 0x4C | 0x50..=0x55 | 0x63 => (2, 1),
        0x33 => (2, 0),
        opcode => return Err(BytecodeError::InvalidOpcode(opcode)),
    };
    Ok(StackEffect::Move { pops, pushes })
}

/// Unknown values on underflow preserve the conservative analyses' existing behavior;
/// the CFG pass is responsible for rejecting reachable operand-stack underflow.
pub(super) fn apply_unknown_effect<T: Copy>(
    instruction: &Instruction,
    stack: &mut Vec<T>,
    unknown: T,
    budget: &mut ValidationBudget,
) -> Result<(), BytecodeError> {
    match effect(instruction)? {
        StackEffect::Move { pops, pushes } => {
            budget.work(pops + pushes)?;
            for _ in 0..pops {
                stack.pop();
            }
            for _ in 0..pushes {
                budget.push(stack, unknown)?;
            }
        }
        StackEffect::Duplicate => {
            let top = stack.last().copied().unwrap_or(unknown);
            budget.push(stack, top)?;
        }
        StackEffect::Swap => {
            if stack.len() < 2 {
                budget.push(stack, unknown)?;
                budget.push(stack, unknown)?;
            }
            let len = stack.len();
            stack.swap(len - 1, len - 2);
        }
    }
    Ok(())
}
