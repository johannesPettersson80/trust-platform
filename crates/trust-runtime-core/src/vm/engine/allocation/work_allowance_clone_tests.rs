//! TIME-08 copy work and allocation: array nesting, reference paths, STRING and WSTRING text,
//! enumerations and structures, shared or not.
use super::*;
use crate::error::ExecutionLimitKind;
use crate::memory::MemoryLocation;
use crate::value::{ArrayValue, Duration, EnumValue, RefSegment, StructValue, ValueRef};
use crate::vm::context::ReferenceContext;
use alloc::{boxed::Box, string::String, sync::Arc, vec};

/// Copies `value` through the engine's top-level clone admission with `work` units
/// available. Returns the outcome, the units left and the allocation bytes charged.
fn copy_under_work(value: &Value, work: usize) -> (Result<(), RuntimeError>, usize, usize) {
    let prepared = PreparedModule::from_bytes(
        include_bytes!("../../../../../trust-runtime/tests/fixtures/portability/f401/gpio.stbc"),
        crate::vm::PreparationLimits::default(),
    )
    .unwrap();
    let mut data = EngineState::construct_data(&prepared, 0, &services::LOGICAL_ONLY).unwrap();
    let state = EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY);
    state.data.resources.work_budget.reset(work);
    state.data.resources.construction_bytes.set(0);
    let result = state.before_value_clone(value);
    (
        result,
        state.data.resources.work_budget.remaining(),
        state.data.resources.construction_bytes.get(),
    )
}

fn int_pair_array(first: i16, second: i16) -> Value {
    Value::Array(Box::new(ArrayValue::from_canonical_parts(
        vec![Value::Int(first), Value::Int(second)],
        vec![(0, 1)],
    )))
}

#[test]
fn nested_arrays_with_two_elements_at_two_levels_cost_nine_units_exactly() {
    // TIME-08 COPY: the outer array costs 1 + 2 elements; each inner array costs 1 + 2.
    let outer = Value::Array(Box::new(ArrayValue::from_canonical_parts(
        vec![int_pair_array(1, 2), int_pair_array(3, 4)],
        vec![(0, 1)],
    )));

    let (result, remaining, _) = copy_under_work(&outer, 9);
    assert_eq!(result, Ok(()));
    assert_eq!(remaining, 0);

    // One unit below, the second inner array's element charge is refused. No allocation
    // is charged, because the work refusal precedes the byte charge.
    let (result, remaining, bytes) = copy_under_work(&outer, 8);
    assert_eq!(
        result,
        Err(RuntimeError::ExecutionLimit {
            kind: ExecutionLimitKind::Work,
            used: 7,
            requested: 2,
            limit: 8,
        })
    );
    assert_eq!(remaining, 1);
    assert_eq!(bytes, 0);
}

#[test]
fn copy_work_follows_the_time_08_table_for_each_listed_value_kind() {
    let reference = Value::Reference(Some(ValueRef {
        location: MemoryLocation::Global,
        offset: 0,
        generation: crate::memory::Generation::FIRST,
        path: vec![RefSegment::Field("x".into()), RefSegment::Index(vec![1, 2])],
    }));
    let array = Value::Array(Box::new(ArrayValue::from_canonical_parts(
        vec![Value::Int(7), Value::WString(String::from("owned"))],
        vec![(0, 1)],
    )));
    let structure = Value::Struct(Arc::new(StructValue::from_canonical_parts(
        "Record".into(),
        crate::collections::OrderedMap::default(),
    )));
    let enumeration = Value::Enum(Box::new(EnumValue::from_canonical_parts(
        "Mode".into(),
        "Run".into(),
        1,
    )));
    // Scalars cost nothing; a structure costs one unit for itself and one for each field (none
    // here); a WSTRING or enumeration costs one unit;
    // a reference costs 1 plus 1 per path segment (2) and per index (2) = 5; an array costs
    // 1 + 2 elements, and its WSTRING element costs one more.
    let cases = [
        (Value::Bool(true), 0),
        (Value::DInt(-3), 0),
        (Value::Real(1.5), 0),
        (Value::Time(Duration::ZERO), 0),
        (Value::WChar(u16::from(b'x')), 0),
        (structure, 1),
        (Value::WString(String::from("wide text")), 1),
        (enumeration, 1),
        (reference, 5),
        (array, 4),
    ];
    for (value, work) in cases {
        let (result, remaining, _) = copy_under_work(&value, work);
        assert_eq!(result, Ok(()), "{value:?} is admitted with {work} units");
        assert_eq!(remaining, 0, "{value:?} consumes exactly {work} units");
        if work > 0 {
            let (result, _, bytes) = copy_under_work(&value, work - 1);
            assert!(
                matches!(
                    result,
                    Err(RuntimeError::ExecutionLimit {
                        kind: ExecutionLimitKind::Work,
                        ..
                    })
                ),
                "{value:?} is refused with {} units",
                work - 1
            );
            assert_eq!(bytes, 0, "{value:?} charges no allocation when refused");
        }
    }
}

