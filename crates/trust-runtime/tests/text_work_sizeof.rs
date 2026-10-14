//! TIME-08: SIZEOF of a value charges 1 unit per value node visited, plus 1 per byte of text
//! and 1 per array dimension; an array visits its first element only. SIZEOF of a type charges
//! 1 unit plus the nesting depth per type node, plus 1 per array dimension.
use trust_runtime_core::bytecode::{Field, TypeData, TypeEntry, TypeKind, TypeTable};
use trust_runtime_core::collections::OrderedMap;
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::value::{
    size_of_value_with_abi_and_visitor, ArrayValue, StorageAbi, StructValue, Value,
};
use trust_runtime_core::vm::sizeof_type_from_table_with;

/// Units that the value walk charges for `value`, summed from every visit.
fn value_units(value: &Value) -> usize {
    let mut units = 0usize;
    let mut enum_size = |_: &str| -> Result<u64, RuntimeError> { Ok(0) };
    let mut structure_size = |_: &str| -> Result<Option<u64>, RuntimeError> { Ok(None) };
    let mut visit = |count: usize| -> Result<(), RuntimeError> {
        units += count;
        Ok(())
    };
    size_of_value_with_abi_and_visitor(
        value,
        StorageAbi::default(),
        &mut enum_size,
        &mut structure_size,
        &mut visit,
    )
    .expect("a sized value");
    units
}

/// Units that the type walk charges for the type at `index`.
fn type_units(types: &TypeTable, index: u32) -> usize {
    let mut units = 0usize;
    sizeof_type_from_table_with(types, index, &mut |count: usize| {
        units += count;
        Ok(())
    })
    .expect("a sized type");
    units
}

fn entry(kind: TypeKind, data: TypeData) -> TypeEntry {
    TypeEntry {
        kind,
        name_idx: None,
        data,
    }
}

#[test]
fn scalar_value_visits_one_node() {
    assert_eq!(value_units(&Value::Int(7)), 1);
}

#[test]
fn string_value_charges_one_unit_per_byte_of_its_text() {
    // "åäö" is 3 characters in 6 UTF-8 bytes: one node plus 6 text bytes.
    assert_eq!(value_units(&Value::String("åäö".into())), 1 + 6);
}

#[test]
fn wstring_value_charges_one_unit_per_byte_of_its_text() {
    // "αβγ" is 3 characters in 6 UTF-8 bytes: one node plus 6 text bytes.
    assert_eq!(value_units(&Value::WString("αβγ".into())), 1 + 6);
}

#[test]
fn array_value_visits_only_its_first_element() {
    // Array node 1, first element 1 + 6 text bytes, one dimension 1: total 9. Visiting every
    // element would give 1 + 3 × 7 + 1 = 23.
    let elements = vec![Value::WString("αβγ".into()); 3];
    let array = Value::Array(Box::new(ArrayValue::from_canonical_parts(
        elements,
        vec![(1, 3)],
    )));
    assert_eq!(value_units(&array), 1 + (1 + 6) + 1);
}

#[test]
fn structure_value_visits_every_field() {
    // Structure node 1, text field 1 + 6 bytes, integer field 1: total 9.
    let mut fields = OrderedMap::default();
    fields.insert("text".into(), Value::String("åäö".into()));
    fields.insert("count".into(), Value::Int(1));
    let structure = Value::Struct(std::sync::Arc::new(StructValue::from_canonical_parts(
        "Pair".into(),
        fields,
    )));
    assert_eq!(value_units(&structure), 1 + (1 + 6) + 1);
}

/// Type table: 0 INT, 1 STRING[4], 2 ARRAY[1..3] OF STRING[4], 3 ARRAY[1..2, 1..3] OF INT,
/// 4 ARRAY[1..2] OF INT, 5 STRUCT { a : INT; b : ARRAY[1..2] OF INT }.
fn type_table() -> TypeTable {
    TypeTable {
        entries: vec![
            entry(
                TypeKind::Primitive,
                TypeData::Primitive {
                    // Primitive id 7 is INT (id 3 is WORD); both are two bytes here.
                    prim_id: 7,
                    max_length: 0,
                },
            ),
            entry(
                TypeKind::Primitive,
                TypeData::Primitive {
                    prim_id: 24,
                    max_length: 4,
                },
            ),
            entry(
                TypeKind::Array,
                TypeData::Array {
                    elem_type_id: 1,
                    dims: vec![(1, 3)],
                },
            ),
            entry(
                TypeKind::Array,
                TypeData::Array {
                    elem_type_id: 0,
                    dims: vec![(1, 2), (1, 3)],
                },
            ),
            entry(
                TypeKind::Array,
                TypeData::Array {
                    elem_type_id: 0,
                    dims: vec![(1, 2)],
                },
            ),
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
                            type_id: 4,
                            bit_offset: None,
                        },
                    ],
                },
            ),
        ],
        ..Default::default()
    }
}

#[test]
fn primitive_type_charges_one_unit() {
    assert_eq!(type_units(&type_table(), 0), 1);
}

#[test]
fn array_type_charges_its_node_its_dimension_and_its_element_depth() {
    // ARRAY[1..3] OF STRING[4]: array node at depth 0 costs 1; its dimension costs 1; the
    // element at depth 1 costs 1 + 1 = 2. Total 4.
    assert_eq!(type_units(&type_table(), 2), 4);
}

#[test]
fn two_dimensional_array_type_charges_each_dimension() {
    // ARRAY[1..2, 1..3] OF INT: node 1, two dimensions 2, element at depth 1 costs 2. Total 5.
    assert_eq!(type_units(&type_table(), 3), 5);
}

#[test]
fn structure_type_charges_nesting_depth_for_every_nested_type_node() {
    // STRUCT { a : INT; b : ARRAY[1..2] OF INT }:
    //   structure node at depth 0: 1
    //   a at depth 1: 2
    //   b at depth 1: 2, its dimension 1, its element INT at depth 2: 3
    // Total 1 + 2 + (2 + 1 + 3) = 9.
    assert_eq!(type_units(&type_table(), 5), 9);
}
