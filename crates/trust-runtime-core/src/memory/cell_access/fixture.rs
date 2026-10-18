//! The bytecode fixture of the cell arm's tests (specification 34 §6.2, MEM-10; C1-9b).
//!
//! It is the module `tests/store_regions.rs` checks, extended with one global of each scalar kind
//! and two persistent instances: `Base`, which holds an `ARRAY` and a structure as members, and
//! `Derived`, which extends it. The ST source it encodes is in `super::tests`, where every expected
//! value is worked out by hand.

use std::sync::Arc;

use smol_str::SmolStr;

use crate::bytecode::{
    compute_type_offsets_for_entries, AccessBindings, BytecodeModule, BytecodeVersion, ConstPool,
    ConstructionRoot, ConstructionRoots, Field, InitializationOnce, InitializationPhase,
    InitializationStage, InitializationTarget, InitializationTrigger, InitializerBodyKind,
    InitializerEntry, InitializerIndex, IoMap, PouClassMeta, PouEntry, PouIndex, PouKind, RefEntry,
    RefLocation, RefTable, ResourceEntry, ResourceMeta, Section, SectionData, SectionId,
    StorageDeclaration, StorageLayout, StorageOwner, StorageRole, StringTable, TaskEntry, TypeData,
    TypeEntry, TypeKind, TypeTable, VarMeta, VarMetaEntry, HEADER_FLAG_CRC32,
};
use crate::collections::OrderedMap;
use crate::memory::InstanceId;
use crate::value::{ArrayValue, StructValue, Value};
use crate::vm::{PreparationLimits, PreparedModule};

/// The `TYPE_TABLE` ordinals of the fixture's types.
pub(super) const DINT: u32 = 0;
pub(super) const REAL: u32 = 1;
pub(super) const STRING_10: u32 = 2;
const INT: u32 = 3;
pub(super) const INT_ARRAY: u32 = 4;
pub(super) const PAIR: u32 = 5;
pub(super) const WSTRING_5: u32 = 6;
pub(super) const BASE: u32 = 7;
pub(super) const DERIVED: u32 = 8;

/// The POU ids of the fixture's `PROGRAM Main`, `Base` and `Derived`.
const MAIN_POU: u32 = 1;
pub(super) const BASE_POU: u32 = 2;
pub(super) const DERIVED_POU: u32 = 3;

/// The identities the `Value` store issues. `BASE_INSTANCE` is the inheritance parent of `fb_g`'s
/// instance, which construction creates for the derived root; `DERIVED_INSTANCE` is `fb_g`'s instance;
/// `BASE_G_INSTANCE` is the `Base` instance the global `base_g` holds.
pub(super) const BASE_INSTANCE: InstanceId = InstanceId(0);
pub(super) const DERIVED_INSTANCE: InstanceId = InstanceId(1);
pub(super) const BASE_G_INSTANCE: InstanceId = InstanceId(2);

/// The globals' ordinals in declaration order (`PreparedModule::declarations` for the global
/// owner, which leaves out the program's own root), as the `Value` store and the references number
/// them.
pub(super) const DINT_G: usize = 1;
pub(super) const BASE_G: usize = 5;
pub(super) const FB_G: usize = 6;
pub(super) const GLOBAL_NAMES: [&str; 8] = [
    "Main", "dint_g", "real_g", "text_g", "wide_g", "base_g", "fb_g", "ref_g",
];

/// The declaration ids (STORAGE_LAYOUT indexes) of the globals with cells.
pub(super) const DINT_DECL: u32 = 1;
pub(super) const REAL_DECL: u32 = 2;
pub(super) const TEXT_DECL: u32 = 3;
pub(super) const WIDE_DECL: u32 = 4;
pub(super) const BASE_DECL: u32 = 5;
pub(super) const FB_DECL: u32 = 6;

/// The `REF_TO DINT` global `ref_g` (O9): its ordinal, its declaration index, its TYPE_TABLE entry and its
/// reference binding (RefEntry 17, after the 10 initializer results).
pub(super) const REF_G: usize = 7;
/// The storage slot of `ref_g`: its ordinal among the globals, which the slots of the globals are dense in.
const REF_SLOT: u32 = 7;
pub(super) const REF_DECL: u32 = 11;
pub(super) const REF_DINT: u32 = 9;

