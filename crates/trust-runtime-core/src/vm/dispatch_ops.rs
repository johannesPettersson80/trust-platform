use crate::program_model::{apply_binary, apply_unary, BinaryOp, UnaryOp};
use crate::value::{DateTimeProfile, Value};

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

/// Whether two operands of which at least one is a reference are equal, as `apply_binary` decides
/// `=` for them: a null reference and `NULL` equal each other and nothing else, and two references
/// are equal when they are the same reference, to the same root by the same path in the same
/// generation. `None` when neither operand is a reference.
pub fn reference_equality(left: &Value, right: &Value) -> Option<bool> {
    if !matches!(left, Value::Reference(_)) && !matches!(right, Value::Reference(_)) {
        return None;
    }
    let null = |value: &Value| matches!(value, Value::Null | Value::Reference(None));
    Some(if null(left) || null(right) {
        null(left) && null(right)
    } else {
        left == right
    })
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

#[cfg(test)]
mod reference_equality_tests {
    use super::*;
    use crate::memory::{Generation, InstanceId, MemoryLocation};
    use crate::value::{RefSegment, ValueRef};

    fn reference(slot: u32, index: i64) -> Value {
        Value::Reference(Some(ValueRef {
            location: MemoryLocation::Instance(InstanceId(1)),
            offset: slot,
            generation: Generation(1),
            path: alloc::vec![RefSegment::Index(alloc::vec![index])],
        }))
    }

    /// IEC 61131-3 §6.6.6 (references): `=` of two references is true when both are null or both
    /// name the same variable. The engine decides it without moving the references, and every pair
    /// below gets the answer `apply_binary` gives it. Catches a comparison that treats a null
    /// reference as equal to a non-null one, that compares only the root and not the element, or
    /// that answers for operands that are no reference.
    #[test]
    fn references_compare_as_the_binary_operator_compares_them() {
        let profile = DateTimeProfile::default();
        let pairs = [
            (Value::Reference(None), Value::Null, true),
            (Value::Reference(None), Value::Reference(None), true),
            (reference(2, 1), Value::Null, false),
            (Value::Null, reference(2, 1), false),
            (reference(2, 1), reference(2, 1), true),
            (reference(2, 1), reference(2, 3), false),
            (reference(2, 1), reference(4, 1), false),
        ];
        for (left, right, equal) in pairs {
            assert_eq!(
                reference_equality(&left, &right),
                Some(equal),
                "{left:?} = {right:?}"
            );
            assert_eq!(
                apply_binary(BinaryOp::Eq, left.clone(), right.clone(), &profile).unwrap(),
                Value::Bool(equal),
                "{left:?} = {right:?}"
            );
        }
        assert_eq!(reference_equality(&Value::Int(1), &Value::Int(1)), None);
        assert_eq!(reference_equality(&Value::Null, &Value::Null), None);
    }
}
