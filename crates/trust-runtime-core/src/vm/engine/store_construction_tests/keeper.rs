//! The bytecode module of the construction tests that need references in aggregates and dates
//! (specification 34 §6.2, MEM-10; C1-9d2b-1). It is built with the builders of the cell arm's fixture.
//!
//! The ST source it encodes, from which every expected value of the tests is worked out by hand:
//!
//! ```text
//! TYPE Holder : STRUCT r : REF_TO DINT; n : DINT; END_STRUCT END_TYPE
//! FUNCTION_BLOCK Keeper
//!     VAR held : Holder; slots : ARRAY[1..2] OF REF_TO DINT; END_VAR
//! END_FUNCTION_BLOCK
//! PROGRAM Main END_PROGRAM
//! VAR_GLOBAL
//!     holder_g : Holder;  slots_g : ARRAY[1..2] OF REF_TO DINT;  keeper_g : Keeper;
//!     day_g : DATE;  stamp_g : DATE_AND_TIME;
//! END_VAR
//! ```
//!
//! The globals take 24 + 32 + 8 + 8 + 8 = 80 bytes at their declared sizes (MEM-12: `holder_g` a
//! 16-byte reference and a DINT rounded up to 24, `slots_g` two 16-byte references, `keeper_g` an
//! 8-byte instance leaf, the dates 8 each). The roots follow in CONSTRUCTION_ROOTS order: `Main` (no
//! bytes of its own) at byte 80, then the `Keeper` instance of `keeper_g` at byte 80 with 56 bytes of
//! its own, `held` (24) at 0 and `slots` (32) at 24.

use crate::bytecode::{
    compute_type_offsets_for_entries, AccessBindings, BytecodeModule, BytecodeVersion,
    ConstructionRoot, ConstructionRoots, InitializerIndex, PouClassMeta, PouEntry, PouIndex,
    PouKind, RefTable, Section, SectionData, SectionId, StorageDeclaration, StorageLayout,
    StorageOwner, StorageRole, StringTable, TypeData, TypeKind, TypeTable, VarMeta,
    HEADER_FLAG_CRC32,
};
use crate::memory::cell_access::fixture::{
    anonymous, base_module, declaration, field, global_ref, initializer, named, primitive,
    result_ref, var_meta,
};
use crate::vm::{PreparationLimits, PreparedModule};
use smol_str::SmolStr;

/// The `TYPE_TABLE` ordinals of the module's types.
pub(super) const DINT: u32 = 0;
pub(super) const REF_DINT: u32 = 1;
pub(super) const HOLDER: u32 = 2;
pub(super) const REFS: u32 = 3;
pub(super) const DATE: u32 = 4;
pub(super) const DT: u32 = 5;
pub(super) const KEEPER: u32 = 6;

/// The POU ids of `PROGRAM Main` and `FUNCTION_BLOCK Keeper`.
const MAIN_POU: u32 = 1;
pub(super) const KEEPER_POU: u32 = 2;

/// The number of initializers: one for each of the five globals and the two members of `Keeper`.
const INITIALIZERS: u32 = 7;

/// The module, validated and prepared with the default limits.
pub(super) fn prepared() -> PreparedModule {
    let module = keeper_module();
    module
        .validate()
        .expect("the Keeper module is a valid source-free module");
    PreparedModule::from_bytes(
        &module.encode().expect("encode"),
        PreparationLimits::default(),
    )
    .expect("the Keeper module prepares")
}

fn keeper_module() -> BytecodeModule {
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
            data: SectionData::Initializers(initializers()),
        },
        Section {
            id: SectionId::AccessBindings.as_raw(),
            flags: 0,
            data: SectionData::AccessBindings(AccessBindings::default()),
        },
        Section {
            id: SectionId::VarMeta.as_raw(),
            flags: 0,
            data: SectionData::VarMeta(var_meta_entries()),
        },
    ]);
    module
}

/// Strings 0 to 2 are the base module's `R`, `T` and `Main`. Then the globals (3 to 7), the type and
/// member names (8 to 13) and one `@init/N/result` for each initializer (14 to 20).
fn strings() -> StringTable {
    let mut entries: Vec<SmolStr> = [
        "R", "T", "Main", "holder_g", "slots_g", "keeper_g", "day_g", "stamp_g", "Holder", "r",
        "n", "Keeper", "held", "slots",
    ]
    .into_iter()
    .map(SmolStr::new)
    .collect();
    entries.extend((0..INITIALIZERS).map(|index| SmolStr::new(format!("@init/{index}/result"))));
    StringTable { entries }
}

