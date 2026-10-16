//! Retiring a suspended activation removes its liveness, owner and suspended payload together.
use super::*;
use crate::vm::context::ExecutionContext;

fn prepared() -> PreparedModule {
    PreparedModule::from_bytes(
        include_bytes!("../../../../trust-runtime/tests/fixtures/portability/f401/gpio.stbc"),
        crate::vm::PreparationLimits::default(),
    )
    .unwrap()
}

fn frame(id: Option<FrameId>) -> VmFrame {
    VmFrame {
        activation: id,
        pou_id: Some(0),
        return_pc: 0,
        code_start: 0,
        code_end: 0,
        local_ref_start: 0,
        local_ref_count: 0,
        locals: Vec::new(),
        parameter_values_present: Vec::new(),
        runtime_instance: None,
        instance_owner: None,
    }
}

/// Spec 34 §4.3 HOST-04 "Activations": retirement of a suspended frame that holds locals removes its liveness,
/// its owner, its suspended payload and its suspended-activation entry together, and drops the owner count by one.
#[test]
fn call_path_retiring_a_suspended_frame_removes_owner_and_payload_together() {
    let prepared = prepared();
    let mut data = EngineState::construct_data(&prepared, 0, &services::LOGICAL_ONLY).unwrap();
    let mut state = EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY);
    let parent = state.data.storage.reserve_execution_frame().unwrap();
    state.data.storage.activate_execution_frame(parent).unwrap();
    state
        .data
        .storage
        .set_execution_frame_owner(parent, 0, None)
        .unwrap();
    let child = state.data.storage.reserve_execution_frame().unwrap();
    state.data.storage.activate_execution_frame(child).unwrap();
    state
        .data
        .storage
        .set_execution_frame_owner(child, 1, None)
        .unwrap();
    let mut locals = vec![Value::DInt(9)];
    state
        .data
        .storage
        .suspend_execution_frame(child, &mut locals)
        .unwrap();
    let suspended = state
        .data
        .storage
        .suspended_execution_activations()
        .find(|(id, _, _)| *id == child)
        .expect("the child is a suspended activation");
    assert_eq!(suspended.1.pou_id, 1);
    assert_eq!(suspended.2.first(), Some(&Value::DInt(9)));
    let owners_before = state.data.storage.execution_frame_owner_count();
    state.data.resources.work_budget.reset(1);
    assert_eq!(state.retire_frame(&frame(Some(child))), Ok(()));
    assert_eq!(state.data.resources.work_budget.remaining(), 1);
    assert!(!state.data.storage.execution_frame_is_live(child));
    assert!(state.data.storage.execution_frame_owner(child).is_none());
    assert!(state
        .data
        .storage
        .suspended_execution_activations()
        .all(|(id, _, _)| id != child));
    assert_eq!(state.data.storage.suspended_execution_values().count(), 0);
    assert_eq!(
        state.data.storage.execution_frame_owner_count(),
        owners_before - 1
    );
    assert!(state.data.storage.execution_frame_is_live(parent));
}
