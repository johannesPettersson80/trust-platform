mod bytecode_helpers;

use bytecode_helpers::{base_module, module_with_debug};
use trust_runtime::bytecode::{
    BytecodeError, BytecodeModule, ConstEntry, PouKind, RefEntry, RefLocation, Section,
    SectionData, SectionId, TypeData, TypeEntry, TypeKind, VarMeta, VarMetaEntry,
};
use trust_runtime::error::{RuntimeError, StableErrorCode};
use trust_runtime::harness::bytecode_module_from_source;
use trust_runtime::Runtime;

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
    if let Some(SectionData::TypeTable(types)) = module.section_mut(SectionId::TypeTable) {
        types.entries.push(TypeEntry {
            kind: TypeKind::Primitive,
            name_idx: None,
            data: TypeData::Primitive {
                prim_id: 8,
                max_length: 0,
            },
        });
        types.offsets = vec![12, 24];
    }
    if let Some(SectionData::ConstPool(pool)) = module.section_mut(SectionId::ConstPool) {
        pool.entries.push(ConstEntry {
            type_id: 1,
            payload: 2_i32.to_le_bytes().to_vec(),
        });
    }
    let mut code_len = 0;
    if let Some(SectionData::PouBodies(bodies)) = module.section_mut(SectionId::PouBodies) {
        let mut code = vec![0x25, 0x10];
        code.extend_from_slice(&0_u32.to_le_bytes());
        code.extend_from_slice(&[0x31, 0x32, 0x12, 0x25, 0x10]);
        code.extend_from_slice(&0_u32.to_le_bytes());
        code.extend_from_slice(&[0x33, 0x10]);
        code.extend_from_slice(&0_u32.to_le_bytes());
        code.push(0x10);
        code.extend_from_slice(&0_u32.to_le_bytes());
        code.extend_from_slice(&[0x4C, 0x12, 0x09]);
        code.extend_from_slice(&0_u32.to_le_bytes());
        code.extend_from_slice(&0_u32.to_le_bytes());
        code.extend_from_slice(&0_u32.to_le_bytes());
        code.push(0x12);
        code_len = code.len() as u32;
        *bodies = code;
    }
    if let Some(SectionData::PouIndex(index)) = module.section_mut(SectionId::PouIndex) {
        index.entries[0].code_length = code_len;
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
fn jump_target_must_land_on_instruction_boundary() {
    let mut module = base_module();
    if let Some(SectionData::PouBodies(bodies)) = module.section_mut(SectionId::PouBodies) {
        let mut code = vec![0x02];
        code.extend_from_slice(&(-4_i32).to_le_bytes());
        code.push(0x01);
        *bodies = code;
    }
    if let Some(SectionData::PouIndex(index)) = module.section_mut(SectionId::PouIndex) {
        index.entries[0].code_length = 6;
    }

    let bytes = module.encode().expect("encode");
    let decoded = BytecodeModule::decode(&bytes).expect("decode");
    let err = decoded
        .validate()
        .expect_err("jump into operand byte must fail");
    assert!(matches!(err, BytecodeError::InvalidJumpTarget(1)));
}

#[test]
fn required_sections_are_rejected_before_runtime_apply() {
    let required = [
        SectionId::StringTable,
        SectionId::TypeTable,
        SectionId::ConstPool,
        SectionId::RefTable,
        SectionId::PouIndex,
        SectionId::PouBodies,
        SectionId::ResourceMeta,
        SectionId::IoMap,
    ];

    for section_id in required {
        let mut module = base_module();
        module.flags = 0;
        module
            .sections
            .retain(|section| section.id != section_id.as_raw());
        let bytes = module
            .encode()
            .expect("encode module missing required section");
        let decoded = BytecodeModule::decode(&bytes).expect("decode structurally valid module");
        assert!(
            matches!(decoded.validate(), Err(BytecodeError::MissingSection(_))),
            "direct validation accepted module missing {section_id:?}"
        );
        assert!(
            matches!(
                Runtime::new().apply_bytecode_bytes(&bytes, None),
                Err(RuntimeError::Bytecode {
                    code: StableErrorCode::BytecodeMissingSection,
                    ..
                })
            ),
            "product apply accepted module missing {section_id:?}"
        );
    }
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

#[test]
fn var_meta_rejects_duplicate_ref_idx() {
    let (mut module, local) = module_with_valid_scoped_local_meta();
    let duplicate = local_entry(&module, local.ref_idx).clone();
    var_meta_mut(&mut module).entries.push(duplicate);

    assert_invalid_section_contains(
        &module,
        &format!("duplicate VAR_META ref_idx {}", local.ref_idx),
    );
}

#[test]
fn var_meta_rejects_duplicate_textual_name_at_different_string_indices() {
    let (mut module, local) = module_with_valid_scoped_local_meta();
    let original = local_entry(&module, local.ref_idx).clone();
    let original_name = match module.section(SectionId::StringTable) {
        Some(SectionData::StringTable(strings)) => {
            strings.entries[original.name_idx as usize].clone()
        }
        other => panic!("expected STRING_TABLE, got {other:?}"),
    };
    let duplicate_name_idx = match module.section_mut(SectionId::StringTable) {
        Some(SectionData::StringTable(strings)) => {
            strings.entries.push(original_name);
            (strings.entries.len() - 1) as u32
        }
        other => panic!("expected STRING_TABLE, got {other:?}"),
    };
    let alternate_ref_idx = local.ref_idx + 1;
    var_meta_mut(&mut module)
        .entries
        .retain(|entry| entry.ref_idx != alternate_ref_idx);
    var_meta_mut(&mut module).entries.push(VarMetaEntry {
        name_idx: duplicate_name_idx,
        ref_idx: alternate_ref_idx,
        ..original
    });

    assert_invalid_section_contains(&module, "duplicate VAR_META name");
}

#[test]
fn pou_local_ranges_reject_shared_frame_owner() {
    let (mut module, local) = module_with_valid_scoped_local_meta();
    let new_ref_idx = match module.section_mut(SectionId::RefTable) {
        Some(SectionData::RefTable(refs)) => {
            let owner_id = refs.entries[local.ref_idx as usize].owner_id;
            let new_ref_idx = refs.entries.len() as u32;
            refs.entries.push(RefEntry {
                location: RefLocation::Local,
                owner_id,
                offset: 0,
                segments: Vec::new(),
            });
            new_ref_idx
        }
        other => panic!("expected REF_TABLE, got {other:?}"),
    };
    let existing_pou = match module.section(SectionId::PouIndex) {
        Some(SectionData::PouIndex(index)) => index
            .entries
            .iter()
            .find(|entry| entry.id == local.pou_id)
            .expect("fixture function POU")
            .clone(),
        other => panic!("expected POU_INDEX, got {other:?}"),
    };
    let mut duplicate_owner_pou = existing_pou;
    duplicate_owner_pou.id += 1000;
    duplicate_owner_pou.local_ref_start = new_ref_idx;
    duplicate_owner_pou.local_ref_count = 1;
    match module.section_mut(SectionId::PouIndex) {
        Some(SectionData::PouIndex(index)) => index.entries.push(duplicate_owner_pou),
        other => panic!("expected POU_INDEX, got {other:?}"),
    }

    assert_invalid_section_contains(&module, "POU local ref ranges share a frame owner");
}

#[test]
fn var_meta_rejects_local_retain_and_initializer_state() {
    let (mut retained, local) = module_with_valid_scoped_local_meta();
    local_entry_mut(&mut retained, local.ref_idx).retain = 1;
    assert_invalid_section_contains(
        &retained,
        &format!(
            "local VAR_META ref {} must use retain=0 and no initializer",
            local.ref_idx
        ),
    );

    let (mut initialized, local) = module_with_valid_scoped_local_meta();
    let has_const = matches!(
        initialized.section(SectionId::ConstPool),
        Some(SectionData::ConstPool(pool)) if !pool.entries.is_empty()
    );
    assert!(
        has_const,
        "fixture must contain a valid initializer constant"
    );
    local_entry_mut(&mut initialized, local.ref_idx).init_const_idx = Some(0);
    assert_invalid_section_contains(
        &initialized,
        &format!(
            "local VAR_META ref {} must use retain=0 and no initializer",
            local.ref_idx
        ),
    );
}

#[test]
fn var_meta_rejects_local_ref_outside_every_pou_range() {
    let (mut module, local) = module_with_valid_scoped_local_meta();
    let (owner_id, orphan_slot) = match module.section(SectionId::RefTable) {
        Some(SectionData::RefTable(refs)) => {
            let base = &refs.entries[local.ref_idx as usize];
            (base.owner_id, base.offset + 100)
        }
        other => panic!("expected REF_TABLE, got {other:?}"),
    };
    let orphan_ref_idx = match module.section_mut(SectionId::RefTable) {
        Some(SectionData::RefTable(refs)) => {
            let ref_idx = refs.entries.len() as u32;
            refs.entries.push(RefEntry {
                location: RefLocation::Local,
                owner_id,
                offset: orphan_slot,
                segments: Vec::new(),
            });
            ref_idx
        }
        other => panic!("expected REF_TABLE, got {other:?}"),
    };
    let name = format!("@local/{}/{orphan_slot}/orphan", local.pou_id);
    let name_idx = intern_string(&mut module, &name);
    var_meta_mut(&mut module).entries.push(VarMetaEntry {
        name_idx,
        type_id: local.type_id,
        ref_idx: orphan_ref_idx,
        retain: 0,
        init_const_idx: None,
    });

    assert_invalid_section_contains(
        &module,
        &format!("local VAR_META ref {orphan_ref_idx} is outside every POU local range"),
    );
}

#[derive(Clone, Copy)]
struct ScopedLocalFixture {
    pou_id: u32,
    ref_idx: u32,
    type_id: u32,
}

fn module_with_valid_scoped_local_meta() -> (BytecodeModule, ScopedLocalFixture) {
    let source = r#"
FUNCTION EchoBounded : STRING[5]
VAR_INPUT
    source : STRING[5];
END_VAR
VAR
    scratch : STRING[5];
END_VAR
scratch := 'ABCDE';
EchoBounded := scratch;
END_FUNCTION

PROGRAM Main
END_PROGRAM
"#;
    let mut module = bytecode_module_from_source(source).expect("build local metadata fixture");
    let (pou_id, ref_idx, type_id, declared_name) = {
        let strings = match module.section(SectionId::StringTable) {
            Some(SectionData::StringTable(strings)) => strings,
            other => panic!("expected STRING_TABLE, got {other:?}"),
        };
        let pou_index = match module.section(SectionId::PouIndex) {
            Some(SectionData::PouIndex(index)) => index,
            other => panic!("expected POU_INDEX, got {other:?}"),
        };
        let function = pou_index
            .entries
            .iter()
            .find(|entry| entry.kind == PouKind::Function)
            .expect("function POU");
        (
            function.id,
            function.local_ref_start,
            function.return_type_id.expect("function return type"),
            strings.entries[function.name_idx as usize].clone(),
        )
    };
    let expected_name = format!("@local/{pou_id}/0/{declared_name}");
    let existing = match module.section(SectionId::VarMeta) {
        Some(SectionData::VarMeta(meta)) => {
            meta.entries.iter().any(|entry| entry.ref_idx == ref_idx)
        }
        _ => false,
    };
    if !existing {
        let name_idx = intern_string(&mut module, &expected_name);
        var_meta_mut(&mut module).entries.push(VarMetaEntry {
            name_idx,
            type_id,
            ref_idx,
            retain: 0,
            init_const_idx: None,
        });
    }
    module
        .validate()
        .expect("valid scoped local VAR_META fixture");
    (
        module,
        ScopedLocalFixture {
            pou_id,
            ref_idx,
            type_id,
        },
    )
}

fn intern_string(module: &mut BytecodeModule, value: &str) -> u32 {
    let strings = match module.section_mut(SectionId::StringTable) {
        Some(SectionData::StringTable(strings)) => strings,
        other => panic!("expected STRING_TABLE, got {other:?}"),
    };
    if let Some(index) = strings.entries.iter().position(|entry| entry == value) {
        return index as u32;
    }
    strings.entries.push(value.into());
    (strings.entries.len() - 1) as u32
}

fn var_meta_mut(module: &mut BytecodeModule) -> &mut VarMeta {
    if module.section(SectionId::VarMeta).is_none() {
        module.sections.push(Section {
            id: SectionId::VarMeta.as_raw(),
            flags: 0,
            data: SectionData::VarMeta(VarMeta::default()),
        });
    }
    match module.section_mut(SectionId::VarMeta) {
        Some(SectionData::VarMeta(meta)) => meta,
        other => panic!("expected VAR_META, got {other:?}"),
    }
}

fn local_entry(module: &BytecodeModule, ref_idx: u32) -> &VarMetaEntry {
    match module.section(SectionId::VarMeta) {
        Some(SectionData::VarMeta(meta)) => meta
            .entries
            .iter()
            .find(|entry| entry.ref_idx == ref_idx)
            .unwrap_or_else(|| panic!("missing VAR_META for local ref {ref_idx}")),
        other => panic!("expected VAR_META, got {other:?}"),
    }
}

fn local_entry_mut(module: &mut BytecodeModule, ref_idx: u32) -> &mut VarMetaEntry {
    match module.section_mut(SectionId::VarMeta) {
        Some(SectionData::VarMeta(meta)) => meta
            .entries
            .iter_mut()
            .find(|entry| entry.ref_idx == ref_idx)
            .unwrap_or_else(|| panic!("missing VAR_META for local ref {ref_idx}")),
        other => panic!("expected VAR_META, got {other:?}"),
    }
}

fn assert_invalid_section_contains(module: &BytecodeModule, expected: &str) {
    let error = module.validate().expect_err("metadata must fail closed");
    assert!(
        matches!(error, BytecodeError::InvalidSection(ref message) if message.contains(expected)),
        "expected invalid-section message containing {expected:?}, got {error:?}"
    );
}
