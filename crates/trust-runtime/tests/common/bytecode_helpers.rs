#![allow(dead_code)]

use trust_runtime::bytecode::{
    BytecodeModule, BytecodeVersion, ConstEntry, ConstPool, DebugEntry, DebugMap, Field,
    InterfaceMethod, IoBinding, IoMap, MethodEntry, ParamEntry, PouClassMeta, PouEntry, PouIndex,
    PouKind, RefEntry, RefLocation, RefSegment, RefTable, ResourceEntry, ResourceMeta, Section,
    SectionData, SectionId, StringTable, TaskEntry, TypeData, TypeEntry, TypeKind, TypeTable,
};

fn type_entry_len(entry: &TypeEntry) -> usize {
    let header = 8usize;
    let payload = match &entry.data {
        TypeData::Primitive { .. } => 4,
        TypeData::Array { dims, .. } => 8 + dims.len() * 16,
        TypeData::Struct { fields } | TypeData::Union { fields } => 4 + fields.len() * 8,
        TypeData::Enum { variants, .. } => 8 + variants.len() * 12,
        TypeData::Alias { .. } => 4,
        TypeData::Subrange { .. } => 20,
        TypeData::Reference { .. } => 4,
        TypeData::Pou { .. } => 4,
        TypeData::Interface { methods } => 4 + methods.len() * 8,
    };
    header + payload
}

fn with_type_offsets(mut table: TypeTable) -> TypeTable {
    let mut offsets = Vec::with_capacity(table.entries.len());
    let mut cursor = 4usize + table.entries.len() * 4;
    for entry in &table.entries {
        offsets.push(cursor as u32);
        cursor += type_entry_len(entry);
    }
    table.offsets = offsets;
    table
}

pub fn base_string_table() -> StringTable {
    StringTable {
        entries: vec![
            "R".into(),
            "T".into(),
            "Main".into(),
            "trigger".into(),
            "field".into(),
            "File".into(),
        ],
    }
}

pub fn base_type_table() -> TypeTable {
    with_type_offsets(TypeTable {
        offsets: Vec::new(),
        entries: vec![TypeEntry {
            kind: TypeKind::Primitive,
            name_idx: None,
            data: TypeData::Primitive {
                prim_id: 1,
                max_length: 0,
            },
        }],
    })
}

pub fn base_const_pool() -> ConstPool {
    ConstPool {
        entries: Vec::new(),
    }
}

pub fn base_ref_table() -> RefTable {
    RefTable {
        entries: Vec::new(),
    }
}

pub fn base_pou_index() -> PouIndex {
    PouIndex {
        entries: vec![PouEntry {
            id: 1,
            name_idx: 2,
            kind: PouKind::Program,
            code_offset: 0,
            code_length: 1,
            local_ref_start: 0,
            local_ref_count: 0,
            return_type_id: None,
            owner_pou_id: None,
            params: Vec::new(),
            class_meta: None,
        }],
    }
}

pub fn base_resource_meta() -> ResourceMeta {
    ResourceMeta {
        resources: vec![ResourceEntry {
            name_idx: 0,
            inputs_size: 0,
            outputs_size: 0,
            memory_size: 0,
            tasks: vec![TaskEntry {
                name_idx: 1,
                priority: 0,
                interval_nanos: 1_000_000,
                single_name_idx: Some(3),
                program_name_idx: vec![2],
                fb_ref_idx: Vec::new(),
            }],
        }],
    }
}

pub fn base_io_map() -> IoMap {
    IoMap {
        bindings: Vec::new(),
    }
}

pub fn base_pou_bodies() -> Vec<u8> {
    vec![0x00]
}

pub fn base_module() -> BytecodeModule {
    let version = BytecodeVersion::new(1, 1);
    let mut module = BytecodeModule::new(version);
    module.sections = vec![
        Section {
            id: SectionId::StringTable.as_raw(),
            flags: 0,
            data: SectionData::StringTable(base_string_table()),
        },
        Section {
            id: SectionId::TypeTable.as_raw(),
            flags: 0,
            data: SectionData::TypeTable(base_type_table()),
        },
        Section {
            id: SectionId::ConstPool.as_raw(),
            flags: 0,
            data: SectionData::ConstPool(base_const_pool()),
        },
        Section {
            id: SectionId::RefTable.as_raw(),
            flags: 0,
            data: SectionData::RefTable(base_ref_table()),
        },
        Section {
            id: SectionId::PouIndex.as_raw(),
            flags: 0,
            data: SectionData::PouIndex(base_pou_index()),
        },
        Section {
            id: SectionId::PouBodies.as_raw(),
            flags: 0,
            data: SectionData::PouBodies(base_pou_bodies()),
        },
        Section {
            id: SectionId::ResourceMeta.as_raw(),
            flags: 0,
            data: SectionData::ResourceMeta(base_resource_meta()),
        },
        Section {
            id: SectionId::IoMap.as_raw(),
            flags: 0,
            data: SectionData::IoMap(base_io_map()),
        },
    ];
    module
}

