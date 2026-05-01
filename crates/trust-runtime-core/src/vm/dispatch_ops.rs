use crate::program_model::{apply_binary, apply_unary, BinaryOp, UnaryOp};
use crate::value::DateTimeProfile;

use super::{OperandStack, VmFrame, VmTrap};

/// Execute one unary VM operation against the operand stack.
pub fn execute_unary(stack: &mut OperandStack, op: UnaryOp) -> Result<(), VmTrap> {
    let value = stack.pop()?;
    let result = apply_unary(op, value)?;
    stack.push(result)
}

/// Execute one binary VM operation against the operand stack.
pub fn execute_binary(
    profile: &DateTimeProfile,
    stack: &mut OperandStack,
    op: BinaryOp,
) -> Result<(), VmTrap> {
    let (left, right) = stack.pop_pair()?;
    let result = apply_binary(op, left, right, profile)?;
    stack.push(result)
}

/// Apply a relative jump after validating that it stays inside the active frame.
pub fn apply_jump(pc: &mut usize, offset: i32, frame: &VmFrame) -> Result<(), VmTrap> {
    let base = *pc as i64;
    let target = base + i64::from(offset);
    if target < frame.code_start as i64 || target > frame.code_end as i64 {
        return Err(VmTrap::InvalidJumpTarget(target));
    }
    *pc = target as usize;
    Ok(())
}

/// Read one little-endian `u32` VM operand and advance the program counter.
pub fn read_u32(code: &[u8], pc: &mut usize) -> Result<u32, VmTrap> {
    if *pc + 4 > code.len() {
        return Err(VmTrap::BytecodeDecode(
            "vm operand read overflow (u32)".into(),
        ));
    }
    let bytes = [code[*pc], code[*pc + 1], code[*pc + 2], code[*pc + 3]];
    *pc += 4;
    Ok(u32::from_le_bytes(bytes))
}

/// Read one little-endian `i32` VM operand and advance the program counter.
pub fn read_i32(code: &[u8], pc: &mut usize) -> Result<i32, VmTrap> {
    if *pc + 4 > code.len() {
        return Err(VmTrap::BytecodeDecode(
            "vm operand read overflow (i32)".into(),
        ));
    }
    let bytes = [code[*pc], code[*pc + 1], code[*pc + 2], code[*pc + 3]];
    *pc += 4;
    Ok(i32::from_le_bytes(bytes))
}
