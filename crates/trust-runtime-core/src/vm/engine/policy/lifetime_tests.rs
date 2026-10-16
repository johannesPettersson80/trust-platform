use super::*;

use crate::value::{
    ArrayValue, DateTimeValue, DateValue, Duration, EnumValue, LDateTimeValue, LDateValue,
    LTimeOfDayValue, TimeOfDayValue,
};
use crate::vm::PreparedModule;
use alloc::{boxed::Box, string::String, vec, vec::Vec};
use core::cell::Cell;

fn prepared() -> PreparedModule {
    PreparedModule::from_bytes(
        include_bytes!("../../../../../trust-runtime/tests/fixtures/portability/f401/gpio.stbc"),
        crate::vm::PreparationLimits::default(),
    )
    .unwrap()
}

fn leaf_values() -> Vec<Value> {
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
    ]
}

#[test]
fn non_recursive_values_match_the_recursive_lifetime_oracle_and_scalars_are_free() {
    let prepared = prepared();
    let services = &super::super::services::LOGICAL_ONLY;
    let mut data = EngineState::construct_data(&prepared, 0, services).unwrap();
    let state = EngineState::borrow(&mut data, &prepared, services);

    for value in leaf_values() {
        // TIME-08 LIFETIME: a scalar outer check visits nothing and charges nothing.
        state.data.resources.work_budget.reset(1);
        assert_eq!(state.check_value_lifetime(&value, None), Ok(()));
        assert_eq!(state.data.resources.work_budget.remaining(), 1);

        // The recursive node visit keeps its one unit.
        state.data.resources.work_budget.reset(1);
        assert_eq!(
            state.check_value_lifetime_inner(&value, None, 0, &mut InstanceSet::default(),),
            Ok(()),
            "recursive oracle for {value:?}"
        );
        assert_eq!(state.data.resources.work_budget.remaining(), 0);
    }
}

#[test]
fn non_recursive_work_refusal_precedes_deadline_sampling_for_visited_nodes() {
    let prepared = prepared();
    let probe = DeadlineProbe::default();
    let mut data = EngineState::construct_data(&prepared, 0, &probe).unwrap();
    probe.calls.set(0);
    let state = EngineState::borrow(&mut data, &prepared, &probe);
    let value = Value::Enum(Box::new(EnumValue::from_canonical_parts(
        "Mode".into(),
        "Run".into(),
        1,
    )));
    let expected = RuntimeError::ExecutionLimit {
        kind: crate::error::ExecutionLimitKind::Work,
        used: 0,
        requested: 1,
        limit: 0,
    };

    // TIME-08 LIFETIME: an enumeration is a scalar, so the outer check neither refuses
    // nor samples the deadline. The visiting recursive entry below still does both.
    state.data.resources.work_budget.reset(0);
    probe.expired.set(true);
    assert_eq!(state.check_value_lifetime(&value, None), Ok(()));
    assert_eq!(probe.calls.get(), 0);

    state.data.resources.work_budget.reset(0);
    assert_eq!(
        state.check_value_lifetime_inner(&value, None, 0, &mut InstanceSet::default()),
        Err(expected)
    );
    assert_eq!(probe.calls.get(), 0);
}

#[test]
fn non_recursive_deadline_stride_matches_the_recursive_oracle() {
    let prepared = prepared();
    let probe = DeadlineProbe::default();
    let mut data = EngineState::construct_data(&prepared, 0, &probe).unwrap();
    probe.calls.set(0);
    let state = EngineState::borrow(&mut data, &prepared, &probe);
    let value = Value::String("leaf".into());
    let deadline = crate::vm::VmTrap::DeadlineExceeded.into_runtime_error();

    // TIME-08 LIFETIME: the scalar outer check charges nothing, so it never reaches the stride.
    state.data.resources.work_budget.reset(64);
    assert!(!state.data.resources.work_budget.charge(31).unwrap());
    probe.expired.set(true);
    assert_eq!(state.check_value_lifetime(&value, None), Ok(()));
    assert_eq!(probe.calls.get(), 0);

    state.data.resources.work_budget.reset(64);
    assert!(!state.data.resources.work_budget.charge(31).unwrap());
    probe.calls.set(0);
    assert_eq!(
        state.check_value_lifetime_inner(&value, None, 0, &mut InstanceSet::default()),
        Err(deadline)
    );
    assert_eq!(probe.calls.get(), 1);
}