/// The CONSTRUCTION_ROOTS indexes of the `Base` and `Derived` instances (the program is index 0), and
/// of the inheritance parent of `fb_g`'s instance, which lies inside `FB_ROOT`'s range (R0).
pub(super) const BASE_ROOT: usize = 5;
pub(super) const FB_ROOT: usize = 6;
pub(super) const PARENT_ROOT: usize = 7;

/// The number of initializers: one per variable declaration with storage.
const INITIALIZERS: u32 = 10;

fn primitive(prim_id: u16, max_length: u16) -> TypeEntry {
    TypeEntry {
        kind: TypeKind::Primitive,
        name_idx: None,
        data: TypeData::Primitive {
            prim_id,
            max_length,
        },
    }
}

fn named(kind: TypeKind, name_idx: u32, data: TypeData) -> TypeEntry {
    TypeEntry {
        kind,
        name_idx: Some(name_idx),
        data,
    }
}

fn anonymous(kind: TypeKind, data: TypeData) -> TypeEntry {
    TypeEntry {
        kind,
        name_idx: None,
        data,
    }
}

fn field(name_idx: u32, type_id: u32) -> Field {
    Field {
        name_idx,
        type_id,
        bit_offset: None,
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "a storage declaration mirrors its wire fields one-to-one in the fixture"
)]
fn declaration(
    owner: StorageOwner,
    role: StorageRole,
    owner_pou_id: Option<u32>,
    name_idx: u32,
    type_id: Option<u32>,
    slot: u32,
    ref_idx: Option<u32>,
    construction_nodes: u32,
) -> StorageDeclaration {
    StorageDeclaration {
        owner,
        role,
        retain: 0,
        flags: 0,
        owner_pou_id,
        name_idx,
        type_id,
        slot,
        ref_idx,
        default_const_idx: None,
        construction_nodes,
        related_declaration_idx: None,
        source_name_idx: None,
    }
}

fn initializer(
    declaration_idx: u32,
    owner_pou_id: Option<u32>,
    result_ref_idx: u32,
) -> InitializerEntry {
    InitializerEntry {
        declaration_idx: Some(declaration_idx),
        owner_pou_id,
        result_ref_idx,
        code_offset: 0,
        code_length: 0,
        visible_local_count: 0,
        visible_static_count: 0,
        phase: if owner_pou_id.is_some() {
            InitializationPhase::Instance
        } else {
            InitializationPhase::Resource
        },
        once: InitializationOnce::None,
        stage: InitializationStage::Default,
        trigger: InitializationTrigger::Ordinary,
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
    }
}

fn var_meta(name_idx: u32, type_id: u32, ref_idx: u32) -> VarMetaEntry {
    VarMetaEntry {
        name_idx,
        type_id,
        ref_idx,
        retain: 0,
        init_const_idx: None,
    }
}

fn global_ref(offset: u32) -> RefEntry {
    RefEntry {
        location: RefLocation::Global,
        owner_id: 0,
        offset,
        segments: vec![],
    }
}

fn result_ref(initializer: u32) -> RefEntry {
    RefEntry {
        location: RefLocation::InitializerResult,
        owner_id: initializer,
        offset: 0,
        segments: vec![],
    }
}