#[test]
fn reference_path_of_three_fields_and_two_indices_costs_eight_units() {
    let path = vec![
        RefSegment::Field("a".into()),
        RefSegment::Field("b".into()),
        RefSegment::Field("c".into()),
        RefSegment::Index(vec![1]),
        RefSegment::Index(vec![2]),
    ];
    let reference = Value::Reference(Some(ValueRef {
        location: MemoryLocation::Global,
        offset: 0,
        generation: crate::memory::Generation::FIRST,
        path,
    }));
    // TIME-08 COPY: 1 unit, plus 1 per path segment (3 fields + 2 indices) and 1 per index.
    let (result, remaining, _) = copy_under_work(&reference, 8);
    assert_eq!(result, Ok(()));
    assert_eq!(remaining, 0);

    let (result, _, bytes) = copy_under_work(&reference, 7);
    assert_eq!(
        result,
        Err(RuntimeError::ExecutionLimit {
            kind: ExecutionLimitKind::Work,
            used: 7,
            requested: 1,
            limit: 7,
        })
    );
    assert_eq!(bytes, 0);
}

#[test]
fn enumeration_copy_costs_one_unit_and_an_empty_allowance_refuses_it() {
    let enumeration = Value::Enum(Box::new(EnumValue::from_canonical_parts(
        "Mode".into(),
        "Run".into(),
        1,
    )));
    let (result, remaining, _) = copy_under_work(&enumeration, 1);
    assert_eq!(result, Ok(()));
    assert_eq!(remaining, 0);

    let (result, remaining, bytes) = copy_under_work(&enumeration, 0);
    assert_eq!(
        result,
        Err(RuntimeError::ExecutionLimit {
            kind: ExecutionLimitKind::Work,
            used: 0,
            requested: 1,
            limit: 0,
        })
    );
    assert_eq!(remaining, 0);
    assert_eq!(bytes, 0);
}

#[test]
fn copying_a_shared_structure_charges_its_structure_and_field_units() {
    let mut fields = crate::collections::OrderedMap::default();
    fields.insert("text".into(), Value::WString(String::from("abc")));
    fields.insert(
        "items".into(),
        Value::Array(Box::new(ArrayValue::from_canonical_parts(
            vec![Value::Int(1); 4],
            vec![(0, 3)],
        ))),
    );
    let shared = Arc::new(StructValue::from_canonical_parts("Probe".into(), fields));
    let copy = Value::Struct(Arc::clone(&shared));

    // TIME-08 COPY: 2 structures and arrays (the probe and items) plus 6 fields and elements (text,
    // items and the four INTs) make 8 units, and the WSTRING leaf "abc" pays its own unit: 9.
    // Its allocation is one 16-byte cell per copied leaf, shared or not: the WSTRING leaf (16 +
    // 6 bytes for 3 UTF-16 code units) and the four INT leaves (4 x 16 = 64): 86.
    let (result, remaining, bytes) = copy_under_work(&copy, 9);
    assert_eq!(result, Ok(()));
    assert_eq!(remaining, 0);
    assert_eq!(bytes, 86);

    // With 8 units the element count of items (4) is refused at used 5: 1 for the structure, 2 for
    // its fields, 1 for the WSTRING and 1 for items.
    let (result, remaining, bytes) = copy_under_work(&copy, 8);
    assert_eq!(
        result,
        Err(RuntimeError::ExecutionLimit {
            kind: ExecutionLimitKind::Work,
            used: 5,
            requested: 4,
            limit: 8,
        })
    );
    assert_eq!(remaining, 3);
    assert_eq!(bytes, 0);
}

