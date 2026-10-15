use super::*;
use alloc::vec;

#[test]
fn reference_attempt_reports_invalid_type_index_before_type_kind() {
    let strings = StringTable::default();
    let index = PouIndex::default();
    let types = TypeTable::default();
    let const_pool = ConstPool::default();
    let refs = RefTable::default();
    let mut budget = ValidationBudget::new(ValidationLimits::default());
    let tables = ValidationContext::new(
        &strings,
        &index,
        &types,
        &const_pool,
        &refs,
        None,
        &mut budget,
    )
    .unwrap();
    let instruction = Instruction {
        pc: 0,
        opcode: 0x64,
        operands: [0, 0, 0],
    };

    assert_eq!(
        validate_instruction_operands(&tables, None, &instruction, &mut budget),
        Err(BytecodeError::InvalidIndex {
            kind: "type".into(),
            index: 0,
        })
    );
}

#[test]
fn reference_attempt_checks_valid_type_kind_after_index() {
    let strings = StringTable::default();
    let index = PouIndex::default();
    let types = TypeTable {
        offsets: Vec::new(),
        entries: vec![TypeEntry {
            kind: TypeKind::Primitive,
            name_idx: None,
            data: TypeData::Primitive {
                prim_id: 7,
                max_length: 0,
            },
        }],
    };
    let const_pool = ConstPool::default();
    let refs = RefTable::default();
    let mut budget = ValidationBudget::new(ValidationLimits::default());
    let tables = ValidationContext::new(
        &strings,
        &index,
        &types,
        &const_pool,
        &refs,
        None,
        &mut budget,
    )
    .unwrap();
    let instruction = Instruction {
        pc: 0,
        opcode: 0x64,
        operands: [0, 0, 0],
    };

    assert_eq!(
        validate_instruction_operands(&tables, None, &instruction, &mut budget),
        Err(BytecodeError::from(
            RejectionReason::ReferenceAttemptExpectsReferenceOrInterfaceTargetType,
        ))
    );
}

#[test]
fn call_native_argument_count_admits_1024_and_refuses_1025() {
    use crate::bytecode::NATIVE_CALL_KIND_FUNCTION;

    // Kind 0 and symbol 0 are valid, so arg_count alone decides the outcome.
    // Count check: pou_and_instr.rs:214-218; reason text: reasons.rs:229.
    let strings = StringTable {
        entries: vec!["Native".into()],
    };
    let index = PouIndex::default();
    let types = TypeTable::default();
    let const_pool = ConstPool::default();
    let refs = RefTable::default();
    let mut budget = ValidationBudget::new(ValidationLimits::default());
    let tables = ValidationContext::new(
        &strings,
        &index,
        &types,
        &const_pool,
        &refs,
        None,
        &mut budget,
    )
    .unwrap();
    let call = |arg_count| Instruction {
        pc: 0,
        opcode: 0x09,
        operands: [NATIVE_CALL_KIND_FUNCTION, 0, arg_count],
    };

    assert_eq!(
        validate_instruction_operands(&tables, None, &call(1024), &mut budget),
        Ok(())
    );
    assert_eq!(
        validate_instruction_operands(&tables, None, &call(1025), &mut budget),
        Err(BytecodeError::from(
            RejectionReason::CallNativeArgCountOutOfRange
        ))
    );
}
