//! Spec 34 §10: carry images into staging before ordered restart initialization.
use super::*;
use crate::bytecode::{
    BytecodeModule, ConstEntry, InitializationPhase, InitializationStage, InitializationTarget,
    InitializerBodyKind, SectionData, SectionId,
};
use crate::io_address::IoAddress;
use crate::io_image::IoAddressKey;
use crate::retain::RestartMode;
use crate::vm::PreparationLimits;

fn direct_image_module() -> BytecodeModule {
    let mut module = BytecodeModule::decode(include_bytes!(
        "../../../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc"
    ))
    .unwrap();
    let Some(SectionData::StringTable(strings)) = module.section(SectionId::StringTable) else {
        panic!("strings")
    };
    let Some(SectionData::StorageLayout(layout)) = module.section(SectionId::StorageLayout) else {
        panic!("layout")
    };
    let declaration = layout
        .entries
        .iter()
        .position(|entry| strings.entries[entry.name_idx as usize] == "enabled")
        .unwrap() as u32;
    let Some(SectionData::Initializers(initializers)) = module.section(SectionId::Initializers)
    else {
        panic!("initializers")
    };
    let template = initializers
        .entries
        .iter()
        .find(|entry| {
            entry.declaration_idx == Some(declaration)
                && entry.stage == InitializationStage::Explicit
        })
        .unwrap()
        .clone();
    let Some(SectionData::VarMeta(meta)) = module.section(SectionId::VarMeta) else {
        panic!("metadata")
    };
    let result_meta = meta
        .entries
        .iter()
        .find(|entry| entry.ref_idx == template.result_ref_idx)
        .unwrap()
        .clone();

    for (address, value) in [
        ("%IX0.0", true),
        ("%QX0.0", true),
        ("%MX0.0", true),
        ("%QX0.1", false),
        ("%QX0.1", true),
        ("%IX1.2.3", true),
        ("%QX1.2.3", false),
        ("%QX1.2.3", true),
        ("%MX1.2.3", true),
    ] {
        let Some(SectionData::Initializers(index)) = module.section(SectionId::Initializers) else {
            panic!("index")
        };
        let id = index.entries.len() as u32;
        let Some(SectionData::StringTable(strings)) = module.section_mut(SectionId::StringTable)
        else {
            panic!("strings")
        };
        let target = strings.entries.len() as u32;
        strings.entries.push(address.into());
        let name = strings.entries.len() as u32;
        strings
            .entries
            .push(alloc::format!("restart_image_result_{id}").into());
        let Some(SectionData::ConstPool(constants)) = module.section_mut(SectionId::ConstPool)
        else {
            panic!("constants")
        };
        let constant = constants.entries.len() as u32;
        constants.entries.push(ConstEntry {
            type_id: result_meta.type_id,
            payload: vec![u8::from(value)],
        });
        let Some(SectionData::RefTable(refs)) = module.section_mut(SectionId::RefTable) else {
            panic!("refs")
        };
        let result = refs.entries.len() as u32;
        refs.entries.push(crate::bytecode::RefEntry {
            location: crate::bytecode::RefLocation::InitializerResult,
            owner_id: id,
            offset: 0,
            segments: Vec::new(),
        });
        let Some(SectionData::VarMeta(meta)) = module.section_mut(SectionId::VarMeta) else {
            panic!("metadata")
        };
        let mut metadata = result_meta.clone();
        metadata.name_idx = name;
        metadata.ref_idx = result;
        meta.entries.push(metadata);
        let Some(SectionData::PouBodies(code)) = module.section_mut(SectionId::PouBodies) else {
            panic!("code")
        };
        let offset = code.len() as u32;
        // Ordinary LOAD_CONST and STORE_REF into this action's typed staging result.
        code.push(0x10);
        code.extend_from_slice(&constant.to_le_bytes());
        code.push(0x21);
        code.extend_from_slice(&result.to_le_bytes());
        let mut entry = template.clone();
        entry.declaration_idx = None;
        entry.owner_pou_id = None;
        entry.result_ref_idx = result;
        entry.code_offset = offset;
        entry.code_length = 10;
        entry.visible_local_count = 0;
        entry.visible_static_count = 0;
        entry.phase = InitializationPhase::Configuration;
        entry.once = crate::bytecode::InitializationOnce::None;
        entry.target_idx = Some(target);
        entry.target_kind = InitializationTarget::DirectIo;
        entry.partial_kind = 0;
        entry.partial_index = 0;
        entry.context_initializer_idx = None;
        entry.recipe_type_id = None;
        entry.recipe_member_idx = None;
        entry.body_kind = InitializerBodyKind::Action;
        let Some(SectionData::Initializers(index)) = module.section_mut(SectionId::Initializers)
        else {
            panic!("index")
        };
        index.entries.push(entry);
    }
    let Some(SectionData::ResourceMeta(resources)) = module.section_mut(SectionId::ResourceMeta)
    else {
        panic!("resources")
    };
    resources.resources[0].inputs_size = 2;
    resources.resources[0].outputs_size = 2;
    resources.resources[0].memory_size = 2;
    module
}

fn key(address: &str) -> IoAddressKey {
    IoAddressKey::from(&IoAddress::parse(address).unwrap())
}

#[test]
fn warm_and_cold_restart_reapply_ordered_direct_actions_before_publishing_images() {
    let module = direct_image_module();
    // Admit the complete wire contract; the test has no private validation bypass.
    let prepared =
        PreparedModule::from_bytes(&module.encode().unwrap(), PreparationLimits::default())
            .unwrap();
    for mode in [RestartMode::Warm, RestartMode::Cold] {
        let mut state = EngineState::new(&prepared, 0, &services::LOGICAL_ONLY).unwrap();
        for image in [
            &mut state.images.inputs,
            &mut state.images.outputs,
            &mut state.images.memory,
        ] {
            image.copy_from_slice(&[0xa8, 0x5a]);
        }
        for address in ["%IX1.2.3", "%QX1.2.3", "%MX1.2.3"] {
            state
                .images
                .hierarchical
                .insert(key(address), Value::Bool(false));
        }
        state
            .images
            .hierarchical
            .insert(key("%MX9.8.7"), Value::Bool(true));
        state.restart(mode).unwrap();
        assert_eq!(state.images.inputs, [0xa9, 0x5a], "input {mode:?}");
        assert_eq!(
            state.images.outputs,
            [0xab, 0x5a],
            "ordered overlapping output {mode:?}"
        );
        assert_eq!(state.images.memory, [0xa9, 0x5a], "marker {mode:?}");
        for address in ["%IX1.2.3", "%QX1.2.3", "%MX1.2.3", "%MX9.8.7"] {
            assert_eq!(
                state.images.hierarchical.get(&key(address)),
                Some(&Value::Bool(true)),
                "{address} {mode:?}"
            );
        }
    }
}
