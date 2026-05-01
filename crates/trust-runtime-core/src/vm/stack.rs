use alloc::vec::Vec;

use crate::value::Value;

use super::VmTrap;

const MAX_OPERAND_STACK: usize = 16 * 1024;

#[derive(Debug, Default)]
pub struct OperandStack {
    values: Vec<Value>,
}

impl OperandStack {
    #[inline]
    /// Remove every value from the operand stack.
    pub fn clear(&mut self) {
        self.values.clear();
    }

    #[inline]
    /// Push one operand value, enforcing the VM stack limit.
    pub fn push(&mut self, value: Value) -> Result<(), VmTrap> {
        if self.values.len() >= MAX_OPERAND_STACK {
            return Err(VmTrap::StackOverflow);
        }
        self.values.push(value);
        Ok(())
    }

    #[inline]
    /// Pop one operand value.
    pub fn pop(&mut self) -> Result<Value, VmTrap> {
        self.values.pop().ok_or(VmTrap::StackUnderflow)
    }

    #[inline]
    /// Pop the right and left operands for a binary operation.
    pub fn pop_pair(&mut self) -> Result<(Value, Value), VmTrap> {
        let right = self.pop()?;
        let left = self.pop()?;
        Ok((left, right))
    }

    #[inline]
    /// Duplicate the top operand value.
    pub fn duplicate_top(&mut self) -> Result<(), VmTrap> {
        let value = self.values.last().cloned().ok_or(VmTrap::StackUnderflow)?;
        self.push(value)
    }

    #[inline]
    /// Swap the top two operand values.
    pub fn swap_top(&mut self) -> Result<(), VmTrap> {
        if self.values.len() < 2 {
            return Err(VmTrap::StackUnderflow);
        }
        let len = self.values.len();
        self.values.swap(len - 1, len - 2);
        Ok(())
    }
}
