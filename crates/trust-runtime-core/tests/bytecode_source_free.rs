#[path = "common/bytecode_helpers.rs"]
mod bytecode_helpers;

use bytecode_helpers::base_module;
use trust_runtime_core::bytecode::{
    AccessBindings, BytecodeError, BytecodeModule, BytecodeVersion, ConstructionRoots,
    InitializerBodyKind, InitializerIndex, Section, SectionData, SectionId, StorageLayout,
    ValidationLimits, HEADER_FLAG_CRC32,
};

fn source_free_empty() -> BytecodeModule {
    let mut module = base_module();
    module.version = BytecodeVersion::SOURCE_FREE;
    module.flags = HEADER_FLAG_CRC32;
    // No configured task or executable POU: an empty source-free resource still
    // requires all four construction/initialization/alias tables.
    if let Some(SectionData::PouIndex(index)) = module.section_mut(SectionId::PouIndex) {
        index.entries.clear();
    }
    if let Some(SectionData::PouBodies(code)) = module.section_mut(SectionId::PouBodies) {
        code.clear();
    }
    if let Some(SectionData::ResourceMeta(meta)) = module.section_mut(SectionId::ResourceMeta) {
        meta.resources[0].tasks.clear();
    }
    module.sections.extend([
        Section {
            id: SectionId::StorageLayout.as_raw(),
            flags: 0,
            data: SectionData::StorageLayout(StorageLayout::default()),
        },
        Section {
            id: SectionId::ConstructionRoots.as_raw(),
            flags: 0,
            data: SectionData::ConstructionRoots(ConstructionRoots::default()),
        },
        Section {
            id: SectionId::Initializers.as_raw(),
            flags: 0,
            data: SectionData::Initializers(InitializerIndex::default()),
        },
        Section {
            id: SectionId::AccessBindings.as_raw(),
            flags: 0,
            data: SectionData::AccessBindings(AccessBindings::default()),
        },
    ]);
    module
}

#[test]
fn source_free_empty_resource_round_trips_with_mandatory_tables() {
    let module = source_free_empty();
    module
        .validated_source_free(ValidationLimits::default())
        .expect("admit 2.0 metadata");
    let bytes = module.encode().expect("encode 2.0");
    let decoded = BytecodeModule::decode(&bytes).expect("read 2.0");
    decoded
        .validated_source_free(ValidationLimits::default())
        .expect("validate saved 2.0");
    assert_eq!(decoded.encode().unwrap(), bytes);
    assert_eq!(decoded.version, BytecodeVersion::SOURCE_FREE);
}

#[test]
fn portable_consumer_rejects_both_legacy_versions() {
    for minor in [0, 1] {
        let mut module = base_module();
        module.version = BytecodeVersion::new(1, minor);
        module.flags = if minor == 0 { 0 } else { HEADER_FLAG_CRC32 };
        let decoded = BytecodeModule::decode(&module.encode().unwrap()).unwrap();
        decoded.validate().expect("legacy reader remains available");
        assert!(matches!(
            decoded.validated_source_free(ValidationLimits::default()),
            Err(BytecodeError::UnsupportedVersion { major: 1, .. })
        ));
    }
}

#[test]
fn two_point_zero_requires_each_construction_section_even_when_empty() {
    for id in [
        SectionId::StorageLayout,
        SectionId::ConstructionRoots,
        SectionId::Initializers,
        SectionId::AccessBindings,
    ] {
        let mut module = source_free_empty();
        module.sections.retain(|section| section.id != id.as_raw());
        assert!(matches!(
            module.validate(),
            Err(BytecodeError::MissingSection(_))
        ));
    }
}

