//! TIME-08 / TIME-09: sorted metadata growth is charged as bytes, never as the moved entries' work,
//! and a construction node is charged one unit before its metadata is published.
use super::*;
use crate::error::ExecutionLimitKind;

fn program_v2() -> super::super::PreparedModule {
    super::super::PreparedModule::from_bytes(
        include_bytes!(
            "../../../../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc"
        ),
        crate::vm::PreparationLimits::default(),
    )
    .unwrap()
}

#[test]
fn sorted_growth_charges_its_bytes_and_not_the_moved_entries_work() {
    let mut entries: Entries<u32, u32> = Entries::default();
    for key in [10, 20, 30, 40] {
        entries.insert(key, key).unwrap();
    }
    // The list is full, so a new smallest key needs growth and moves all four entries.
    assert_eq!(entries.len(), entries.capacity());
    let (bytes, work) = entries.insertion_demand(&5).unwrap();
    assert!(bytes > 0, "growth allocates bytes");
    assert!(
        work > 0,
        "the search and the moved entries are reported as work"
    );

    let prepared = program_v2();
    let mut data =
        EngineState::construct_data(&prepared, 0, &super::super::services::LOGICAL_ONLY).unwrap();
    let state = EngineState::borrow(&mut data, &prepared, &super::super::services::LOGICAL_ONLY);
    state.data.resources.work_budget.reset(0);
    state.data.resources.construction_bytes.set(0);

    // TIME-08: the moved-entry component is bookkeeping. With no work at all, the growth is
    // still admitted, and only its bytes are charged.
    assert_eq!(state.charge_sorted_growth(Ok((bytes, work))), Ok(()));
    assert_eq!(state.data.resources.work_budget.remaining(), 0);
    assert_eq!(state.data.resources.construction_bytes.get(), bytes);
}

#[test]
fn reserve_instance_refuses_its_construct_node_before_publishing_metadata() {
    let prepared = program_v2();
    let mut state_data =
        EngineState::construct_data(&prepared, 0, &super::super::services::LOGICAL_ONLY).unwrap();
    let mut state = EngineState::borrow(
        &mut state_data,
        &prepared,
        &super::super::services::LOGICAL_ONLY,
    );
    let instances = state.data.storage.instances().len();
    let templates = state.data.construction.instance_templates.len();
    let lifetimes = state.data.lifetimes.instance_lifetimes.len();
    let owner = state.data.storage.reserve_execution_frame().unwrap();
    state.data.storage.activate_execution_frame(owner).unwrap();
    let pou = *prepared.vm.function_block_ids.get("COUNTER").unwrap();

    // TIME-08 CONSTRUCT: the instance node costs one unit. With none left it is refused at
    // the first unit, before any instance identity or metadata is published.
    state.data.resources.work_budget.reset(0);
    assert_eq!(
        state.reserve_instance(pou, Some(owner)),
        Err(RuntimeError::ExecutionLimit {
            kind: ExecutionLimitKind::Work,
            used: 0,
            requested: 1,
            limit: 0,
        })
    );
    assert_eq!(state.data.storage.instances().len(), instances);

    // With one unit, the instance node is charged and the first declaration's CONSTRUCT node
    // is refused. Nothing is published.
    state.data.resources.work_budget.reset(1);
    assert_eq!(
        state.reserve_instance(pou, Some(owner)),
        Err(RuntimeError::ExecutionLimit {
            kind: ExecutionLimitKind::Work,
            used: 1,
            requested: 1,
            limit: 1,
        })
    );
    assert_eq!(state.data.storage.instances().len(), instances);
    assert_eq!(state.data.construction.instance_templates.len(), templates);
    assert_eq!(state.data.lifetimes.instance_lifetimes.len(), lifetimes);
    assert_eq!(state.data.lifetimes.owned_instances.count(owner), 0);
}
