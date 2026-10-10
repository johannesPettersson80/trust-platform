use alloc::vec::Vec;

use crate::value::Value;

use super::{VmTrap, VM_MAX_OPERAND_STACK};

/// Bounded operand storage used by the shared interpreter and hosted tiers.
#[derive(Debug, Default)]
pub struct OperandStack {
    values: Vec<Value>,
}

impl OperandStack {
    /// Remove every value from the operand stack.
    #[inline]
    pub fn clear(&mut self) {
        self.values.clear();
    }

    /// Push one operand value, enforcing the VM stack limit.
    #[inline]
    pub fn push(&mut self, value: Value) -> Result<(), VmTrap> {
        if self.values.len() >= VM_MAX_OPERAND_STACK {
            return Err(VmTrap::StackOverflow);
        }
        self.values.push(value);
        Ok(())
    }

    /// Pop one operand value.
    #[inline]
    pub fn pop(&mut self) -> Result<Value, VmTrap> {
        self.values.pop().ok_or(VmTrap::StackUnderflow)
    }

    /// Pop the right and left operands for a binary operation.
    #[inline]
    pub fn pop_pair(&mut self) -> Result<(Value, Value), VmTrap> {
        let right = self.pop()?;
        let left = self.pop()?;
        Ok((left, right))
    }

    /// Borrow the next value without allocating or changing the stack.
    #[inline]
    pub fn peek(&self) -> Result<&Value, VmTrap> {
        self.values.last().ok_or(VmTrap::StackUnderflow)
    }

    /// Duplicate the top operand value.
    #[inline]
    #[cfg(any(feature = "hir", test))]
    pub fn duplicate_top(&mut self) -> Result<(), VmTrap> {
        let value = self.values.last().cloned().ok_or(VmTrap::StackUnderflow)?;
        self.push(value)
    }

    /// Swap the top two operand values.
    #[inline]
    pub fn swap_top(&mut self) -> Result<(), VmTrap> {
        if self.values.len() < 2 {
            return Err(VmTrap::StackUnderflow);
        }
        let len = self.values.len();
        self.values.swap(len - 1, len - 2);
        Ok(())
    }
}
