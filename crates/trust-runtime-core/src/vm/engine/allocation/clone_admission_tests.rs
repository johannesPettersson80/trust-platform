//! Top-level copy admission must match the recursive estimator's accounting.
use super::*;
use crate::value::{
    ArrayValue, DateTimeValue, DateValue, Duration, EnumValue, LDateTimeValue, LDateValue,
    LTimeOfDayValue, RefSegment, StructValue, TimeOfDayValue, ValueRef,
};
use crate::vm::context::ReferenceContext;
use alloc::{boxed::Box, string::String, sync::Arc, vec, vec::Vec};

fn values() -> Vec<Value> {
    vec![
        Value::Bool(true),
        Value::SInt(-1),
        Value::Int(-2),
        Value::DInt(-3),
        Value::LInt(-4),
        Value::USInt(1),
        Value::UInt(2),
        Value::UDInt(3),
        Value::ULInt(4),
        Value::Real(1.5),
        Value::LReal(2.5),
        Value::Byte(1),
        Value::Word(2),
        Value::DWord(3),
        Value::LWord(4),
        Value::Time(Duration::ZERO),
        Value::LTime(Duration::ZERO),
        Value::Date(DateValue::new(0)),
        Value::LDate(LDateValue::new(0)),
        Value::Tod(TimeOfDayValue::new(0)),
        Value::LTod(LTimeOfDayValue::new(0)),
        Value::Dt(DateTimeValue::new(0)),
        Value::Ldt(LDateTimeValue::new(0)),
        Value::String("text".into()),
        Value::WString(String::from("wide text")),
        Value::Char(b'x'),
        Value::WChar(u16::from(b'x')),
        Value::Enum(Box::new(EnumValue::from_canonical_parts(
            "Mode".into(),
            "Run".into(),
            1,
        ))),
        Value::Reference(None),
        Value::Null,
        Value::String("long string with a shared heap allocation beyond inline capacity".into()),
        Value::Instance(crate::memory::InstanceId(7)),
        Value::Struct(Arc::new(StructValue::from_canonical_parts(
            "Record".into(),
            crate::collections::OrderedMap::default(),
        ))),
        Value::Reference(Some(ValueRef {
            location: crate::memory::MemoryLocation::Global,
            offset: 0,
            path: vec![RefSegment::Field("x".into()), RefSegment::Index(vec![1, 2])],
        })),
        Value::Array(Box::new(ArrayValue::from_canonical_parts(
            vec![Value::Int(7), Value::WString(String::from("owned"))],
            vec![(0, 1)],
        ))),
    ]
}

#[test]
fn top_level_admission_matches_estimator_for_every_variant_and_refusal_boundary() {
    let mut prepared = PreparedModule::from_bytes(
        include_bytes!("../../../../../trust-runtime/tests/fixtures/portability/f401/gpio.stbc"),
        crate::vm::PreparationLimits::default(),
    )
    .unwrap();
    let mut data = EngineState::construct_data(&prepared, 0, &services::LOGICAL_ONLY).unwrap();
    let size = core::mem::size_of::<Value>();
    for limit in [0, size - 1, size, 8192] {
        prepared.limits.max_construction_bytes = limit;
        let state = EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY);
        for value in values() {
            for work in [0, 1, 2, 64] {
                state.data.resources.work_budget.reset(work);
                state.data.resources.construction_bytes.set(0);
                let expected = state
                    .value_clone_charge(&value, 0)
                    .and_then(|bytes| state.charge_allocation_bytes(bytes));
                let expected_work = state.data.resources.work_budget.remaining();
                let expected_bytes = state.data.resources.construction_bytes.get();
                state.data.resources.work_budget.reset(work);
                state.data.resources.construction_bytes.set(0);
                assert_eq!(
                    state.before_value_clone(&value),
                    expected,
                    "variant {value:?}, work {work}, allocation limit {limit}"
                );
                assert_eq!(state.data.resources.work_budget.remaining(), expected_work);
                assert_eq!(
                    state.data.resources.construction_bytes.get(),
                    expected_bytes
                );
            }
        }
    }
}

#[test]
fn top_level_owned_copy_retains_recursive_depth_refusal_before_allocation() {
    let prepared = PreparedModule::from_bytes(
        include_bytes!("../../../../../trust-runtime/tests/fixtures/portability/f401/gpio.stbc"),
        crate::vm::PreparationLimits::default(),
    )
    .unwrap();
    let mut data = EngineState::construct_data(&prepared, 0, &services::LOGICAL_ONLY).unwrap();
    let state = EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY);
    let mut nested = Value::Null;
    for _ in 0..=crate::bytecode::BYTECODE_MAX_TYPE_NESTING {
        nested = Value::Array(Box::new(ArrayValue::from_canonical_parts(
            vec![nested],
            vec![(0, 0)],
        )));
    }
    // TIME-08 COPY: each of the MAX+1 arrays costs 1 (the copy) + 1 (its one element);
    // the depth refusal at the innermost Null follows those charges.
    let work = 2 * (crate::bytecode::BYTECODE_MAX_TYPE_NESTING + 1);
    state.data.resources.work_budget.reset(work);
    state.data.resources.construction_bytes.set(0);
    assert_eq!(
        state.before_value_clone(&nested),
        Err(RuntimeError::TypeMismatch)
    );
    assert_eq!(state.data.resources.work_budget.remaining(), 0);
    assert_eq!(state.data.resources.construction_bytes.get(), 0);
}

#[cfg(all(feature = "std", feature = "test-support"))]
#[test]
fn actual_copy_admission_records_caller_and_refused_charge_stage() {
    use crate::vm::diagnostic_timing::{
        finish_callsite_accounting_log, start_callsite_accounting_log,
    };
    let mut prepared = PreparedModule::from_bytes(
        include_bytes!("../../../../../trust-runtime/tests/fixtures/portability/f401/gpio.stbc"),
        crate::vm::PreparationLimits::default(),
    )
    .unwrap();
    let mut data = EngineState::construct_data(&prepared, 0, &services::LOGICAL_ONLY).unwrap();
    let size = core::mem::size_of::<Value>();
    for (work, limit, estimated, admitted) in
        [(0, size, 0, 0), (1, 0, size, 0), (1, size, size, size)]
    {
        prepared.limits.max_construction_bytes = limit;
        let state = EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY);
        state.data.resources.work_budget.reset(work);
        state.data.resources.construction_bytes.set(0);
        // An empty WSTRING has one COPY unit and exactly one Value of estimated bytes.
        start_callsite_accounting_log();
        let caller_line = line!() + 1;
        let result = state.before_value_clone(&Value::WString(String::new()));
        let log = finish_callsite_accounting_log();
        assert!(log.is_valid());
        assert_eq!(log.copy_len, 1);
        let row = log.copies[0];
        assert_eq!(row.file, file!());
        assert_eq!(row.line, caller_line);
        assert_eq!(row.calls, 1);
        assert_eq!(row.estimated_bytes, estimated as u64);
        assert_eq!(row.admitted_bytes, admitted as u64);
        assert_eq!(row.work_refused, u64::from(work == 0));
        assert_eq!(row.allocation_refused, u64::from(work > 0 && limit == 0));
        assert_eq!(row.admitted, u64::from(result.is_ok()));
        assert_eq!(result.is_ok(), admitted != 0);
    }
}