#[test]
fn source_free_reserved_flags_and_future_versions_reject() {
    for flags in [0, HEADER_FLAG_CRC32 | 2] {
        let mut module = source_free_empty();
        module.flags = flags;
        assert!(matches!(
            module.validate(),
            Err(BytecodeError::InvalidHeader(_))
        ));
        assert!(BytecodeModule::decode(&module.encode().unwrap()).is_err());
    }
    for version in [BytecodeVersion::new(2, 1), BytecodeVersion::new(3, 0)] {
        let mut module = source_free_empty();
        module.version = version;
        assert!(matches!(
            module.validate(),
            Err(BytecodeError::UnsupportedVersion { .. })
        ));
    }
    let mut module = source_free_empty();
    module.sections.last_mut().unwrap().flags = 1;
    assert!(module.validate().is_err());
}

#[test]
fn legacy_unknown_extension_ids_do_not_acquire_two_point_zero_meaning() {
    let mut module = base_module();
    for _ in 0..2 {
        module.sections.push(Section {
            id: SectionId::StorageLayout.as_raw(),
            flags: 0,
            data: SectionData::Raw(vec![0xFF]),
        });
    }
    let bytes = module.encode().expect("legacy opaque extensions");
    let decoded = BytecodeModule::decode(&bytes).expect("preserve legacy extension framing");
    decoded
        .validate()
        .expect("1.x does not interpret new sections");
    assert_eq!(decoded.encode().unwrap(), bytes);
}

fn value_default_module() -> BytecodeModule {
    use trust_runtime_core::bytecode::{
        InitializationOnce, InitializationPhase, InitializationStage, InitializationTarget,
        InitializerEntry, RefEntry, RefLocation, VarMeta, VarMetaEntry,
    };
    let mut module = source_free_empty();
    let name_idx = match module.section_mut(SectionId::StringTable).unwrap() {
        SectionData::StringTable(strings) => {
            let id = strings.entries.len() as u32;
            strings.entries.push("@init/0/result".into());
            id
        }
        _ => unreachable!(),
    };
    if let Some(SectionData::RefTable(table)) = module.section_mut(SectionId::RefTable) {
        table.entries.push(RefEntry {
            location: RefLocation::InitializerResult,
            owner_id: 0,
            offset: 0,
            segments: vec![],
        });
    }
    if let Some(SectionData::Initializers(index)) = module.section_mut(SectionId::Initializers) {
        index.entries.push(InitializerEntry {
            declaration_idx: None,
            owner_pou_id: None,
            result_ref_idx: 0,
            code_offset: 0,
            code_length: 0,
            visible_local_count: 0,
            visible_static_count: 0,
            phase: InitializationPhase::ValueDefault,
            once: InitializationOnce::None,
            stage: InitializationStage::Default,
            trigger: trust_runtime_core::bytecode::InitializationTrigger::Ordinary,
            target_idx: None,
            partial_kind: 0,
            target_kind: InitializationTarget::Declaration,
            target_reserved: [0; 2],
            partial_index: 0,
            context_initializer_idx: None,
            recipe_type_id: None,
            recipe_member_idx: None,
            body_kind: InitializerBodyKind::Action,
            recipe_reserved: [0; 3],
        });
    }
    module.sections.push(Section {
        id: SectionId::VarMeta.as_raw(),
        flags: 0,
        data: SectionData::VarMeta(VarMeta {
            entries: vec![VarMetaEntry {
                name_idx,
                type_id: 0,
                ref_idx: 0,
                retain: 0,
                init_const_idx: None,
            }],
        }),
    });
    module
}

#[test]
fn on_demand_default_has_typed_result_and_no_runtime_owner() {
    let module = value_default_module();
    module.validate().expect("typed on-demand default");
    let decoded = BytecodeModule::decode(&module.encode().unwrap()).unwrap();
    decoded
        .validated_source_free(ValidationLimits::default())
        .unwrap();
    assert!(decoded.disassemble().unwrap().contains("ValueDefault"));
    for field in 0..4 {
        let mut bad = module.clone();
        let Some(SectionData::Initializers(index)) = bad.section_mut(SectionId::Initializers)
        else {
            unreachable!()
        };
        match field {
            0 => index.entries[0].visible_local_count = 1,
            1 => index.entries[0].visible_static_count = 1,
            2 => index.entries[0].partial_index = 1,
            _ => {
                index.entries[0].trigger =
                    trust_runtime_core::bytecode::InitializationTrigger::AfterRestart
            }
        }
        assert!(bad.validate().is_err(), "accepted invalid field {field}");
    }
}

