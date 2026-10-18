//! Round-tripping a value of a prepared type through its layout's cells.
//!
//! Specification: `docs/specs/34-runtime-portability.md` §6.2 (MEM-10), the paragraphs "Prepared
//! cell layouts", "Cells and the cell store" and the value-conversion paragraph after them;
//! design `~/projects/.artifacts/bounded-c/design.md` "Cells" and "Boundaries" ("Engineering
//! writes convert a `Value` into the destination cells (no allocation); reads materialize a
//! `Value`"). Slice table row 13 (C1-6 cells, test `store_round_trip`).
//!
//! Every expected cell below is worked out by hand from the ST declarations the fixture encodes:
//!
//! ```st
//! TYPE Mode : (Idle, Running, Stopped) END_TYPE   (* Idle = 0, Running = 1, Stopped = 5 *)
//! TYPE Band : STRUCT low : REAL; high : REAL; END_STRUCT END_TYPE
//! TYPE Pair : STRUCT x : INT; y : INT; END_STRUCT END_TYPE
//! TYPE Axis : STRUCT
//!     position : REAL;
//!     limits   : Band;
//!     mode     : Mode;
//!     series   : ARRAY[1..3] OF INT;
//! END_STRUCT END_TYPE
//! TYPE Cells : ARRAY[-1..1, 2..3] OF Pair END_TYPE
//! TYPE Packed : STRUCT OVERLAP
//!     as_word AT %B0 : DWORD;
//!     as_int  AT %B0 : DINT;
//! END_STRUCT; END_TYPE
//! ```
//!
//! `Axis` is `position` at cell 0, `Band`'s `low` and `high` at cells 1 and 2, `mode` at cell 3
//! and `series` at cells 4 to 6: seven cells. `Cells` is six `Pair` elements of two cells each, so
//! its stride is two and its cells twelve. Every scalar is one cell, and `Packed`'s two members
//! share cell 0 with a four-byte backing.

#[path = "common/bytecode_helpers.rs"]
mod bytecode_helpers;

use bytecode_helpers::base_module;
use smol_str::SmolStr;
use std::sync::Arc;
use trust_runtime_core::bytecode::{
    compute_type_offsets_for_entries, AccessBindings, BytecodeModule, BytecodeVersion,
    ConstructionRoot, ConstructionRoots, EnumVariant, Field, InitializationOnce,
    InitializationPhase, InitializationStage, InitializationTarget, InitializationTrigger,
    InitializerBodyKind, InitializerEntry, InitializerIndex, PouClassMeta, PouEntry, PouIndex,
    PouKind, RefEntry, RefLocation, RefTable, Section, SectionData, SectionId, StorageDeclaration,
    StorageLayout, StorageOwner, StorageRole, StringTable, TypeData, TypeEntry, TypeKind,
    TypeTable, VarMeta, VarMetaEntry, HEADER_FLAG_CRC32,
};
use trust_runtime_core::collections::OrderedMap;
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::memory::cells::{cell_offset, read_value, write_value, Cell, CellStore};
use trust_runtime_core::memory::layout::{CellRange, LayoutRegion};
use trust_runtime_core::memory::{Generation, InstanceId, MemoryLocation};
use trust_runtime_core::value::{
    ArrayValue, DateTimeValue, DateValue, Duration, EnumValue, LDateTimeValue, LDateValue,
    LTimeOfDayValue, RefSegment, StructValue, TimeOfDayValue, Value, ValueRef,
};
use trust_runtime_core::vm::{PreparationLimits, PreparedModule};

/// The fixture's scalar type ordinals: one per primitive encoding, in `TYPE_TABLE` order.
const BOOL: u32 = 0;
const SINT: u32 = 1;
const INT: u32 = 2;
const DINT: u32 = 3;
const LINT: u32 = 4;
const USINT: u32 = 5;
const UINT: u32 = 6;
const UDINT: u32 = 7;
const ULINT: u32 = 8;
const REAL: u32 = 9;
const LREAL: u32 = 10;
const BYTE: u32 = 11;
const WORD: u32 = 12;
const DWORD: u32 = 13;
const LWORD: u32 = 14;
const CHAR: u32 = 15;
const WCHAR: u32 = 16;
const TIME: u32 = 17;
const LTIME: u32 = 18;
const DATE: u32 = 19;
const LDATE: u32 = 20;
const TOD: u32 = 21;
const LTOD: u32 = 22;
const DT: u32 = 23;
const LDT: u32 = 24;
const STRING: u32 = 25;
const WSTRING: u32 = 26;

/// The fixture's compound type ordinals.
const MODE: u32 = 28;
const BAND: u32 = 29;
const INT_SERIES: u32 = 30;
const PAIR: u32 = 31;
const AXIS: u32 = 32;
const CELLS: u32 = 33;
const REF_TO_INT: u32 = 34;
const REFS: u32 = 35;
const PACKED: u32 = 36;
const BASE_FB: u32 = 37;
const AXIS_ARRAY: u32 = 38;

/// The fixture's POU identifiers.
const MAIN_POU: u32 = 1;
const BASE_FB_POU: u32 = 2;

/// The string-table ordinals the fixture's declarations use.
const TRIGGER_NAME: u32 = 3;
const MODE_NAME: u32 = 4;
const IDLE_NAME: u32 = 5;
const RUNNING_NAME: u32 = 6;
const STOPPED_NAME: u32 = 7;
const BAND_NAME: u32 = 8;
const LOW_NAME: u32 = 9;
const HIGH_NAME: u32 = 10;
const AXIS_NAME: u32 = 11;
const POSITION_NAME: u32 = 12;
const LIMITS_NAME: u32 = 13;
const MODE_MEMBER_NAME: u32 = 14;
const SERIES_NAME: u32 = 15;
const PAIR_NAME: u32 = 16;
const X_NAME: u32 = 17;
const Y_NAME: u32 = 18;
const CELLS_NAME: u32 = 19;
const REF_TO_INT_NAME: u32 = 20;
const R_NAME: u32 = 21;
const REFS_NAME: u32 = 22;
const PACKED_NAME: u32 = 23;
const AS_WORD_NAME: u32 = 24;
const AS_INT_NAME: u32 = 25;
const BASE_NAME: u32 = 26;
const RESULT_NAME: u32 = 27;

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

/// A field of a relative layout, at bit 0.
fn located_field(name_idx: u32, type_id: u32) -> Field {
    Field {
        name_idx,
        type_id,
        bit_offset: Some(0),
    }
}