fn types() -> TypeTable {
    let mut table = TypeTable {
        offsets: Vec::new(),
        entries: vec![
            primitive(8, 0),
            anonymous(
                TypeKind::Reference,
                TypeData::Reference {
                    target_type_id: DINT,
                },
            ),
            named(
                TypeKind::Struct,
                8,
                TypeData::Struct {
                    fields: vec![field(9, REF_DINT), field(10, DINT)],
                },
            ),
            anonymous(
                TypeKind::Array,
                TypeData::Array {
                    elem_type_id: REF_DINT,
                    dims: vec![(1, 2)],
                },
            ),
            primitive(18, 0),
            primitive(22, 0),
            named(
                TypeKind::FunctionBlock,
                11,
                TypeData::Pou { pou_id: KEEPER_POU },
            ),
        ],
    };
    table.offsets = compute_type_offsets_for_entries(&table.entries).expect("type offsets");
    table
}

/// References 0 to 5 bind the six globals; 6 to 12 name the initializers' results.
fn refs() -> RefTable {
    let mut entries: Vec<_> = (0..6).map(global_ref).collect();
    entries.extend((0..INITIALIZERS).map(result_ref));
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
                id: KEEPER_POU,
                name_idx: 11,
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
    }
}

/// A global variable of `type_id` in slot `slot`, bound by the reference of the same index.
fn global(name_idx: u32, type_id: u32, slot: u32, nodes: u32) -> StorageDeclaration {
    declaration(
        StorageOwner::Global,
        StorageRole::Variable,
        None,
        name_idx,
        Some(type_id),
        slot,
        Some(slot),
        nodes,
    )
}

/// A member of `Keeper` of `type_id` in slot `slot`.
fn member(name_idx: u32, type_id: u32, slot: u32) -> StorageDeclaration {
    declaration(
        StorageOwner::Instance,
        StorageRole::Variable,
        Some(KEEPER_POU),
        name_idx,
        Some(type_id),
        slot,
        None,
        3,
    )
}

fn storage_layout() -> StorageLayout {
    StorageLayout {
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
            // `Holder` is 1 + 1 + 1 nodes, `REFS` 1 + 2 x 1, and `Keeper` 1 + 3 + 3.
            global(3, HOLDER, 1, 3),
            global(4, REFS, 2, 3),
            global(5, KEEPER, 3, 7),
            global(6, DATE, 4, 1),
            global(7, DT, 5, 1),
            member(12, HOLDER, 0),
            member(13, REFS, 1),
        ],
    }
}

fn construction_roots() -> ConstructionRoots {
    let root = |declaration_idx: u32,
                instance_owner_id: Option<u32>,
                template_pou_id: Option<u32>| ConstructionRoot {
        declaration_idx,
        binding_ref_idx: Some(declaration_idx),
        instance_owner_id,
        parent_root_idx: None,
        template_pou_id,
        flags: 0,
    };
    ConstructionRoots {
        entries: vec![
            root(0, Some(0), Some(MAIN_POU)),
            root(1, None, None),
            root(2, None, None),
            root(3, Some(1), Some(KEEPER_POU)),
            root(4, None, None),
            root(5, None, None),
        ],
    }
}

fn initializers() -> InitializerIndex {
    InitializerIndex {
        entries: vec![
            initializer(1, None, 6),
            initializer(2, None, 7),
            initializer(3, None, 8),
            initializer(4, None, 9),
            initializer(5, None, 10),
            initializer(6, Some(KEEPER_POU), 11),
            initializer(7, Some(KEEPER_POU), 12),
        ],
    }
}

fn var_meta_entries() -> VarMeta {
    VarMeta {
        entries: vec![
            var_meta(14, HOLDER, 6),
            var_meta(15, REFS, 7),
            var_meta(16, KEEPER, 8),
            var_meta(17, DATE, 9),
            var_meta(18, DT, 10),
            var_meta(19, HOLDER, 11),
            var_meta(20, REFS, 12),
        ],
    }
}
