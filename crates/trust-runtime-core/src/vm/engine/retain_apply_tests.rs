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
/// snapshot carries an aggregate whose elements are migrated one by one. The module is changed
/// before preparation, which lays the variables out at their declared types (spec 34 MEM-12):
/// `result` is retyped wherever the module types it (its declaration, the variable metadata of
/// the references to it and of its initializers' results, and its default initializer), and each
/// construction demand is the validator's own.
fn program_fixture() -> PreparedModule {
    use crate::bytecode::{construction_demands, BytecodeModule, SectionData, SectionId};
    let mut module = BytecodeModule::decode(PROGRAM).unwrap();
    let Some(SectionData::TypeTable(types)) = module.section(SectionId::TypeTable) else {
        panic!("type table")
    };
    let history = types
        .entries
        .iter()
        .position(|entry| {
            matches!(
                &entry.data,
                TypeData::Array { dims, .. } if dims.len() == 1 && dims[0] == (1, 3)
            )
        })
        .expect("program-v2 declares ARRAY[1..3] OF INT") as u32;
    let Some(SectionData::StringTable(strings)) = module.section(SectionId::StringTable) else {
        panic!("string table")
    };
    let names = strings.entries.clone();
    let Some(SectionData::StorageLayout(layout)) = module.section_mut(SectionId::StorageLayout)
    else {
        panic!("storage layout")
    };
    let mut result = None;
    for (index, entry) in layout.entries.iter_mut().enumerate() {
        if entry.owner != StorageOwner::Instance || entry.role != StorageRole::Variable {
            continue;
        }
        let name = &names[entry.name_idx as usize];
        if name.eq_ignore_ascii_case("enabled") {
            entry.retain = 1;
        } else if name.eq_ignore_ascii_case("result") {
            // An array takes its type's default; the source's INT default constant no longer fits.
            entry.retain = 1;
            entry.type_id = Some(history);
            entry.default_const_idx = None;
            result = Some((index as u32, entry.owner_pou_id, entry.slot));
        }
    }
    // The variable metadata types each reference that names `result` and each result of its
    // initializers, and validation compares those types with the declaration's (specification 12
    // §6.4), so they are retyped with it. An instance reference names its instance by the owner a
    // construction root gives the program's instance.
    let (result, template, slot) = result.expect("program-v2 declares result");
    let Some(SectionData::ConstructionRoots(roots)) = module.section(SectionId::ConstructionRoots)
    else {
        panic!("construction roots")
    };
    let owners: Vec<u32> = roots
        .entries
        .iter()
        .filter(|root| root.template_pou_id == template)
        .filter_map(|root| root.instance_owner_id)
        .collect();
    let Some(SectionData::RefTable(refs)) = module.section(SectionId::RefTable) else {
        panic!("reference table")
    };
    let mut retyped: Vec<u32> = refs
        .entries
        .iter()
        .zip(0u32..)
        .filter(|(reference, _)| {
            reference.location == crate::bytecode::RefLocation::Instance
                && owners.contains(&reference.owner_id)
                && reference.offset == slot
                && reference.segments.is_empty()
        })
        .map(|(_, id)| id)
        .collect();
    let Some(SectionData::Initializers(initializers)) = module.section(SectionId::Initializers)
    else {
        panic!("initializers")
    };
    retyped.extend(
        initializers
            .entries
            .iter()
            .filter(|entry| entry.declaration_idx == Some(result))
            .map(|entry| entry.result_ref_idx),
    );
    // The default initializer of `result` constructs the default of the type its `DEFAULT_TYPED`
    // names (specification 12, the opcode 0x66 and its TYPE_TABLE operand), the array's now.
    let default = initializers
        .entries
        .iter()
        .find(|entry| {
            entry.declaration_idx == Some(result)
                && entry.stage == crate::bytecode::InitializationStage::Default
        })
        .expect("result has a default initializer")
        .code_offset as usize;
    let Some(SectionData::PouBodies(bodies)) = module.section_mut(SectionId::PouBodies) else {
        panic!("code")
    };
    assert_eq!(
        bodies[default], 0x66,
        "the default initializer opens with DEFAULT_TYPED"
    );
    bodies[default + 1..default + 5].copy_from_slice(&history.to_le_bytes());
    let Some(SectionData::VarMeta(meta)) = module.section_mut(SectionId::VarMeta) else {
        panic!("variable metadata")
    };
    for entry in &mut meta.entries {
        if retyped.contains(&entry.ref_idx) {
            entry.type_id = history;
        }
    }
    let demands =
        construction_demands(module.view(), crate::bytecode::ValidationLimits::default()).unwrap();
    let Some(SectionData::StorageLayout(layout)) = module.section_mut(SectionId::StorageLayout)
    else {
        panic!("storage layout")
    };
    for (entry, demand) in layout.entries.iter_mut().zip(demands) {
        entry.construction_nodes = demand;
    }
    module
        .validate()
        .expect("the retyped program is a valid module");
    PreparedModule::from_bytes(
        &module.encode().unwrap(),
        crate::vm::PreparationLimits::default(),
    )
    .unwrap()
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

fn plant_instance(state: &EngineState<'_>) -> InstanceId {
    crate::vm::engine::test_store::global_instance(state, "Plant")
}

fn observe(state: &EngineState<'_>) -> Observed {
    let plant = plant_instance(state);
    Observed {
        activations: crate::vm::engine::test_store::instance_var(state, plant, "activations"),
        enabled: crate::vm::engine::test_store::instance_var(state, plant, "enabled"),
        result: crate::vm::engine::test_store::instance_var(state, plant, "result"),
        outputs: state.data.images.outputs.clone(),
        memory: state.data.images.memory.clone(),
        now: state.data.now,
        faulted: state.data.fault.is_some(),
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
        let before = observe(&EngineState::borrow(
            &mut data,
            &prepared,
            &services::LOGICAL_ONLY,
        ));
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
            observe(&state),
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
    let before = observe(&EngineState::borrow(
        &mut data,
        &prepared,
        &services::LOGICAL_ONLY,
    ));
    let mut state = EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY);
    let events = state
        .apply_retained_snapshot(&retained_snapshot())
        .expect("the measured allowance admits the snapshot");
    let after = observe(&state);
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
        let mut state = EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY);
        let plant = plant_instance(&state);
        assert!(crate::memory::view::StoreMutate::write_instance_var(
            &mut state,
            plant,
            "activations",
            Value::DInt(5)
        ));
        assert!(crate::memory::view::StoreMutate::write_instance_var(
            &mut state,
            plant,
            "result",
            restored_result()
        ));
        let before = observe(&state);
        match state.restart(RestartMode::Warm) {
            Ok(()) => {
                let restored = observe(&state);
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
                    observe(&state),
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