/// One `TYPE_TABLE`: every scalar primitive, an enumeration, a structure with a nested structure,
/// an array member and an enumeration member, a two-dimensional array with lower bounds other
/// than zero, a reference, an instance, a `STRUCT OVERLAP` and an array of structures.
fn type_table() -> TypeTable {
    let mut types = TypeTable {
        offsets: Vec::new(),
        entries: vec![
            primitive(1, 0),      // 0 BOOL
            primitive(6, 0),      // 1 SINT
            primitive(7, 0),      // 2 INT
            primitive(8, 0),      // 3 DINT
            primitive(9, 0),      // 4 LINT
            primitive(10, 0),     // 5 USINT
            primitive(11, 0),     // 6 UINT
            primitive(12, 0),     // 7 UDINT
            primitive(13, 0),     // 8 ULINT
            primitive(14, 0),     // 9 REAL
            primitive(15, 0),     // 10 LREAL
            primitive(2, 0),      // 11 BYTE
            primitive(3, 0),      // 12 WORD
            primitive(4, 0),      // 13 DWORD
            primitive(5, 0),      // 14 LWORD
            primitive(26, 0),     // 15 CHAR
            primitive(27, 0),     // 16 WCHAR
            primitive(16, 0),     // 17 TIME
            primitive(17, 0),     // 18 LTIME
            primitive(18, 0),     // 19 DATE
            primitive(19, 0),     // 20 LDATE
            primitive(20, 0),     // 21 TOD
            primitive(21, 0),     // 22 LTOD
            primitive(22, 0),     // 23 DT
            primitive(23, 0),     // 24 LDT
            primitive(24, 0),     // 25 STRING
            primitive(25, 0),     // 26 WSTRING
            primitive(0x0100, 0), // 27 the generic native state slot
            named(
                TypeKind::Enum,
                MODE_NAME,
                TypeData::Enum {
                    base_type_id: DINT,
                    variants: vec![
                        EnumVariant {
                            name_idx: IDLE_NAME,
                            value: 0,
                        },
                        EnumVariant {
                            name_idx: RUNNING_NAME,
                            value: 1,
                        },
                        EnumVariant {
                            name_idx: STOPPED_NAME,
                            value: 5,
                        },
                    ],
                },
            ), // 28 Mode
            named(
                TypeKind::Struct,
                BAND_NAME,
                TypeData::Struct {
                    fields: vec![field(LOW_NAME, REAL), field(HIGH_NAME, REAL)],
                },
            ), // 29 Band
            anonymous(
                TypeKind::Array,
                TypeData::Array {
                    elem_type_id: INT,
                    dims: vec![(1, 3)],
                },
            ), // 30 ARRAY[1..3] OF INT
            named(
                TypeKind::Struct,
                PAIR_NAME,
                TypeData::Struct {
                    fields: vec![field(X_NAME, INT), field(Y_NAME, INT)],
                },
            ), // 31 Pair
            named(
                TypeKind::Struct,
                AXIS_NAME,
                TypeData::Struct {
                    fields: vec![
                        field(POSITION_NAME, REAL),
                        field(LIMITS_NAME, BAND),
                        field(MODE_MEMBER_NAME, MODE),
                        field(SERIES_NAME, INT_SERIES),
                    ],
                },
            ), // 32 Axis
            named(
                TypeKind::Array,
                CELLS_NAME,
                TypeData::Array {
                    elem_type_id: PAIR,
                    dims: vec![(-1, 1), (2, 3)],
                },
            ), // 33 ARRAY[-1..1, 2..3] OF Pair
            named(
                TypeKind::Reference,
                REF_TO_INT_NAME,
                TypeData::Reference {
                    target_type_id: INT,
                },
            ), // 34 REF_TO INT
            named(
                TypeKind::Struct,
                REFS_NAME,
                TypeData::Struct {
                    fields: vec![field(R_NAME, REF_TO_INT)],
                },
            ), // 35 Refs { r : REF_TO INT }
            named(
                TypeKind::OverlappingStruct,
                PACKED_NAME,
                TypeData::Struct {
                    fields: vec![
                        located_field(AS_WORD_NAME, DWORD),
                        located_field(AS_INT_NAME, DINT),
                    ],
                },
            ), // 36 Packed: a DWORD and a DINT both at bit 0
            named(
                TypeKind::FunctionBlock,
                BASE_NAME,
                TypeData::Pou {
                    pou_id: BASE_FB_POU,
                },
            ), // 37 Base
            anonymous(
                TypeKind::Array,
                TypeData::Array {
                    elem_type_id: AXIS,
                    dims: vec![(0, 1)],
                },
            ), // 38 ARRAY[0..1] OF Axis
        ],
    };
    types.offsets = compute_type_offsets_for_entries(&types.entries).expect("type offsets");
    types
}

