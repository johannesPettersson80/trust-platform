//! Portable VM execution helpers.

#![allow(missing_docs)]

mod const_pool;
mod dispatch_ops;
mod dispatch_sizeof;
mod errors;
mod frames;
mod helpers;
mod stack;

pub use const_pool::decode_const_pool_entries;
pub use dispatch_ops::{apply_jump, execute_binary, execute_unary, read_i32, read_u32};
pub use dispatch_sizeof::sizeof_type_from_table;
pub use errors::VmTrap;
pub use frames::{ensure_global_call_depth, FrameStack, VmFrame, VM_MAX_CALL_DEPTH};
pub use helpers::{materialize_borrowed_value, opcode_operand_len};
pub use stack::OperandStack;

#[cfg(test)]
mod tests {
    use alloc::{boxed::Box, vec, vec::Vec};
    use smol_str::SmolStr;

    use crate::{
        bytecode::{
            ConstEntry, ConstPool, EnumVariant, Field, StringTable, TypeData, TypeEntry, TypeKind,
            TypeTable,
        },
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
        assert_eq!(super::opcode_operand_len(0x62), Some(4));
        assert_eq!(super::opcode_operand_len(0x63), Some(4));
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

    fn type_entry(kind: TypeKind, data: TypeData) -> TypeEntry {
        TypeEntry {
            kind,
            name_idx: None,
            data,
        }
    }

    fn const_entry(type_id: u32, payload: Vec<u8>) -> ConstEntry {
        ConstEntry { type_id, payload }
    }

    #[test]
    fn vm_const_pool_decoder_preserves_primitive_enum_and_alias_contracts() {
        let strings = StringTable {
            entries: vec![SmolStr::new("Color"), SmolStr::new("Red")],
        };
        let types = TypeTable {
            offsets: vec![],
            entries: vec![
                type_entry(
                    TypeKind::Primitive,
                    TypeData::Primitive {
                        prim_id: 1,
                        max_length: 0,
                    },
                ),
                type_entry(
                    TypeKind::Primitive,
                    TypeData::Primitive {
                        prim_id: 8,
                        max_length: 0,
                    },
                ),
                type_entry(
                    TypeKind::Primitive,
                    TypeData::Primitive {
                        prim_id: 24,
                        max_length: 16,
                    },
                ),
                type_entry(
                    TypeKind::Primitive,
                    TypeData::Primitive {
                        prim_id: 25,
                        max_length: 16,
                    },
                ),
                TypeEntry {
                    kind: TypeKind::Enum,
                    name_idx: Some(0),
                    data: TypeData::Enum {
                        base_type_id: 1,
                        variants: vec![EnumVariant {
                            name_idx: 1,
                            value: 2,
                        }],
                    },
                },
                type_entry(TypeKind::Alias, TypeData::Alias { target_type_id: 1 }),
                type_entry(
                    TypeKind::Subrange,
                    TypeData::Subrange {
                        base_type_id: 1,
                        lower: -10,
                        upper: 10,
                    },
                ),
            ],
        };
        let const_pool = ConstPool {
            entries: vec![
                const_entry(0, vec![1]),
                const_entry(1, 42_i32.to_le_bytes().to_vec()),
                const_entry(2, b"abc".to_vec()),
                const_entry(3, vec![b'A', 0]),
                const_entry(4, 2_i64.to_le_bytes().to_vec()),
                const_entry(5, (-7_i32).to_le_bytes().to_vec()),
                const_entry(6, 5_i32.to_le_bytes().to_vec()),
            ],
        };

        let decoded = super::decode_const_pool_entries(&const_pool, &types, &strings).unwrap();
        assert_eq!(decoded[0], Value::Bool(true));
        assert_eq!(decoded[1], Value::DInt(42));
        assert_eq!(decoded[2], Value::String("abc".into()));
        assert_eq!(decoded[3], Value::WString("A".into()));
        assert_eq!(
            decoded[4],
            Value::Enum(Box::new(crate::value::EnumValue::from_canonical_parts(
                "Color".into(),
                "Red".into(),
                2
            )))
        );
        assert_eq!(decoded[5], Value::DInt(-7));
        assert_eq!(decoded[6], Value::DInt(5));
    }

    #[test]
    fn vm_const_pool_decoder_rejects_bad_payload_and_type_shapes() {
        let strings = StringTable::default();
        let string_types = TypeTable {
            offsets: vec![],
            entries: vec![type_entry(
                TypeKind::Primitive,
                TypeData::Primitive {
                    prim_id: 24,
                    max_length: 16,
                },
            )],
        };
        let bad_utf8 = ConstPool {
            entries: vec![const_entry(0, vec![0xff])],
        };
        assert!(matches!(
            super::decode_const_pool_entries(&bad_utf8, &string_types, &strings),
            Err(RuntimeError::InvalidBytecode(message)) if message.as_str().contains("UTF-8")
        ));

        let unsupported_types = TypeTable {
            offsets: vec![],
            entries: vec![type_entry(
                TypeKind::Struct,
                TypeData::Struct { fields: vec![] },
            )],
        };
        let unsupported = ConstPool {
            entries: vec![const_entry(0, vec![])],
        };
        assert!(matches!(
            super::decode_const_pool_entries(&unsupported, &unsupported_types, &strings),
            Err(RuntimeError::InvalidBytecode(message)) if message.as_str().contains("unsupported const type kind")
        ));
    }

    #[test]
    fn vm_sizeof_helpers_preserve_type_table_contracts() {
        let types = TypeTable {
            offsets: vec![],
            entries: vec![
                type_entry(
                    TypeKind::Primitive,
                    TypeData::Primitive {
                        prim_id: 8,
                        max_length: 0,
                    },
                ),
                type_entry(
                    TypeKind::Array,
                    TypeData::Array {
                        elem_type_id: 0,
                        dims: vec![(1, 3)],
                    },
                ),
                type_entry(
                    TypeKind::Struct,
                    TypeData::Struct {
                        fields: vec![
                            Field {
                                name_idx: 0,
                                type_id: 0,
                            },
                            Field {
                                name_idx: 1,
                                type_id: 1,
                            },
                        ],
                    },
                ),
                type_entry(
                    TypeKind::Reference,
                    TypeData::Reference { target_type_id: 0 },
                ),
                type_entry(TypeKind::Alias, TypeData::Alias { target_type_id: 2 }),
            ],
        };

        assert_eq!(super::sizeof_type_from_table(&types, 0).unwrap(), 4);
        assert_eq!(super::sizeof_type_from_table(&types, 1).unwrap(), 12);
        assert_eq!(super::sizeof_type_from_table(&types, 2).unwrap(), 16);
        assert_eq!(
            super::sizeof_type_from_table(&types, 3).unwrap(),
            core::mem::size_of::<usize>() as u64
        );
        assert_eq!(super::sizeof_type_from_table(&types, 4).unwrap(), 16);
        let recursive = TypeTable {
            offsets: vec![],
            entries: vec![type_entry(
                TypeKind::Alias,
                TypeData::Alias { target_type_id: 0 },
            )],
        };
        assert!(matches!(
            super::sizeof_type_from_table(&recursive, 0),
            Err(RuntimeError::InvalidBytecode(message)) if message.as_str().contains("recursion")
        ));
        assert!(matches!(
            super::sizeof_type_from_table(&types, 99),
            Err(RuntimeError::InvalidBytecode(message)) if message.as_str().contains("invalid type index")
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
