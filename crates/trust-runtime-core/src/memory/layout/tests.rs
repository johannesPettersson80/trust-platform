//! Specification 34 MEM-12: the reference leaves of a type's layout, one for each reference a value
//! of the type holds, so that a block's table keeps an entry for each of them (C1-19d).

use super::*;
use crate::bytecode::{Field, TypeData, TypeEntry, TypeKind, TypeTable};

/// The layouts of `entries`, whose member names are `first` (0) and `pointers` (1).
fn layouts(entries: Vec<TypeEntry>) -> Vec<TypeLayout> {
    let types = TypeTable {
        offsets: Vec::new(),
        entries,
    };
    let names: Names = ["first", "pointers"].into_iter().collect();
    compute_type_layouts(&types, &names, &mut |_| Ok(None), &mut |_, _| Ok(())).expect("layouts")
}

fn entry(kind: TypeKind, data: TypeData) -> TypeEntry {
    TypeEntry {
        kind,
        name_idx: None,
        data,
    }
}

fn leaf_offsets(layout: &TypeLayout) -> Vec<u32> {
    layout
        .reference_leaves
        .iter()
        .map(|leaf| leaf.byte_offset)
        .collect()
}

/// Specification 34 MEM-12 (C1-19d): an array of references holds one reference per element, so its
/// layout lists a leaf for each element. Worked by hand: a reference leaf is 16 bytes at alignment 8,
/// so `ARRAY[1..3] OF REF_TO INT` has a 16-byte stride and leaves at 0, 16 and 32. In
/// `STRUCT first : INT; pointers : ARRAY[1..2] OF REF_TO INT; END_STRUCT` the 2-byte `first` lies at
/// 0 and `pointers` at the next multiple of 8, 8, so the leaves are at 8 and 24. An `ARRAY[*] OF
/// REF_TO INT` has no elements of its own and lists none, yet the argument it stands for can hold
/// references. Catches the layout that listed only the first element's leaf, which left a reference
/// that names its target by a path, stored in a later element of a frame's array, without the table
/// entry its write needs.
#[test]
fn an_array_lists_the_reference_leaf_of_every_element() {
    let layouts = layouts(vec![
        // 0: INT
        entry(
            TypeKind::Primitive,
            TypeData::Primitive {
                prim_id: 7,
                max_length: 0,
            },
        ),
        // 1: REF_TO INT
        entry(
            TypeKind::Reference,
            TypeData::Reference { target_type_id: 0 },
        ),
        // 2: ARRAY[1..3] OF REF_TO INT
        entry(
            TypeKind::Array,
            TypeData::Array {
                elem_type_id: 1,
                dims: vec![(1, 3)],
            },
        ),
        // 3: ARRAY[1..2] OF REF_TO INT
        entry(
            TypeKind::Array,
            TypeData::Array {
                elem_type_id: 1,
                dims: vec![(1, 2)],
            },
        ),
        // 4: STRUCT first : INT; pointers : ARRAY[1..2] OF REF_TO INT; END_STRUCT
        entry(
            TypeKind::Struct,
            TypeData::Struct {
                fields: vec![
                    Field {
                        name_idx: 0,
                        type_id: 0,
                        bit_offset: None,
                    },
                    Field {
                        name_idx: 1,
                        type_id: 3,
                        bit_offset: None,
                    },
                ],
            },
        ),
        // 5: ARRAY[*] OF REF_TO INT
        entry(
            TypeKind::Array,
            TypeData::Array {
                elem_type_id: 1,
                dims: vec![(0, i64::MAX)],
            },
        ),
    ]);
    assert_eq!(leaf_offsets(&layouts[2]), [0, 16, 32]);
    assert_eq!(layouts[2].shape.bytes, 48);
    assert_eq!(leaf_offsets(&layouts[4]), [8, 24]);
    assert_eq!(layouts[4].shape.bytes, 40);
    assert_eq!(leaf_offsets(&layouts[5]), [] as [u32; 0]);
    assert_eq!(layouts[5].shape.bytes, 0);
    assert!(layouts[5].holds_reference_or_instance);
}