/// The module the fixture starts from: the sections it does not replace (constant pool, POU bodies,
/// resource metadata, I/O map), and empty placeholders for those it does.
fn base_module() -> BytecodeModule {
    let mut module = BytecodeModule::new(BytecodeVersion::new(1, 1));
    module.sections = vec![
        Section {
            id: SectionId::StringTable.as_raw(),
            flags: 0,
            data: SectionData::StringTable(StringTable {
                entries: Vec::new(),
            }),
        },
        Section {
            id: SectionId::TypeTable.as_raw(),
            flags: 0,
            data: SectionData::TypeTable(TypeTable {
                offsets: Vec::new(),
                entries: Vec::new(),
            }),
        },
        Section {
            id: SectionId::ConstPool.as_raw(),
            flags: 0,
            data: SectionData::ConstPool(ConstPool {
                entries: Vec::new(),
            }),
        },
        Section {
            id: SectionId::RefTable.as_raw(),
            flags: 0,
            data: SectionData::RefTable(RefTable {
                entries: Vec::new(),
            }),
        },
        Section {
            id: SectionId::PouIndex.as_raw(),
            flags: 0,
            data: SectionData::PouIndex(PouIndex {
                entries: Vec::new(),
            }),
        },
        Section {
            id: SectionId::PouBodies.as_raw(),
            flags: 0,
            data: SectionData::PouBodies(vec![0x00]),
        },
        Section {
            id: SectionId::ResourceMeta.as_raw(),
            flags: 0,
            data: SectionData::ResourceMeta(ResourceMeta {
                resources: vec![ResourceEntry {
                    name_idx: 0,
                    inputs_size: 0,
                    outputs_size: 0,
                    memory_size: 0,
                    tasks: vec![TaskEntry {
                        name_idx: 1,
                        priority: 0,
                        interval_nanos: 1_000_000,
                        single_name_idx: None,
                        program_name_idx: vec![2],
                        fb_ref_idx: Vec::new(),
                    }],
                }],
            }),
        },
        Section {
            id: SectionId::IoMap.as_raw(),
            flags: 0,
            data: SectionData::IoMap(IoMap {
                bindings: Vec::new(),
            }),
        },
    ];
    module
}

/// The string table: `R`, `T` and `Main` as the base module names them, the declared names, then
/// one `@init/N/result` per initializer.
fn strings() -> StringTable {
    let mut entries: Vec<SmolStr> = [
        "R", "T", "Main", "dint_g", "real_g", "text_g", "wide_g", "base_g", "fb_g", "Pair", "x",
        "y", "Base", "Derived", "base_v", "series_v", "pair_v", "own_v",
    ]
    .into_iter()
    .map(SmolStr::new)
    .collect();
    entries.extend((0..INITIALIZERS).map(|index| SmolStr::new(format!("@init/{index}/result"))));
    entries.push(SmolStr::new("ref_g"));
    StringTable { entries }
}

fn types() -> TypeTable {
    let mut table = TypeTable {
        offsets: Vec::new(),
        entries: vec![
            primitive(8, 0),
            primitive(14, 0),
            primitive(24, 10),
            primitive(7, 0),
            anonymous(
                TypeKind::Array,
                TypeData::Array {
                    elem_type_id: INT,
                    dims: vec![(1, 3)],
                },
            ),
            named(
                TypeKind::Struct,
                9,
                TypeData::Struct {
                    fields: vec![field(10, INT), field(11, INT)],
                },
            ),
            primitive(25, 5),
            named(
                TypeKind::FunctionBlock,
                12,
                TypeData::Pou { pou_id: BASE_POU },
            ),
            named(
                TypeKind::FunctionBlock,
                13,
                TypeData::Pou {
                    pou_id: DERIVED_POU,
                },
            ),
            anonymous(
                TypeKind::Reference,
                TypeData::Reference {
                    target_type_id: DINT,
                },
            ),
        ],
    };
    table.offsets = compute_type_offsets_for_entries(&table.entries).expect("type offsets");
    table
}

fn refs() -> RefTable {
    let mut entries: Vec<RefEntry> = (0..=FB_DECL).map(global_ref).collect();
    entries.extend((0..INITIALIZERS).map(result_ref));
    entries.push(global_ref(REF_SLOT));
    entries.push(result_ref(INITIALIZERS));
    RefTable { entries }
}

fn pous() -> PouIndex {
    PouIndex {
        entries: vec![
            PouEntry {
                id: MAIN_POU,
                name_idx: 2,
                kind: PouKind::Program,
                code_offset: 0,
                code_length: 1,
                local_ref_start: 0,
                local_ref_count: 0,
                return_type_id: None,
                owner_pou_id: None,
                params: vec![],
                class_meta: None,
            },
            PouEntry {
                id: BASE_POU,
                name_idx: 12,
                kind: PouKind::FunctionBlock,
                code_offset: 1,
                code_length: 0,
                local_ref_start: 0,
                local_ref_count: 0,
                return_type_id: None,
                owner_pou_id: None,
                params: vec![],
                class_meta: Some(PouClassMeta {
                    parent_pou_id: None,
                    interfaces: vec![],
                    methods: vec![],
                }),
            },
            PouEntry {
                id: DERIVED_POU,
                name_idx: 13,
                kind: PouKind::FunctionBlock,
                code_offset: 1,
                code_length: 0,
                local_ref_start: 0,
                local_ref_count: 0,
                return_type_id: None,
                owner_pou_id: None,
                params: vec![],
                class_meta: Some(PouClassMeta {
                    parent_pou_id: Some(BASE_POU),
                    interfaces: vec![],
                    methods: vec![],
                }),
            },
        ],
    }
}

