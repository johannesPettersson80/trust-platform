//! Engine-policy assertions for the single authoritative activation registry.
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

#[test]
fn reserved_storage_never_grants_an_active_reference_lifetime() {
    let prepared = prepared();
    let mut data = EngineState::construct_data(&prepared, 0, &services::LOGICAL_ONLY).unwrap();
    let state = EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY);
    let parent = state.data.storage.reserve_execution_frame().unwrap();
    state.data.storage.activate_execution_frame(parent).unwrap();
    let reserved = state.data.storage.reserve_execution_frame().unwrap();
    assert!(state.data.storage.execution_frame_is_live(reserved));
    assert!(!state.data.storage.execution_activation_is_live(reserved));
    assert_eq!(
        state.check_local_lifetime(parent, Some(reserved)),
        Err(RuntimeError::ReferenceLifetime)
    );
    assert_eq!(
        state.check_local_lifetime(reserved, Some(parent)),
        Err(RuntimeError::ReferenceLifetime)
    );
    state
        .data
        .storage
        .activate_execution_frame(reserved)
        .unwrap();
    assert_eq!(state.check_local_lifetime(parent, Some(reserved)), Ok(()));
    assert_eq!(
        state.check_local_lifetime(reserved, Some(parent)),
        Err(RuntimeError::ReferenceLifetime)
    );
    assert!(state.data.storage.release_execution_frame(parent));
    assert_eq!(
        state.check_local_lifetime(parent, Some(reserved)),
        Err(RuntimeError::ReferenceLifetime)
    );
}

#[test]
fn owner_refusal_on_an_exhausted_allowance_retains_activation_for_unwind() {
    let prepared = prepared();
    let mut data = EngineState::construct_data(&prepared, 0, &services::LOGICAL_ONLY).unwrap();
    let mut state = EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY);
    // TIME-08 VERIFY: activation records are bookkeeping, so an exhausted allowance does
    // not refuse the frame before its owner check; the owner refusal is the first error.
    state.data.resources.work_budget.reset(0);
    let mut frame = frame(None);
    frame.pou_id = None;
    assert_eq!(
        state.initialize_frame(&prepared.vm, &mut frame, 0),
        Err(crate::vm::module::invalid_bytecode(
            smol_str::SmolStr::new_static("ordinary frame has no POU owner")
        ))
    );
    let id = frame
        .activation
        .expect("identity precedes owner validation");
    assert!(state.data.storage.execution_activation_is_live(id));
    assert!(state.data.storage.execution_frame_owner(id).is_none());
    assert_eq!(state.retire_frame(&frame), Ok(()));
    assert_eq!(state.data.resources.work_budget.remaining(), 0);
    assert_eq!(state.data.storage.execution_frame_count(), 0);
    assert_eq!(state.data.storage.execution_activation_count(), 0);
}

#[test]
fn unified_retirement_is_bookkeeping_and_releases_frames_in_order() {
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
    state.data.resources.work_budget.reset(1);
    // TIME-08 VERIFY: retirement records are bookkeeping; the one-unit allowance is untouched.
    assert_eq!(state.retire_frame(&frame(Some(parent))), Ok(()));
    assert_eq!(state.data.resources.work_budget.remaining(), 1);
    assert!(!state.data.storage.execution_frame_is_live(parent));
    assert!(state.data.storage.execution_activation_is_live(child));
    assert_eq!(
        state
            .data
            .storage
            .execution_frame_owner(child)
            .unwrap()
            .pou_id,
        1
    );
    assert_eq!(
        state.data.storage.previous_execution_activation(child),
        None
    );
    assert_eq!(state.retire_frame(&frame(Some(child))), Ok(()));
    assert_eq!(state.data.resources.work_budget.remaining(), 1);
    assert_eq!(state.data.storage.execution_frame_count(), 0);
}

#[test]
fn missing_owner_rejects_without_consuming_work_and_still_unwinds() {
    let prepared = prepared();
    let mut data = EngineState::construct_data(&prepared, 0, &services::LOGICAL_ONLY).unwrap();
    let mut state = EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY);
    state.data.resources.work_budget.reset(2);
    let mut frame = frame(None);
    frame.pou_id = None;
    assert_eq!(
        state.initialize_frame(&prepared.vm, &mut frame, 0),
        Err(crate::vm::module::invalid_bytecode(
            smol_str::SmolStr::new_static("ordinary frame has no POU owner")
        ))
    );
    // TIME-08 VERIFY: the activation record is bookkeeping, so the rejection consumes no work.
    assert_eq!(state.data.resources.work_budget.remaining(), 2);
    let id = frame
        .activation
        .expect("identity precedes owner validation");
    assert!(state.data.storage.execution_activation_is_live(id));
    assert!(state.data.storage.execution_frame_owner(id).is_none());
    assert_eq!(state.retire_frame(&frame), Ok(()));
    assert_eq!(state.data.resources.work_budget.remaining(), 2);
    assert_eq!(state.data.storage.execution_frame_count(), 0);
}
