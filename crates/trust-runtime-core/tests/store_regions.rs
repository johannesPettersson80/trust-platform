//! The persistent block: one byte range per persistent root (O8), a derived template's own bytes and
//! parent link (O6, O7), the charge taken before the block is reserved (O18), and every text leaf
//! occurrence inline (O10).
//!
//! Specification: `docs/specs/34-runtime-portability.md` §6.2 (MEM-10, MEM-12). Every expected value
//! below is worked out by hand from the source this fixture encodes:
//!
//! ```st
//! FUNCTION_BLOCK Names
//! VAR v : ARRAY[1..3] OF STRING[5]; END_VAR
//! END_FUNCTION_BLOCK
//! FUNCTION_BLOCK Derived EXTENDS Names
//! END_FUNCTION_BLOCK
//! PROGRAM Main
//! VAR trigger : BOOL; g_a : Names; g_b : Names; END_VAR
//! END_PROGRAM
//! ```
//!
//! The globals are `trigger` (a BOOL, byte 0), `g_a` and `g_b` (an 8-byte instance leaf each, bytes 8
//! to 23). The roots are `Main` (no own bytes), `trigger` (no template), `g_a` and `g_b` (each a
//! `Names`: three `STRING[5]` of a 2-byte length and five bytes, at a stride of 8, 24 bytes).

#[path = "common/bytecode_helpers.rs"]
mod bytecode_helpers;

use bytecode_helpers::base_module;
use trust_runtime_core::bytecode::{
    compute_type_offsets_for_entries, AccessBindings, BytecodeModule, BytecodeVersion,
    ConstructionRoot, ConstructionRoots, InitializationOnce, InitializationPhase,
    InitializationStage, InitializationTarget, InitializationTrigger, InitializerBodyKind,
    InitializerEntry, InitializerIndex, PouClassMeta, PouEntry, PouIndex, PouKind, RefEntry,
    RefLocation, RefTable, Section, SectionData, SectionId, StorageDeclaration, StorageLayout,
    StorageOwner, StorageRole, StringTable, TypeData, TypeEntry, TypeKind, TypeTable, VarMeta,
    VarMetaEntry, HEADER_FLAG_CRC32,
};
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::memory::layout::{ByteRange, LayoutRegion};
use trust_runtime_core::memory::regions::CellStorage;
use trust_runtime_core::memory::Generation;
use trust_runtime_core::vm::{PreparationLimits, PreparedModule};

const BOOL: u32 = 0;
const STRING_5: u32 = 1;
const ARRAY_V: u32 = 2;
const NAMES_TYPE: u32 = 3;

const MAIN_POU: u32 = 1;
const NAMES_POU: u32 = 2;
const DERIVED_POU: u32 = 3;

/// The persistent bytes of the fixture's globals, before the roots' ranges.
const GLOBAL_BYTES: u32 = 24;
/// One `Names` instance's own bytes: its array `v`, three 7-byte texts at a stride of 8.
const NAMES_BYTES: u32 = 24;

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

/// One storage declaration. `construction_nodes` is the type's construction demand, which the
/// validator recomputes: a leaf is one node, a structure is one plus its members, an array is one
/// plus its count times its element, and an instance is its POU's node (one plus its own members).
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