#[test]
fn recursive_stale_references_and_overdeep_values_still_use_the_full_checker() {
    let prepared = prepared();
    let services = &super::super::services::LOGICAL_ONLY;
    let mut data = EngineState::construct_data(&prepared, 0, services).unwrap();
    let state = EngineState::borrow(&mut data, &prepared, services);
    let stale_reference = Value::Reference(Some(crate::value::ValueRef {
        location: crate::memory::MemoryLocation::Local(crate::memory::FrameId(u32::MAX)),
        offset: 0,
        path: Vec::new(),
    }));

    state.data.resources.work_budget.reset(16);
    assert_eq!(
        state.check_value_lifetime(&stale_reference, None),
        Err(RuntimeError::ReferenceLifetime)
    );
    assert_eq!(state.data.resources.work_budget.remaining(), 15);

    state.data.resources.work_budget.reset(16);
    assert_eq!(
        state.check_value_lifetime_inner(&stale_reference, None, 0, &mut InstanceSet::default(),),
        Err(RuntimeError::ReferenceLifetime)
    );
    assert_eq!(state.data.resources.work_budget.remaining(), 15);

    let mut nested = Value::Null;
    for _ in 0..=crate::bytecode::BYTECODE_MAX_TYPE_NESTING {
        nested = Value::Array(Box::new(ArrayValue::from_canonical_parts(
            vec![nested],
            vec![(0, 0)],
        )));
    }
    let expected_work = crate::bytecode::BYTECODE_MAX_TYPE_NESTING + 2;

    state.data.resources.work_budget.reset(expected_work);
    assert_eq!(
        state.check_value_lifetime(&nested, None),
        Err(RuntimeError::TypeMismatch)
    );
    assert_eq!(state.data.resources.work_budget.remaining(), 0);

    state.data.resources.work_budget.reset(expected_work);
    assert_eq!(
        state.check_value_lifetime_inner(&nested, None, 0, &mut InstanceSet::default()),
        Err(RuntimeError::TypeMismatch)
    );
    assert_eq!(state.data.resources.work_budget.remaining(), 0);
}

#[test]
fn initialized_marker_lookup_consumes_no_allowance_before_visibility_read() {
    use crate::bytecode::{InitializerBodyKind, StorageOwner, StorageRole};
    use crate::memory::MemoryLocation;

    let prepared = prepared();
    let mut data =
        EngineState::construct_data(&prepared, 0, &super::super::services::LOGICAL_ONLY).unwrap();
    let (declaration, instance) = prepared
        .layout
        .entries
        .iter()
        .enumerate()
        .find_map(|(index, entry)| {
            if entry.owner != StorageOwner::Instance || entry.role != StorageRole::Variable {
                return None;
            }
            let template = entry.owner_pou_id?;
            let instance = data.storage.instances().iter().find_map(|(instance, _)| {
                (data.construction.instance_templates.get(instance) == Some(&template))
                    .then_some(*instance)
            })?;
            Some((u32::try_from(index).ok()?, instance))
        })
        .expect("fixture contains an initialized instance variable");

    let marker = (declaration, Some(instance));
    data.construction
        .initialized_declarations
        .insert(marker)
        .unwrap();
    let template = data
        .construction
        .instance_templates
        .get(&instance)
        .copied()
        .expect("the selected instance has a POU template");
    assert!(
        prepared
            .declaration(
                StorageOwner::Instance,
                Some(template),
                prepared.layout.entries[declaration as usize].slot as usize,
            )
            .is_some(),
        "the selected slot resolves to its declaration"
    );
    let marks_before = data.construction.initialized_declarations.len();

    let (initializer_id, entry) = prepared
        .initializers
        .entries
        .iter()
        .enumerate()
        .find(|(_, entry)| entry.body_kind == InitializerBodyKind::Action)
        .map(|(id, entry)| (id as u32, entry.clone()))
        .expect("fixture has an action context");
    data.construction
        .initializers
        .push(super::super::ActiveInitializer {
            id: initializer_id,
            context: 0,
            entry,
            result: crate::memory::FrameId(u32::MAX),
            lexical_frame: None,
            instance: Some(instance),
            depth: 0,
            writable_instances: super::super::InstanceSet::default(),
        });

    let state = EngineState::borrow(&mut data, &prepared, &super::super::services::LOGICAL_ONLY);
    // TIME-08: declaration and marker lookups are bookkeeping, so an empty allowance
    // still admits the visible, initialized instance variable.
    state.data.resources.work_budget.reset(0);
    assert_eq!(
        state.check_read(
            crate::value::ValueRef {
                location: MemoryLocation::Instance(instance),
                offset: prepared.layout.entries[declaration as usize].slot as usize,
                path: Vec::new(),
            }
            .as_view()
        ),
        Ok(())
    );
    assert_eq!(state.data.resources.work_budget.remaining(), 0);
    assert_eq!(
        state.data.construction.initialized_declarations.len(),
        marks_before
    );
    assert!(state
        .data
        .construction
        .initialized_declarations
        .contains(&marker)
        .unwrap());
}

#[test]
fn primitive_byte_array_lifetime_check_consumes_no_work() {
    let prepared = prepared();
    let services = &super::super::services::LOGICAL_ONLY;
    let mut data = EngineState::construct_data(&prepared, 0, services).unwrap();
    let state = EngineState::borrow(&mut data, &prepared, services);
    let value = Value::Array(Box::new(ArrayValue::from_canonical_parts(
        vec![Value::Byte(7); 1_000],
        vec![(0, 999)],
    )));

    // TIME-08 LIFETIME: arrays are homogeneous, so a primitive first element skips traversal.
    // An empty allowance would refuse any charge, so success proves none was made.
    state.data.resources.work_budget.reset(0);
    assert_eq!(state.check_value_lifetime(&value, None), Ok(()));
    assert_eq!(state.data.resources.work_budget.remaining(), 0);
}