/// One initializer of a storage declaration that owns storage.
fn initializer(declaration_idx: u32, result_ref_idx: u32) -> InitializerEntry {
    InitializerEntry {
        declaration_idx: Some(declaration_idx),
        owner_pou_id: None,
        result_ref_idx,
        code_offset: 0,
        code_length: 0,
        visible_local_count: 0,
        visible_static_count: 0,
        phase: InitializationPhase::Resource,
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

/// A source-free program: the resource's program root and `trigger` BOOL globals plus the type
/// table above, whose types no storage declaration needs.
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
                "Mode",
                "Idle",
                "Running",
                "Stopped",
                "Band",
                "low",
                "high",
                "Axis",
                "position",
                "limits",
                "mode",
                "series",
                "Pair",
                "x",
                "y",
                "Cells",
                "RefToInt",
                "r",
                "Refs",
                "Packed",
                "as_word",
                "as_int",
                "Base",
                "@init/0/result",
            ]
            .into_iter()
            .map(SmolStr::new)
            .collect(),
        });
    *module.section_mut(SectionId::TypeTable).expect("types") =
        SectionData::TypeTable(type_table());

    *module.section_mut(SectionId::RefTable).expect("refs") = SectionData::RefTable(RefTable {
        entries: vec![
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
                id: BASE_FB_POU,
                name_idx: BASE_NAME,
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
        ],
    });

    module.sections.extend([
        Section {
            id: SectionId::StorageLayout.as_raw(),
            flags: 0,
            data: SectionData::StorageLayout(StorageLayout {
                entries: vec![
                    // The program root owns no typed global; the `trigger` BOOL is persistent cell 0.
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
                    StorageDeclaration {
                        owner: StorageOwner::Global,
                        role: StorageRole::Variable,
                        retain: 0,
                        flags: 0,
                        owner_pou_id: None,
                        name_idx: TRIGGER_NAME,
                        type_id: Some(BOOL),
                        slot: 1,
                        ref_idx: Some(1),
                        default_const_idx: None,
                        construction_nodes: 1,
                        related_declaration_idx: None,
                        source_name_idx: None,
                    },
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
                ],
            }),
        },
        Section {
            id: SectionId::Initializers.as_raw(),
            flags: 0,
            data: SectionData::Initializers(InitializerIndex {
                entries: vec![initializer(1, 2)],
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
                entries: vec![VarMetaEntry {
                    name_idx: RESULT_NAME,
                    type_id: BOOL,
                    ref_idx: 2,
                    retain: 0,
                    init_const_idx: None,
                }],
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

/// A structure value from its members, which the model keeps in insertion order.
fn structure(type_name: &str, members: Vec<(&str, Value)>) -> Value {
    let fields: OrderedMap<SmolStr, Value> = members
        .into_iter()
        .map(|(name, value)| (SmolStr::new(name), value))
        .collect();
    Value::Struct(Arc::new(StructValue::from_canonical_parts(
        type_name.into(),
        fields,
    )))
}

fn band(low: f32, high: f32) -> Value {
    structure(
        "Band",
        vec![("low", Value::Real(low)), ("high", Value::Real(high))],
    )
}

fn pair(x: i16, y: i16) -> Value {
    structure("Pair", vec![("x", Value::Int(x)), ("y", Value::Int(y))])
}

/// The fixture's `Axis`: `position`, `limits`, `mode` and `series`, in declaration order.
fn axis(position: f32, limits: Value, mode: (&str, i64), series: [i16; 3]) -> Value {
    structure(
        "Axis",
        vec![
            ("position", Value::Real(position)),
            ("limits", limits),
            ("mode", enumeration("Mode", mode.0, mode.1)),
            (
                "series",
                array(series.into_iter().map(Value::Int).collect(), vec![(1, 3)]),
            ),
        ],
    )
}

fn enumeration(type_name: &str, variant: &str, numeric: i64) -> Value {
    Value::Enum(Box::new(EnumValue::from_canonical_parts(
        type_name.into(),
        variant.into(),
        numeric,
    )))
}

fn array(elements: Vec<Value>, dims: Vec<(i64, i64)>) -> Value {
    Value::Array(Box::new(ArrayValue::from_canonical_parts(elements, dims)))
}

fn field_path(name: &str) -> RefSegment {
    RefSegment::Field(SmolStr::new(name))
}

fn index_path(indices: &[i64]) -> RefSegment {
    RefSegment::Index(indices.to_vec())
}

/// Compare one cell with an expected cell, a floating one by its bits (NaN never equals itself).
fn assert_cell(actual: Cell, expected: Cell, context: &str) {
    match (actual, expected) {
        (Cell::Real(actual), Cell::Real(expected)) => {
            assert_eq!(actual.to_bits(), expected.to_bits(), "{context}");
        }
        (Cell::LReal(actual), Cell::LReal(expected)) => {
            assert_eq!(actual.to_bits(), expected.to_bits(), "{context}");
        }
        _ => assert_eq!(actual, expected, "{context}"),
    }
}

/// Compare two values exactly: floats by their bits, compounds member by member, and enumerations
/// by their type, variant and value (`PartialEq` ignores an enumeration's variant name).
fn assert_same(actual: &Value, expected: &Value, context: &str) {
    match (actual, expected) {
        (Value::Real(actual), Value::Real(expected)) => {
            assert_eq!(actual.to_bits(), expected.to_bits(), "{context}");
        }
        (Value::LReal(actual), Value::LReal(expected)) => {
            assert_eq!(actual.to_bits(), expected.to_bits(), "{context}");
        }
        (Value::Array(actual), Value::Array(expected)) => {
            assert_eq!(actual.dimensions(), expected.dimensions(), "{context}");
            assert_eq!(
                actual.elements().len(),
                expected.elements().len(),
                "{context}"
            );
            for (actual, expected) in actual.elements().iter().zip(expected.elements()) {
                assert_same(actual, expected, context);
            }
        }
        (Value::Struct(actual), Value::Struct(expected)) => {
            assert_eq!(actual.type_name(), expected.type_name(), "{context}");
            assert_eq!(actual.fields().len(), expected.fields().len(), "{context}");
            for (name, actual) in actual.fields() {
                let expected = expected
                    .field(name.as_str())
                    .expect("the structure keeps every member");
                assert_same(actual, expected, context);
            }
        }
        (Value::Enum(actual), Value::Enum(expected)) => {
            assert_eq!(actual.type_name(), expected.type_name(), "{context}");
            assert_eq!(actual.variant_name(), expected.variant_name(), "{context}");
            assert_eq!(
                actual.numeric_value(),
                expected.numeric_value(),
                "{context}"
            );
        }
        _ => assert_eq!(actual, expected, "{context}"),
    }
}

/// Write one scalar value, compare its one cell with the hand-written one, and read it back.
fn scalar_case(prepared: &PreparedModule, type_id: u32, value: &Value, cell: Cell) {
    let mut cells = [Cell::Null];
    write_value(&mut cells, type_id, value, prepared).expect("a scalar of its type writes");
    assert_cell(cells[0], cell, &format!("type {type_id}"));
    let read = read_value(&cells, type_id, prepared).expect("a scalar of its type reads");
    assert_same(&read, value, &format!("type {type_id}"));
}

/// The minimum, zero and maximum of every integer and bit-string scalar, with its cell.
fn integer_extremes() -> Vec<(u32, Value, Cell)> {
    vec![
        (BOOL, Value::Bool(false), Cell::Bool(false)),
        (BOOL, Value::Bool(false), Cell::Bool(false)),
        (BOOL, Value::Bool(true), Cell::Bool(true)),
        (SINT, Value::SInt(i8::MIN), Cell::SInt(i8::MIN)),
        (SINT, Value::SInt(0), Cell::SInt(0)),
        (SINT, Value::SInt(i8::MAX), Cell::SInt(i8::MAX)),
        (INT, Value::Int(i16::MIN), Cell::Int(i16::MIN)),
        (INT, Value::Int(0), Cell::Int(0)),
        (INT, Value::Int(i16::MAX), Cell::Int(i16::MAX)),
        (DINT, Value::DInt(i32::MIN), Cell::DInt(i32::MIN)),
        (DINT, Value::DInt(0), Cell::DInt(0)),
        (DINT, Value::DInt(i32::MAX), Cell::DInt(i32::MAX)),
        (LINT, Value::LInt(i64::MIN), Cell::LInt(i64::MIN)),
        (LINT, Value::LInt(0), Cell::LInt(0)),
        (LINT, Value::LInt(i64::MAX), Cell::LInt(i64::MAX)),
        (USINT, Value::USInt(0), Cell::USInt(0)),
        (USINT, Value::USInt(0), Cell::USInt(0)),
        (USINT, Value::USInt(u8::MAX), Cell::USInt(u8::MAX)),
        (UINT, Value::UInt(0), Cell::UInt(0)),
        (UINT, Value::UInt(0), Cell::UInt(0)),
        (UINT, Value::UInt(u16::MAX), Cell::UInt(u16::MAX)),
        (UDINT, Value::UDInt(0), Cell::UDInt(0)),
        (UDINT, Value::UDInt(0), Cell::UDInt(0)),
        (UDINT, Value::UDInt(u32::MAX), Cell::UDInt(u32::MAX)),
        (ULINT, Value::ULInt(0), Cell::ULInt(0)),
        (ULINT, Value::ULInt(0), Cell::ULInt(0)),
        (ULINT, Value::ULInt(u64::MAX), Cell::ULInt(u64::MAX)),
        (BYTE, Value::Byte(0), Cell::Byte(0)),
        (BYTE, Value::Byte(0), Cell::Byte(0)),
        (BYTE, Value::Byte(u8::MAX), Cell::Byte(u8::MAX)),
        (WORD, Value::Word(0), Cell::Word(0)),
        (WORD, Value::Word(0), Cell::Word(0)),
        (WORD, Value::Word(u16::MAX), Cell::Word(u16::MAX)),
        (DWORD, Value::DWord(0), Cell::DWord(0)),
        (DWORD, Value::DWord(0), Cell::DWord(0)),
        (DWORD, Value::DWord(u32::MAX), Cell::DWord(u32::MAX)),
        (LWORD, Value::LWord(0), Cell::LWord(0)),
        (LWORD, Value::LWord(0), Cell::LWord(0)),
        (LWORD, Value::LWord(u64::MAX), Cell::LWord(u64::MAX)),
        (CHAR, Value::Char(0), Cell::Char(0)),
        (CHAR, Value::Char(0), Cell::Char(0)),
        (CHAR, Value::Char(u8::MAX), Cell::Char(u8::MAX)),
        (WCHAR, Value::WChar(0), Cell::WChar(0)),
        (WCHAR, Value::WChar(0), Cell::WChar(0)),
        (WCHAR, Value::WChar(u16::MAX), Cell::WChar(u16::MAX)),
    ]
}

/// The minimum, zero and maximum of every floating and temporal scalar, with its cell.
fn float_and_temporal_extremes() -> Vec<(u32, Value, Cell)> {
    vec![
        (REAL, Value::Real(f32::MIN), Cell::Real(f32::MIN)),
        (REAL, Value::Real(0.0), Cell::Real(0.0)),
        (REAL, Value::Real(f32::MAX), Cell::Real(f32::MAX)),
        (LREAL, Value::LReal(f64::MIN), Cell::LReal(f64::MIN)),
        (LREAL, Value::LReal(0.0), Cell::LReal(0.0)),
        (LREAL, Value::LReal(f64::MAX), Cell::LReal(f64::MAX)),
        (
            TIME,
            Value::Time(Duration::from_nanos(i64::MIN)),
            Cell::Time(i64::MIN),
        ),
        (TIME, Value::Time(Duration::ZERO), Cell::Time(0)),
        (
            TIME,
            Value::Time(Duration::from_nanos(i64::MAX)),
            Cell::Time(i64::MAX),
        ),
        (
            LTIME,
            Value::LTime(Duration::from_nanos(i64::MIN)),
            Cell::LTime(i64::MIN),
        ),
        (LTIME, Value::LTime(Duration::ZERO), Cell::LTime(0)),
        (
            LTIME,
            Value::LTime(Duration::from_nanos(i64::MAX)),
            Cell::LTime(i64::MAX),
        ),
        (
            DATE,
            Value::Date(DateValue::new(i64::MIN)),
            Cell::Date(i64::MIN),
        ),
        (DATE, Value::Date(DateValue::new(0)), Cell::Date(0)),
        (
            DATE,
            Value::Date(DateValue::new(i64::MAX)),
            Cell::Date(i64::MAX),
        ),
        (
            LDATE,
            Value::LDate(LDateValue::new(i64::MIN)),
            Cell::LDate(i64::MIN),
        ),
        (LDATE, Value::LDate(LDateValue::new(0)), Cell::LDate(0)),
        (
            LDATE,
            Value::LDate(LDateValue::new(i64::MAX)),
            Cell::LDate(i64::MAX),
        ),
        (
            TOD,
            Value::Tod(TimeOfDayValue::new(i64::MIN)),
            Cell::Tod(i64::MIN),
        ),
        (TOD, Value::Tod(TimeOfDayValue::new(0)), Cell::Tod(0)),
        (
            TOD,
            Value::Tod(TimeOfDayValue::new(i64::MAX)),
            Cell::Tod(i64::MAX),
        ),
        (
            LTOD,
            Value::LTod(LTimeOfDayValue::new(i64::MIN)),
            Cell::LTod(i64::MIN),
        ),
        (LTOD, Value::LTod(LTimeOfDayValue::new(0)), Cell::LTod(0)),
        (
            LTOD,
            Value::LTod(LTimeOfDayValue::new(i64::MAX)),
            Cell::LTod(i64::MAX),
        ),
        (
            DT,
            Value::Dt(DateTimeValue::new(i64::MIN)),
            Cell::Dt(i64::MIN),
        ),
        (DT, Value::Dt(DateTimeValue::new(0)), Cell::Dt(0)),
        (
            DT,
            Value::Dt(DateTimeValue::new(i64::MAX)),
            Cell::Dt(i64::MAX),
        ),
        (
            LDT,
            Value::Ldt(LDateTimeValue::new(i64::MIN)),
            Cell::Ldt(i64::MIN),
        ),
        (LDT, Value::Ldt(LDateTimeValue::new(0)), Cell::Ldt(0)),
        (
            LDT,
            Value::Ldt(LDateTimeValue::new(i64::MAX)),
            Cell::Ldt(i64::MAX),
        ),
    ]
}

#[test]
fn every_scalar_round_trips_exactly() {
    // Specification 34 §6.2 after MEM-10: one cell holds a scalar of the value model inline. A
    // conversion that narrowed a payload (an `Int` in an `i8` cell), that turned a temporal count
    // into ticks, or that swapped a pair of limits fails here; the cells are the hand-written
    // minimum, zero and maximum of every scalar type.
    let prepared = prepared();
    for (type_id, value, cell) in integer_extremes()
        .into_iter()
        .chain(float_and_temporal_extremes())
    {
        scalar_case(&prepared, type_id, &value, cell);
    }

    // A `REAL` and an `LREAL` keep their exact IEEE 754 bits, so a NaN with a payload, a negative
    // zero and both infinities survive, compared by bits because NaN never equals itself. A
    // conversion through a decimal rendering or a canonicalizing float fails here.
    let specials = [
        (
            REAL,
            Value::Real(f32::from_bits(0x7FC0_0001)),
            Cell::Real(f32::from_bits(0x7FC0_0001)),
        ),
        (REAL, Value::Real(-0.0), Cell::Real(-0.0)),
        (REAL, Value::Real(f32::INFINITY), Cell::Real(f32::INFINITY)),
        (
            REAL,
            Value::Real(f32::NEG_INFINITY),
            Cell::Real(f32::NEG_INFINITY),
        ),
        (
            LREAL,
            Value::LReal(f64::from_bits(0x7FF8_0000_0000_0001)),
            Cell::LReal(f64::from_bits(0x7FF8_0000_0000_0001)),
        ),
        (LREAL, Value::LReal(-0.0), Cell::LReal(-0.0)),
        (
            LREAL,
            Value::LReal(f64::INFINITY),
            Cell::LReal(f64::INFINITY),
        ),
        (
            LREAL,
            Value::LReal(f64::NEG_INFINITY),
            Cell::LReal(f64::NEG_INFINITY),
        ),
    ];
    for (type_id, value, cell) in specials {
        scalar_case(&prepared, type_id, &value, cell);
    }
}

#[test]
fn structures_and_arrays_round_trip() {
    // Specification 34 §6.2 after MEM-10: a structure or array is a contiguous cell range of its
    // prepared layout with its nested compounds inline, an enumeration keeps its TYPE_TABLE type
    // and its value, an instance its identity, a null reference an empty reference cell and
    // `Value::Null` an empty cell. A conversion that wrote a member ordinal instead of its cell
    // offset, that used a one-cell stride for a two-cell element, or that named a variant wrongly
    // fails the hand-written cells below.
    let prepared = prepared();
    assert_axis_round_trips(&prepared);
    assert_pair_array_round_trips(&prepared);
    assert_axis_array_round_trips(&prepared);
    assert_null_and_instances_round_trip(&prepared);
    assert_empty_values_round_trip(&prepared);
}

/// `Axis`: position at cell 0, Band's low and high at cells 1 and 2, mode at cell 3 and series at
/// cells 4 to 6.
fn assert_axis_round_trips(prepared: &PreparedModule) {
    let value = axis(1.5, band(-2.25, 8.0), ("Stopped", 5), [10, 20, 30]);
    let mut cells = [Cell::Null; 7];
    write_value(&mut cells, AXIS, &value, prepared).expect("an Axis writes");
    assert_eq!(
        cells,
        [
            Cell::Real(1.5),
            Cell::Real(-2.25),
            Cell::Real(8.0),
            Cell::Enum {
                type_idx: MODE,
                value: 5,
            },
            Cell::Int(10),
            Cell::Int(20),
            Cell::Int(30),
        ]
    );
    let read = read_value(&cells, AXIS, prepared).expect("an Axis reads");
    assert_same(&read, &value, "Axis");
}

/// `Cells` is `ARRAY[-1..1, 2..3] OF Pair`: six two-cell elements, so element `k` starts at cell
/// `2k` whatever its lower bounds are.
fn assert_pair_array_round_trips(prepared: &PreparedModule) {
    let value = array(
        vec![
            pair(1, 2),
            pair(3, 4),
            pair(5, 6),
            pair(7, 8),
            pair(9, 10),
            pair(11, 12),
        ],
        vec![(-1, 1), (2, 3)],
    );
    let mut cells = [Cell::Null; 12];
    write_value(&mut cells, CELLS, &value, prepared).expect("a two-dimensional array writes");
    assert_eq!(
        cells,
        [
            Cell::Int(1),
            Cell::Int(2),
            Cell::Int(3),
            Cell::Int(4),
            Cell::Int(5),
            Cell::Int(6),
            Cell::Int(7),
            Cell::Int(8),
            Cell::Int(9),
            Cell::Int(10),
            Cell::Int(11),
            Cell::Int(12),
        ]
    );
    let read = read_value(&cells, CELLS, prepared).expect("a two-dimensional array reads");
    assert_same(&read, &value, "Cells");
}

/// `ARRAY[0..1] OF Axis` inlines each `Axis` through a stride of seven cells.
fn assert_axis_array_round_trips(prepared: &PreparedModule) {
    let value = array(
        vec![
            axis(0.5, band(1.0, 2.0), ("Idle", 0), [1, 2, 3]),
            axis(-0.5, band(-1.0, -2.0), ("Running", 1), [-1, -2, -3]),
        ],
        vec![(0, 1)],
    );
    let mut cells = [Cell::Null; 14];
    write_value(&mut cells, AXIS_ARRAY, &value, prepared).expect("an Axis array writes");
    // The second element starts at cell 7: its position at cell 7, its mode at cell 10 and the
    // last series element at cell 13.
    assert_eq!(cells[7], Cell::Real(-0.5));
    assert_eq!(
        cells[10],
        Cell::Enum {
            type_idx: MODE,
            value: 1,
        }
    );
    assert_eq!(cells[13], Cell::Int(-3));
    let read = read_value(&cells, AXIS_ARRAY, prepared).expect("an Axis array reads");
    assert_same(&read, &value, "an Axis array");
}

/// A null reference is an empty reference cell, and an instance its identity.
fn assert_null_and_instances_round_trip(prepared: &PreparedModule) {
    let refs = structure("Refs", vec![("r", Value::Reference(None))]);
    let mut cells = [Cell::Null];
    write_value(&mut cells, REFS, &refs, prepared).expect("a null reference writes");
    assert_eq!(cells, [Cell::NullRef]);
    let read = read_value(&cells, REFS, prepared).expect("a null reference reads");
    assert_same(&read, &refs, "Refs");

    let instance = Value::Instance(InstanceId(7));
    let mut cells = [Cell::Null];
    write_value(&mut cells, BASE_FB, &instance, prepared).expect("an instance writes");
    assert_eq!(cells, [Cell::Instance(InstanceId(7))]);
    let read = read_value(&cells, BASE_FB, prepared).expect("an instance reads");
    assert_same(&read, &instance, "an instance");
}

/// `Value::Null` is the empty value: it fills every cell of the range, and an empty range reads
/// back as `Value::Null`.
fn assert_empty_values_round_trip(prepared: &PreparedModule) {
    let mut cells = [Cell::DInt(9); 7];
    write_value(&mut cells, AXIS, &Value::Null, prepared).expect("an empty value writes");
    assert_eq!(cells, [Cell::Null; 7]);
    assert_eq!(
        read_value(&cells, AXIS, prepared).expect("an empty range reads"),
        Value::Null
    );

    let mut cells = [Cell::DInt(9)];
    write_value(&mut cells, DINT, &Value::Null, prepared).expect("an empty scalar writes");
    assert_eq!(cells, [Cell::Null]);
    assert_eq!(
        read_value(&cells, DINT, prepared).expect("an empty cell reads"),
        Value::Null
    );
}

/// The seed every fixture's first value uses, with the type's ordinal mixed in.
const SEED: u64 = 0x5EED_0FC1_0600_0000;

/// The bounded-c corpus fixtures whose every convertible type this slice round-trips.
const CORPUS: [(&str, &[u8]); 4] = [
    (
        "structures",
        include_bytes!("../../trust-runtime/tests/fixtures/bounded-c/structures/program.stbc"),
    ),
    (
        "arrays",
        include_bytes!("../../trust-runtime/tests/fixtures/bounded-c/arrays/program.stbc"),
    ),
    (
        "instances",
        include_bytes!("../../trust-runtime/tests/fixtures/bounded-c/instances/program.stbc"),
    ),
    (
        "scalar",
        include_bytes!("../../trust-runtime/tests/fixtures/bounded-c/scalar/program.stbc"),
    ),
];

#[test]
fn generated_values_round_trip() {
    // Specification 34 §6.2 after MEM-10 and design "Boundaries": an engineering write converts a
    // `Value` into the destination cells and a read materializes it again, so 200 seeded values of
    // every type of the bounded-c corpus that holds no text and no non-null reference come back
    // exactly. A conversion that dropped a nested compound, that read a member at a wrong offset,
    // that misnamed an enumeration's variant or that rebuilt another array shape fails here.
    let mut converted = 0usize;
    for (name, bytes) in CORPUS {
        let prepared = PreparedModule::from_bytes(bytes, PreparationLimits::default())
            .unwrap_or_else(|error| panic!("the {name} fixture prepares: {error}"));
        let mut fixture_types = 0usize;
        for type_id in 0..u32::try_from(prepared.type_layouts().len()).expect("a type count") {
            if !convertible(&prepared, type_id) {
                continue;
            }
            fixture_types += 1;
            let layout = prepared.type_layout(type_id).expect("a laid-out type");
            let mut rng = Rng::new(SEED ^ u64::from(type_id));
            for _ in 0..200 {
                let value = generate(&prepared, type_id, &mut rng);
                let context = format!("{name}, type {type_id}");
                let mut cells = vec![Cell::Null; layout.cells as usize];
                write_value(&mut cells, type_id, &value, &prepared)
                    .unwrap_or_else(|error| panic!("{context}: {value:?} writes: {error}"));
                let read = read_value(&cells, type_id, &prepared)
                    .unwrap_or_else(|error| panic!("{context}: {value:?} reads: {error}"));
                assert_same(&read, &value, &context);
            }
        }
        converted += fixture_types;
        // Every fixture declares convertible types (`structures`: Mode, Band and Axis; `arrays`:
        // Point and its arrays; `instances`: DINT and its classes; `scalar`: its eight scalar
        // types), and the four together far more than ten, so a filter that skipped everything
        // cannot pass.
        assert!(fixture_types >= 1, "the {name} fixture converted no type");
    }
    assert!(
        converted >= 10,
        "the corpus converted only {converted} types"
    );
}

/// A `SplitMix64` generator, written here so the test needs no new dependency.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed)
    }

    /// The next 64 generated bits.
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mixed = (self.0 ^ (self.0 >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        let mixed = (mixed ^ (mixed >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        mixed ^ (mixed >> 31)
    }

    /// The next 64 bits as eight little-endian bytes, so a narrower kind needs no truncating cast.
    fn bytes(&mut self) -> [u8; 8] {
        self.next().to_le_bytes()
    }
}

/// Whether this slice converts every value of the type: no text leaf, and no `STRUCT OVERLAP` or
/// union anywhere inside it.
fn convertible(prepared: &PreparedModule, type_id: u32) -> bool {
    let Some(layout) = prepared.type_layout(type_id) else {
        return false;
    };
    // The layout flattens every nested text leaf, so an empty leaf list is text-free throughout.
    layout.text_leaves.is_empty() && shared_storage_free(prepared, type_id, 0)
}

/// Whether the type holds no `STRUCT OVERLAP` or union at any depth, following aliases.
fn shared_storage_free(prepared: &PreparedModule, type_id: u32, depth: usize) -> bool {
    if depth > 32 {
        return false;
    }
    let Some(entry) = prepared.type_table().entries.get(type_id as usize) else {
        return false;
    };
    match &entry.data {
        TypeData::Alias { target_type_id }
        | TypeData::Subrange {
            base_type_id: target_type_id,
            ..
        } => shared_storage_free(prepared, *target_type_id, depth + 1),
        TypeData::Struct { fields } => {
            entry.kind != TypeKind::OverlappingStruct
                && fields
                    .iter()
                    .all(|field| shared_storage_free(prepared, field.type_id, depth + 1))
        }
        TypeData::Union { .. } => false,
        TypeData::Array { elem_type_id, .. } => {
            shared_storage_free(prepared, *elem_type_id, depth + 1)
        }
        _ => true,
    }
}

/// The element count of a declared array's bounds.
fn element_count(dims: &[(i64, i64)]) -> usize {
    dims.iter()
        .map(|(lower, upper)| usize::try_from(upper - lower + 1).expect("an admitted bound"))
        .product()
}

/// The name of a `TYPE_TABLE` entry, or none for an anonymous type.
fn entry_name(prepared: &PreparedModule, type_id: u32) -> SmolStr {
    prepared
        .type_table()
        .entries
        .get(type_id as usize)
        .and_then(|entry| entry.name_idx)
        .and_then(|name_idx| prepared.strings().get(name_idx as usize))
        .cloned()
        .unwrap_or_default()
}

/// One generated value of the type: a real value of every kind, and a null reference for a
/// reference leaf.
fn generate(prepared: &PreparedModule, type_id: u32, rng: &mut Rng) -> Value {
    let entry = prepared
        .type_table()
        .entries
        .get(type_id as usize)
        .expect("a prepared type");
    match &entry.data {
        TypeData::Alias { target_type_id } => generate(prepared, *target_type_id, rng),
        TypeData::Subrange { base_type_id, .. } => generate(prepared, *base_type_id, rng),
        TypeData::Primitive { prim_id, .. } => generate_primitive(*prim_id, rng),
        TypeData::Enum { variants, .. } => {
            let count = u64::try_from(variants.len()).expect("a variant count");
            let index = usize::try_from(rng.next() % count).expect("an index");
            let variant = &variants[index];
            enumeration(
                &entry_name(prepared, type_id),
                prepared.strings()[variant.name_idx as usize].as_str(),
                variant.value,
            )
        }
        TypeData::Struct { fields } | TypeData::Union { fields } => {
            let mut members: OrderedMap<SmolStr, Value> = OrderedMap::default();
            for field in fields {
                let name = prepared.strings()[field.name_idx as usize].clone();
                members.insert(name, generate(prepared, field.type_id, rng));
            }
            Value::Struct(Arc::new(StructValue::from_canonical_parts(
                entry_name(prepared, type_id),
                members,
            )))
        }
        TypeData::Array { elem_type_id, dims } => {
            let elements = (0..element_count(dims))
                .map(|_| generate(prepared, *elem_type_id, rng))
                .collect();
            array(elements, dims.clone())
        }
        TypeData::Reference { .. } => Value::Reference(None),
        TypeData::Pou { .. } | TypeData::Interface { .. } => {
            let bytes = rng.bytes();
            Value::Instance(InstanceId(u32::from_le_bytes([
                bytes[0], bytes[1], bytes[2], bytes[3],
            ])))
        }
    }
}

/// One generated scalar of a primitive encoding: any bits of that width.
fn generate_primitive(prim_id: u16, rng: &mut Rng) -> Value {
    let bytes = rng.bytes();
    let first = [bytes[0]];
    let two = [bytes[0], bytes[1]];
    let four = [bytes[0], bytes[1], bytes[2], bytes[3]];
    match prim_id {
        1 => Value::Bool(bytes[0] & 1 == 0),
        2 => Value::Byte(bytes[0]),
        3 => Value::Word(u16::from_le_bytes(two)),
        4 => Value::DWord(u32::from_le_bytes(four)),
        5 => Value::LWord(u64::from_le_bytes(bytes)),
        6 => Value::SInt(i8::from_le_bytes(first)),
        7 => Value::Int(i16::from_le_bytes(two)),
        8 => Value::DInt(i32::from_le_bytes(four)),
        9 => Value::LInt(i64::from_le_bytes(bytes)),
        10 => Value::USInt(bytes[0]),
        11 => Value::UInt(u16::from_le_bytes(two)),
        12 => Value::UDInt(u32::from_le_bytes(four)),
        13 => Value::ULInt(u64::from_le_bytes(bytes)),
        14 => Value::Real(f32::from_bits(u32::from_le_bytes(four))),
        15 => Value::LReal(f64::from_bits(u64::from_le_bytes(bytes))),
        16 => Value::Time(Duration::from_nanos(i64::from_le_bytes(bytes))),
        17 => Value::LTime(Duration::from_nanos(i64::from_le_bytes(bytes))),
        18 => Value::Date(DateValue::new(i64::from_le_bytes(bytes))),
        19 => Value::LDate(LDateValue::new(i64::from_le_bytes(bytes))),
        20 => Value::Tod(TimeOfDayValue::new(i64::from_le_bytes(bytes))),
        21 => Value::LTod(LTimeOfDayValue::new(i64::from_le_bytes(bytes))),
        22 => Value::Dt(DateTimeValue::new(i64::from_le_bytes(bytes))),
        23 => Value::Ldt(LDateTimeValue::new(i64::from_le_bytes(bytes))),
        26 => Value::Char(bytes[0]),
        27 => Value::WChar(u16::from_le_bytes(two)),
        0x0100 => Value::DInt(i32::from_le_bytes(four)),
        other => panic!("the corpus has no text leaf here: primitive {other}"),
    }
}

/// Write a value the check refuses against marked cells and require the exact refusal and unchanged
/// cells.
fn refused_write(
    marked: &[Cell; 12],
    expected: &RuntimeError,
    attempt: impl FnOnce(&mut [Cell]) -> Result<(), RuntimeError>,
) {
    let mut cells = *marked;
    let error = attempt(&mut cells).expect_err("the value is refused");
    assert_eq!(error, *expected);
    assert_eq!(cells, *marked, "the refused write changed the cells");
}

#[test]
fn a_mismatched_value_is_refused_and_writes_nothing() {
    // Specification 34 §6.2, the value-conversion paragraph: a write checks the whole value before
    // it writes a cell and refuses a value whose kind, members, bounds, element count or slice
    // length does not match the type, with an error that names the type and the reason. A
    // conversion that wrote member by member and refused in the middle would leave a partly
    // written range; one that fell back to a default, that ignored a member's name or that
    // accepted another region's length fails here.
    let prepared = prepared();
    let marked = [Cell::DInt(-1); 12];

    // A value of another kind, and a slice that is not the layout's cells.
    refused_write(
        &marked,
        &RuntimeError::InitFailed {
            owner: "#3".into(),
            variable: "REAL".into(),
            error: "the value's kind does not match the type".into(),
        },
        |cells| write_value(&mut cells[..1], DINT, &Value::Real(1.0), &prepared),
    );
    refused_write(
        &marked,
        &RuntimeError::InitFailed {
            owner: "#3".into(),
            variable: "cells".into(),
            error: "the slice holds 2 cells, the type's layout needs 1".into(),
        },
        |cells| write_value(&mut cells[..2], DINT, &Value::DInt(0), &prepared),
    );

    // A structure with a missing member and one with a member the type does not declare.
    refused_write(
        &marked,
        &RuntimeError::InitFailed {
            owner: "Axis".into(),
            variable: "series".into(),
            error: "the value has no member of that name".into(),
        },
        |cells| {
            let missing = structure(
                "Axis",
                vec![
                    ("position", Value::Real(0.0)),
                    ("limits", band(0.0, 0.0)),
                    ("mode", enumeration("Mode", "Idle", 0)),
                ],
            );
            write_value(&mut cells[..7], AXIS, &missing, &prepared)
        },
    );
    refused_write(
        &marked,
        &RuntimeError::InitFailed {
            owner: "Axis".into(),
            variable: "nope".into(),
            error: "the structure has no member of that name".into(),
        },
        |cells| {
            let extra = structure(
                "Axis",
                vec![
                    ("position", Value::Real(0.0)),
                    ("limits", band(0.0, 0.0)),
                    ("mode", enumeration("Mode", "Idle", 0)),
                    ("series", array(vec![Value::Int(0); 3], vec![(1, 3)])),
                    ("nope", Value::Null),
                ],
            );
            write_value(&mut cells[..7], AXIS, &extra, &prepared)
        },
    );

    // An array whose bounds or element count are not the type's (`ARRAY[1..3] OF INT`).
    refused_write(
        &marked,
        &RuntimeError::InitFailed {
            owner: "#30".into(),
            variable: "bounds".into(),
            error: "the value's bounds [(0, 2)] are not the type's [(1, 3)]".into(),
        },
        |cells| {
            let wrong = array(vec![Value::Int(0); 3], vec![(0, 2)]);
            write_value(&mut cells[..3], INT_SERIES, &wrong, &prepared)
        },
    );
    refused_write(
        &marked,
        &RuntimeError::InitFailed {
            owner: "#30".into(),
            variable: "elements".into(),
            error: "the value holds 2 elements, the type holds 3".into(),
        },
        |cells| {
            let short = array(vec![Value::Int(0); 2], vec![(1, 3)]);
            write_value(&mut cells[..3], INT_SERIES, &short, &prepared)
        },
    );

    // A text leaf and a non-null reference: the leaves C1-7 and C1-9 convert.
    refused_write(
        &marked,
        &RuntimeError::InitFailed {
            owner: "#25".into(),
            variable: "STRING".into(),
            error: "text is converted in C1-7".into(),
        },
        |cells| {
            write_value(
                &mut cells[..1],
                STRING,
                &Value::String("x".into()),
                &prepared,
            )
        },
    );
    refused_write(
        &marked,
        &RuntimeError::InitFailed {
            owner: "#26".into(),
            variable: "WSTRING".into(),
            error: "text is converted in C1-7".into(),
        },
        |cells| {
            write_value(
                &mut cells[..1],
                WSTRING,
                &Value::WString("x".into()),
                &prepared,
            )
        },
    );
    refused_write(
        &marked,
        &RuntimeError::InitFailed {
            owner: "RefToInt".into(),
            variable: "REFERENCE".into(),
            error: "a non-null reference is stored in C1-9".into(),
        },
        |cells| {
            let reference = ValueRef {
                location: MemoryLocation::Global,
                offset: 0,
                generation: Generation::FIRST,
                path: vec![field_path("x")],
            };
            write_value(
                &mut cells[..1],
                REF_TO_INT,
                &Value::Reference(Some(reference)),
                &prepared,
            )
        },
    );
}

#[test]
fn a_struct_overlap_is_refused() {
    // Specification 34 §6.2 after MEM-10: a `STRUCT OVERLAP`'s members share their cells through a
    // byte backing that C1-15 owns, so a conversion that stored both members in the same cell
    // would lose one. It is refused with the type and the reason, and a write of the same module's
    // other types is unaffected. A union shares its members' cells the same way and is refused as
    // well.
    let prepared = prepared();
    let marked = [Cell::DInt(-1); 12];
    let packed = structure(
        "Packed",
        vec![("as_word", Value::DWord(1)), ("as_int", Value::DInt(1))],
    );
    refused_write(
        &marked,
        &RuntimeError::InitFailed {
            owner: "Packed".into(),
            variable: "OVERLAP".into(),
            error: "an overlapping structure is written through its byte backing in C1-15".into(),
        },
        |cells| write_value(&mut cells[..1], PACKED, &packed, &prepared),
    );
    assert_eq!(prepared.type_layout(PACKED).expect("Packed").cells, 1);

    // The refusals above disturbed nothing: the same module's Axis still writes and reads.
    let good = axis(1.0, band(0.0, 0.0), ("Idle", 0), [1, 2, 3]);
    let mut cells = [Cell::DInt(-1); 7];
    write_value(&mut cells, AXIS, &good, &prepared).expect("the same type still writes");
    let read = read_value(&cells, AXIS, &prepared).expect("and reads");
    assert_same(&read, &good, "Axis");
}

#[test]
fn cell_offsets_follow_the_layout() {
    // Specification 34 §6.2 and design "Operands and references": a reference's path converts to
    // and from a cell offset through the layout, a member by name and an index by its IEC index
    // with the array's lower bounds. An implementation that returned a member ordinal, that
    // ignored a lower bound, that indexed from zero or that answered a path the type does not have
    // with a wrong offset fails here.
    let prepared = prepared();
    let cases: [(u32, Vec<RefSegment>, Option<u32>); 22] = [
        // Axis: position at 0, limits (Band: low, high) at 1 and 2, mode at 3, series at 4.
        (AXIS, vec![], Some(0)),
        (AXIS, vec![field_path("position")], Some(0)),
        (AXIS, vec![field_path("limits")], Some(1)),
        (
            AXIS,
            vec![field_path("limits"), field_path("high")],
            Some(2),
        ),
        (AXIS, vec![field_path("mode")], Some(3)),
        // ST names are case-insensitive.
        (AXIS, vec![field_path("LIMITS")], Some(1)),
        // series is ARRAY[1..3] OF INT at cell 4, so index 2 is element 1 at cell 5.
        (AXIS, vec![field_path("series"), index_path(&[2])], Some(5)),
        // An index below the declared lower bound, and an index count the type does not have.
        (AXIS, vec![field_path("series"), index_path(&[0])], None),
        (AXIS, vec![field_path("series"), index_path(&[1, 1])], None),
        // Cells is ARRAY[-1..1, 2..3] OF Pair: element (i, j) is at 2 * ((i + 1) * 2 + (j - 2)).
        (CELLS, vec![index_path(&[-1, 2])], Some(0)),
        (CELLS, vec![index_path(&[0, 3]), field_path("y")], Some(7)),
        (CELLS, vec![index_path(&[1, 3])], Some(10)),
        (AXIS_ARRAY, vec![index_path(&[1])], Some(7)),
        (
            AXIS_ARRAY,
            vec![index_path(&[1]), field_path("limits"), field_path("low")],
            Some(8),
        ),
        // A path the type does not have: an unknown member, a member of a scalar, an index
        // outside the bounds, a member of an array and a type with no layout.
        (AXIS, vec![field_path("nope")], None),
        (AXIS, vec![field_path("position"), field_path("x")], None),
        (INT_SERIES, vec![field_path("x")], None),
        (INT_SERIES, vec![index_path(&[1])], Some(0)),
        (INT_SERIES, vec![index_path(&[4])], None),
        (CELLS, vec![index_path(&[0, 2]), field_path("low")], None),
        (CELLS, vec![index_path(&[0, 2]), field_path("y")], Some(5)),
        (REF_TO_INT, vec![field_path("x")], None),
    ];
    for (type_id, path, expected) in cases {
        assert_eq!(cell_offset(type_id, &path, &prepared), expected, "{path:?}");
    }
}

#[test]
fn a_store_refuses_a_range_of_another_region() {
    // Specification 34 §6.2 after MEM-10: a region owns one cell store, and every accessor answers
    // `None` outside the reserved region. A store that ignored the range's region would let an
    // activation index address a persistent cell. The store records the region it holds, so the
    // persistent store refuses an activation range and the activation store a persistent one.
    let prepared = prepared();
    let mut persistent = CellStore::persistent(&prepared).expect("the persistent region reserves");
    let activation_range = CellRange {
        region: LayoutRegion::Activation,
        start: 0,
        cells: 1,
    };
    let persistent_range = CellRange {
        region: LayoutRegion::Persistent,
        start: 0,
        cells: 1,
    };

    // The trigger BOOL is the persistent region's one cell.
    assert_eq!(persistent.region(), LayoutRegion::Persistent);
    assert_eq!(persistent.reserved(), (1, 0));
    assert_eq!(persistent.cells(persistent_range), Some(&[Cell::Null][..]));
    assert_eq!(persistent.cells(activation_range), None);
    assert_eq!(persistent.cells_mut(activation_range), None);

    let mut activation =
        CellStore::reserve_in(LayoutRegion::Activation, 2, 0).expect("an activation region");
    assert_eq!(activation.region(), LayoutRegion::Activation);
    assert_eq!(
        activation.cells(CellRange {
            region: LayoutRegion::Activation,
            start: 1,
            cells: 1,
        }),
        Some(&[Cell::Null][..])
    );
    assert_eq!(activation.cells(persistent_range), None);
    assert_eq!(activation.cells_mut(persistent_range), None);
    assert_eq!(
        activation.cells_mut(CellRange {
            region: LayoutRegion::Activation,
            start: 0,
            cells: 3,
        }),
        None
    );
}
