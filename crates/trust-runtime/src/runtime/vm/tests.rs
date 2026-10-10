use super::*;

use crate::bytecode::{
    BytecodeVersion, ConstPool, MethodEntry, PouClassMeta, PouEntry, PouIndex,
    RefSegment as BytecodeRefSegment, Section, TypeTable, VarMetaEntry, SUPPORTED_MAJOR_VERSION,
    SUPPORTED_MINOR_VERSION,
};
use crate::error::StableErrorCode;
use trust_hir::TypeId;

#[test]
fn vm_module_materialization_requires_every_execution_section() {
    let complete = bytecode_module(vec!["Main".into()], vec![pou(1, 0, PouKind::Program)]);

    for required in [
        SectionId::StringTable,
        SectionId::TypeTable,
        SectionId::ConstPool,
        SectionId::RefTable,
        SectionId::PouIndex,
        SectionId::PouBodies,
    ] {
        let mut missing = complete.clone();
        missing
            .sections
            .retain(|section| section.id != required.as_raw());
        let error = VmModule::from_bytecode(&missing)
            .expect_err("missing required execution section must reject");
        assert_eq!(error.stable_code(), StableErrorCode::BytecodeMissingSection);
    }
}

#[test]
fn vm_module_materialization_preserves_case_insensitive_pou_metadata() {
    let mut class = pou(4, 3, PouKind::Class);
    class.class_meta = Some(PouClassMeta {
        parent_pou_id: None,
        interfaces: Vec::new(),
        methods: vec![MethodEntry {
            name_idx: 4,
            pou_id: 5,
            vtable_slot: 0,
            access: 0,
            flags: 0,
        }],
    });
    let mut function = pou(2, 1, PouKind::Function);
    function.return_type_id = Some(TypeId::DINT.0);
    function.params = vec![crate::bytecode::ParamEntry {
        name_idx: 5,
        type_id: TypeId::INT.0,
        direction: 2,
        default_const_idx: Some(7),
    }];
    let module = bytecode_module(
        vec![
            "MainProgram".into(),
            "Compute".into(),
            "MotorFb".into(),
            "MotorClass".into(),
            "Start".into(),
            "Input".into(),
        ],
        vec![
            pou(1, 0, PouKind::Program),
            function,
            pou(3, 2, PouKind::FunctionBlock),
            class,
            pou(5, 4, PouKind::Method),
        ],
    );

    let materialized = VmModule::from_bytecode(&module).unwrap();

    assert_eq!(materialized.program_ids.get("MAINPROGRAM"), Some(&1));
    assert_eq!(materialized.function_ids.get("COMPUTE"), Some(&2));
    assert_eq!(materialized.function_block_ids.get("MOTORFB"), Some(&3));
    assert_eq!(materialized.class_ids.get("MOTORCLASS"), Some(&4));
    assert_eq!(materialized.pou_name(1), Some("MainProgram"));
    assert!(materialized.pou_has_return_slot(2));
    let params = materialized.pou_params(2).expect("function parameters");
    assert_eq!(params.len(), 1);
    assert_eq!(params[0].name, "Input");
    assert_eq!(params[0].type_id, TypeId::INT.0);
    assert_eq!(params[0].direction, 2);
    assert_eq!(params[0].default_const_idx, Some(7));
    assert_eq!(
        materialized.resolve_method_pou_id_uppercase(4, "START"),
        Some(5)
    );
    assert_eq!(
        materialized.resolve_method_pou_id_uppercase(3, "START"),
        None
    );
}

#[test]
fn vm_module_materialization_rejects_duplicate_pou_ids_and_kind_names() {
    let duplicate_id = bytecode_module(
        vec!["First".into(), "Second".into()],
        vec![pou(1, 0, PouKind::Program), pou(1, 1, PouKind::Function)],
    );
    let error = VmModule::from_bytecode(&duplicate_id)
        .expect_err("one POU id cannot identify two declarations");
    assert_eq!(error.stable_code(), StableErrorCode::BytecodeInvalidSection);
    assert!(error.to_string().contains("duplicate POU id"));

    let duplicate_name = bytecode_module(
        vec!["Main".into(), "mAiN".into()],
        vec![pou(1, 0, PouKind::Program), pou(2, 1, PouKind::Program)],
    );
    let error = VmModule::from_bytecode(&duplicate_name)
        .expect_err("one case-insensitive kind name cannot identify two POUs");
    assert_eq!(error.stable_code(), StableErrorCode::VmBytecodeDecode);
    assert!(error.to_string().contains("duplicate PROGRAM name"));
}

