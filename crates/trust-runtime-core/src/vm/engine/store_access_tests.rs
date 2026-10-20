//! Tests of the store-access seam's writes through a runtime state (specification 34 §6.2, MEM-10;
//! C1-9c, R1). The state holds the cell arm in these tests, the engine's default store (C1-19d).

use super::EngineStore;
use crate::memory::cell_access::CellArm;
use crate::memory::regions::CellStorage;
use crate::memory::view::{StoreAccess, StoreMutate};
use crate::memory::Generation;
use crate::value::Value;
use crate::vm::{PreparationLimits, PreparedModule};

#[test]
fn runtime_state_writes_reach_the_store_it_holds() {
    // Specification 34 §6.2 (MEM-10): a runtime state writes through the store it holds, as the engine
    // state does. `probe_depth` is the INT global of the GPIO fixture. The state's store is the cell arm
    // here, so the write of 2 lands in the cells, and a read through the state sees 2. A write that reached
    // the `Value` storage instead would change `probe_depth` there, and a write that went nowhere would read
    // the cell's default of 0 back, not 2.
    let prepared = PreparedModule::from_bytes(
        include_bytes!("../../../../trust-runtime/tests/fixtures/portability/f401/gpio.stbc"),
        PreparationLimits::default(),
    )
    .expect("the GPIO fixture prepares");
    let mut state = prepared
        .instantiate(0)
        .expect("the GPIO fixture instantiates");
    let values_before = state
        .engine_parts()
        .0
        .storage
        .get_global("probe_depth")
        .cloned();
    let cells = CellStorage::construct(&prepared, Generation::FIRST, |_| Ok(()))
        .expect("the persistent region reserves");
    let (data, _) = state.engine_parts_mut();
    data.store = EngineStore::Cells(Box::new(CellArm::new(cells)));

    assert!(StoreMutate::set_global(
        &mut state,
        "probe_depth",
        Value::Int(2)
    ));
    let read = StoreAccess::read_global(&state, "probe_depth")
        .map(|view| StoreAccess::materialize(&state, &view));
    assert!(matches!(read, Some(Ok(Value::Int(2)))));
    assert_eq!(
        state
            .engine_parts()
            .0
            .storage
            .get_global("probe_depth")
            .cloned(),
        values_before
    );
}
