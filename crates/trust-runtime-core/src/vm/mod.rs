//! Portable VM execution helpers.

#![allow(missing_docs)]

mod errors;
mod helpers;
mod stack;

pub use errors::VmTrap;
pub use helpers::{materialize_borrowed_value, opcode_operand_len};
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

    #[test]
    fn vm_helpers_preserve_opcode_and_borrow_materialization_contracts() {
        assert_eq!(super::opcode_operand_len(0x00), Some(0));
        assert_eq!(super::opcode_operand_len(0x02), Some(4));
        assert_eq!(super::opcode_operand_len(0x08), Some(8));
        assert_eq!(super::opcode_operand_len(0x09), Some(12));
        assert_eq!(super::opcode_operand_len(0x16), Some(1));
        assert_eq!(super::opcode_operand_len(0xFF), None);

        assert_eq!(
            super::materialize_borrowed_value(&Value::DInt(7)),
            (Value::DInt(7), false)
        );
        assert_eq!(
            super::materialize_borrowed_value(&Value::String("x".into())),
            (Value::String("x".into()), true)
        );
    }
}
