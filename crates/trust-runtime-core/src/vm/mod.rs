//! Portable VM execution helpers.

#![allow(missing_docs)]

mod errors;
mod stack;

pub use errors::VmTrap;
pub use stack::OperandStack;

#[cfg(test)]
mod tests {
    use smol_str::SmolStr;

    use crate::{error::RuntimeError, value::Value};

    use super::{OperandStack, VmTrap};

    #[test]
    fn operand_stack_preserves_lifo_pair_and_swap_contracts() {
        let mut stack = OperandStack::default();

        stack.push(Value::Int(1)).unwrap();
        stack.push(Value::Int(2)).unwrap();
        stack.duplicate_top().unwrap();
        assert_eq!(stack.pop().unwrap(), Value::Int(2));

        stack.swap_top().unwrap();
        assert_eq!(stack.pop_pair().unwrap(), (Value::Int(2), Value::Int(1)));
        assert!(matches!(stack.pop(), Err(VmTrap::StackUnderflow)));
    }

    #[test]
    fn vm_trap_preserves_runtime_error_mapping() {
        assert!(matches!(
            VmTrap::ConditionNotBool.into_runtime_error(),
            RuntimeError::ConditionNotBool
        ));
        assert!(matches!(
            VmTrap::MissingProgram(SmolStr::new("Main")).into_runtime_error(),
            RuntimeError::UndefinedProgram(name) if name == "Main"
        ));
        assert!(matches!(
            VmTrap::InvalidOpcode(0xFF).into_runtime_error(),
            RuntimeError::InvalidBytecode(message) if message.contains("0xFF")
        ));
    }
}