/// Two integers and a structure, the same whether it is shared or not (TIME-08 copy allocation).
fn text_and_items_probe() -> Arc<StructValue> {
    let mut fields = crate::collections::OrderedMap::default();
    fields.insert("text".into(), Value::WString(String::from("abc")));
    fields.insert(
        "items".into(),
        Value::Array(Box::new(ArrayValue::from_canonical_parts(
            vec![Value::Int(1); 4],
            vec![(0, 3)],
        ))),
    );
    Arc::new(StructValue::from_canonical_parts("Probe".into(), fields))
}

/// Spec 34 TIME-08 copy allocation: a copy charges what it copies, never how the engine shares it.
/// It catches a charge that reads the owner count: a shared structure charged differently from an
/// unshared one, or a copy charged as a detach.
#[test]
fn copy_charge_is_the_same_when_a_structure_is_shared() {
    // The probe has one WSTRING leaf (16 + 6 bytes) and four INT leaves (4 x 16): 86 bytes, and
    // 9 units of work (see copying_a_shared_structure_charges_its_structure_and_field_units).
    let unshared = Value::Struct(text_and_items_probe());
    let (result, remaining, bytes) = copy_under_work(&unshared, 9);
    assert_eq!(result, Ok(()));
    assert_eq!(remaining, 0);
    assert_eq!(bytes, 86);

    // A second owner makes the same structure shared; the copy still charges 9 units and 86 bytes.
    let owner = text_and_items_probe();
    let shared = Value::Struct(Arc::clone(&owner));
    let (result, remaining, bytes) = copy_under_work(&shared, 9);
    assert_eq!(result, Ok(()));
    assert_eq!(remaining, 0);
    assert_eq!(bytes, 86);
}

/// Spec 34 TIME-08 copy allocation: a structure copy charges 16 bytes per copied leaf field,
/// counted through nested structures and arrays. It catches a charge per structure or per array
/// (a header), a nested structure counted as a leaf, and a leaf inside a nested structure or array
/// left out.
#[test]
fn a_structure_copy_charges_16_bytes_per_leaf() {
    // Outer has six leaves: the INT `a`; the DINT `b` and the BOOL `c` of the nested structure
    // `inner`; and the three INT elements of the array `arr`. 6 x 16 = 96 bytes.
    let mut inner = crate::collections::OrderedMap::default();
    inner.insert("b".into(), Value::DInt(2));
    inner.insert("c".into(), Value::Bool(true));
    let mut outer = crate::collections::OrderedMap::default();
    outer.insert("a".into(), Value::Int(1));
    outer.insert(
        "inner".into(),
        Value::Struct(Arc::new(StructValue::from_canonical_parts(
            "Inner".into(),
            inner,
        ))),
    );
    outer.insert(
        "arr".into(),
        Value::Array(Box::new(ArrayValue::from_canonical_parts(
            vec![Value::Int(3); 3],
            vec![(1, 3)],
        ))),
    );
    let copy = Value::Struct(Arc::new(StructValue::from_canonical_parts(
        "Outer".into(),
        outer,
    )));
    // The same shape costs 11 units: 3 structures and arrays (outer, inner, arr) and 8 fields and
    // elements (a, inner, arr, b, c and the three elements of arr).
    let (result, remaining, bytes) = copy_under_work(&copy, 11);
    assert_eq!(result, Ok(()));
    assert_eq!(remaining, 0);
    assert_eq!(bytes, 96);
}

