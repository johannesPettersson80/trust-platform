//! TIME-08 lifetime and access checks: a struct or instance visit costs one unit per node,
//! while writability and staging checks draw no work.
use super::*;

use crate::bytecode::InitializerBodyKind;
use crate::collections::OrderedMap;
use crate::error::ExecutionLimitKind;
use crate::memory::FrameId;
use crate::value::StructValue;
use crate::vm::PreparedModule;
use alloc::sync::Arc;

fn prepared() -> PreparedModule {
    PreparedModule::from_bytes(
        include_bytes!("../../../../../trust-runtime/tests/fixtures/portability/f401/gpio.stbc"),
        crate::vm::PreparationLimits::default(),
    )
    .unwrap()
}

#[test]
fn struct_visit_costs_one_unit_for_the_node_and_one_for_each_field() {
    let prepared = prepared();
    let services = &super::super::services::LOGICAL_ONLY;
    let mut data = EngineState::construct_data(&prepared, 0, services).unwrap();
    let state = EngineState::borrow(&mut data, &prepared, services);
    let mut fields = OrderedMap::default();
    fields.insert("link".into(), Value::Reference(None));
    fields.insert("count".into(), Value::Int(1));
    let value = Value::Struct(Arc::new(StructValue::from_canonical_parts(
        "Record".into(),
        fields,
    )));

    // TIME-08 LIFETIME: the struct node and each of its two field nodes cost 1 unit.
    state.data.resources.work_budget.reset(3);
    assert_eq!(state.check_value_lifetime(&value, None), Ok(()));
    assert_eq!(state.data.resources.work_budget.remaining(), 0);

    // With 2 units, the second field node is refused; the refused charge is not taken.
    state.data.resources.work_budget.reset(2);
    assert_eq!(
        state.check_value_lifetime(&value, None),
        Err(RuntimeError::ExecutionLimit {
            kind: ExecutionLimitKind::Work,
            used: 2,
            requested: 1,
            limit: 2,
        })
    );
    assert_eq!(state.data.resources.work_budget.remaining(), 0);
}

#[test]
fn instance_visit_costs_one_unit_for_the_node_and_one_for_each_variable() {
    let prepared = prepared();
    let services = &super::super::services::LOGICAL_ONLY;
    let mut data = EngineState::construct_data(&prepared, 0, services).unwrap();
    let instance = data.storage.create_instance("Probe");
    assert!(data
        .storage
        .set_instance_var(instance, "count", Value::Int(1)));
    assert!(data
        .storage
        .set_instance_var(instance, "link", Value::Reference(None)));
    data.resources.construction_bytes.set(0);
    let state = EngineState::borrow(&mut data, &prepared, services);
    let value = Value::Instance(instance);

    // TIME-08 LIFETIME: the instance node plus its two variables cost 3 units. The visited-set
    // growth is allocation, not work.
    state.data.resources.work_budget.reset(3);
    assert_eq!(state.check_value_lifetime(&value, None), Ok(()));
    assert_eq!(state.data.resources.work_budget.remaining(), 0);

    state.data.resources.work_budget.reset(2);
    assert_eq!(
        state.check_value_lifetime(&value, None),
        Err(RuntimeError::ExecutionLimit {
            kind: ExecutionLimitKind::Work,
            used: 2,
            requested: 1,
            limit: 2,
        })
    );
    assert_eq!(state.data.resources.work_budget.remaining(), 0);
}

#[test]
fn writability_check_of_a_scalar_global_draws_no_work() {
    let prepared = prepared();
    let services = &super::super::services::LOGICAL_ONLY;
    let mut data = EngineState::construct_data(&prepared, 0, services).unwrap();
    let reference = data.storage.ref_for_global("probe_depth").unwrap();
    let state = EngineState::borrow(&mut data, &prepared, services);

    // TIME-08: writability, visibility and staging are bookkeeping. An empty allowance admits
    // a scalar write to a writable global.
    state.data.resources.work_budget.reset(0);
    assert_eq!(
        state.check_write(reference.as_view(), &Value::Int(2)),
        Ok(())
    );
    assert_eq!(state.data.resources.work_budget.remaining(), 0);
}

#[test]
fn staging_refusal_of_a_global_write_draws_no_work() {
    let prepared = prepared();
    let services = &super::super::services::LOGICAL_ONLY;
    let mut data = EngineState::construct_data(&prepared, 0, services).unwrap();
    let reference = data.storage.ref_for_global("probe_depth").unwrap();
    let before = data.storage.get_global("probe_depth").cloned();
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
            result: FrameId(u32::MAX),
            lexical_frame: None,
            instance: None,
            depth: 0,
            writable_instances: super::super::InstanceSet::default(),
        });
    let state = EngineState::borrow(&mut data, &prepared, services);

    // An initializer may stage only its own result and instances. A global is not staged,
    // so the write is refused before any lifetime traversal, and it draws no work.
    state.data.resources.work_budget.reset(0);
    assert_eq!(
        state.check_write(reference.as_view(), &Value::Int(3)),
        Err(RuntimeError::StagingViolation)
    );
    assert_eq!(state.data.resources.work_budget.remaining(), 0);
    assert_eq!(
        state.data.storage.get_global("probe_depth").cloned(),
        before
    );
}
