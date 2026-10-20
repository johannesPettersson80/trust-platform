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
    // The cell fixture's `base_g : Base` holds `base_v : DINT`, `series_v : ARRAY[1..3] OF INT`
    // and `pair_v : Pair`, a structure of the INTs `x` and `y`. TIME-08 LIFETIME charges one unit
    // per node visited: the instance 1, `base_v` 1, the array of primitives 1 (its elements hold
    // nothing to visit) and the structure 1 plus its two fields, 6 units in all.
    let prepared = crate::memory::cell_access::fixture::prepared();
    let services = &super::super::services::LOGICAL_ONLY;
    let mut data = EngineState::construct_data(&prepared, 0, services).unwrap();
    data.resources.construction_bytes.set(0);
    let state = EngineState::borrow(&mut data, &prepared, services);
    let value = Value::Instance(crate::vm::engine::test_store::global_instance(
        &state, "base_g",
    ));

    // The visited-set growth is allocation, not work.
    state.data.resources.work_budget.reset(6);
    assert_eq!(state.check_value_lifetime(&value, None), Ok(()));
    assert_eq!(state.data.resources.work_budget.remaining(), 0);

    state.data.resources.work_budget.reset(5);
    assert_eq!(
        state.check_value_lifetime(&value, None),
        Err(RuntimeError::ExecutionLimit {
            kind: ExecutionLimitKind::Work,
            used: 5,
            requested: 1,
            limit: 5,
        })
    );
    assert_eq!(state.data.resources.work_budget.remaining(), 0);
}

#[test]
fn writability_check_of_a_scalar_global_draws_no_work() {
    let prepared = prepared();
    let services = &super::super::services::LOGICAL_ONLY;
    let mut data = EngineState::construct_data(&prepared, 0, services).unwrap();
    let state = EngineState::borrow(&mut data, &prepared, services);
    let reference =
        crate::memory::view::StoreAccess::ref_for_global(&state, "probe_depth").unwrap();

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
    let (reference, before) = {
        let state = EngineState::borrow(&mut data, &prepared, services);
        (
            crate::memory::view::StoreAccess::ref_for_global(&state, "probe_depth").unwrap(),
            crate::vm::engine::test_store::global(&state, "probe_depth"),
        )
    };
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
        crate::vm::engine::test_store::global(&state, "probe_depth"),
        before
    );
}
