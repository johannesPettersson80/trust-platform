//! TIME-08 / TIME-09: an instance node takes one unit before its member default preflight.
use super::*;
use crate::bytecode::StorageOwner;
use crate::error::ExecutionLimitKind;
use crate::vm::PreparationLimits;

fn fixture() -> PreparedModule {
    PreparedModule::from_bytes(
        include_bytes!(
            "../../../../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc"
        ),
        PreparationLimits::default(),
    )
    .unwrap()
}

#[test]
fn instance_node_and_first_member_preflight_each_take_one_unit() {
    let prepared = fixture();
    let mut data = EngineState::construct_data(&prepared, 0, &services::LOGICAL_ONLY).unwrap();
    let pou = prepared
        .layout
        .entries
        .iter()
        .find(|entry| entry.owner == StorageOwner::Instance && entry.type_id.is_some())
        .and_then(|entry| entry.owner_pou_id)
        .unwrap();
    let instances = data.storage.instances().len();
    let templates = data.construction.instance_templates.len();
    let mut state = EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY);

    // TIME-08: the instance node is the first unit. With none available, the instance is
    // refused before anything is issued.
    state.data.resources.work_budget.reset(0);
    assert_eq!(
        state.reserve_instance(pou, None),
        Err(RuntimeError::ExecutionLimit {
            kind: ExecutionLimitKind::Work,
            used: 0,
            requested: 1,
            limit: 0,
        })
    );
    assert_eq!(state.data.storage.instances().len(), instances);

    // With one unit, the instance node takes it, and the first member preflight (one unit per
    // declaration, CONSTRUCT) is refused with the instance node already charged.
    state.data.resources.work_budget.reset(1);
    assert_eq!(
        state.reserve_instance(pou, None),
        Err(RuntimeError::ExecutionLimit {
            kind: ExecutionLimitKind::Work,
            used: 1,
            requested: 1,
            limit: 1,
        })
    );
    assert_eq!(state.data.storage.instances().len(), instances);
    assert_eq!(state.data.construction.instance_templates.len(), templates);
}
