use super::*;
use crate::bytecode::{RefEntry, TypeEntry};
use alloc::vec;

fn validate(code: &[u8], symbol: &str) -> Result<(), BytecodeError> {
    let strings = StringTable {
        entries: vec![symbol.into()],
    };
    let pous = PouIndex::default();
    let types = TypeTable {
        offsets: vec![],
        entries: vec![TypeEntry {
            name_idx: None,
            kind: TypeKind::Primitive,
            data: TypeData::Primitive {
                prim_id: 1,
                max_length: 0,
            },
        }],
    };
    let constants = ConstPool::default();
    let refs = RefTable {
        entries: vec![
            RefEntry {
                location: RefLocation::InitializerResult,
                owner_id: 0,
                offset: 0,
                segments: vec![],
            },
            RefEntry {
                location: RefLocation::Global,
                owner_id: 0,
                offset: 0,
                segments: vec![],
            },
        ],
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
    )?;
    let instructions = decode_instructions(code, &mut 0, &mut budget, |_, _| Ok(()))?;
    validate_initializer_stack_shape(&tables, &instructions, code.len(), false, &mut budget)
}

fn emit(code: &mut Vec<u8>, opcode: u8, operand: u32) {
    code.push(opcode);
    code.extend_from_slice(&operand.to_le_bytes());
}

#[test]
fn initializer_dynamic_store_requires_staging_provenance() {
    for destination in [0, 1] {
        let mut code = Vec::new();
        emit(&mut code, 0x22, destination);
        emit(&mut code, crate::bytecode::opcodes::DEFAULT_TYPED, 0);
        code.push(0x33);
        let result = validate(&code, "");
        if destination == 0 {
            result.unwrap();
        } else {
            assert_eq!(
                result,
                Err(RejectionReason::InitializerWriteOutsideStaging.into())
            );
        }
    }
}

#[test]
fn initializer_merge_never_promotes_a_global_address_to_staging() {
    for (fallthrough, branch) in [(0, 1), (1, 0), (0, 0)] {
        let mut code = Vec::new();
        emit(&mut code, 0x20, 1); // unknown BOOL condition
        emit(&mut code, 0x03, 10); // target 20
        emit(&mut code, 0x22, fallthrough);
        emit(&mut code, 0x02, 5); // target 25
        emit(&mut code, 0x22, branch);
        emit(&mut code, crate::bytecode::opcodes::DEFAULT_TYPED, 0);
        code.push(0x33);
        let result = validate(&code, "");
        if fallthrough == branch {
            result.unwrap();
        } else {
            assert_eq!(
                result,
                Err(RejectionReason::InitializerWriteOutsideStaging.into())
            );
        }
    }
}

#[test]
fn initializer_native_targets_and_argument_encoding_are_checked() {
    for (reference, symbol, expected) in [
        (0, "NATIVE|T", Ok(())),
        (
            1,
            "NATIVE|T",
            Err(RejectionReason::InitializerWriteOutsideStaging.into()),
        ),
        (
            0,
            "NATIVE|E",
            Err(RejectionReason::InitializerReferenceEscape.into()),
        ),
        (1, "NATIVE|E", Ok(())),
        (
            0,
            "NATIVE|invalid",
            Err(RejectionReason::InvalidInitializerNativeArguments.into()),
        ),
        (
            0,
            "NATIVE",
            Err(RejectionReason::InvalidInitializerNativeArguments.into()),
        ),
    ] {
        let mut code = Vec::new();
        emit(&mut code, 0x22, reference);
        emit(&mut code, 0x09, crate::bytecode::NATIVE_CALL_KIND_STDLIB);
        code.extend_from_slice(&0u32.to_le_bytes());
        code.extend_from_slice(&1u32.to_le_bytes());
        code.push(0x12);
        assert_eq!(validate(&code, symbol), expected, "{reference}: {symbol}");
    }
}
