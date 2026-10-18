//! The cell, the unit of the bounded value store, and one region's cell store.
//!
//! Specification: `docs/specs/34-runtime-portability.md` §6.2 (MEM-10) and the paragraph "Cells and
//! the cell store" after "Prepared cell layouts"; design
//! `~/projects/.artifacts/bounded-c/design.md` "Cells", "Regions" and "Operands and references".
//! Every expected number below is worked out by hand from the ST declarations the fixture encodes:
//!
//! ```st
//! TYPE Refs : STRUCT r : REF_TO INT; END_STRUCT END_TYPE
//! FUNCTION_BLOCK Base
//! VAR x : INT; p : REF_TO INT; END_VAR
//! END_FUNCTION_BLOCK
//! FUNCTION_BLOCK Derived EXTENDS Base
//! VAR y : INT; END_VAR
//! END_FUNCTION_BLOCK
//! ```
//!
//! The fixture's persistent region is the `trigger` BOOL and the `g_refs` structure (one cell each),
//! then Base's template (`x` and `p`: two cells) and Derived's, which places Base's two cells first
//! and adds `y` (three cells): seven cells. Its reference leaves are `g_refs`'s one `REF_TO INT`
//! member, Base's one and the one Derived inherits: three records.

#[path = "common/bytecode_helpers.rs"]
mod bytecode_helpers;

use bytecode_helpers::base_module;
use trust_runtime_core::bytecode::{
    compute_type_offsets_for_entries, AccessBindings, BytecodeModule, BytecodeVersion,
    ConstructionRoot, ConstructionRoots, Field, InitializationOnce, InitializationPhase,
    InitializationStage, InitializationTarget, InitializationTrigger, InitializerBodyKind,
    InitializerEntry, InitializerIndex, PouClassMeta, PouEntry, PouIndex, PouKind, RefEntry,
    RefLocation, RefTable, Section, SectionData, SectionId, StorageDeclaration, StorageLayout,
    StorageOwner, StorageRole, StringTable, TypeData, TypeEntry, TypeKind, TypeTable, VarMeta,
    VarMetaEntry, HEADER_FLAG_CRC32,
};
use trust_runtime_core::memory::cells::{Cell, CellStore, RefRecord, RefSlot, TextSlot};
use trust_runtime_core::memory::layout::{CellRange, LayoutRegion, CELL_BYTES};
use trust_runtime_core::memory::{Generation, InstanceId};
use trust_runtime_core::vm::{PreparationLimits, PreparedModule};

const BOOL: u32 = 0;
const INT: u32 = 1;
const INT_REF: u32 = 2;
const REFS: u32 = 3;

const PROGRAM_ROOT_DECL: u32 = 0;
const TRIGGER_DECL: u32 = 1;
const REFS_DECL: u32 = 2;
const BASE_X_DECL: u32 = 3;
const BASE_P_DECL: u32 = 4;
const DERIVED_Y_DECL: u32 = 5;

const MAIN_POU: u32 = 1;
const BASE_POU: u32 = 2;
const DERIVED_POU: u32 = 3;

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

