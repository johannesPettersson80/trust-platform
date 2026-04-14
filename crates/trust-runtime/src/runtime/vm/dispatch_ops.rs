use crate::program_model::{apply_binary, apply_unary, BinaryOp, UnaryOp};

use super::super::core::Runtime;
use super::errors::VmTrap;
use super::frames::VmFrame;
use super::stack::OperandStack;

pub(super) fn execute_unary(stack: &mut OperandStack, op: UnaryOp) -> Result<(), VmTrap> {
    let value = stack.pop()?;
    let result = apply_unary(op, value)?;
    stack.push(result)
}

pub(super) fn execute_binary(
    runtime: &Runtime,
    stack: &mut OperandStack,
    op: BinaryOp,
) -> Result<(), VmTrap> {
    let (left, right) = stack.pop_pair()?;
    let result = apply_binary(op, left, right, &runtime.profile)?;
    stack.push(result)
}

pub(super) fn apply_jump(pc: &mut usize, offset: i32, frame: &VmFrame) -> Result<(), VmTrap> {
    let base = *pc as i64;
    let target = base + i64::from(offset);
    if target < frame.code_start as i64 || target > frame.code_end as i64 {
        return Err(VmTrap::InvalidJumpTarget(target));
    }
    *pc = target as usize;
    Ok(())
}

pub(super) fn read_u32(code: &[u8], pc: &mut usize) -> Result<u32, VmTrap> {
    if *pc + 4 > code.len() {
        return Err(VmTrap::BytecodeDecode(
            "vm operand read overflow (u32)".into(),
        ));
    }
    let bytes = [code[*pc], code[*pc + 1], code[*pc + 2], code[*pc + 3]];
    *pc += 4;
    Ok(u32::from_le_bytes(bytes))
}

pub(super) fn read_i32(code: &[u8], pc: &mut usize) -> Result<i32, VmTrap> {
    if *pc + 4 > code.len() {
        return Err(VmTrap::BytecodeDecode(
            "vm operand read overflow (i32)".into(),
        ));
    }
    let bytes = [code[*pc], code[*pc + 1], code[*pc + 2], code[*pc + 3]];
    *pc += 4;
    Ok(i32::from_le_bytes(bytes))
}