/// The storage declarations: the program's root, the globals with cells, and the members of `Base`
/// and `Derived`. `Base`'s demand is its own members (1 + 1 + 4 + 3 = 9); `Derived`'s is its parent's
/// plus its own (1 + 9 + 1 = 11), as the validator's demand model counts it.
fn storage_layout() -> StorageLayout {
    let mut layout = StorageLayout {
        entries: vec![
            declaration(
                StorageOwner::Global,
                StorageRole::ProgramRoot,
                Some(MAIN_POU),
                2,
                None,
                0,
                Some(0),
                1,
            ),
            declaration(
                StorageOwner::Global,
                StorageRole::Variable,
                None,
                3,
                Some(DINT),
                1,
                Some(1),
                1,
            ),
            declaration(
                StorageOwner::Global,
                StorageRole::Variable,
                None,
                4,
                Some(REAL),
                2,
                Some(2),
                1,
            ),
            declaration(
                StorageOwner::Global,
                StorageRole::Variable,
                None,
                5,
                Some(STRING_10),
                3,
                Some(3),
                1,
            ),
            declaration(
                StorageOwner::Global,
                StorageRole::Variable,
                None,
                6,
                Some(WSTRING_5),
                4,
                Some(4),
                1,
            ),
            declaration(
                StorageOwner::Global,
                StorageRole::Variable,
                None,
                7,
                Some(BASE),
                5,
                Some(5),
                9,
            ),
            declaration(
                StorageOwner::Global,
                StorageRole::Variable,
                None,
                8,
                Some(DERIVED),
                6,
                Some(6),
                11,
            ),
            declaration(
                StorageOwner::Instance,
                StorageRole::Variable,
                Some(BASE_POU),
                14,
                Some(DINT),
                0,
                None,
                1,
            ),
            declaration(
                StorageOwner::Instance,
                StorageRole::Variable,
                Some(BASE_POU),
                15,
                Some(INT_ARRAY),
                1,
                None,
                4,
            ),
            declaration(
                StorageOwner::Instance,
                StorageRole::Variable,
                Some(BASE_POU),
                16,
                Some(PAIR),
                2,
                None,
                3,
            ),
            declaration(
                StorageOwner::Instance,
                StorageRole::Variable,
                Some(DERIVED_POU),
                17,
                Some(REAL),
                0,
                None,
                1,
            ),
        ],
    };
    layout.entries.push(declaration(
        StorageOwner::Global,
        StorageRole::Variable,
        None,
        28,
        Some(REF_DINT),
        REF_SLOT,
        Some(17),
        1,
    ));
    layout
}

/// The construction roots: the program's root, one for every global variable (the validator
/// requires each global to be a root), the `Base` and `Derived` instances' roots, and the inheritance
/// parent of `fb_g`'s instance, which the encoder emits with the inheritance flag (`flags` 1).
fn construction_roots() -> ConstructionRoots {
    let root =
        |declaration_idx: u32, instance_owner_id: Option<u32>, template_pou_id: Option<u32>| {
            ConstructionRoot {
                declaration_idx,
                binding_ref_idx: Some(declaration_idx),
                instance_owner_id,
                parent_root_idx: None,
                template_pou_id,
                flags: 0,
            }
        };
    let mut entries = vec![root(0, Some(0), Some(MAIN_POU))];
    entries.extend((DINT_DECL..=WIDE_DECL).map(|declaration| root(declaration, None, None)));
    entries.push(root(BASE_DECL, Some(1), Some(BASE_POU)));
    entries.push(root(FB_DECL, Some(2), Some(DERIVED_POU)));
    entries.push(ConstructionRoot {
        declaration_idx: FB_DECL,
        binding_ref_idx: None,
        instance_owner_id: Some(3),
        parent_root_idx: Some(u32::try_from(FB_ROOT).expect("a root index fits a u32")),
        template_pou_id: Some(BASE_POU),
        flags: 1,
    });
    entries.push(ConstructionRoot {
        declaration_idx: REF_DECL,
        binding_ref_idx: Some(17),
        instance_owner_id: None,
        parent_root_idx: None,
        template_pou_id: None,
        flags: 0,
    });
    ConstructionRoots { entries }
}

