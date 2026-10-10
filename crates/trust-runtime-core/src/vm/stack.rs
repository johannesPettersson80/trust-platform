use alloc::vec::Vec;

use crate::value::Value;

use super::{VmTrap, VM_MAX_OPERAND_STACK};

/// Bounded operand storage used by the shared interpreter and hosted tiers.
#[derive(Debug, Default)]
pub struct OperandStack {
    values: Vec<Value>,
    floor: usize,
}

impl OperandStack {
    pub(super) fn enter_call(&mut self) -> (usize, usize) {
        let previous = self.floor;
        self.floor = self.values.len();
        (self.floor, previous)
    }
    pub(super) fn leave_call(&mut self, base: usize, previous: usize) {
        self.values.truncate(base);
        self.floor = previous;
    }

    /// Remove every value from the operand stack.
    #[inline]
    pub fn clear(&mut self) {
        self.values.clear();
        self.floor = 0;
    }

    /// Push one operand value, enforcing the VM stack limit.
    #[inline]
    pub fn push(&mut self, value: Value) -> Result<(), VmTrap> {
        if self.values.len() - self.floor >= VM_MAX_OPERAND_STACK {
            return Err(VmTrap::StackOverflow);
        }
        self.values.push(value);
        Ok(())
    }

    /// Pop one operand value.
    #[inline]
    pub fn pop(&mut self) -> Result<Value, VmTrap> {
        if self.values.len() == self.floor {
            return Err(VmTrap::StackUnderflow);
        }
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
        self.values[self.floor..]
            .last()
            .ok_or(VmTrap::StackUnderflow)
    }

    /// Duplicate the top operand value.
    #[inline]
    #[cfg(any(feature = "hir", test))]
    pub fn duplicate_top(&mut self) -> Result<(), VmTrap> {
        let value = self.peek()?.clone();
        self.push(value)
    }

    /// Swap the top two operand values.
    #[inline]
    pub fn swap_top(&mut self) -> Result<(), VmTrap> {
        if self.values.len() - self.floor < 2 {
            return Err(VmTrap::StackUnderflow);
        }
        let len = self.values.len();
        self.values.swap(len - 1, len - 2);
        Ok(())
    }
}

#[cfg(test)]
mod continuation_tests {
    use super::*;
    #[test]
    fn deferred_calls_isolate_operands_and_discard_only_callee_residue() {
        let mut stack = OperandStack::default();
        stack.push(Value::Int(7)).unwrap();
        let outer = stack.enter_call();
        assert!(matches!(stack.pop(), Err(VmTrap::StackUnderflow)));
        assert!(matches!(stack.peek(), Err(VmTrap::StackUnderflow)));
        assert!(matches!(stack.swap_top(), Err(VmTrap::StackUnderflow)));
        stack.push(Value::Int(9)).unwrap();
        assert!(matches!(stack.swap_top(), Err(VmTrap::StackUnderflow)));
        let inner = stack.enter_call();
        stack.push(Value::Int(11)).unwrap();
        stack.push(Value::Int(13)).unwrap();
        stack.leave_call(inner.0, inner.1);
        assert_eq!(stack.pop().unwrap(), Value::Int(9));
        assert!(matches!(stack.pop(), Err(VmTrap::StackUnderflow)));
        stack.leave_call(outer.0, outer.1);
        assert_eq!(stack.pop().unwrap(), Value::Int(7));
    }
    #[test]
    fn operand_limit_is_per_callee_not_the_sum_of_suspended_operands() {
        let mut stack = OperandStack::default();
        for _ in 0..VM_MAX_OPERAND_STACK {
            stack.push(Value::Null).unwrap();
        }
        assert!(matches!(
            stack.push(Value::Null),
            Err(VmTrap::StackOverflow)
        ));
        let scope = stack.enter_call();
        for _ in 0..VM_MAX_OPERAND_STACK {
            stack.push(Value::Null).unwrap();
        }
        assert!(matches!(
            stack.push(Value::Null),
            Err(VmTrap::StackOverflow)
        ));
        stack.leave_call(scope.0, scope.1);
        assert!(matches!(
            stack.push(Value::Null),
            Err(VmTrap::StackOverflow)
        ));
        stack.clear();
        stack.push(Value::Int(1)).unwrap();
        assert_eq!(stack.pop().unwrap(), Value::Int(1));
    }
}
