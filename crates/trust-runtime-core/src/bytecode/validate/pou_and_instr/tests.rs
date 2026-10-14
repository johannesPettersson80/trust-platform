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
