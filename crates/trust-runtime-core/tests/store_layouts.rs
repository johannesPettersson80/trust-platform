//! Prepared layouts: sizes, offsets, leaves, capacities, region ranges and preparation charges.
//!
//! Specification: `docs/specs/34-runtime-portability.md` §6.2 (MEM-10, MEM-12) and the
//! prepared-layouts paragraph; `~/projects/.artifacts/bounded-c/decisions.md` D-C1-1 and D-C1-2
//! (`STRING[n]` holds `n` bytes and `WSTRING[n]` `2n` bytes; a `STRING`/`WSTRING` declared without a
//! length is 80 characters' worth: 80 and 160 bytes). Every expected size, offset and capacity below
//! is worked out by hand from the ST declarations the fixture encodes:
//!
//! ```st
//! TYPE Pair  : STRUCT a : INT; b : INT; END_STRUCT END_TYPE
//! TYPE Outer : STRUCT pp : Pair; arr : ARRAY[0..1] OF INT; END_STRUCT END_TYPE
//! FUNCTION_BLOCK Base
//! VAR x : INT; s : STRING[10]; END_VAR
//! END_FUNCTION_BLOCK
//! FUNCTION_BLOCK Derived EXTENDS Base
//! VAR y : INT; END_VAR
//! END_FUNCTION_BLOCK
//! FUNCTION F : INT
//! VAR_INPUT p : INT; END_VAR
//! VAR tmp : STRING[10]; END_VAR
//! END_FUNCTION
//! ```

#[path = "common/bytecode_helpers.rs"]
mod bytecode_helpers;

use bytecode_helpers::base_module;
use trust_runtime_core::bytecode::{
    compute_type_offsets_for_entries, AccessBindings, BytecodeModule, BytecodeVersion,
    ConstructionRoot, ConstructionRoots, Field, InitializationOnce, InitializationPhase,
    InitializationStage, InitializationTarget, InitializationTrigger, InitializerBodyKind,
    InitializerEntry, InitializerIndex, ParamEntry, PouClassMeta, PouEntry, PouIndex, PouKind,
    RefEntry, RefLocation, RefTable, Section, SectionData, SectionId, StorageDeclaration,
    StorageLayout, StorageOwner, StorageRole, StringTable, TypeData, TypeEntry, TypeKind,
    TypeTable, VarMeta, VarMetaEntry, HEADER_FLAG_CRC32,
};
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::memory::layout::{ByteRange, LayoutRegion};
use trust_runtime_core::vm::{PreparationLimits, PreparedModule};

const BOOL: u32 = 0;
const INT: u32 = 1;
const DWORD: u32 = 2;
const STRING_10: u32 = 3;
const WSTRING_10: u32 = 4;
const STRING_DEFAULT: u32 = 5;
const WSTRING_DEFAULT: u32 = 6;
const PAIR: u32 = 7;
const PAIR_ARRAY: u32 = 8;
const INT_ARRAY: u32 = 9;
const OUTER: u32 = 10;
const INT_REF: u32 = 11;
const BASE_FB_TYPE: u32 = 12;
const DERIVED_FB_TYPE: u32 = 13;
const OVERLAY: u32 = 14;
const REFS: u32 = 15;
const MIXED: u32 = 16;
const MODE: u32 = 17;
const SMALL: u32 = 18;

const PROGRAM_ROOT_DECL: usize = 0;
const TRIGGER_DECL: usize = 1;

const BASE_POU: u32 = 2;
const DERIVED_POU: u32 = 3;
const F_POU: u32 = 4;

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
    flags: u8,
) -> StorageDeclaration {
    StorageDeclaration {
        owner,
        role,
        retain: 0,
        flags,
        owner_pou_id,
        name_idx,
        type_id,
        slot,
        ref_idx,
        default_const_idx: None,
        construction_nodes: 1,
        related_declaration_idx: None,
        source_name_idx: None,
    }
}

