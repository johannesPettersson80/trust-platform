//! Allocation refusal during a retained snapshot apply restores every retained value,
//! output, memory cell and time base (spec 10 §4.8; spec 34 RETAIN-05 and §4.3.1).
use super::*;
use crate::bytecode::{StorageOwner, StorageRole, TypeData};
use crate::memory::InstanceId;
use crate::retain::{RestartMode, RetainSnapshot};
use crate::value::{ArrayValue, Duration};
use alloc::{boxed::Box, vec};

const PROGRAM: &[u8] =
    include_bytes!("../../../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc");

/// Program-v2 with `enabled` and `result` marked retained beside the source's retained
/// `activations`. `result` is retyped to the fixture's `ARRAY[1..3] OF INT`, so the
/// snapshot carries an aggregate whose elements are migrated one by one.
fn program_fixture() -> PreparedModule {
    let mut prepared =
        PreparedModule::from_bytes(PROGRAM, crate::vm::PreparationLimits::default()).unwrap();
    let history = prepared
        .vm
        .types
        .entries
        .iter()
        .position(|entry| {
            matches!(
                &entry.data,
                TypeData::Array { dims, .. } if dims.len() == 1 && dims[0] == (1, 3)
            )
        })
        .expect("program-v2 declares ARRAY[1..3] OF INT") as u32;
    let enabled = program_declaration(&prepared, "enabled");
    prepared.layout.entries[enabled].retain = 1;
    let result = program_declaration(&prepared, "result");
    prepared.layout.entries[result].retain = 1;
    prepared.layout.entries[result].type_id = Some(history);
    prepared
}

fn program_declaration(prepared: &PreparedModule, name: &str) -> usize {
    prepared
        .layout
        .entries
        .iter()
        .position(|entry| {
            entry.owner == StorageOwner::Instance
                && entry.role == StorageRole::Variable
                && prepared.vm.strings[entry.name_idx as usize].eq_ignore_ascii_case(name)
        })
        .expect("program-v2 declares the variable")
}

fn retained_snapshot() -> RetainSnapshot {
    let mut snapshot = RetainSnapshot::default();
    // A narrower integer is admitted into the DINT destination and reported as migrated.
    snapshot.insert("@program/Plant/activations", Value::Int(7));
    snapshot.insert("@program/Plant/enabled", Value::Bool(false));
    snapshot.insert(
        "@program/Plant/result",
        Value::Array(Box::new(ArrayValue::from_canonical_parts(
            vec![Value::Int(4), Value::Int(5), Value::Int(6)],
            vec![(1, 3)],
        ))),
    );
    snapshot
}

fn fresh(prepared: &PreparedModule) -> Box<EngineData> {
    EngineState::construct_data(prepared, 0, &services::LOGICAL_ONLY).unwrap()
}

/// A constructed owner whose engineering counters start as `engineering_operation`
/// leaves them, with `used` allocation bytes already charged.
fn armed(prepared: &PreparedModule, used: usize) -> Box<EngineData> {
    let mut data = fresh(prepared);
    data.resources.work_budget.reset(prepared.limits.max_work);
    data.resources.constructed_values = 0;
    data.resources.construction_bytes.set(used);
    data
}

/// Allocation bytes that one apply charges, measured from an empty counter.
fn charged_bytes(prepared: &PreparedModule) -> usize {
    let mut data = armed(prepared, 0);
    let mut state = EngineState::borrow(&mut data, prepared, &services::LOGICAL_ONLY);
    state
        .apply_retained_snapshot(&retained_snapshot())
        .expect("an unbudgeted apply admits the snapshot");
    state.data.resources.construction_bytes.get()
}

#[derive(Debug, PartialEq)]
struct Observed {
    activations: Option<Value>,
    enabled: Option<Value>,
    result: Option<Value>,
    outputs: Vec<u8>,
    memory: Vec<u8>,
    now: Duration,
    faulted: bool,
}

fn plant_instance(data: &EngineData) -> InstanceId {
    match data.storage.get_global("Plant") {
        Some(Value::Instance(instance)) => *instance,
        other => panic!("program root Plant: {other:?}"),
    }
}

fn observe(data: &EngineData) -> Observed {
    let plant = plant_instance(data);
    Observed {
        activations: data.storage.get_instance_var(plant, "activations").cloned(),
        enabled: data.storage.get_instance_var(plant, "enabled").cloned(),
        result: data.storage.get_instance_var(plant, "result").cloned(),
        outputs: data.images.outputs.clone(),
        memory: data.images.memory.clone(),
        now: data.now,
        faulted: data.fault.is_some(),
    }
}

