#[path = "common/bytecode_helpers.rs"]
mod bytecode_helpers;

use bytecode_helpers::{
    base_module, base_string_table, sample_const_pool, sample_io_map, sample_pou_index,
    sample_ref_table, sample_type_table,
};
use trust_runtime::bytecode::{BytecodeModule, Section, SectionData, SectionId, StringTable};

#[test]
fn string_table_decode() {
    let table = StringTable {
        entries: vec!["A".into(), "B".into()],
    };
    let module = BytecodeModule {
        version: trust_runtime::bytecode::BytecodeVersion::new(1, 1),
        flags: 0,
        sections: vec![Section {
            id: SectionId::StringTable.as_raw(),
            flags: 0,
            data: SectionData::StringTable(table.clone()),
        }],
    };
    let bytes = module.encode().expect("encode");
    let decoded = BytecodeModule::decode(&bytes).expect("decode");
    match decoded.section(SectionId::StringTable) {
        Some(SectionData::StringTable(decoded_table)) => assert_eq!(decoded_table, &table),
        other => panic!("expected string table, got {other:?}"),
    }
}

#[test]
fn type_table_decode() {
    let (strings, types) = sample_type_table();
    let module = BytecodeModule {
        version: trust_runtime::bytecode::BytecodeVersion::new(1, 1),
        flags: 0,
        sections: vec![
            Section {
                id: SectionId::StringTable.as_raw(),
                flags: 0,
                data: SectionData::StringTable(strings.clone()),
            },
            Section {
                id: SectionId::TypeTable.as_raw(),
                flags: 0,
                data: SectionData::TypeTable(types.clone()),
            },
        ],
    };
    let bytes = module.encode().expect("encode");
    let decoded = BytecodeModule::decode(&bytes).expect("decode");
    match decoded.section(SectionId::TypeTable) {
        Some(SectionData::TypeTable(decoded_table)) => {
            assert_eq!(decoded_table, &types);
            assert_eq!(decoded_table.offsets.len(), decoded_table.entries.len());
            if !decoded_table.offsets.is_empty() {
                let base = 4u32 + (decoded_table.entries.len() as u32 * 4);
                assert_eq!(decoded_table.offsets[0], base);
            }
        }
        other => panic!("expected type table, got {other:?}"),
    }
}

#[test]
fn const_pool_decode() {
    let mut module = base_module();
    if let Some(SectionData::ConstPool(pool)) = module.section_mut(SectionId::ConstPool) {
        *pool = sample_const_pool();
    }
    let bytes = module.encode().expect("encode");
    let _decoded = BytecodeModule::decode(&bytes).expect("decode");
}

#[test]
fn ref_table_decode() {
    let strings = base_string_table();
    let refs = sample_ref_table();
    let module = BytecodeModule {
        version: trust_runtime::bytecode::BytecodeVersion::new(1, 1),
        flags: 0,
        sections: vec![
            Section {
                id: SectionId::StringTable.as_raw(),
                flags: 0,
                data: SectionData::StringTable(strings.clone()),
            },
            Section {
                id: SectionId::RefTable.as_raw(),
                flags: 0,
                data: SectionData::RefTable(refs.clone()),
            },
        ],
    };
    let bytes = module.encode().expect("encode");
    let decoded = BytecodeModule::decode(&bytes).expect("decode");
    match decoded.section(SectionId::RefTable) {
        Some(SectionData::RefTable(decoded_table)) => assert_eq!(decoded_table, &refs),
        other => panic!("expected ref table, got {other:?}"),
    }
}

#[test]
fn pou_index_decode() {
    let strings = StringTable {
        entries: vec![
            "R".into(),
            "T".into(),
            "Main".into(),
            "Param".into(),
            "Extra1".into(),
            "Extra2".into(),
            "Extra3".into(),
            "Extra4".into(),
            "Extra5".into(),
            "Extra6".into(),
            "FB".into(),
        ],
    };
    let index = sample_pou_index();
    let module = BytecodeModule {
        version: trust_runtime::bytecode::BytecodeVersion::new(1, 1),
        flags: 0,
        sections: vec![
            Section {
                id: SectionId::StringTable.as_raw(),
                flags: 0,
                data: SectionData::StringTable(strings.clone()),
            },
            Section {
                id: SectionId::PouIndex.as_raw(),
                flags: 0,
                data: SectionData::PouIndex(index.clone()),
            },
            Section {
                id: SectionId::PouBodies.as_raw(),
                flags: 0,
                data: SectionData::PouBodies(vec![0x00, 0x00]),
            },
        ],
    };
    let bytes = module.encode().expect("encode");
    let decoded = BytecodeModule::decode(&bytes).expect("decode");
    match decoded.section(SectionId::PouIndex) {
        Some(SectionData::PouIndex(decoded_index)) => assert_eq!(decoded_index, &index),
        other => panic!("expected pou index, got {other:?}"),
    }
}