#[test]
fn array_with_a_leading_reference_is_traversed_and_refuses_a_dead_later_reference() {
    let prepared = prepared();
    let services = &super::super::services::LOGICAL_ONLY;
    let mut data = EngineState::construct_data(&prepared, 0, services).unwrap();
    let state = EngineState::borrow(&mut data, &prepared, services);
    let dead_reference = Value::Reference(Some(crate::value::ValueRef {
        location: crate::memory::MemoryLocation::Local(crate::memory::FrameId(u32::MAX)),
        offset: 0,
        path: Vec::new(),
    }));
    let value = Value::Array(Box::new(ArrayValue::from_canonical_parts(
        vec![Value::Reference(None), dead_reference],
        vec![(0, 1)],
    )));

    // A leading Reference(None) is not a primitive scalar, so the array is traversed:
    // the array node (1) and both elements (1 each) are visited before the dead one refuses.
    state.data.resources.work_budget.reset(16);
    assert_eq!(
        state.check_value_lifetime(&value, None),
        Err(RuntimeError::ReferenceLifetime)
    );
    assert_eq!(state.data.resources.work_budget.remaining(), 13);
}

#[derive(Default)]
struct DeadlineProbe {
    calls: Cell<usize>,
    expired: Cell<bool>,
}

impl super::super::services::ExecutionServices for DeadlineProbe {
    fn deadline_exceeded(&self) -> bool {
        self.calls.set(self.calls.get() + 1);
        self.expired.get()
    }

    fn has_wall_clock(&self) -> bool {
        false
    }

    fn current_dt(&self) -> Result<DateTimeValue, RuntimeError> {
        Err(RuntimeError::TypeMismatch)
    }
}

#[cfg(all(feature = "std", feature = "test-support"))]
#[test]
fn lifetime_work_records_distinguish_root_callers_without_changing_refusal() {
    use crate::vm::diagnostic_timing::{
        finish_callsite_accounting_log, start_callsite_accounting_log,
    };

    let prepared = prepared();
    let mut data =
        EngineState::construct_data(&prepared, 0, &super::super::services::LOGICAL_ONLY).unwrap();
    let state = EngineState::borrow(&mut data, &prepared, &super::super::services::LOGICAL_ONLY);
    // Reference elements are not primitive scalars, so the array is traversed: one unit for the
    // array node and one per element, as the recorded counts below expect (TIME-08 LIFETIME).
    let value = Value::Array(Box::new(ArrayValue::from_canonical_parts(
        vec![Value::Reference(None), Value::Reference(None)],
        vec![(0, 1)],
    )));

    state.data.resources.work_budget.reset(3);
    start_callsite_accounting_log();
    let admitted = state.check_value_lifetime(&value, None);
    assert_eq!(admitted, Ok(()));
    assert_eq!(state.data.resources.work_budget.remaining(), 0);

    state.data.resources.work_budget.reset(2);
    let refused = state.check_value_lifetime(&value, None);
    assert_eq!(
        refused,
        Err(RuntimeError::ExecutionLimit {
            kind: crate::error::ExecutionLimitKind::Work,
            used: 2,
            requested: 1,
            limit: 2,
        })
    );
    assert_eq!(state.data.resources.work_budget.remaining(), 0);
    let snapshot = finish_callsite_accounting_log();
    assert!(snapshot.is_valid());
    assert_eq!(snapshot.overflow_count, 0);
    assert_eq!(snapshot.recorder_busy, 0);
    let records = &snapshot.work[..snapshot.work_len];
    assert_eq!(
        records.len(),
        2,
        "recursive visits retain their respective root caller"
    );
    assert!(records.iter().all(|record| record.file == file!()));
    assert_ne!(records[0].line, records[1].line);
    let admitted_record = records
        .iter()
        .find(|record| record.work_refused == 0)
        .unwrap();
    let refused_record = records
        .iter()
        .find(|record| record.work_refused == 1)
        .unwrap();
    assert_eq!(
        (
            admitted_record.calls,
            admitted_record.requested_units,
            admitted_record.consumed_units,
            admitted_record.charged
        ),
        (3, 3, 3, 3)
    );
    assert_eq!(
        (
            refused_record.calls,
            refused_record.requested_units,
            refused_record.consumed_units,
            refused_record.charged
        ),
        (3, 3, 2, 2)
    );
    assert_eq!(
        records
            .iter()
            .map(|record| record.consumed_units)
            .sum::<u64>(),
        5
    );

    // The unlogged recursive entry still has the same exact allowance and fault.
    state.data.resources.work_budget.reset(2);
    assert_eq!(
        state.check_value_lifetime_inner(&value, None, 0, &mut InstanceSet::default()),
        refused
    );
    assert_eq!(state.data.resources.work_budget.remaining(), 0);
    assert_eq!(
        value,
        Value::Array(Box::new(ArrayValue::from_canonical_parts(
            vec![Value::Reference(None), Value::Reference(None)],
            vec![(0, 1)],
        )))
    );
}
