use super::*;
use crate::bytecode::{
    RefEntry, TypeEntry, NATIVE_CALL_KIND_FUNCTION, NATIVE_CALL_KIND_FUNCTION_BLOCK,
    NATIVE_CALL_KIND_STDLIB,
};
use alloc::vec;

const LOAD_GLOBAL: u8 = 0x20;
const POP: u8 = 0x12;
const CALL_NATIVE: u8 = 0x09;

/// Validate each body as a POU body sharing one budget, returning its statistics.
fn validate(bodies: &[Vec<u8>]) -> ValidationStats {
    let strings = StringTable {
        entries: vec!["Callee".into()],
    };
    let pous = PouIndex::default();
    let types = TypeTable {
        offsets: vec![],
        entries: vec![TypeEntry {
            name_idx: None,
            kind: TypeKind::Primitive,
            data: TypeData::Primitive {
                prim_id: 6,
                max_length: 0,
            },
        }],
    };
    let constants = ConstPool::default();
    let refs = RefTable {
        entries: vec![RefEntry {
            location: RefLocation::Global,
            owner_id: 0,
            offset: 0,
            segments: vec![],
        }],
    };
    let mut budget = ValidationBudget::new(ValidationLimits::default());
    let tables = ValidationContext::new(
        &strings,
        &pous,
        &types,
        &constants,
        &refs,
        None,
        &mut budget,
    )
    .unwrap();
    for code in bodies {
        let instructions = decode_instructions(code, &mut 0, &mut budget, |_, _| Ok(())).unwrap();
        validate_stack_shape(&tables, &instructions, code.len(), &mut budget).unwrap();
    }
    budget.stats
}

fn loads(code: &mut Vec<u8>, count: usize) {
    for _ in 0..count {
        code.push(LOAD_GLOBAL);
        code.extend_from_slice(&0u32.to_le_bytes());
    }
}

fn pops(code: &mut Vec<u8>, count: usize) {
    code.extend(core::iter::repeat_n(POP, count));
}

fn call(code: &mut Vec<u8>, kind: u32, arguments: u32) {
    code.push(CALL_NATIVE);
    for operand in [kind, 0, arguments] {
        code.extend_from_slice(&operand.to_le_bytes());
    }
}

/// Spec 34 MEM-03: the validator records the deepest operand stack of a POU body, and a body
/// that calls no user POU records no call residue, so no activation can nest.
#[test]
fn a_body_without_user_calls_records_its_depth_and_no_call_residue() {
    let mut body = Vec::new();
    loads(&mut body, 3);
    call(&mut body, NATIVE_CALL_KIND_STDLIB, 2);
    pops(&mut body, 2);
    let stats = validate(&[body]);
    assert_eq!(stats.max_operand_depth, 3);
    assert_eq!(stats.max_call_operand_residue, None);
}

/// Spec 34 MEM-03: a user call keeps the operands below its arguments while the callee
/// runs; the validator records the most of any call, and the deepest body over all bodies.
#[test]
fn user_calls_record_the_operands_kept_below_their_arguments_over_all_bodies() {
    let mut caller = Vec::new();
    loads(&mut caller, 3);
    call(&mut caller, NATIVE_CALL_KIND_FUNCTION, 1);
    pops(&mut caller, 3);
    let mut block_call = Vec::new();
    loads(&mut block_call, 1);
    call(&mut block_call, NATIVE_CALL_KIND_FUNCTION_BLOCK, 1);
    pops(&mut block_call, 1);
    let mut deep = Vec::new();
    loads(&mut deep, 5);
    pops(&mut deep, 5);
    let stats = validate(&[caller, block_call, deep]);
    assert_eq!(stats.max_operand_depth, 5);
    assert_eq!(stats.max_call_operand_residue, Some(2));
}
