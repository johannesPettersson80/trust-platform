use super::*;

use std::sync::Arc;

use crate::value::{ArrayValue, StructValue};
use indexmap::IndexMap;

fn structure(type_name: &str, fields: &[(&str, Value)]) -> Value {
    Value::Struct(Arc::new(StructValue::from_untyped_parts(
        type_name.into(),
        fields
            .iter()
            .map(|(name, value)| ((*name).into(), value.clone()))
            .collect::<IndexMap<_, _>>(),
    )))
}

fn array(elements: Vec<Value>, dimensions: Vec<(i64, i64)>) -> Value {
    Value::Array(Box::new(
        ArrayValue::from_untyped_parts(elements, dimensions).expect("valid array"),
    ))
}

#[test]
fn empty_path_borrows_the_original_value_without_materializing_a_clone() {
    let value = structure("Root", &[("field", Value::DInt(7))]);
    let borrowed = read_value_path_borrowed(&value, &[]).expect("root borrow");

    assert!(std::ptr::eq(borrowed, &value));
}

#[test]
fn nested_field_index_field_path_returns_exact_borrowed_leaf() {
    let first = structure("Item", &[("value", Value::DInt(10))]);
    let second = structure("Item", &[("value", Value::DInt(20))]);
    let root = structure(
        "Root",
        &[("items", array(vec![first, second], vec![(1, 2)]))],
    );
    let path = vec![
        RefSegment::Field("items".into()),
        RefSegment::Index(ref_indices_from_iter([2])),
        RefSegment::Field("value".into()),
    ];

    assert_eq!(
        read_value_path_borrowed(&root, &path),
        Some(&Value::DInt(20))
    );
}

#[test]
fn multidimensional_index_path_uses_inclusive_bounds_and_row_major_order() {
    let root = array(
        vec![
            Value::Int(10),
            Value::Int(11),
            Value::Int(12),
            Value::Int(13),
        ],
        vec![(5, 6), (-1, 0)],
    );
    let path = [RefSegment::Index(ref_indices_from_iter([6, -1]))];

    assert_eq!(
        read_value_path_borrowed(&root, &path),
        Some(&Value::Int(12))
    );
}

#[test]
fn field_lookup_uses_exact_stored_spelling_and_rejects_wrong_aggregate_kind() {
    let root = structure("Root", &[("Exact", Value::DInt(7))]);

    assert_eq!(
        read_value_path_borrowed(&root, &[RefSegment::Field("Exact".into())]),
        Some(&Value::DInt(7))
    );
    assert_eq!(
        read_value_path_borrowed(&root, &[RefSegment::Field("exact".into())]),
        None
    );
    assert_eq!(
        read_value_path_borrowed(&Value::DInt(7), &[RefSegment::Field("Exact".into())]),
        None
    );
}

#[test]
fn array_path_rejects_wrong_arity_bounds_and_nonarray_target() {
    let root = array(vec![Value::Int(10), Value::Int(11)], vec![(5, 6)]);

    for indices in [Vec::new(), vec![5, 6], vec![4], vec![7]] {
        assert_eq!(
            read_value_path_borrowed(&root, &[RefSegment::Index(ref_indices_from_iter(indices))]),
            None
        );
    }
    assert_eq!(
        read_value_path_borrowed(
            &Value::DInt(7),
            &[RefSegment::Index(ref_indices_from_iter([0]))]
        ),
        None
    );
}

#[test]
fn extreme_signed_array_lower_bound_selects_first_element_without_overflow() {
    let root = Value::Array(Box::new(ArrayValue::from_canonical_parts(
        vec![Value::DInt(7)],
        vec![(i64::MIN, i64::MAX)],
    )));

    assert_eq!(
        read_value_path_borrowed(
            &root,
            &[RefSegment::Index(ref_indices_from_iter([i64::MIN]))]
        ),
        Some(&Value::DInt(7))
    );
}

#[test]
fn traversal_stops_when_an_intermediate_segment_does_not_exist() {
    let root = structure(
        "Root",
        &[("child", structure("Child", &[("value", Value::Int(1))]))],
    );
    let missing_field = [
        RefSegment::Field("missing".into()),
        RefSegment::Field("value".into()),
    ];
    let wrong_shape = [
        RefSegment::Field("child".into()),
        RefSegment::Index(ref_indices_from_iter([1])),
    ];

    assert_eq!(read_value_path_borrowed(&root, &missing_field), None);
    assert_eq!(read_value_path_borrowed(&root, &wrong_shape), None);
}

#[test]
fn borrowed_traversal_does_not_synthesize_string_character_values() {
    for value in [Value::String("AB".into()), Value::WString("AB".into())] {
        assert_eq!(
            read_value_path_borrowed(&value, &[RefSegment::Index(ref_indices_from_iter([1]))]),
            None
        );
    }
}

#[test]
fn string_path_helper_accepts_exactly_one_index() {
    assert_eq!(single_string_index(&[]), None);
    assert_eq!(single_string_index(&[1]), Some(1));
    assert_eq!(single_string_index(&[-1]), Some(-1));
    assert_eq!(single_string_index(&[1, 2]), None);
}

#[test]
fn field_then_empty_tail_returns_the_stored_field_by_reference() {
    let root = structure("Root", &[("value", Value::DInt(9))]);
    let Value::Struct(structure) = &root else {
        panic!("expected structure");
    };
    let stored = structure.field("value").expect("stored field");
    let borrowed = read_value_path_borrowed(&root, &[RefSegment::Field("value".into())])
        .expect("field borrow");

    assert!(std::ptr::eq(borrowed, stored));
}