fn initializer(
    declaration_idx: u32,
    owner_pou_id: Option<u32>,
    result_ref_idx: u32,
    phase: InitializationPhase,
    visible_local_count: u32,
) -> InitializerEntry {
    InitializerEntry {
        declaration_idx: Some(declaration_idx),
        owner_pou_id,
        result_ref_idx,
        code_offset: 0,
        code_length: 0,
        visible_local_count,
        visible_static_count: 0,
        phase,
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

/// A source-free program whose TYPE_TABLE, STORAGE_LAYOUT and INITIALIZERS exercise every layout
/// kind.
fn fixture() -> BytecodeModule {
    let mut module = base_module();
    module.version = BytecodeVersion::SOURCE_FREE;
    module.flags = HEADER_FLAG_CRC32;

    *module.section_mut(SectionId::StringTable).expect("strings") =
        SectionData::StringTable(StringTable {
            entries: [
                "R",
                "T",
                "Main",
                "trigger",
                "Base",
                "Derived",
                "F",
                "x",
                "s",
                "y",
                "g",
                "p",
                "tmp",
                "a",
                "b",
                "Pair",
                "Outer",
                "pp",
                "arr",
                "lo",
                "wide",
                "Overlay",
                "Refs",
                "r",
                "@local/4/0/ret",
                "@local/4/1/p",
                "@local/4/2/tmp",
                "@init/0/result",
                "@init/1/result",
                "@init/2/result",
                "@init/3/result",
                "@init/4/result",
                "@init/5/result",
                "@init/6/result",
                "Mixed",
                "Mode",
            ]
            .into_iter()
            .map(smol_str::SmolStr::new)
            .collect(),
        });

    let mut types = TypeTable {
        offsets: Vec::new(),
        entries: vec![
            primitive(1, 0),   // 0 BOOL
            primitive(7, 0),   // 1 INT
            primitive(4, 0),   // 2 DWORD
            primitive(24, 10), // 3 STRING[10]
            primitive(25, 10), // 4 WSTRING[10]
            primitive(24, 0),  // 5 STRING without a length
            primitive(25, 0),  // 6 WSTRING without a length
            named(
                TypeKind::Struct,
                15,
                TypeData::Struct {
                    fields: vec![field(13, INT), field(14, INT)],
                },
            ), // 7 Pair
            anonymous(
                TypeKind::Array,
                TypeData::Array {
                    elem_type_id: PAIR,
                    dims: vec![(0, 2)],
                },
            ), // 8 ARRAY[0..2] OF Pair
            anonymous(
                TypeKind::Array,
                TypeData::Array {
                    elem_type_id: INT,
                    dims: vec![(0, 1)],
                },
            ), // 9 ARRAY[0..1] OF INT
            named(
                TypeKind::Struct,
                16,
                TypeData::Struct {
                    fields: vec![field(17, PAIR), field(18, INT_ARRAY)],
                },
            ), // 10 Outer
            anonymous(
                TypeKind::Reference,
                TypeData::Reference {
                    target_type_id: INT,
                },
            ), // 11 REF_TO INT
            named(
                TypeKind::FunctionBlock,
                4,
                TypeData::Pou { pou_id: BASE_POU },
            ), // 12 Base
            named(
                TypeKind::FunctionBlock,
                5,
                TypeData::Pou {
                    pou_id: DERIVED_POU,
                },
            ), // 13 Derived
            named(
                TypeKind::OverlappingStruct,
                21,
                TypeData::Struct {
                    fields: vec![
                        Field {
                            name_idx: 19,
                            type_id: BOOL,
                            bit_offset: Some(0),
                        },
                        Field {
                            name_idx: 20,
                            type_id: DWORD,
                            bit_offset: Some(0),
                        },
                    ],
                },
            ), // 14 Overlay: a BOOL and a DWORD both at bit 0, so 4 backing bytes
            named(
                TypeKind::Struct,
                22,
                TypeData::Struct {
                    fields: vec![field(23, INT_REF)],
                },
            ), // 15 Refs
            named(
                TypeKind::Struct,
                34,
                TypeData::Struct {
                    fields: vec![field(19, BOOL), field(20, DWORD)],
                },
            ), // 16 Mixed { lo : BOOL; wide : DWORD }: three bytes of padding before `wide`
            named(
                TypeKind::Enum,
                35,
                TypeData::Enum {
                    base_type_id: DWORD,
                    variants: vec![
                        trust_runtime_core::bytecode::EnumVariant {
                            name_idx: 13,
                            value: 0,
                        },
                        trust_runtime_core::bytecode::EnumVariant {
                            name_idx: 14,
                            value: 1,
                        },
                    ],
                },
            ), // 17 Mode : DWORD (a, b)
            anonymous(
                TypeKind::Subrange,
                TypeData::Subrange {
                    base_type_id: INT,
                    lower: 0,
                    upper: 10,
                },
            ), // 18 INT (0..10)
        ],
    };
    types.offsets = compute_type_offsets_for_entries(&types.entries).expect("type offsets");
    *module.section_mut(SectionId::TypeTable).expect("types") = SectionData::TypeTable(types);

    *module.section_mut(SectionId::RefTable).expect("refs") = SectionData::RefTable(RefTable {
        entries: vec![
            RefEntry {
                location: RefLocation::Local,
                owner_id: F_POU,
                offset: 0,
                segments: vec![],
            },
            RefEntry {
                location: RefLocation::Local,
                owner_id: F_POU,
                offset: 1,
                segments: vec![],
            },
            RefEntry {
                location: RefLocation::Local,
                owner_id: F_POU,
                offset: 2,
                segments: vec![],
            },
            RefEntry {
                location: RefLocation::Global,
                owner_id: 0,
                offset: 0,
                segments: vec![],
            },
            RefEntry {
                location: RefLocation::Global,
                owner_id: 0,
                offset: 1,
                segments: vec![],
            },
            RefEntry {
                location: RefLocation::InitializerResult,
                owner_id: 0,
                offset: 0,
                segments: vec![],
            },
            RefEntry {
                location: RefLocation::InitializerResult,
                owner_id: 1,
                offset: 0,
                segments: vec![],
            },
            RefEntry {
                location: RefLocation::InitializerResult,
                owner_id: 2,
                offset: 0,
                segments: vec![],
            },
            RefEntry {
                location: RefLocation::InitializerResult,
                owner_id: 3,
                offset: 0,
                segments: vec![],
            },
            RefEntry {
                location: RefLocation::InitializerResult,
                owner_id: 4,
                offset: 0,
                segments: vec![],
            },
            RefEntry {
                location: RefLocation::InitializerResult,
                owner_id: 5,
                offset: 0,
                segments: vec![],
            },
            RefEntry {
                location: RefLocation::InitializerResult,
                owner_id: 6,
                offset: 0,
                segments: vec![],
            },
        ],
    });

    *module.section_mut(SectionId::PouIndex).expect("pous") = SectionData::PouIndex(PouIndex {
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
                params: vec![],
                class_meta: None,
            },
            PouEntry {
                id: BASE_POU,
                name_idx: 4,
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
                name_idx: 5,
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
            PouEntry {
                id: F_POU,
                name_idx: 6,
                kind: PouKind::Function,
                code_offset: 1,
                code_length: 0,
                local_ref_start: 0,
                local_ref_count: 3,
                return_type_id: Some(INT),
                owner_pou_id: None,
                params: vec![ParamEntry {
                    name_idx: 11,
                    type_id: INT,
                    direction: 0,
                    default_const_idx: None,
                }],
                class_meta: None,
            },
        ],
    });

    module.sections.extend([
        Section {
            id: SectionId::StorageLayout.as_raw(),
            flags: 0,
            data: SectionData::StorageLayout(StorageLayout {
                entries: vec![
                    declaration(
                        StorageOwner::Global,
                        StorageRole::ProgramRoot,
                        Some(1),
                        2,
                        None,
                        0,
                        Some(3),
                        0,
                    ),
                    declaration(
                        StorageOwner::Global,
                        StorageRole::Variable,
                        None,
                        3,
                        Some(BOOL),
                        1,
                        Some(4),
                        0,
                    ),
                    declaration(
                        StorageOwner::Instance,
                        StorageRole::Variable,
                        Some(BASE_POU),
                        7,
                        Some(INT),
                        0,
                        None,
                        0,
                    ),
                    declaration(
                        StorageOwner::Instance,
                        StorageRole::Variable,
                        Some(BASE_POU),
                        8,
                        Some(STRING_10),
                        1,
                        None,
                        0,
                    ),
                    declaration(
                        StorageOwner::Instance,
                        StorageRole::Variable,
                        Some(DERIVED_POU),
                        9,
                        Some(INT),
                        0,
                        None,
                        0,
                    ),
                    declaration(
                        StorageOwner::Frame,
                        StorageRole::Return,
                        Some(F_POU),
                        6,
                        Some(INT),
                        0,
                        Some(0),
                        0,
                    ),
                    declaration(
                        StorageOwner::Frame,
                        StorageRole::Parameter,
                        Some(F_POU),
                        11,
                        Some(INT),
                        1,
                        Some(1),
                        2,
                    ),
                    declaration(
                        StorageOwner::Frame,
                        StorageRole::Variable,
                        Some(F_POU),
                        12,
                        Some(STRING_10),
                        2,
                        Some(2),
                        0,
                    ),
                ],
            }),
        },
        Section {
            id: SectionId::ConstructionRoots.as_raw(),
            flags: 0,
            data: SectionData::ConstructionRoots(ConstructionRoots {
                entries: vec![
                    ConstructionRoot {
                        declaration_idx: PROGRAM_ROOT_DECL as u32,
                        binding_ref_idx: Some(3),
                        instance_owner_id: Some(0),
                        parent_root_idx: None,
                        template_pou_id: Some(1),
                        flags: 0,
                    },
                    ConstructionRoot {
                        declaration_idx: TRIGGER_DECL as u32,
                        binding_ref_idx: Some(4),
                        instance_owner_id: None,
                        parent_root_idx: None,
                        template_pou_id: None,
                        flags: 0,
                    },
                ],
            }),
        },
        Section {
            id: SectionId::Initializers.as_raw(),
            flags: 0,
            data: SectionData::Initializers(InitializerIndex {
                entries: vec![
                    initializer(1, None, 5, InitializationPhase::Resource, 0),
                    initializer(2, Some(BASE_POU), 6, InitializationPhase::Instance, 0),
                    initializer(3, Some(BASE_POU), 7, InitializationPhase::Instance, 0),
                    initializer(4, Some(DERIVED_POU), 8, InitializationPhase::Instance, 0),
                    initializer(5, Some(F_POU), 9, InitializationPhase::Return, 0),
                    initializer(6, Some(F_POU), 10, InitializationPhase::Parameter, 1),
                    initializer(7, Some(F_POU), 11, InitializationPhase::Frame, 2),
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
                    var_meta(24, INT, 0),
                    var_meta(25, INT, 1),
                    var_meta(26, STRING_10, 2),
                    var_meta(3, BOOL, 4),
                    var_meta(27, BOOL, 5),
                    var_meta(28, INT, 6),
                    var_meta(29, STRING_10, 7),
                    var_meta(30, INT, 8),
                    var_meta(31, INT, 9),
                    var_meta(32, INT, 10),
                    var_meta(33, STRING_10, 11),
                ],
            }),
        },
    ]);

    module
}

fn prepared() -> PreparedModule {
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

#[test]
fn text_leaves_have_byte_capacities() {
    let prepared = prepared();

    let cases = [
        (STRING_10, 10_u32),    // STRING[10] -> 10 bytes
        (WSTRING_10, 20),       // WSTRING[10] -> 2 * 10 = 20 bytes
        (STRING_DEFAULT, 80),   // STRING without a length -> 80 bytes (D-C1-1)
        (WSTRING_DEFAULT, 160), // WSTRING without a length -> 2 * 80 = 160 bytes
    ];
    for (type_id, capacity) in cases {
        let layout = prepared.type_layout(type_id).expect("text type");
        // MEM-12: a 2-byte length, then the characters, inline.
        assert_eq!(layout.shape.bytes, 2 + capacity, "type {type_id}");
        assert_eq!(
            layout.text_leaves,
            vec![trust_runtime_core::memory::layout::TextLeaf {
                capacity_bytes: capacity,
                byte_offset: 0,
            }],
            "type {type_id}"
        );
    }
}

#[test]
fn reference_and_instance_leaves_are_found_and_scalar_only_types_are_marked() {
    let prepared = prepared();

    let int = prepared.type_layout(INT).expect("INT");
    assert!(!int.holds_reference_or_instance);
    assert!(int.reference_leaves.is_empty());
    assert!(int.instance_leaves.is_empty());

    let reference = prepared.type_layout(INT_REF).expect("REF_TO INT");
    assert!(reference.holds_reference_or_instance);
    assert_eq!(
        reference.reference_leaves,
        vec![trust_runtime_core::memory::layout::ReferenceLeaf {
            byte_offset: 0,
            target_type_id: INT,
        }]
    );

    // Refs { r : REF_TO INT } finds the reference nested one level down.
    let refs = prepared.type_layout(REFS).expect("Refs");
    assert!(refs.holds_reference_or_instance);
    assert_eq!(refs.reference_leaves.len(), 1);
    assert_eq!(refs.reference_leaves[0].byte_offset, 0);

    for (type_id, expected_type) in [
        (BASE_FB_TYPE, BASE_FB_TYPE),
        (DERIVED_FB_TYPE, DERIVED_FB_TYPE),
    ] {
        let layout = prepared.type_layout(type_id).expect("function block type");
        assert!(layout.holds_reference_or_instance);
        assert_eq!(layout.instance_leaves.len(), 1);
        assert_eq!(layout.instance_leaves[0].byte_offset, 0);
        assert_eq!(layout.instance_leaves[0].type_id, expected_type);
    }
}

#[test]
fn an_overlap_struct_has_its_byte_backing() {
    let prepared = prepared();
    // Overlay { lo : BOOL; wide : DWORD } places both fields at bit 0, so its whole extent is the
    // DWORD's 32 bits: four backing bytes.
    let overlay = prepared.type_layout(OVERLAY).expect("Overlay");
    assert_eq!(overlay.overlap_backing_bytes, 4);
}

#[test]
fn globals_templates_locals_and_initializers_map_to_byte_ranges() {
    let prepared = prepared();

    // The program root owns no typed global; the "trigger" BOOL is the persistent block's byte 0.
    assert_eq!(prepared.global_range(PROGRAM_ROOT_DECL as u32), None);
    assert_eq!(
        prepared.global_range(TRIGGER_DECL as u32),
        Some(ByteRange {
            region: LayoutRegion::Persistent,
            start: 0,
            bytes: 1,
        })
    );

    // Base's template is its own x : INT at 0 and s : STRING[10] (a 2-byte length and 10 bytes) at
    // 2: 14 bytes from the instance's first byte, rounded up to 16 (spec 34 §6.2, MEM-12, O6).
    let base = prepared.instance_template(BASE_POU).expect("Base");
    assert_eq!(
        base.range,
        ByteRange {
            region: LayoutRegion::Persistent,
            start: 0,
            bytes: 16,
        }
    );
    assert_eq!(base.parent, None);

    // Derived extends Base: its template holds only its own y : INT, 2 bytes rounded up to 8, and
    // links to Base's template, which it does not repeat (spec 34 §6.2, O6). A flattened template
    // would give 24.
    let derived = prepared.instance_template(DERIVED_POU).expect("Derived");
    assert_eq!(
        derived.range,
        ByteRange {
            region: LayoutRegion::Persistent,
            start: 0,
            bytes: 8,
        }
    );
    assert_eq!(derived.parent, Some(BASE_POU));

    // F's frame holds the INT return at 0, the INT parameter at 2 and a STRING[10] local at 4: 16
    // bytes.
    assert_eq!(
        prepared.local_range(F_POU),
        Some(ByteRange {
            region: LayoutRegion::Activation,
            start: 0,
            bytes: 16,
        })
    );

    // Initializer results follow the locals on the activation stack, each at its declared size and
    // alignment: the BOOL result at 16, the INT at 18, the STRING[10] at 20 to 31, then the INTs of
    // Derived's y, F's return and F's parameter at 32, 34 and 36, and F's STRING[10] local at 38.
    assert_eq!(
        prepared.initializer_result_range(0),
        Some(ByteRange {
            region: LayoutRegion::Activation,
            start: 16,
            bytes: 1,
        })
    );
    assert_eq!(
        prepared.initializer_result_range(6),
        Some(ByteRange {
            region: LayoutRegion::Activation,
            start: 38,
            bytes: 12,
        })
    );
}

#[test]
fn ref_field_names_resolve_to_member_ordinals() {
    let prepared = prepared();

    // Outer's members are pp (ordinal 0, byte 0) and arr (ordinal 1, byte 4): the ordinal is not
    // the byte offset.
    assert_eq!(prepared.ref_field_member_ordinal(OUTER, "pp"), Some(0));
    assert_eq!(prepared.ref_field_member_ordinal(OUTER, "arr"), Some(1));
    // ST names are case-insensitive.
    assert_eq!(prepared.ref_field_member_ordinal(PAIR, "B"), Some(1));
    assert_eq!(prepared.ref_field_member_ordinal(PAIR, "a"), Some(0));
    assert_eq!(prepared.ref_field_member_ordinal(OUTER, "missing"), None);
    assert_eq!(prepared.ref_field_member_ordinal(INT, "a"), None);
}

/// A source-free program with the program root, its `trigger` BOOL and `int_globals` INT globals:
/// the same types and the same non-global metadata as its ten-times-larger twin, so only the
/// globals' region ranges (and bytes) differ (specification 34 §6.2).
fn globals_module(int_globals: u32) -> BytecodeModule {
    let mut module = base_module();
    module.version = BytecodeVersion::SOURCE_FREE;
    module.flags = HEADER_FLAG_CRC32;

    // 0..3: the resource and program names; then one name per global; then one initializer
    // result name per declaration that owns storage (the trigger and every INT global).
    let mut names: Vec<smol_str::SmolStr> = ["R", "T", "Main", "trigger"]
        .into_iter()
        .map(smol_str::SmolStr::new)
        .collect();
    for index in 0..int_globals {
        names.push(smol_str::SmolStr::new(format!("g{index}")));
    }
    let result_name_base = u32::try_from(names.len()).expect("string count fits u32");
    for index in 0..int_globals + 1 {
        names.push(smol_str::SmolStr::new(format!("@init/{index}/result")));
    }
    *module.section_mut(SectionId::StringTable).expect("strings") =
        SectionData::StringTable(StringTable { entries: names });

    // BOOL at 0 and INT at 1; the resource's `trigger` single name must be a BOOL global.
    let mut types = TypeTable {
        offsets: Vec::new(),
        entries: vec![primitive(1, 0), primitive(7, 0)],
    };
    types.offsets = compute_type_offsets_for_entries(&types.entries).expect("type offsets");
    *module.section_mut(SectionId::TypeTable).expect("types") = SectionData::TypeTable(types);

    // One Global reference per declaration, then one initializer-result reference per initializer.
    let mut references: Vec<RefEntry> = (0..int_globals + 2)
        .map(|offset| RefEntry {
            location: RefLocation::Global,
            owner_id: 0,
            offset,
            segments: vec![],
        })
        .collect();
    let result_ref_base = u32::try_from(references.len()).expect("reference count fits u32");
    for id in 0..int_globals + 1 {
        references.push(RefEntry {
            location: RefLocation::InitializerResult,
            owner_id: id,
            offset: 0,
            segments: vec![],
        });
    }
    *module.section_mut(SectionId::RefTable).expect("refs") = SectionData::RefTable(RefTable {
        entries: references,
    });

    let mut declarations = vec![
        // The program root owns no typed global; the `trigger` BOOL is the resource's single name.
        declaration(
            StorageOwner::Global,
            StorageRole::ProgramRoot,
            Some(1),
            2,
            None,
            0,
            Some(0),
            0,
        ),
        declaration(
            StorageOwner::Global,
            StorageRole::Variable,
            None,
            3,
            Some(0),
            1,
            Some(1),
            0,
        ),
    ];
    let mut roots = vec![
        ConstructionRoot {
            declaration_idx: 0,
            binding_ref_idx: Some(0),
            instance_owner_id: Some(0),
            parent_root_idx: None,
            template_pou_id: Some(1),
            flags: 0,
        },
        ConstructionRoot {
            declaration_idx: 1,
            binding_ref_idx: Some(1),
            instance_owner_id: None,
            parent_root_idx: None,
            template_pou_id: None,
            flags: 0,
        },
    ];
    // Every declaration that owns storage needs one Resource initializer, and its result
    // reference needs a VarMeta entry that carries the declaration's type.
    let mut initializers = vec![initializer(
        1,
        None,
        result_ref_base,
        InitializationPhase::Resource,
        0,
    )];
    let mut meta = vec![var_meta(result_name_base, 0, result_ref_base)];
    for index in 0..int_globals {
        let slot = index + 2;
        declarations.push(declaration(
            StorageOwner::Global,
            StorageRole::Variable,
            None,
            4 + index,
            Some(1),
            slot,
            Some(slot),
            0,
        ));
        roots.push(ConstructionRoot {
            declaration_idx: slot,
            binding_ref_idx: Some(slot),
            instance_owner_id: None,
            parent_root_idx: None,
            template_pou_id: None,
            flags: 0,
        });
        let result_ref = result_ref_base + 1 + index;
        initializers.push(initializer(
            slot,
            None,
            result_ref,
            InitializationPhase::Resource,
            0,
        ));
        meta.push(var_meta(result_name_base + 1 + index, 1, result_ref));
    }

    module.sections.extend([
        Section {
            id: SectionId::StorageLayout.as_raw(),
            flags: 0,
            data: SectionData::StorageLayout(StorageLayout {
                entries: declarations,
            }),
        },
        Section {
            id: SectionId::ConstructionRoots.as_raw(),
            flags: 0,
            data: SectionData::ConstructionRoots(ConstructionRoots { entries: roots }),
        },
        Section {
            id: SectionId::Initializers.as_raw(),
            flags: 0,
            data: SectionData::Initializers(InitializerIndex {
                entries: initializers,
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
            data: SectionData::VarMeta(VarMeta { entries: meta }),
        },
    ]);
    module
}

fn prepared_globals_module(int_globals: u32) -> PreparedModule {
    let module = globals_module(int_globals);
    assert_eq!(
        module.validate().err(),
        None,
        "the globals module validates"
    );
    PreparedModule::from_bytes(
        &module.encode().expect("encode"),
        PreparationLimits::default(),
    )
    .expect("the globals module prepares")
}

/// A program whose prepared layout tables are far larger than its other preparation demand:
/// `ALIAS_COUNT` aliases of a structure with `MEMBER_COUNT` scalar members. Every alias's layout
/// table copies the structure's member list (specification 34 §6.2).
fn layout_heavy_module() -> BytecodeModule {
    const MEMBER_COUNT: u32 = 64;
    const ALIAS_COUNT: u32 = 1024;

    // The program root and its BOOL trigger come first, then the layout-heavy type table.
    let mut module = globals_module(0);

    let mut names: Vec<smol_str::SmolStr> = ["R", "T", "Main", "trigger", "S"]
        .into_iter()
        .map(smol_str::SmolStr::new)
        .collect();
    for index in 0..MEMBER_COUNT {
        names.push(smol_str::SmolStr::new(format!("m{index}")));
    }
    *module.section_mut(SectionId::StringTable).expect("strings") =
        SectionData::StringTable(StringTable { entries: names });

    let mut entries = vec![
        primitive(1, 0), // 0 BOOL, the trigger global's type
        primitive(7, 0), // 1 INT
        named(
            TypeKind::Struct,
            4, // "S"
            TypeData::Struct {
                fields: (0..MEMBER_COUNT).map(|index| field(5 + index, 1)).collect(),
            },
        ), // 2 S with 64 INT members
    ];
    for _ in 0..ALIAS_COUNT {
        entries.push(anonymous(
            TypeKind::Alias,
            TypeData::Alias { target_type_id: 2 },
        ));
    }
    let mut types = TypeTable {
        offsets: Vec::new(),
        entries,
    };
    types.offsets = compute_type_offsets_for_entries(&types.entries).expect("type offsets");
    *module.section_mut(SectionId::TypeTable).expect("types") = SectionData::TypeTable(types);

    module
}

#[test]
fn layout_tables_are_charged_and_region_bytes_are_not() {
    // A region's bytes are not charged at preparation. Both programs have the same two types and
    // the same program root and BOOL trigger; one declares one INT global and one declares ten.
    // The nine extra globals add nine global region range entries and, because every storage
    // declaration owns one initializer, nine initializer-result entries, each an
    // `Option<ByteRange>` of 12 bytes (region, start, bytes), and nine declaration offsets of 4
    // bytes: 18 × 12 + 9 × 4 = 252 bytes. Their region bytes, the nine INT globals and the nine INT
    // results, 18 × 2 = 36 bytes, are not part of it: construction charges those when it reserves
    // the regions (spec 34 §6.2). A charge of 288 would count them here.
    let one = prepared_globals_module(1);
    let ten = prepared_globals_module(10);
    let grew = ten.layout_preparation_usage().bytes - one.layout_preparation_usage().bytes;
    assert_eq!(
        grew, 252,
        "nine extra globals charged {grew} layout bytes, not their region range entries and offsets"
    );

    // The layout tables are charged. Every alias of the layout-heavy type table copies the
    // structure's 64 members, a `MemberLayout` is two u32 fields (8 bytes), and the 1024 aliases'
    // layout tables alone need at least 1024 × 64 × 8 = 524,288 bytes. The default profile admits
    // the program; a 524,288-byte allowance is below the layout tables' own charge and refuses it
    // at preparation, never at run time (spec 34 §6.2, TIME-08).
    let module = layout_heavy_module();
    module
        .validate()
        .expect("the layout-heavy type table is well formed");
    let bytes = module.encode().expect("encode");
    PreparedModule::from_bytes(&bytes, PreparationLimits::default())
        .expect("the default profile admits the layout-heavy program");
    assert_eq!(
        PreparedModule::from_bytes(
            &bytes,
            PreparationLimits {
                max_preparation_bytes: 524_288,
                ..PreparationLimits::default()
            }
        )
        .err(),
        Some(RuntimeError::PreparationLimit)
    );
}

/// Specification 34 MEM-12 (owner, 19 October 2026): every variable takes its declared size at its
/// natural alignment in a flat block. Worked out by hand from MEM-12:
/// - `BOOL` 1, `INT` 2, `DWORD` 4 bytes, each aligned to its size;
/// - `STRING[10]` a 2-byte length and 10 bytes of UTF-8 = 12; `WSTRING[10]` the length and 10 UTF-16
///   units = 2 + 20 = 22; without a length `STRING[80]` = 82 and `WSTRING[80]` = 162; aligned to 2;
/// - `Pair { a : INT; b : INT }` = 4 bytes, `a` at 0 and `b` at 2; `ARRAY[0..2] OF Pair` three
///   4-byte elements = 12; `ARRAY[0..1] OF INT` = 4; `Outer { pp : Pair; arr : ARRAY[0..1] OF INT }`
///   = 8, `arr` at byte 4;
/// - `Mixed { lo : BOOL; wide : DWORD }`: `lo` at 0, three bytes of padding, `wide` at 4, so 8 bytes
///   aligned to 4;
/// - an enumeration on `DWORD` 4 bytes and a subrange of `INT` 2: the base type's size;
/// - `REF_TO INT` 16 bytes aligned to 8, and `Refs { r : REF_TO INT }` 16 with its reference leaf at
///   byte 0; a function-block instance leaf 8 bytes.
///
/// Catches a layout that counts 16-byte cells, one that packs without alignment (`wide` at 1),
/// one without the string length, one that gives an enumeration a fixed size, and one that forgets
/// a structure's trailing padding.
#[test]
fn byte_shapes_follow_mem_12() {
    use trust_runtime_core::memory::layout::ByteShape;
    let prepared = prepared();
    let shape = |type_id: u32| prepared.type_layout(type_id).expect("a type").shape;
    let at = |bytes: u32, align: u32| ByteShape { bytes, align };
    assert_eq!(shape(BOOL), at(1, 1));
    assert_eq!(shape(INT), at(2, 2));
    assert_eq!(shape(DWORD), at(4, 4));
    assert_eq!(shape(STRING_10), at(12, 2));
    assert_eq!(shape(WSTRING_10), at(22, 2));
    assert_eq!(shape(STRING_DEFAULT), at(82, 2));
    assert_eq!(shape(WSTRING_DEFAULT), at(162, 2));
    assert_eq!(shape(PAIR), at(4, 2));
    let pair = prepared.type_layout(PAIR).expect("Pair");
    assert_eq!(
        pair.members
            .iter()
            .map(|m| m.byte_offset)
            .collect::<Vec<_>>(),
        vec![0, 2]
    );
    assert_eq!(shape(PAIR_ARRAY), at(12, 2));
    assert_eq!(
        prepared
            .type_layout(PAIR_ARRAY)
            .and_then(|layout| layout.array.as_ref())
            .map(|array| array.byte_stride),
        Some(4)
    );
    assert_eq!(shape(INT_ARRAY), at(4, 2));
    assert_eq!(shape(OUTER), at(8, 2));
    let outer = prepared.type_layout(OUTER).expect("Outer");
    assert_eq!(
        outer
            .members
            .iter()
            .map(|m| m.byte_offset)
            .collect::<Vec<_>>(),
        vec![0, 4]
    );
    assert_eq!(shape(MIXED), at(8, 4));
    let mixed = prepared.type_layout(MIXED).expect("Mixed");
    assert_eq!(
        mixed
            .members
            .iter()
            .map(|m| m.byte_offset)
            .collect::<Vec<_>>(),
        vec![0, 4]
    );
    assert_eq!(shape(MODE), at(4, 4));
    assert_eq!(shape(SMALL), at(2, 2));
    assert_eq!(shape(INT_REF), at(16, 8));
    assert_eq!(shape(REFS), at(16, 8));
    assert_eq!(
        prepared
            .type_layout(REFS)
            .map(|layout| layout.reference_leaves[0].byte_offset),
        Some(0)
    );
    assert_eq!(shape(BASE_FB_TYPE), at(8, 8));
}