#[test]
fn legacy_instruction_stream_rejects_default_value_opcode() {
    let mut module = base_module();
    let code = vec![0x65, 0, 0, 0, 0, 0x12, 0x00];
    if let Some(SectionData::PouIndex(index)) = module.section_mut(SectionId::PouIndex) {
        index.entries[0].code_length = code.len() as u32;
    }
    if let Some(SectionData::PouBodies(body)) = module.section_mut(SectionId::PouBodies) {
        *body = code;
    }
    assert_eq!(module.validate(), Err(BytecodeError::InvalidOpcode(0x65)));
}

fn with_type_recipe() -> BytecodeModule {
    let mut module = value_default_module();
    let Some(SectionData::Initializers(index)) = module.section_mut(SectionId::Initializers) else {
        unreachable!()
    };
    let mut recipe = index.entries[0].clone();
    recipe.result_ref_idx = 1;
    recipe.body_kind = InitializerBodyKind::TypeDefault;
    recipe.context_initializer_idx = Some(0);
    recipe.recipe_type_id = Some(0);
    index.entries.push(recipe);
    let Some(SectionData::RefTable(refs)) = module.section_mut(SectionId::RefTable) else {
        unreachable!()
    };
    let mut reference = refs.entries[0].clone();
    reference.owner_id = 1;
    refs.entries.push(reference);
    let Some(SectionData::StringTable(strings)) = module.section_mut(SectionId::StringTable) else {
        unreachable!()
    };
    let name = strings.entries.len() as u32;
    strings.entries.push("@init/1/result".into());
    let Some(SectionData::VarMeta(meta)) = module.section_mut(SectionId::VarMeta) else {
        unreachable!()
    };
    let mut result = meta.entries[0].clone();
    result.ref_idx = 1;
    result.name_idx = name;
    meta.entries.push(result);
    module
}

#[test]
fn callable_default_identity_survives_the_wire_round_trip() {
    let module = with_type_recipe();
    module
        .validate()
        .expect("callable default bound to an action");
    let bytes = module.encode().unwrap();
    let decoded = BytecodeModule::decode(&bytes).unwrap();
    decoded.validate().unwrap();
    assert_eq!(
        decoded.section(SectionId::Initializers),
        module.section(SectionId::Initializers)
    );
}

#[test]
fn callable_defaults_reject_foreign_context_and_inconsistent_identity() {
    for mutation in 0..7 {
        let mut module = with_type_recipe();
        let Some(SectionData::Initializers(index)) = module.section_mut(SectionId::Initializers)
        else {
            unreachable!()
        };
        match mutation {
            0 => index.entries[1].context_initializer_idx = None,
            1 => index.entries[1].context_initializer_idx = Some(1),
            2 => index.entries[1].recipe_type_id = Some(u32::MAX),
            3 => index.entries[1].recipe_member_idx = Some(0),
            4 => index.entries[1].recipe_reserved[0] = 1,
            5 => index.entries[1].visible_local_count = 1,
            _ => index.entries[0].recipe_type_id = Some(0),
        }
        assert!(
            module.validate().is_err(),
            "accepted recipe mutation {mutation}"
        );
    }
}

#[test]
fn typed_initializer_opcodes_are_not_ordinary_pou_instructions() {
    for opcode in 0x66..=0x6C {
        let mut module = source_free_empty();
        let baseline = base_module();
        let Some(SectionData::PouIndex(original)) = baseline.section(SectionId::PouIndex) else {
            unreachable!()
        };
        let mut pou = original.entries[0].clone();
        pou.code_offset = 0;
        pou.code_length = 5;
        let Some(SectionData::PouIndex(index)) = module.section_mut(SectionId::PouIndex) else {
            unreachable!()
        };
        index.entries.push(pou);
        let Some(SectionData::PouBodies(code)) = module.section_mut(SectionId::PouBodies) else {
            unreachable!()
        };
        *code = vec![opcode, 0, 0, 0, 0];
        assert_eq!(module.validate(), Err(BytecodeError::InvalidOpcode(opcode)));
    }
}

