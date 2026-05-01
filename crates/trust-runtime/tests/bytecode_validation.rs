mod bytecode_helpers;

use bytecode_helpers::{base_module, module_with_debug};
use trust_runtime::bytecode::{BytecodeError, BytecodeModule, SectionData, SectionId};

#[test]
fn opcode_validation() {
    let mut module = base_module();
    if let Some(SectionData::PouBodies(bodies)) = module.section_mut(SectionId::PouBodies) {
        *bodies = vec![0xFF];
    }
    let bytes = module.encode().expect("encode");
    let decoded = BytecodeModule::decode(&bytes).expect("decode");
    let err = decoded.validate().unwrap_err();
    assert!(matches!(err, BytecodeError::InvalidOpcode(0xFF)));
}

#[test]
fn validator_rejects_unsupported_runtime_opcodes_before_dispatch() {
    let cases = [
        (0x07, "CALL_METHOD"),
        (0x08, "CALL_VIRTUAL"),
        (0x14, "ROT3"),
        (0x15, "ROT4"),
        (0x16, "CAST_IMPLICIT"),
        (0x4A, "SHL"),
        (0x4B, "SHR"),
        (0x4D, "ROL"),
        (0x4E, "ROR"),
    ];

    for (opcode, name) in cases {
        let mut module = base_module();
        if let Some(SectionData::PouBodies(bodies)) = module.section_mut(SectionId::PouBodies) {
            *bodies = vec![opcode];
        }
        if let Some(SectionData::PouIndex(index)) = module.section_mut(SectionId::PouIndex) {
            index.entries[0].code_length = 1;
        }

        let bytes = module.encode().expect("encode");
        let decoded = BytecodeModule::decode(&bytes).expect("decode");
        let err = decoded.validate().unwrap_err();
        assert!(
            matches!(err, BytecodeError::InvalidSection(ref message) if message.contains(&format!("unsupported runtime opcode {name}"))),
            "unexpected validation error for {name}: {err:?}"
        );
    }
}

#[test]
fn opcode_validation_extended() {
    let mut module = base_module();
    if let Some(SectionData::PouBodies(bodies)) = module.section_mut(SectionId::PouBodies) {
        let mut code = vec![0x25, 0x31, 0x32, 0x33, 0x4C, 0x09];
        code.extend_from_slice(&0_u32.to_le_bytes());
        code.extend_from_slice(&0_u32.to_le_bytes());
        code.extend_from_slice(&0_u32.to_le_bytes());
        *bodies = code;
    }
    if let Some(SectionData::PouIndex(index)) = module.section_mut(SectionId::PouIndex) {
        index.entries[0].code_length = 18;
    }
    let bytes = module.encode().expect("encode");
    let decoded = BytecodeModule::decode(&bytes).expect("decode");
    decoded.validate().expect("validate");
}

#[test]
fn jump_validation() {
    let mut module = base_module();
    if let Some(SectionData::PouBodies(bodies)) = module.section_mut(SectionId::PouBodies) {
        let mut code = vec![0x02];
        code.extend_from_slice(&100i32.to_le_bytes());
        *bodies = code;
    }
    if let Some(SectionData::PouIndex(index)) = module.section_mut(SectionId::PouIndex) {
        index.entries[0].code_length = 5;
    }
    let bytes = module.encode().expect("encode");
    let decoded = BytecodeModule::decode(&bytes).expect("decode");
    let err = decoded.validate().unwrap_err();
    assert!(matches!(err, BytecodeError::InvalidJumpTarget(_)));
}

#[test]
fn call_validation() {
    let mut module = base_module();
    if let Some(SectionData::PouBodies(bodies)) = module.section_mut(SectionId::PouBodies) {
        let mut code = vec![0x05];
        code.extend_from_slice(&99u32.to_le_bytes());
        *bodies = code;
    }
    if let Some(SectionData::PouIndex(index)) = module.section_mut(SectionId::PouIndex) {
        index.entries[0].code_length = 5;
    }
    let bytes = module.encode().expect("encode");
    let decoded = BytecodeModule::decode(&bytes).expect("decode");
    let err = decoded.validate().unwrap_err();
    assert!(matches!(err, BytecodeError::InvalidPouId(99)));
}

#[test]
fn debug_map_validation() {
    let mut module = module_with_debug();
    if let Some(SectionData::DebugMap(map)) = module.section_mut(SectionId::DebugMap) {
        map.entries[0].code_offset = 2;
    }
    let bytes = module.encode().expect("encode");
    let decoded = BytecodeModule::decode(&bytes).expect("decode");
    let err = decoded.validate().unwrap_err();
    assert!(matches!(err, BytecodeError::InvalidSection(_)));
}