#[test]
fn resource_meta_decode() {
    let module = base_module();
    let bytes = module.encode().expect("encode");
    let decoded = BytecodeModule::decode(&bytes).expect("decode");
    assert!(matches!(
        decoded.section(SectionId::ResourceMeta),
        Some(SectionData::ResourceMeta(_))
    ));
}

#[test]
fn io_map_decode() {
    let strings = base_string_table();
    let refs = sample_ref_table();
    let map = sample_io_map();
    let module = BytecodeModule {
        version: trust_runtime::bytecode::BytecodeVersion::new(1, 1),
        flags: 0,
        sections: vec![
            Section {
                id: SectionId::StringTable.as_raw(),
                flags: 0,
                data: SectionData::StringTable(strings.clone()),
            },
            Section {
                id: SectionId::RefTable.as_raw(),
                flags: 0,
                data: SectionData::RefTable(refs.clone()),
            },
            Section {
                id: SectionId::IoMap.as_raw(),
                flags: 0,
                data: SectionData::IoMap(map.clone()),
            },
        ],
    };
    let bytes = module.encode().expect("encode");
    let decoded = BytecodeModule::decode(&bytes).expect("decode");
    match decoded.section(SectionId::IoMap) {
        Some(SectionData::IoMap(decoded_map)) => assert_eq!(decoded_map, &map),
        other => panic!("expected io map, got {other:?}"),
    }
}

#[test]
fn debug_map_decode() {
    let module = bytecode_helpers::module_with_debug();
    let bytes = module.encode().expect("encode");
    let decoded = BytecodeModule::decode(&bytes).expect("decode");
    assert!(matches!(
        decoded.section(SectionId::DebugMap),
        Some(SectionData::DebugMap(_))
    ));
}

#[test]
fn debug_string_table_decode() {
    let module = bytecode_helpers::module_with_debug();
    let bytes = module.encode().expect("encode");
    let decoded = BytecodeModule::decode(&bytes).expect("decode");
    assert!(matches!(
        decoded.section(SectionId::DebugStringTable),
        Some(SectionData::DebugStringTable(_))
    ));
}

#[test]
fn string_table_padding() {
    let table = StringTable {
        entries: vec!["abc".into()],
    };
    let module = BytecodeModule {
        version: trust_runtime::bytecode::BytecodeVersion::new(1, 1),
        flags: 0,
        sections: vec![Section {
            id: SectionId::StringTable.as_raw(),
            flags: 0,
            data: SectionData::StringTable(table),
        }],
    };
    let bytes = module.encode().expect("encode");
    let section_table_off = u32::from_le_bytes(bytes[16..20].try_into().unwrap()) as usize;
    let length = u32::from_le_bytes(
        bytes[section_table_off + 8..section_table_off + 12]
            .try_into()
            .unwrap(),
    ) as usize;
    assert_eq!(length, 12);
    let _decoded = BytecodeModule::decode(&bytes).expect("decode");
}

#[test]
fn var_meta_decode() {
    let mut module = base_module();
    if let Some(SectionData::ConstPool(pool)) = module.section_mut(SectionId::ConstPool) {
        *pool = sample_const_pool();
    }
    if let Some(SectionData::RefTable(table)) = module.section_mut(SectionId::RefTable) {
        *table = sample_ref_table();
    }
    module.sections.push(Section {
        id: SectionId::VarMeta.as_raw(),
        flags: 0,
        data: SectionData::VarMeta(trust_runtime::bytecode::VarMeta {
            entries: vec![trust_runtime::bytecode::VarMetaEntry {
                name_idx: 2,
                type_id: 0,
                ref_idx: 0,
                retain: 1,
                init_const_idx: Some(0),
            }],
        }),
    });
    module.sections.push(Section {
        id: SectionId::RetainInit.as_raw(),
        flags: 0,
        data: SectionData::RetainInit(trust_runtime::bytecode::RetainInit {
            entries: vec![trust_runtime::bytecode::RetainInitEntry {
                ref_idx: 0,
                const_idx: 0,
            }],
        }),
    });

    let bytes = module.encode().expect("encode");
    let decoded = BytecodeModule::decode(&bytes).expect("decode");
    decoded.validate().expect("validate");
    assert!(matches!(
        decoded.section(SectionId::VarMeta),
        Some(SectionData::VarMeta(_))
    ));
    assert!(matches!(
        decoded.section(SectionId::RetainInit),
        Some(SectionData::RetainInit(_))
    ));
}
