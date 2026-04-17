use crate::value::Value;

use super::errors::VmTrap;

const MAX_OPERAND_STACK: usize = 16 * 1024;

#[derive(Debug, Default)]
pub(super) struct OperandStack {
    values: Vec<Value>,
}

impl OperandStack {
    #[inline]
    pub(super) fn clear(&mut self) {
        self.values.clear();
    }

    #[inline]
    pub(super) fn push(&mut self, value: Value) -> Result<(), VmTrap> {
        if self.values.len() >= MAX_OPERAND_STACK {
            return Err(VmTrap::StackOverflow);
        }
        self.values.push(value);
        Ok(())
    }

    #[inline]
    pub(super) fn pop(&mut self) -> Result<Value, VmTrap> {
        self.values.pop().ok_or(VmTrap::StackUnderflow)
    }

    #[inline]
    pub(super) fn pop_pair(&mut self) -> Result<(Value, Value), VmTrap> {
        let right = self.pop()?;
        let left = self.pop()?;
        Ok((left, right))
    }

    #[inline]
    pub(super) fn duplicate_top(&mut self) -> Result<(), VmTrap> {
        let value = self.values.last().cloned().ok_or(VmTrap::StackUnderflow)?;
        self.push(value)
    }

    #[inline]
    pub(super) fn swap_top(&mut self) -> Result<(), VmTrap> {
        if self.values.len() < 2 {
            return Err(VmTrap::StackUnderflow);
        }
        let len = self.values.len();
        self.values.swap(len - 1, len - 2);
        Ok(())
    }
}