/// Spec 34 TIME-08 copy work: one unit for each structure and each array, plus one for each of
/// its fields and elements, through every nesting level. It catches a structure that copies for
/// no work, a unit counted per leaf instead of per field, and a nested structure left out.
#[test]
fn a_structure_copy_charges_one_unit_per_structure_and_field_through_nesting() {
    // STRUCT{a: INT; b: STRUCT{c: DINT; d: BOOL}; e: ARRAY[1..3] OF INT}: structures and arrays
    // (the outer structure, b and e) make 3 units; fields and elements (a, b, e, c, d and the three
    // elements of e) make 8 units: 11 units in all.
    let mut inner = crate::collections::OrderedMap::default();
    inner.insert("c".into(), Value::DInt(2));
    inner.insert("d".into(), Value::Bool(true));
    let mut outer = crate::collections::OrderedMap::default();
    outer.insert("a".into(), Value::Int(1));
    outer.insert(
        "b".into(),
        Value::Struct(Arc::new(StructValue::from_canonical_parts(
            "Inner".into(),
            inner,
        ))),
    );
    outer.insert(
        "e".into(),
        Value::Array(Box::new(ArrayValue::from_canonical_parts(
            vec![Value::Int(3); 3],
            vec![(1, 3)],
        ))),
    );
    let value = Value::Struct(Arc::new(StructValue::from_canonical_parts(
        "Outer".into(),
        outer,
    )));
    let (result, remaining, bytes) = copy_under_work(&value, 11);
    assert_eq!(result, Ok(()));
    assert_eq!(remaining, 0);
    assert_eq!(bytes, 96);

    // With 10 units the three elements of e are refused: at used 8 (the outer structure, its three
    // fields, b and its two fields, and e), the count of its 3 elements requires 3 more.
    let (result, remaining, bytes) = copy_under_work(&value, 10);
    assert_eq!(
        result,
        Err(RuntimeError::ExecutionLimit {
            kind: ExecutionLimitKind::Work,
            used: 8,
            requested: 3,
            limit: 10,
        })
    );
    assert_eq!(remaining, 2);
    assert_eq!(bytes, 0);

    // ARRAY[1..2] OF ARRAY[1..3] OF INT: 2 arrays and 2 inner arrays of 3 elements each. Each
    // array is 1 unit plus 1 per element: 1 + 2 + 2 x (1 + 3) = 11 units.
    let grid = Value::Array(Box::new(ArrayValue::from_canonical_parts(
        vec![int_triple(), int_triple()],
        vec![(1, 2)],
    )));
    let (result, remaining, bytes) = copy_under_work(&grid, 11);
    assert_eq!(result, Ok(()));
    assert_eq!(remaining, 0);
    assert_eq!(bytes, 96);
}

/// Three INT elements in an array of bounds 1..3 (a helper for the nested-array work test).
fn int_triple() -> Value {
    Value::Array(Box::new(ArrayValue::from_canonical_parts(
        vec![Value::Int(7); 3],
        vec![(1, 3)],
    )))
}

/// Spec 34 TIME-08 copy allocation: a WSTRING copy allocates 2 bytes per UTF-16 code unit. It
/// catches a count of UTF-8 bytes (5 for this text) or of code points (2).
#[test]
fn wstring_charge_counts_utf16_code_units() {
    // "a" is one unit; U+1F600 is a surrogate pair, two units. 3 units x 2 bytes = 6 bytes.
    // The copy costs one unit of work.
    let copy = Value::WString(String::from("a\u{1F600}"));
    let (result, remaining, bytes) = copy_under_work(&copy, 1);
    assert_eq!(result, Ok(()));
    assert_eq!(remaining, 0);
    assert_eq!(bytes, 6);
}

/// Spec 34 TIME-08 copy allocation: a STRING copy allocates 1 byte per UTF-8 byte of its text. It
/// catches a count of characters (2) or of UTF-16 code units at two bytes each (4).
#[test]
fn string_charge_counts_utf8_bytes() {
    // U+00C4 takes 2 UTF-8 bytes and "b" one: 3 bytes. A STRING copy costs no work.
    let copy = Value::String("\u{00C4}b".into());
    let (result, remaining, bytes) = copy_under_work(&copy, 0);
    assert_eq!(result, Ok(()));
    assert_eq!(remaining, 0);
    assert_eq!(bytes, 3);
}