fn fixture() -> BytecodeModule {
    let mut module = base_module();
    module.version = BytecodeVersion::SOURCE_FREE;
    module.flags = HEADER_FLAG_CRC32;

    // Strings 0 to 5 are the base module's. Then: 6 Names, 7 Derived, 8 g_a, 9 g_b, 10 v, and the
    // four initializer results (11 to 14).
    *module.section_mut(SectionId::StringTable).expect("strings") =
        SectionData::StringTable(StringTable {
            entries: [
                "R",
                "T",
                "Main",
                "trigger",
                "field",
                "File",
                "Names",
                "Derived",
                "g_a",
                "g_b",
                "v",
                "@init/0/result",
                "@init/1/result",
                "@init/2/result",
                "@init/3/result",
            ]
            .into_iter()
            .map(smol_str::SmolStr::new)
            .collect(),
        });

    let mut types = TypeTable {
        offsets: Vec::new(),
        entries: vec![
            primitive(1, 0),
            primitive(24, 5),
            anonymous(
                TypeKind::Array,
                TypeData::Array {
                    elem_type_id: STRING_5,
                    dims: vec![(1, 3)],
                },
            ),
            named(
                TypeKind::FunctionBlock,
                6,
                TypeData::Pou { pou_id: NAMES_POU },
            ),
            named(
                TypeKind::FunctionBlock,
                7,
                TypeData::Pou {
                    pou_id: DERIVED_POU,
                },
            ),
        ],
    };
    types.offsets = compute_type_offsets_for_entries(&types.entries).expect("type offsets");
    *module.section_mut(SectionId::TypeTable).expect("types") = SectionData::TypeTable(types);

    *module.section_mut(SectionId::RefTable).expect("refs") = SectionData::RefTable(RefTable {
        entries: vec![
            global_ref(0),
            global_ref(1),
            global_ref(2),
            global_ref(3),
            result_ref(0),
            result_ref(1),
            result_ref(2),
            result_ref(3),
        ],
    });

    *module.section_mut(SectionId::PouIndex).expect("pous") = SectionData::PouIndex(PouIndex {
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
                id: NAMES_POU,
                name_idx: 6,
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
                name_idx: 7,
                kind: PouKind::FunctionBlock,
                code_offset: 1,
                code_length: 0,
                local_ref_start: 0,
                local_ref_count: 0,
                return_type_id: None,
                owner_pou_id: None,
                params: vec![],
                class_meta: Some(PouClassMeta {
                    parent_pou_id: Some(NAMES_POU),
                    interfaces: vec![],
                    methods: vec![],
                }),
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
                        Some(BOOL),
                        1,
                        Some(1),
                        1,
                    ),
                    declaration(
                        StorageOwner::Global,
                        StorageRole::Variable,
                        None,
                        8,
                        Some(NAMES_TYPE),
                        2,
                        Some(2),
                        5,
                    ),
                    declaration(
                        StorageOwner::Global,
                        StorageRole::Variable,
                        None,
                        9,
                        Some(NAMES_TYPE),
                        3,
                        Some(3),
                        5,
                    ),
                    declaration(
                        StorageOwner::Instance,
                        StorageRole::Variable,
                        Some(NAMES_POU),
                        10,
                        Some(ARRAY_V),
                        0,
                        None,
                        4,
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
                        declaration_idx: 0,
                        binding_ref_idx: Some(0),
                        instance_owner_id: Some(0),
                        parent_root_idx: None,
                        template_pou_id: Some(MAIN_POU),
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
                    ConstructionRoot {
                        declaration_idx: 2,
                        binding_ref_idx: Some(2),
                        instance_owner_id: Some(1),
                        parent_root_idx: None,
                        template_pou_id: Some(NAMES_POU),
                        flags: 0,
                    },
                    ConstructionRoot {
                        declaration_idx: 3,
                        binding_ref_idx: Some(3),
                        instance_owner_id: Some(2),
                        parent_root_idx: None,
                        template_pou_id: Some(NAMES_POU),
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
                    initializer(1, None, 4),
                    initializer(2, None, 5),
                    initializer(3, None, 6),
                    initializer(4, Some(NAMES_POU), 7),
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
                    var_meta(11, BOOL, 4),
                    var_meta(12, NAMES_TYPE, 5),
                    var_meta(13, NAMES_TYPE, 6),
                    var_meta(14, ARRAY_V, 7),
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

fn construct(prepared: &PreparedModule) -> CellStorage {
    CellStorage::construct(prepared, Generation::FIRST, |_| Ok(()))
        .expect("the persistent region reserves")
}

#[test]
fn persistent_region_reserves_one_range_per_root() {
    // Specification 34 §6.2 and O8: each persistent root has its own byte range. The globals take
    // bytes 0 to 23 (trigger at 0, g_a and g_b at 8 and 16). Then the roots in order: Main has no
    // own bytes, trigger has no template, and g_a and g_b each take the 24 bytes of a Names
    // instance. The ranges are [24, 48) for g_a and [48, 72) for g_b, 72 bytes in all. One range
    // per template would give g_b g_a's range and reserve 48 bytes.
    let prepared = prepared();
    let storage = construct(&prepared);
    let regions = storage.regions();
    assert_eq!(
        regions.root_range(2),
        Some(ByteRange {
            region: LayoutRegion::Persistent,
            start: GLOBAL_BYTES,
            bytes: NAMES_BYTES,
        })
    );
    assert_eq!(
        regions.root_range(3),
        Some(ByteRange {
            region: LayoutRegion::Persistent,
            start: GLOBAL_BYTES + NAMES_BYTES,
            bytes: NAMES_BYTES,
        })
    );
    assert_eq!(regions.persistent().reserved(), 72);
}

#[test]
fn derived_template_exists_without_own_variables() {
    // Specification 34 §6.2, O6 and O7: a derived function block without variables of its own has
    // an empty template with a link to its parent's template, and the parent's template holds only
    // the parent's own array. A missing template would make the derived instance unreservable; a
    // flattened template would count the parent's 24 bytes again.
    let prepared = prepared();
    let derived = prepared
        .instance_template(DERIVED_POU)
        .expect("a derived template exists");
    assert_eq!(derived.range.bytes, 0);
    assert_eq!(derived.parent, Some(NAMES_POU));
    let names = prepared
        .instance_template(NAMES_POU)
        .expect("the parent template exists");
    assert_eq!(names.range.bytes, NAMES_BYTES);
    assert_eq!(names.parent, None);
}

#[test]
fn reservation_is_charged_before_it_is_reserved() {
    // Specification 34 §6.2 and O18: the region's bytes are charged before anything is reserved.
    // The block is 72 bytes and holds no reference leaf. Construction charges the memory it holds
    // with it (C1-9d2b-1, R6): the `globals` table has one 12-byte entry for each of the 5
    // declarations and the `roots` table one for each of the 4 roots, 108 bytes; the transient
    // list holds 5 initial values at 24 bytes (`trigger`, `g_a`, `g_b` and the `v` of each `Names`
    // root), 120 bytes. The instance table holds one 28-byte entry for each of the 3 roots that
    // have a template POU (`Main`, `g_a` and `g_b`; `trigger` has none), 84 bytes (C1-9d2b-2, R5).
    // The charge is 72 + 108 + 120 + 84 = 384 bytes, requested once. An allowance one byte short
    // refuses it and reserves nothing; an allowance of exactly 384 admits the region. A charge
    // that still counted 16-byte cells and separate text buffers would ask for more.
    let prepared = prepared();
    let requested = std::cell::Cell::new(0usize);
    let refused = CellStorage::construct(&prepared, Generation::FIRST, |bytes| {
        requested.set(bytes);
        if bytes > 383 {
            Err(RuntimeError::Overflow)
        } else {
            Ok(())
        }
    });
    assert_eq!(refused.err(), Some(RuntimeError::Overflow));
    assert_eq!(requested.get(), 384);

    let admitted = CellStorage::construct(&prepared, Generation::FIRST, |bytes| {
        if bytes > 384 {
            Err(RuntimeError::Overflow)
        } else {
            Ok(())
        }
    });
    assert!(admitted.is_ok());
}

#[test]
fn text_leaves_lie_inline_per_array_element_and_per_instance() {
    // Specification 34 MEM-12 and O10: every text leaf occurrence lies inline at its declared size.
    // `v : ARRAY[1..3] OF STRING[5]` is three texts of a 2-byte length and five bytes, each 7 bytes
    // aligned to 2, so the elements are 8 bytes apart and the array 24 bytes; one text leaf, at
    // byte 0 of the element, with a capacity of 5 bytes. Each Names instance holds its own copy in
    // its own range, so the six texts lie at 24, 32 and 40 (g_a) and 48, 56 and 64 (g_b). An array
    // that kept one leaf for all its elements, or a stride without the length, fails here.
    let prepared = prepared();
    let array = prepared.type_layout(ARRAY_V).expect("the array of texts");
    assert_eq!(array.shape.bytes, 24);
    assert_eq!(
        array.array.as_ref().map(|layout| layout.byte_stride),
        Some(8)
    );
    assert_eq!(array.text_leaves.len(), 1);
    assert_eq!(array.text_leaves[0].capacity_bytes, 5);
    assert_eq!(array.text_leaves[0].byte_offset, 0);
    let storage = construct(&prepared);
    let regions = storage.regions();
    let starts: Vec<u32> = [2, 3]
        .into_iter()
        .flat_map(|root| {
            let range = regions.root_range(root).expect("a Names root");
            (0..3).map(move |element| range.start + element * 8)
        })
        .collect();
    assert_eq!(starts, vec![24, 32, 40, 48, 56, 64]);
}
