//! TIME-08 copy work: array nesting, reference paths, WSTRING, enumerations and shared structures.
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
    // Scalars and a shared structure cost nothing; a WSTRING or enumeration costs one unit;
    // a reference costs 1 plus 1 per path segment (2) and per index (2) = 5; an array costs
    // 1 + 2 elements, and its WSTRING element costs one more.
    let cases = [
        (Value::Bool(true), 0),
        (Value::DInt(-3), 0),
        (Value::Real(1.5), 0),
        (Value::Time(Duration::ZERO), 0),
        (Value::WChar(u16::from(b'x')), 0),
        (structure, 0),
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
fn copying_a_shared_structure_costs_no_work_whatever_its_fields_hold() {
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

    // TIME-08: a shared structure is one Arc clone. Its fields are charged only when a
    // write detaches them (path_write work_allowance tests).
    let (result, remaining, bytes) = copy_under_work(&copy, 0);
    assert_eq!(result, Ok(()));
    assert_eq!(remaining, 0);
    assert_eq!(bytes, core::mem::size_of::<Value>());
}
