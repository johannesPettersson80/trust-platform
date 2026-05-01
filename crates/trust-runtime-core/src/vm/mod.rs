//! Portable VM execution helpers.

#![allow(missing_docs)]

mod dispatch_ops;
mod errors;
mod frames;
mod helpers;
mod stack;

pub use dispatch_ops::{apply_jump, execute_binary, execute_unary, read_i32, read_u32};
pub use errors::VmTrap;
pub use frames::{ensure_global_call_depth, FrameStack, VmFrame, VM_MAX_CALL_DEPTH};
pub use helpers::{materialize_borrowed_value, opcode_operand_len};
pub use stack::OperandStack;

#[cfg(test)]
mod tests {
    use smol_str::SmolStr;

    use crate::{
        error::RuntimeError,
        memory::InstanceId,
        program_model::{BinaryOp, UnaryOp},
        value::{DateTimeProfile, Value},
    };

    use super::{FrameStack, OperandStack, VmFrame, VmTrap, VM_MAX_CALL_DEPTH};

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

    fn test_frame() -> VmFrame {
        VmFrame {
            pou_id: 7,
            return_pc: 11,
            code_start: 3,
            code_end: 19,
            local_ref_start: 40,
            local_ref_count: 2,
            locals: vec![Value::DInt(1), Value::String("local".into())],
            runtime_instance: Some(InstanceId(5)),
            instance_owner: Some(6),
        }
    }

    #[test]
    fn vm_frame_preserves_local_slot_bounds_and_materialization_contracts() {
        let mut frame = test_frame();

        assert_eq!(frame.local_slot_index(40).unwrap(), 0);
        assert_eq!(frame.local_slot_index(41).unwrap(), 1);
        assert_eq!(frame.load_local(40).unwrap(), Value::DInt(1));
        assert_eq!(frame.load_local(41).unwrap(), Value::String("local".into()));

        frame.store_local(40, Value::DInt(9)).unwrap();
        assert_eq!(frame.load_local(40).unwrap(), Value::DInt(9));

        assert!(matches!(
            frame.local_slot_index(39),
            Err(VmTrap::InvalidLocalRef {
                ref_index: 39,
                start: 40,
                count: 2
            })
        ));
        assert!(matches!(
            frame.load_local(42),
            Err(VmTrap::InvalidLocalRef {
                ref_index: 42,
                start: 40,
                count: 2
            })
        ));
    }

    #[test]
    fn vm_dispatch_ops_preserve_stack_jump_and_operand_decode_contracts() {
        let profile = DateTimeProfile::default();
        let mut stack = OperandStack::default();
        stack.push(Value::Int(2)).unwrap();
        stack.push(Value::Int(3)).unwrap();
        super::execute_binary(&profile, &mut stack, BinaryOp::Add).unwrap();
        assert_eq!(stack.pop().unwrap(), Value::Int(5));

        stack.push(Value::Int(3)).unwrap();
        super::execute_unary(&mut stack, UnaryOp::Neg).unwrap();
        assert_eq!(stack.pop().unwrap(), Value::Int(-3));

        let mut pc = 11;
        super::apply_jump(&mut pc, 2, &test_frame()).unwrap();
        assert_eq!(pc, 13);
        assert!(matches!(
            super::apply_jump(&mut pc, -20, &test_frame()),
            Err(VmTrap::InvalidJumpTarget(-7))
        ));

        let bytes = [0x78, 0x56, 0x34, 0x12, 0xff, 0xff, 0xff, 0xff];
        let mut read_pc = 0;
        assert_eq!(super::read_u32(&bytes, &mut read_pc).unwrap(), 0x1234_5678);
        assert_eq!(read_pc, 4);
        assert_eq!(super::read_i32(&bytes, &mut read_pc).unwrap(), -1);
        assert_eq!(read_pc, 8);
        assert!(matches!(
            super::read_u32(&bytes, &mut read_pc),
            Err(VmTrap::BytecodeDecode(message)) if message.as_str().contains("u32")
        ));
    }

    #[test]
    fn frame_stack_preserves_lifo_and_call_depth_contracts() {
        let mut frames = FrameStack::default();
        assert!(frames.is_empty());

        frames.push(test_frame()).unwrap();
        assert_eq!(frames.len(), 1);
        assert_eq!(frames.current().unwrap().pou_id, 7);
        frames.current_mut().unwrap().return_pc = 99;
        assert_eq!(frames.pop().unwrap().return_pc, 99);
        assert!(matches!(frames.pop(), Err(VmTrap::CallStackUnderflow)));

        for _ in 0..VM_MAX_CALL_DEPTH {
            frames.push(test_frame()).unwrap();
        }
        assert_eq!(frames.len(), VM_MAX_CALL_DEPTH);
        assert!(matches!(
            frames.push(test_frame()),
            Err(VmTrap::CallStackOverflow)
        ));

        frames.clear();
        assert!(frames.is_empty());
        assert!(super::ensure_global_call_depth(0, VM_MAX_CALL_DEPTH).is_ok());
        assert!(matches!(
            super::ensure_global_call_depth(1, VM_MAX_CALL_DEPTH),
            Err(VmTrap::CallStackOverflow)
        ));
    }
}