#[test]
fn vm_module_materialization_rejects_duplicate_owner_local_method_names() {
    let mut class = pou(1, 0, PouKind::Class);
    class.class_meta = Some(PouClassMeta {
        parent_pou_id: None,
        interfaces: Vec::new(),
        methods: vec![
            MethodEntry {
                name_idx: 1,
                pou_id: 2,
                vtable_slot: 0,
                access: 0,
                flags: 0,
            },
            MethodEntry {
                name_idx: 2,
                pou_id: 3,
                vtable_slot: 1,
                access: 0,
                flags: 0,
            },
        ],
    });
    let module = bytecode_module(
        vec!["Motor".into(), "Start".into(), "sTaRt".into()],
        vec![
            class,
            pou(2, 1, PouKind::Method),
            pou(3, 2, PouKind::Method),
        ],
    );

    let error = VmModule::from_bytecode(&module)
        .expect_err("one owner cannot expose duplicate case-insensitive method names");
    assert_eq!(error.stable_code(), StableErrorCode::VmBytecodeDecode);
    assert!(error.to_string().contains("duplicate METHOD name"));
}

#[test]
fn vm_ref_type_map_is_optional_and_rejects_duplicate_references() {
    assert!(build_ref_type_map(None).unwrap().is_empty());

    let metadata = VarMeta {
        entries: vec![var_meta(2, 5), var_meta(9, 16)],
    };
    let map = build_ref_type_map(Some(&metadata)).unwrap();
    assert_eq!(map.get(&2), Some(&5));
    assert_eq!(map.get(&9), Some(&16));

    let duplicate = VarMeta {
        entries: vec![var_meta(2, 5), var_meta(2, 16)],
    };
    let error = build_ref_type_map(Some(&duplicate))
        .expect_err("duplicate metadata for one reference must reject");
    assert_eq!(error.stable_code(), StableErrorCode::VmBytecodeDecode);
    assert!(error.to_string().contains("duplicate VAR_META ref index"));
}

#[test]
fn vm_ref_decoder_preserves_locations_owners_offsets_and_paths() {
    let strings = StringTable {
        entries: vec!["Child".into()],
    };
    let global = decode_vm_ref(
        &reference(
            RefLocation::Global,
            99,
            4,
            vec![
                BytecodeRefSegment::Index(vec![-3, 7]),
                BytecodeRefSegment::Field { name_idx: 0 },
            ],
        ),
        &strings,
    )
    .unwrap();
    assert!(matches!(
        global,
        VmRef::Global { offset: 4, ref path }
            if matches!(
                path.as_slice(),
                [
                    ValueRefSegment::Index(indices),
                    ValueRefSegment::Field(field)
                ] if indices.as_slice() == [-3, 7] && field == "Child"
            )
    ));

    assert!(matches!(
        decode_vm_ref(&reference(RefLocation::Local, 21, 5, Vec::new()), &strings).unwrap(),
        VmRef::Local {
            owner_frame_id: 21,
            offset: 5,
            ..
        }
    ));
    assert!(matches!(
        decode_vm_ref(
            &reference(RefLocation::Instance, 34, 6, Vec::new()),
            &strings
        )
        .unwrap(),
        VmRef::Instance {
            owner_instance_id: 34,
            offset: 6,
            ..
        }
    ));
    assert!(matches!(
        decode_vm_ref(&reference(RefLocation::Retain, 55, 7, Vec::new()), &strings).unwrap(),
        VmRef::Retain { offset: 7, .. }
    ));

    for (owner, area) in [(0, IoArea::Input), (1, IoArea::Output), (2, IoArea::Memory)] {
        assert!(matches!(
            decode_vm_ref(
                &reference(RefLocation::Io, owner, 8, Vec::new()),
                &strings
            )
            .unwrap(),
            VmRef::Io {
                area: actual,
                offset: 8,
                ..
            } if actual == area
        ));
    }
}

#[test]
fn vm_ref_decoder_rejects_invalid_io_owner_and_field_string() {
    let strings = StringTable::default();
    for entry in [
        reference(RefLocation::Io, 3, 0, Vec::new()),
        reference(
            RefLocation::Global,
            0,
            0,
            vec![BytecodeRefSegment::Field { name_idx: 7 }],
        ),
    ] {
        let error = decode_vm_ref(&entry, &strings).expect_err("invalid reference must reject");
        assert_eq!(error.stable_code(), StableErrorCode::VmBytecodeDecode);
    }
}

#[test]
fn infer_primary_instance_owner_scans_partial_access_operands() {
    let mut code = vec![0x22];
    code.extend_from_slice(&0_u32.to_le_bytes());
    code.push(0x62);
    code.extend_from_slice(&0_u32.to_le_bytes());

    let entry = VmPouEntry {
        name: SmolStr::new("Main"),
        code_start: 0,
        code_end: code.len(),
        local_ref_start: 0,
        local_ref_count: 0,
        primary_instance_owner: None,
    };
    let refs = vec![VmRef::Instance {
        owner_instance_id: 42,
        offset: 0,
        path: RefPath::new(),
    }];

    assert_eq!(infer_primary_instance_owner(&entry, &code, &refs), Some(42));
}