fn fixture() -> BytecodeModule {
    let mut module = base_module();
    module.version = BytecodeVersion::SOURCE_FREE;
    module.flags = HEADER_FLAG_CRC32;
    *module.section_mut(SectionId::StringTable).expect("strings") =
        SectionData::StringTable(strings());
    *module.section_mut(SectionId::TypeTable).expect("types") = SectionData::TypeTable(types());
    *module.section_mut(SectionId::RefTable).expect("refs") = SectionData::RefTable(refs());
    *module.section_mut(SectionId::PouIndex).expect("pous") = SectionData::PouIndex(pous());
    module.sections.extend([
        Section {
            id: SectionId::StorageLayout.as_raw(),
            flags: 0,
            data: SectionData::StorageLayout(storage_layout()),
        },
        Section {
            id: SectionId::ConstructionRoots.as_raw(),
            flags: 0,
            data: SectionData::ConstructionRoots(construction_roots()),
        },
        Section {
            id: SectionId::Initializers.as_raw(),
            flags: 0,
            data: SectionData::Initializers(InitializerIndex {
                entries: vec![
                    initializer(DINT_DECL, None, 7),
                    initializer(REAL_DECL, None, 8),
                    initializer(TEXT_DECL, None, 9),
                    initializer(WIDE_DECL, None, 10),
                    initializer(BASE_DECL, None, 11),
                    initializer(FB_DECL, None, 12),
                    initializer(7, Some(BASE_POU), 13),
                    initializer(8, Some(BASE_POU), 14),
                    initializer(9, Some(BASE_POU), 15),
                    initializer(10, Some(DERIVED_POU), 16),
                    initializer(REF_DECL, None, 18),
                ],
            }),
        },
        Section {
            id: SectionId::AccessBindings.as_raw(),
            flags: 0,
            data: SectionData::AccessBindings(AccessBindings::default()),
        },
        Section {
            id: SectionId::VarMeta.as_raw(),
            flags: 0,
            data: SectionData::VarMeta(VarMeta {
                entries: vec![
                    var_meta(18, DINT, 7),
                    var_meta(19, REAL, 8),
                    var_meta(20, STRING_10, 9),
                    var_meta(21, WSTRING_5, 10),
                    var_meta(22, BASE, 11),
                    var_meta(23, DERIVED, 12),
                    var_meta(24, DINT, 13),
                    var_meta(25, INT_ARRAY, 14),
                    var_meta(26, PAIR, 15),
                    var_meta(27, REAL, 16),
                    var_meta(28, REF_DINT, 18),
                ],
            }),
        },
    ]);
    module
}

/// The fixture prepared for execution (`PreparedModule`).
pub(super) fn prepared() -> PreparedModule {
    let module = fixture();
    module
        .validate()
        .expect("the fixture is a valid source-free module");
    PreparedModule::from_bytes(
        &module.encode().expect("encode"),
        PreparationLimits::default(),
    )
    .expect("the fixture prepares")
}

/// The `ARRAY[1..3] OF INT` value `series_v` holds: 1, 2, 3.
pub(super) fn series_value() -> Value {
    Value::Array(Box::new(ArrayValue::from_canonical_parts(
        vec![Value::Int(1), Value::Int(2), Value::Int(3)],
        vec![(1, 3)],
    )))
}

/// The `Pair` value `pair_v` holds: `x` is 10 and `y` is 20.
pub(super) fn pair_value() -> Value {
    let mut fields = OrderedMap::default();
    fields.insert(SmolStr::new("x"), Value::Int(10));
    fields.insert(SmolStr::new("y"), Value::Int(20));
    Value::Struct(Arc::new(StructValue::from_canonical_parts(
        SmolStr::new("Pair"),
        fields,
    )))
}