#[test]
fn source_free_construction_rejects_unknown_or_noncanonical_primitives() {
    for (prim_id, max_length) in [(28, 0), (0, 0), (1, 8), (0x0100, 1)] {
        let mut module = source_free_empty();
        let Some(SectionData::TypeTable(types)) = module.section_mut(SectionId::TypeTable) else {
            unreachable!()
        };
        types.entries[0].data = trust_runtime_core::bytecode::TypeData::Primitive {
            prim_id,
            max_length,
        };
        assert!(module.validate().is_err());
    }
}

fn executable_defaults() -> BytecodeModule {
    let mut module = with_type_recipe();
    let mut code = Vec::new();
    for result in 0u32..2 {
        code.push(0x66); // DEFAULT_TYPED BOOL
        code.extend_from_slice(&0u32.to_le_bytes());
        code.push(0x21); // STORE_REF private result
        code.extend_from_slice(&result.to_le_bytes());
    }
    let Some(SectionData::PouBodies(bodies)) = module.section_mut(SectionId::PouBodies) else {
        unreachable!()
    };
    *bodies = code;
    let Some(SectionData::Initializers(index)) = module.section_mut(SectionId::Initializers) else {
        unreachable!()
    };
    for (id, entry) in index.entries.iter_mut().enumerate() {
        entry.code_offset = id as u32 * 10;
        entry.code_length = 10;
    }
    module
}

#[test]
fn initializer_bodies_have_disjoint_ranges_and_private_results() {
    let module = executable_defaults();
    module
        .validate()
        .expect("each body writes its own typed result");
    BytecodeModule::decode(&module.encode().unwrap())
        .unwrap()
        .validate()
        .unwrap();

    let mut foreign_result = module.clone();
    let Some(SectionData::PouBodies(code)) = foreign_result.section_mut(SectionId::PouBodies)
    else {
        unreachable!()
    };
    code[16..20].copy_from_slice(&0u32.to_le_bytes());
    assert!(
        foreign_result.validate().is_err(),
        "a recipe must not overwrite its root action's staged result"
    );

    for (offset, length) in [(0, 10), (u32::MAX, 10), (10, u32::MAX)] {
        let mut bad = module.clone();
        let Some(SectionData::Initializers(index)) = bad.section_mut(SectionId::Initializers)
        else {
            unreachable!()
        };
        index.entries[1].code_offset = offset;
        index.entries[1].code_length = length;
        assert!(
            bad.validate().is_err(),
            "accepted overlapping or overflowing body range"
        );
    }
}

#[test]
fn recipe_visibility_does_not_admit_a_callers_frame_or_static_aliases() {
    let module = executable_defaults();
    for local in [true, false] {
        let mut bad = module.clone();
        let Some(SectionData::Initializers(index)) = bad.section_mut(SectionId::Initializers)
        else {
            unreachable!()
        };
        if local {
            index.entries[1].visible_local_count = 1;
        } else {
            index.entries[1].visible_static_count = 1;
        }
        assert!(bad.validate().is_err());
    }
}

#[test]
fn construction_decoder_rejects_truncated_records_and_unknown_discriminants() {
    for payload in [
        {
            let mut bytes = vec![0; 43];
            bytes[0] = 1;
            bytes
        }, // count + short 40-byte record
        {
            let mut bytes = vec![0; 44];
            bytes[0] = 1;
            bytes[4] = 3;
            bytes
        }, // invalid owner
        {
            let mut bytes = vec![0; 44];
            bytes[0] = 1;
            bytes[5] = 9;
            bytes
        }, // invalid role
    ] {
        let mut module = source_free_empty();
        *module.section_mut(SectionId::StorageLayout).unwrap() = SectionData::Raw(payload);
        let bytes = module.encode().unwrap(); // Encoder computes a valid CRC for this malformed section.
        assert!(BytecodeModule::decode(&bytes).is_err());
    }
}