/// A declaration that owns storage, with everything the fixture does not exercise at its default.
///
/// `construction_nodes` is the declaration's validated construction demand: one node for a scalar
/// type, and the type's own node plus its members' for a compound.
fn declaration(
    owner: StorageOwner,
    owner_pou_id: Option<u32>,
    name_idx: u32,
    type_id: u32,
    slot: u32,
    ref_idx: Option<u32>,
    construction_nodes: u32,
) -> StorageDeclaration {
    StorageDeclaration {
        owner,
        role: StorageRole::Variable,
        retain: 0,
        flags: 0,
        owner_pou_id,
        name_idx,
        type_id: Some(type_id),
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

/// A source-free program in which every persistent owner is represented: the program root, the
/// resource's `trigger` BOOL, a global structure with a reference member, and two function-block
/// templates (one derived, so a template places its parent's cells first).
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
                "Refs",
                "r",
                "x",
                "p",
                "y",
                "g_refs",
                "@init/0/result",
                "@init/1/result",
                "@init/2/result",
                "@init/3/result",
                "@init/4/result",
            ]
            .into_iter()
            .map(smol_str::SmolStr::new)
            .collect(),
        });

    let mut types = TypeTable {
        offsets: Vec::new(),
        entries: vec![
            primitive(1, 0), // 0 BOOL
            primitive(7, 0), // 1 INT
            anonymous(
                TypeKind::Reference,
                TypeData::Reference {
                    target_type_id: INT,
                },
            ), // 2 REF_TO INT
            named(
                TypeKind::Struct,
                6,
                TypeData::Struct {
                    fields: vec![field(7, INT_REF)],
                },
            ), // 3 Refs { r : REF_TO INT }
            named(
                TypeKind::FunctionBlock,
                4,
                TypeData::Pou { pou_id: BASE_POU },
            ), // 4 Base
            named(
                TypeKind::FunctionBlock,
                5,
                TypeData::Pou {
                    pou_id: DERIVED_POU,
                },
            ), // 5 Derived
        ],
    };
    types.offsets = compute_type_offsets_for_entries(&types.entries).expect("type offsets");
    *module.section_mut(SectionId::TypeTable).expect("types") = SectionData::TypeTable(types);

    *module.section_mut(SectionId::RefTable).expect("refs") = SectionData::RefTable(RefTable {
        entries: vec![
            global_ref(0),
            global_ref(1),
            global_ref(2),
            result_ref(0),
            result_ref(1),
            result_ref(2),
            result_ref(3),
            result_ref(4),
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
        ],
    });

    module.sections.extend([
        Section {
            id: SectionId::StorageLayout.as_raw(),
            flags: 0,
            data: SectionData::StorageLayout(StorageLayout {
                entries: vec![
                    // The program root owns no typed global; it places no persistent cell.
                    StorageDeclaration {
                        owner: StorageOwner::Global,
                        role: StorageRole::ProgramRoot,
                        retain: 0,
                        flags: 0,
                        owner_pou_id: Some(MAIN_POU),
                        name_idx: 2,
                        type_id: None,
                        slot: 0,
                        ref_idx: Some(0),
                        default_const_idx: None,
                        construction_nodes: 1,
                        related_declaration_idx: None,
                        source_name_idx: None,
                    },
                    declaration(StorageOwner::Global, None, 3, BOOL, 1, Some(1), 1),
                    // `g_refs` is a structure: its demand is the structure node and its one member.
                    declaration(StorageOwner::Global, None, 11, REFS, 2, Some(2), 2),
                    declaration(StorageOwner::Instance, Some(BASE_POU), 8, INT, 0, None, 1),
                    declaration(
                        StorageOwner::Instance,
                        Some(BASE_POU),
                        9,
                        INT_REF,
                        1,
                        None,
                        1,
                    ),
                    declaration(
                        StorageOwner::Instance,
                        Some(DERIVED_POU),
                        10,
                        INT,
                        0,
                        None,
                        1,
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
                        declaration_idx: PROGRAM_ROOT_DECL,
                        binding_ref_idx: Some(0),
                        instance_owner_id: Some(0),
                        parent_root_idx: None,
                        template_pou_id: Some(MAIN_POU),
                        flags: 0,
                    },
                    ConstructionRoot {
                        declaration_idx: TRIGGER_DECL,
                        binding_ref_idx: Some(1),
                        instance_owner_id: None,
                        parent_root_idx: None,
                        template_pou_id: None,
                        flags: 0,
                    },
                    ConstructionRoot {
                        declaration_idx: REFS_DECL,
                        binding_ref_idx: Some(2),
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
                    initializer(TRIGGER_DECL, None, 3),
                    initializer(REFS_DECL, None, 4),
                    initializer(BASE_X_DECL, Some(BASE_POU), 5),
                    initializer(BASE_P_DECL, Some(BASE_POU), 6),
                    initializer(DERIVED_Y_DECL, Some(DERIVED_POU), 7),
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
                    var_meta(12, BOOL, 3),
                    var_meta(13, REFS, 4),
                    var_meta(14, INT, 5),
                    var_meta(15, INT_REF, 6),
                    var_meta(16, INT, 7),
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

/// A scalar cell's payload, in the widest form of its kind.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Payload {
    /// A `BOOL`.
    Bool(bool),
    /// A signed integer at its declared width.
    Signed(i64),
    /// An unsigned integer or bit string at its declared width.
    Unsigned(u64),
    /// A `REAL` or `LREAL`.
    Real(f64),
    /// A temporal type's `i64`: nanoseconds or ticks.
    Temporal(i64),
}

/// The payload of one scalar cell, or `None` for a compound, instance, text, reference or empty
/// cell.
///
/// The match names every `Cell` variant and has no wildcard, so a cell variant that is added or
/// removed stops this test file building: the variant set is exactly the one specification 34 §6.2
/// names after MEM-10.
fn payload(cell: Cell) -> Option<Payload> {
    Some(match cell {
        Cell::Bool(value) => Payload::Bool(value),
        Cell::SInt(value) => Payload::Signed(i64::from(value)),
        Cell::Int(value) => Payload::Signed(i64::from(value)),
        Cell::DInt(value) => Payload::Signed(i64::from(value)),
        Cell::LInt(value) => Payload::Signed(value),
        Cell::USInt(value) => Payload::Unsigned(u64::from(value)),
        Cell::UInt(value) => Payload::Unsigned(u64::from(value)),
        Cell::UDInt(value) => Payload::Unsigned(u64::from(value)),
        Cell::ULInt(value) => Payload::Unsigned(value),
        Cell::Real(value) => Payload::Real(f64::from(value)),
        Cell::LReal(value) => Payload::Real(value),
        Cell::Byte(value) => Payload::Unsigned(u64::from(value)),
        Cell::Word(value) => Payload::Unsigned(u64::from(value)),
        Cell::DWord(value) => Payload::Unsigned(u64::from(value)),
        Cell::LWord(value) => Payload::Unsigned(value),
        Cell::Char(value) => Payload::Unsigned(u64::from(value)),
        Cell::WChar(value) => Payload::Unsigned(u64::from(value)),
        Cell::Time(value)
        | Cell::LTime(value)
        | Cell::Date(value)
        | Cell::LDate(value)
        | Cell::Tod(value)
        | Cell::LTod(value)
        | Cell::Dt(value)
        | Cell::Ldt(value) => Payload::Temporal(value),
        Cell::Enum { .. }
        | Cell::Instance(_)
        | Cell::Text(_)
        | Cell::Ref(_)
        | Cell::Null
        | Cell::NullRef => return None,
    })
}

/// Requires its type to be `Copy`; a cell that is not `Copy` does not build.
fn assert_copy<T: Copy>() {}

#[test]
fn a_cell_is_sixteen_bytes_and_copy() {
    // Specification 34 §6.2, the paragraph after MEM-10, and design "Cells": one cell is sixteen
    // bytes, `Copy` and without `Drop`, so a region holds every scalar inline and retiring it frees
    // nothing. A cell that grew (a payload that needs sixteen bytes plus a separate tag makes it
    // 24), or that held a `String` or an `Arc`, fails here.
    assert_eq!(core::mem::size_of::<Cell>(), 16);
    assert_eq!(core::mem::size_of::<Cell>(), CELL_BYTES);
    assert!(!core::mem::needs_drop::<Cell>());
    assert_copy::<Cell>();
    // A reserved cell and a reserved reference cell are distinct values.
    assert_ne!(Cell::Null, Cell::NullRef);
}

#[test]
fn every_scalar_value_has_its_cell() {
    // One cell per scalar `Value` variant (`core/value/types.rs`); the expected values are those
    // types' limits. A cell that narrowed a payload (an `Int` holding an `i8`), shifted one, or
    // swapped a pair of limits fails here (specification 34 §6.2, after MEM-10).
    let cases = [
        (Cell::Bool(true), Payload::Bool(true)),
        (Cell::Bool(false), Payload::Bool(false)),
        (Cell::SInt(-128), Payload::Signed(-128)),
        (Cell::SInt(127), Payload::Signed(127)),
        (Cell::Int(-32_768), Payload::Signed(-32_768)),
        (Cell::DInt(-2_147_483_648), Payload::Signed(-2_147_483_648)),
        (
            Cell::LInt(-9_223_372_036_854_775_808),
            Payload::Signed(i64::MIN),
        ),
        (
            Cell::LInt(9_223_372_036_854_775_807),
            Payload::Signed(i64::MAX),
        ),
        (Cell::USInt(255), Payload::Unsigned(255)),
        (Cell::UInt(65_535), Payload::Unsigned(65_535)),
        (Cell::UDInt(4_294_967_295), Payload::Unsigned(4_294_967_295)),
        (
            Cell::ULInt(18_446_744_073_709_551_615),
            Payload::Unsigned(u64::MAX),
        ),
        (Cell::Real(-3.5), Payload::Real(-3.5)),
        (Cell::LReal(-3.5), Payload::Real(-3.5)),
        (Cell::Byte(255), Payload::Unsigned(255)),
        (Cell::Word(65_535), Payload::Unsigned(65_535)),
        (Cell::DWord(4_294_967_295), Payload::Unsigned(4_294_967_295)),
        (
            Cell::LWord(18_446_744_073_709_551_615),
            Payload::Unsigned(u64::MAX),
        ),
        (Cell::Char(65), Payload::Unsigned(65)),
        (Cell::WChar(0x4E2D), Payload::Unsigned(0x4E2D)),
    ];
    for (cell, expected) in cases {
        assert_eq!(payload(cell), Some(expected), "{cell:?}");
    }

    // Each of the eight temporal types is the value model's one `i64` (`core/value/datetime.rs`),
    // so its minimum, zero and maximum survive exactly. A temporal cell that stored a narrower
    // type, a millisecond count, or a saturating wrapper fails here; the tuple-variant
    // constructors below name all eight variants.
    for build in [
        Cell::Time,
        Cell::LTime,
        Cell::Date,
        Cell::LDate,
        Cell::Tod,
        Cell::LTod,
        Cell::Dt,
        Cell::Ldt,
    ] {
        assert_eq!(payload(build(i64::MIN)), Some(Payload::Temporal(i64::MIN)));
        assert_eq!(payload(build(0)), Some(Payload::Temporal(0)));
        assert_eq!(payload(build(i64::MAX)), Some(Payload::Temporal(i64::MAX)));
    }

    // A compound, instance, text, reference and empty cell have no scalar payload.
    for cell in [
        Cell::Enum {
            type_idx: INT,
            value: 3,
        },
        Cell::Instance(InstanceId(2)),
        Cell::Text(TextSlot(1)),
        Cell::Ref(RefSlot(1)),
        Cell::Null,
        Cell::NullRef,
    ] {
        assert_eq!(payload(cell), None, "{cell:?}");
    }
}

#[test]
fn a_reserved_store_never_grows() {
    // Specification 34 §6.2, the paragraph after MEM-10: a region is reserved once and never
    // grows, and every accessor outside it answers `None`. A store that resized a vector on a
    // write, that clamped a range or a slot outside the region instead of refusing it, or that
    // answered a missing record with `RefRecord::NULL` instead of `None`, fails here.
    let region = CellRange {
        region: LayoutRegion::Persistent,
        start: 0,
        cells: 3,
    };
    let mut store = CellStore::reserve(3, 2).expect("three cells and two reference records");

    // `reserve` fixes the region and fills it: every cell is empty and every record null.
    assert_eq!(store.reserved(), (3, 2));
    assert_eq!(
        store.cells(region),
        Some(&[Cell::Null, Cell::Null, Cell::Null][..])
    );
    assert_eq!(store.record(RefSlot(0)), Some(&RefRecord::NULL));
    assert_eq!(store.record(RefSlot(1)), Some(&RefRecord::NULL));

    // Write every cell of the region and the second record through the accessors.
    let cells = store.cells_mut(region).expect("the whole region");
    for (index, cell) in cells.iter_mut().enumerate() {
        *cell = Cell::DInt(i32::try_from(index).expect("three cells fit an i32"));
    }
    *store.record_mut(RefSlot(1)).expect("the second record") = RefRecord {
        region: LayoutRegion::Activation,
        cell: 5,
        char_index: 7,
        target_type: INT,
        generation: Generation(4),
    };
    assert_eq!(
        store.cells(region),
        Some(&[Cell::DInt(0), Cell::DInt(1), Cell::DInt(2)][..])
    );
    assert_eq!(
        store.record(RefSlot(1)),
        Some(&RefRecord {
            region: LayoutRegion::Activation,
            cell: 5,
            char_index: 7,
            target_type: INT,
            generation: Generation(4),
        })
    );

    // The writes did not grow either region.
    assert_eq!(store.reserved(), (3, 2));

    // The last cell of the region is cell 2 and its last record is slot 1; one past either end,
    // and a range that only reaches past the end, is refused.
    assert_eq!(
        store.cells(CellRange {
            start: 3,
            cells: 1,
            ..region
        }),
        None
    );
    assert_eq!(
        store.cells(CellRange {
            start: 2,
            cells: 2,
            ..region
        }),
        None
    );
    assert_eq!(
        store.cells_mut(CellRange {
            start: 0,
            cells: 4,
            ..region
        }),
        None
    );
    assert_eq!(store.record(RefSlot(2)), None);
    assert_eq!(store.record_mut(RefSlot(2)), None);
    assert_eq!(store.record(RefSlot(u32::MAX)), None);
    assert_eq!(store.record_mut(RefSlot(u32::MAX)), None);
}

#[test]
fn a_persistent_region_is_sized_from_the_prepared_layouts() {
    let prepared = prepared();

    // The globals keep declaration order: the program root owns no typed global, the `trigger`
    // BOOL is persistent cell 0 and the `g_refs` structure cell 1 (each one cell).
    assert_eq!(prepared.global_cells(PROGRAM_ROOT_DECL), None);
    assert_eq!(
        prepared.global_cells(TRIGGER_DECL),
        Some(CellRange {
            region: LayoutRegion::Persistent,
            start: 0,
            cells: 1,
        })
    );
    assert_eq!(
        prepared.global_cells(REFS_DECL),
        Some(CellRange {
            region: LayoutRegion::Persistent,
            start: 1,
            cells: 1,
        })
    );

    // Base's template is x : INT and p : REF_TO INT, two cells at cell 2. Derived extends Base, so
    // its template places Base's two cells first and adds y : INT: three cells at cell 4.
    assert_eq!(
        prepared
            .instance_template_cells(BASE_POU)
            .map(|template| template.range),
        Some(CellRange {
            region: LayoutRegion::Persistent,
            start: 2,
            cells: 2,
        })
    );
    assert_eq!(
        prepared
            .instance_template_cells(DERIVED_POU)
            .map(|template| template.range),
        Some(CellRange {
            region: LayoutRegion::Persistent,
            start: 4,
            cells: 3,
        })
    );

    // One plus one plus two plus three cells, and one reference leaf for `g_refs.r` plus Base's
    // `p` and the one Derived inherits: seven cells and three records.
    let store = CellStore::persistent(&prepared).expect("the persistent region reserves");
    assert_eq!(store.reserved(), (7, 3));

    // The whole region is addressable: its last cell and its last record exist, and one past
    // either end does not.
    assert_eq!(
        store
            .cells(CellRange {
                region: LayoutRegion::Persistent,
                start: 6,
                cells: 1,
            })
            .map(|cells| cells.len()),
        Some(1)
    );
    assert_eq!(
        store.cells(CellRange {
            region: LayoutRegion::Persistent,
            start: 7,
            cells: 1,
        }),
        None
    );
    assert_eq!(store.record(RefSlot(2)), Some(&RefRecord::NULL));
    assert_eq!(store.record(RefSlot(3)), None);
}

#[test]
fn reservation_bytes_counts_cells_and_records() {
    // Specification 34 §6.2, the paragraph after MEM-10: the bytes reserving a region takes are
    // its cells at sixteen bytes each plus its reference records. One record is its region, cell,
    // character index, target type and generation: twenty bytes on every target. A charge that
    // used the value model's `size_of::<Value>()`, that counted the cells but not the records, or
    // that forgot the empty region's zero fails here.
    assert_eq!(core::mem::size_of::<RefRecord>(), 20);
    assert_eq!(CellStore::reservation_bytes(0, 0), Some(0));
    assert_eq!(CellStore::reservation_bytes(3, 2), Some(3 * 16 + 2 * 20));
    assert_eq!(CellStore::reservation_bytes(100, 0), Some(1_600));
    assert_eq!(CellStore::reservation_bytes(0, 100), Some(2_000));
}