pub fn module_with_debug() -> BytecodeModule {
    let mut module = base_module();
    module.sections.push(Section {
        id: SectionId::DebugStringTable.as_raw(),
        flags: 0,
        data: SectionData::DebugStringTable(StringTable {
            entries: vec!["File".into()],
        }),
    });
    module.sections.push(Section {
        id: SectionId::DebugMap.as_raw(),
        flags: 0,
        data: SectionData::DebugMap(DebugMap {
            entries: vec![DebugEntry {
                pou_id: 1,
                code_offset: 0,
                file_idx: 0,
                line: 1,
                column: 1,
                kind: 0,
            }],
        }),
    });
    module
}

pub fn sample_type_table() -> (StringTable, TypeTable) {
    let strings = StringTable {
        entries: vec![
            "Prim".into(),
            "Arr".into(),
            "Struct".into(),
            "Field".into(),
            "Enum".into(),
            "Variant".into(),
            "Alias".into(),
            "Subrange".into(),
            "Ref".into(),
            "Union".into(),
            "FB".into(),
            "Class".into(),
            "Interface".into(),
            "IMethod".into(),
        ],
    };
    let types = TypeTable {
        offsets: Vec::new(),
        entries: vec![
            TypeEntry {
                kind: TypeKind::Primitive,
                name_idx: Some(0),
                data: TypeData::Primitive {
                    prim_id: 1,
                    max_length: 0,
                },
            },
            TypeEntry {
                kind: TypeKind::Array,
                name_idx: Some(1),
                data: TypeData::Array {
                    elem_type_id: 0,
                    dims: vec![(0, 1)],
                },
            },
            TypeEntry {
                kind: TypeKind::Struct,
                name_idx: Some(2),
                data: TypeData::Struct {
                    fields: vec![Field {
                        name_idx: 3,
                        type_id: 0,
                    }],
                },
            },
            TypeEntry {
                kind: TypeKind::Enum,
                name_idx: Some(4),
                data: TypeData::Enum {
                    base_type_id: 0,
                    variants: vec![trust_runtime::bytecode::EnumVariant {
                        name_idx: 5,
                        value: 1,
                    }],
                },
            },
            TypeEntry {
                kind: TypeKind::Alias,
                name_idx: Some(6),
                data: TypeData::Alias { target_type_id: 0 },
            },
            TypeEntry {
                kind: TypeKind::Subrange,
                name_idx: Some(7),
                data: TypeData::Subrange {
                    base_type_id: 0,
                    lower: 0,
                    upper: 10,
                },
            },
            TypeEntry {
                kind: TypeKind::Reference,
                name_idx: Some(8),
                data: TypeData::Reference { target_type_id: 0 },
            },
            TypeEntry {
                kind: TypeKind::Union,
                name_idx: Some(9),
                data: TypeData::Union {
                    fields: vec![Field {
                        name_idx: 3,
                        type_id: 0,
                    }],
                },
            },
            TypeEntry {
                kind: TypeKind::FunctionBlock,
                name_idx: Some(10),
                data: TypeData::Pou { pou_id: 1 },
            },
            TypeEntry {
                kind: TypeKind::Class,
                name_idx: Some(11),
                data: TypeData::Pou { pou_id: 2 },
            },
            TypeEntry {
                kind: TypeKind::Interface,
                name_idx: Some(12),
                data: TypeData::Interface {
                    methods: vec![InterfaceMethod {
                        name_idx: 13,
                        slot: 0,
                    }],
                },
            },
        ],
    };
    (strings, with_type_offsets(types))
}

pub fn sample_pou_index() -> PouIndex {
    PouIndex {
        entries: vec![
            PouEntry {
                id: 1,
                name_idx: 2,
                kind: PouKind::Program,
                code_offset: 0,
                code_length: 1,
                local_ref_start: 0,
                local_ref_count: 0,
                return_type_id: None,
                owner_pou_id: None,
                params: vec![ParamEntry {
                    name_idx: 3,
                    type_id: 0,
                    direction: 0,
                    default_const_idx: None,
                }],
                class_meta: None,
            },
            PouEntry {
                id: 2,
                name_idx: 10,
                kind: PouKind::FunctionBlock,
                code_offset: 1,
                code_length: 1,
                local_ref_start: 0,
                local_ref_count: 0,
                return_type_id: None,
                owner_pou_id: None,
                params: Vec::new(),
                class_meta: Some(PouClassMeta {
                    parent_pou_id: None,
                    interfaces: Vec::new(),
                    methods: vec![MethodEntry {
                        name_idx: 3,
                        pou_id: 1,
                        vtable_slot: 0,
                        access: 0,
                        flags: 0,
                    }],
                }),
            },
        ],
    }
}

pub fn sample_ref_table() -> RefTable {
    RefTable {
        entries: vec![RefEntry {
            location: RefLocation::Global,
            owner_id: 0,
            offset: 1,
            segments: vec![RefSegment::Field { name_idx: 4 }],
        }],
    }
}

pub fn sample_io_map() -> IoMap {
    IoMap {
        bindings: vec![IoBinding {
            address_str_idx: 0,
            ref_idx: 0,
            type_id: None,
        }],
    }
}

pub fn sample_const_pool() -> ConstPool {
    ConstPool {
        entries: vec![ConstEntry {
            type_id: 0,
            payload: vec![1],
        }],
    }
}
