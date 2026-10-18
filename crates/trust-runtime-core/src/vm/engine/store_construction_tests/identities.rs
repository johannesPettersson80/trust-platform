//! The entry floor of the cell store (specification 34 §4.3.1 and HOST-02; C1-9d2b-2, R4). A top-level
//! execution entry reissues the identities that the entry before it released, so a runtime keeps running
//! after one increasing 32-bit sequence would have been exhausted. On the cell store the identities come
//! from the store's one counter, and that counter holds the entry floor.
//!
//! The program is the bounded-C corpus program `initializers`: `PROGRAM Initializers` (instance 0) holds
//! a `Tracker` (instance 1), the two persistent instances. Every other instance is a per-call one.

use super::{arm, build, prepare, services, EngineState, EngineStore, StoreKind, INITIALIZERS};
use crate::error::RuntimeError;
use crate::memory::view::StoreAccess;
use crate::memory::{FrameId, InstanceId};

/// One top-level execution entry that creates and retires a per-call `Tracker`: it reserves the entry's
/// activation, creates the instance owned by that activation, retires it and releases the activation, and
/// answers the activation identity and the instance identity it used.
fn entry(state: &mut EngineState<'_>, tracker: u32) -> Result<(FrameId, InstanceId), RuntimeError> {
    state.engineering_operation(|engine| {
        let scope = engine.reserve_activation_identity()?;
        engine.data.storage.activate_execution_frame(scope)?;
        let owned = engine.reserve_instance(tracker, Some(scope))?;
        engine.remove_owned_instances(scope)?;
        assert!(engine.data.storage.release_execution_frame(scope));
        Ok((scope, owned))
    })
}

#[test]
fn the_entry_floor_comes_from_the_one_identity_counter() {
    // Specification 34 §4.3.1 and HOST-02: when a top-level execution entry begins, the identities issued
    // since the previous entry began are issued again if none of their instances is still live, and the
    // identity of a persistent instance is never issued again. Worked out by hand for `initializers`:
    //
    // - the program and its tracker are persistent, with the identities 0 and 1, so the first entry starts
    //   its floor above them, at 2, and its per-call `Tracker` takes 2;
    // - the entry retires that instance, so none is live, and the next entry starts from the floor again:
    //   every entry takes 2, and the activation identity restarts at 0 each time;
    // - with the counter moved to 2^32 - 2, the last identity the allocator hands out (HOST-02), the first
    //   entry's floor is that identity, and every later entry reissues it. Eight entries run, past the
    //   limit that one increasing sequence would reach on the second.
    //
    // The cell store keeps the floor in the counter that issues the identities. A floor read from the
    // `Value` half's counter, which no instance of the cell store ever advances, would restart at 0 and
    // issue the identity of a persistent instance (the first assertion), and a counter with no floor
    // would give 3, 4, 5 and then fail with `Overflow` at the limit.
    let prepared = prepare(INITIALIZERS);
    let tracker = *prepared
        .vm
        .function_block_ids
        .get("TRACKER")
        .expect("the program declares the function block Tracker");
    let mut data = build(&prepared, StoreKind::Cells).expect("the cell store constructs");
    let mut state = EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY);
    for round in 0..4 {
        assert_eq!(
            entry(&mut state, tracker),
            Ok((FrameId(0), InstanceId(2))),
            "entry {round}"
        );
    }
    assert_eq!(arm(&*state.data).persistent_count(), 2);
    assert!(state.data.storage.instances().is_empty());
    for persistent in [InstanceId(0), InstanceId(1)] {
        assert!(StoreAccess::instance_type_name(&state, persistent).is_some());
    }

    let mut data = build(&prepared, StoreKind::Cells).expect("the cell store constructs");
    match &mut data.store {
        EngineStore::Cells(cell_arm) => cell_arm
            .cells_mut()
            .set_next_instance_id_for_test(u32::MAX - 1),
        EngineStore::Values => panic!("the state holds the Value store"),
    }
    let mut state = EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY);
    for round in 0..8 {
        assert_eq!(
            entry(&mut state, tracker),
            Ok((FrameId(0), InstanceId(u32::MAX - 1))),
            "entry {round} at the identity limit"
        );
    }
    assert_eq!(arm(&*state.data).persistent_count(), 2);
}