#[test]
fn retained_snapshot_refusal_at_every_allocation_allowance_restores_the_owner() {
    let prepared = program_fixture();
    let limit = prepared.limits.max_construction_bytes;
    let required = charged_bytes(&prepared);
    assert!(required > 0, "the snapshot must charge allocation");
    for allowance in 0..required {
        let mut data = armed(&prepared, limit - allowance);
        let before = observe(&data);
        let mut state = EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY);
        let result = state.apply_retained_snapshot(&retained_snapshot());
        assert!(
            matches!(
                result,
                Err(RuntimeError::ExecutionLimit {
                    kind: crate::error::ExecutionLimitKind::AllocationBytes,
                    limit: observed,
                    ..
                }) if observed == limit
            ),
            "allowance {allowance} must refuse with the exact AllocationBytes limit: {result:?}"
        );
        assert_eq!(
            observe(&*state.data),
            before,
            "allowance {allowance} changed retained state"
        );
    }
}

#[test]
fn retained_snapshot_admits_exactly_its_charged_allowance_and_migrates_each_entry() {
    let prepared = program_fixture();
    let limit = prepared.limits.max_construction_bytes;
    let required = charged_bytes(&prepared);
    let mut data = armed(&prepared, limit - required);
    let before = observe(&data);
    let mut state = EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY);
    let events = state
        .apply_retained_snapshot(&retained_snapshot())
        .expect("the measured allowance admits the snapshot");
    let after = observe(&*state.data);
    assert!(
        events.contains(&RetainApplyEvent::Migrated {
            name: "@program/Plant/activations".into(),
            before: Value::Int(7),
            after: Value::DInt(7),
        }),
        "the narrower integer must be reported as migrated: {events:?}"
    );
    assert_eq!(after.activations, Some(Value::DInt(7)));
    assert_eq!(after.enabled, Some(Value::Bool(false)));
    assert_eq!(
        after.result,
        Some(Value::Array(Box::new(ArrayValue::from_canonical_parts(
            vec![Value::Int(4), Value::Int(5), Value::Int(6)],
            vec![(1, 3)],
        ))))
    );
    assert_eq!(after.outputs, before.outputs);
    assert_eq!(after.memory, before.memory);
    assert_eq!(after.now, before.now);
    assert_eq!(after.faulted, before.faulted);
}

#[test]
fn warm_restart_program_values_refusal_at_every_allocation_point_preserves_the_owner() {
    // Live values differ from the candidate's defaults, so an admitted warm restart
    // visibly restores the retained program-member scalar and the aggregate.
    let live_bytes = fresh(&program_fixture()).resources.construction_bytes.get();
    let restored_result = || {
        Value::Array(Box::new(ArrayValue::from_canonical_parts(
            vec![Value::Int(7), Value::Int(8), Value::Int(9)],
            vec![(1, 3)],
        )))
    };
    let mut refusals = 0;
    let mut limit = live_bytes;
    loop {
        let mut prepared = program_fixture();
        prepared.limits.max_construction_bytes = limit;
        let mut data = fresh(&prepared);
        let plant = plant_instance(&data);
        assert!(data
            .storage
            .set_instance_var(plant, "activations", Value::DInt(5)));
        assert!(data
            .storage
            .set_instance_var(plant, "result", restored_result()));
        let before = observe(&data);
        let mut state = EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY);
        match state.restart(RestartMode::Warm) {
            Ok(()) => {
                let restored = observe(&*state.data);
                assert_eq!(
                    restored.activations,
                    Some(Value::DInt(5)),
                    "limit {limit} admitted a restart that dropped the retained scalar"
                );
                assert_eq!(
                    restored.result,
                    Some(restored_result()),
                    "limit {limit} admitted a restart that dropped the retained aggregate"
                );
                break;
            }
            Err(error) => {
                assert!(
                    matches!(
                        error,
                        RuntimeError::ExecutionLimit {
                            kind: crate::error::ExecutionLimitKind::AllocationBytes,
                            limit: observed,
                            ..
                        } if observed == limit
                    ),
                    "limit {limit} must refuse with the exact AllocationBytes limit: {error:?}"
                );
                assert_eq!(
                    observe(&*state.data),
                    before,
                    "limit {limit} changed the live owner"
                );
                refusals += 1;
            }
        }
        limit += 1;
        assert!(
            limit < live_bytes + 65_536,
            "the warm restart was never admitted"
        );
    }
    assert!(refusals > 0, "the sweep must observe refused restarts");
}