#[test]
fn infer_primary_instance_owner_returns_none_for_ambiguous_owners() {
    let mut code = vec![0x20];
    code.extend_from_slice(&0_u32.to_le_bytes());
    code.push(0x20);
    code.extend_from_slice(&1_u32.to_le_bytes());

    let entry = VmPouEntry {
        name: SmolStr::new("Main"),
        code_start: 0,
        code_end: code.len(),
        local_ref_start: 0,
        local_ref_count: 0,
        primary_instance_owner: None,
    };
    let refs = vec![
        VmRef::Instance {
            owner_instance_id: 42,
            offset: 0,
            path: RefPath::new(),
        },
        VmRef::Instance {
            owner_instance_id: 77,
            offset: 0,
            path: RefPath::new(),
        },
    ];

    assert_eq!(infer_primary_instance_owner(&entry, &code, &refs), None);
}

#[test]
fn infer_primary_instance_owner_rejects_absent_unknown_and_truncated_code() {
    let refs = vec![VmRef::Instance {
        owner_instance_id: 42,
        offset: 0,
        path: RefPath::new(),
    }];

    for code in [vec![0x00], vec![0xff], vec![0x20, 0x00, 0x00]] {
        let entry = pou_entry(code.len());
        assert_eq!(infer_primary_instance_owner(&entry, &code, &refs), None);
    }
}

fn var_meta(ref_idx: u32, type_id: u32) -> VarMetaEntry {
    VarMetaEntry {
        name_idx: 0,
        type_id,
        ref_idx,
        retain: 0,
        init_const_idx: None,
    }
}

fn reference(
    location: RefLocation,
    owner_id: u32,
    offset: u32,
    segments: Vec<BytecodeRefSegment>,
) -> RefEntry {
    RefEntry {
        location,
        owner_id,
        offset,
        segments,
    }
}

fn pou_entry(code_end: usize) -> VmPouEntry {
    VmPouEntry {
        name: "Main".into(),
        code_start: 0,
        code_end,
        local_ref_start: 0,
        local_ref_count: 0,
        primary_instance_owner: None,
    }
}

fn bytecode_module(strings: Vec<SmolStr>, pous: Vec<PouEntry>) -> BytecodeModule {
    let mut types = vec![
        crate::bytecode::TypeEntry {
            kind: crate::bytecode::TypeKind::Primitive,
            name_idx: None,
            data: crate::bytecode::TypeData::Primitive {
                prim_id: 1,
                max_length: 0
            },
        };
        (TypeId::DINT.0.max(TypeId::INT.0) + 1) as usize
    ];
    types[TypeId::INT.0 as usize].data = crate::bytecode::TypeData::Primitive {
        prim_id: 7,
        max_length: 0,
    };
    types[TypeId::DINT.0 as usize].data = crate::bytecode::TypeData::Primitive {
        prim_id: 8,
        max_length: 0,
    };
    let consts = vec![
        crate::bytecode::ConstEntry {
            type_id: TypeId::INT.0,
            payload: 0_i16.to_le_bytes().to_vec()
        };
        8
    ];
    let mut module = BytecodeModule::new(BytecodeVersion::new(
        SUPPORTED_MAJOR_VERSION,
        SUPPORTED_MINOR_VERSION,
    ));
    module.sections = vec![
        section(
            SectionId::StringTable,
            SectionData::StringTable(StringTable { entries: strings }),
        ),
        section(
            SectionId::TypeTable,
            SectionData::TypeTable(TypeTable {
                offsets: Vec::new(),
                entries: types,
            }),
        ),
        section(
            SectionId::ConstPool,
            SectionData::ConstPool(ConstPool { entries: consts }),
        ),
        section(
            SectionId::RefTable,
            SectionData::RefTable(RefTable::default()),
        ),
        section(
            SectionId::PouIndex,
            SectionData::PouIndex(PouIndex { entries: pous }),
        ),
        section(SectionId::PouBodies, SectionData::PouBodies(Vec::new())),
        section(
            SectionId::ResourceMeta,
            SectionData::ResourceMeta(crate::bytecode::ResourceMeta::default()),
        ),
        section(
            SectionId::IoMap,
            SectionData::IoMap(crate::bytecode::IoMap::default()),
        ),
    ];
    module
}

fn section(id: SectionId, data: SectionData) -> Section {
    Section {
        id: id.as_raw(),
        flags: 0,
        data,
    }
}

fn pou(id: u32, name_idx: u32, kind: PouKind) -> PouEntry {
    PouEntry {
        id,
        name_idx,
        kind,
        code_offset: 0,
        code_length: 0,
        local_ref_start: 0,
        local_ref_count: 0,
        return_type_id: None,
        owner_pou_id: None,
        params: Vec::new(),
        class_meta: None,
    }
}
