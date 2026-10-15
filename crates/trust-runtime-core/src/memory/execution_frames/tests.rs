use super::*;
use alloc::{vec, vec::Vec};

#[test]
fn identity_overflow_precedes_capacity_refusal_without_issuing_a_frame() {
    let mut storage = VariableStorage::new();
    storage.reserve_execution_frames(0).unwrap();
    storage.next_frame_id = u32::MAX;
    assert_eq!(
        storage.reserve_execution_frame(),
        Err(RuntimeError::Overflow)
    );
    assert_eq!(storage.next_frame_id, u32::MAX);
    assert_eq!(storage.execution_frame_count(), 0);

    storage.next_frame_id = u32::MAX - 1;
    assert_eq!(
        storage.reserve_execution_frame(),
        Err(crate::vm::VmTrap::CallStackOverflow.into_runtime_error())
    );
    assert_eq!(storage.next_frame_id, u32::MAX - 1);
    storage.reserve_execution_frames(1).unwrap();
    let last = storage.reserve_execution_frame().unwrap();
    assert_eq!(last, FrameId(u32::MAX - 1));
    assert_eq!(
        storage.reserve_execution_frame(),
        Err(RuntimeError::Overflow)
    );
    assert_eq!(storage.execution_frame_count(), 1);
    assert!(storage.release_execution_frame(last));
}

#[test]
fn rejected_suspend_and_resume_preserve_the_owned_vector() {
    let mut storage = VariableStorage::new();
    let id = storage.reserve_execution_frame().unwrap();
    let mut locals = vec![Value::Int(17), Value::Int(23)];
    let pointer = locals.as_ptr();
    assert_eq!(
        storage.resume_execution_frame(id, &mut locals),
        Err(RuntimeError::InvalidExecutionState)
    );
    storage.suspend_execution_frame(id, &mut locals).unwrap();
    let mut other = vec![Value::Int(31)];
    assert_eq!(
        storage.suspend_execution_frame(id, &mut other),
        Err(RuntimeError::InvalidExecutionState)
    );
    assert_eq!(other, vec![Value::Int(31)]);
    assert_eq!(
        storage.resume_execution_frame(FrameId(u32::MAX), &mut locals),
        Err(RuntimeError::InvalidExecutionState)
    );
    assert!(locals.is_empty());
    storage.resume_execution_frame(id, &mut locals).unwrap();
    assert_eq!(locals.as_ptr(), pointer);
    assert_eq!(locals, vec![Value::Int(17), Value::Int(23)]);
    assert_eq!(
        storage.resume_execution_frame(id, &mut Vec::new()),
        Err(RuntimeError::InvalidExecutionState)
    );
    assert_eq!(locals, vec![Value::Int(17), Value::Int(23)]);
}

#[test]
fn activation_registry_separates_reserved_slots_from_ordered_live_frames() {
    let mut storage = VariableStorage::new();
    let reserved_only = storage.reserve_execution_frame().unwrap();
    assert!(storage.execution_frame_is_live(reserved_only));
    assert!(!storage.execution_activation_is_live(reserved_only));
    assert_eq!(storage.execution_activation_count(), 0);
    assert_eq!(storage.execution_frame_owner(reserved_only), None);

    let parent = storage.reserve_execution_frame().unwrap();
    storage.activate_execution_frame(parent).unwrap();
    storage
        .set_execution_frame_owner(parent, 10, Some(InstanceId(4)))
        .unwrap();
    let mut parent_locals = vec![Value::Int(17)];
    storage
        .suspend_execution_frame(parent, &mut parent_locals)
        .unwrap();

    let initializer = storage.reserve_execution_frame().unwrap();
    storage.activate_execution_frame(initializer).unwrap();
    let mut initializer_locals = vec![Value::Int(23)];
    storage
        .suspend_execution_frame(initializer, &mut initializer_locals)
        .unwrap();

    let child = storage.reserve_execution_frame().unwrap();
    storage.activate_execution_frame(child).unwrap();
    storage.set_execution_frame_owner(child, 11, None).unwrap();
    let mut child_locals = vec![Value::Int(31)];
    storage
        .suspend_execution_frame(child, &mut child_locals)
        .unwrap();

    assert_eq!(storage.execution_activation_count(), 3);
    assert_eq!(storage.last_execution_activation(), Some(child));
    assert_eq!(
        storage.previous_execution_activation(child),
        Some(initializer)
    );
    assert_eq!(
        storage.previous_execution_activation(initializer),
        Some(parent)
    );
    assert_eq!(
        storage.execution_frame_owner(parent),
        Some(ExecutionFrameOwner {
            pou_id: 10,
            runtime_instance: Some(InstanceId(4)),
        })
    );
    assert_eq!(storage.execution_frame_owner(initializer), None);
    assert_eq!(storage.execution_frame_owner(child).unwrap().pou_id, 11);
    assert_eq!(storage.execution_frame_owner_count(), 2);
    // The registry holds the inactive reservation as well as active frames.
    assert_eq!(storage.execution_frame_count(), 4);
    assert_eq!(
        storage
            .suspended_execution_activations()
            .map(|(id, _, _)| id)
            .collect::<Vec<_>>(),
        vec![parent, child]
    );

    assert!(storage.release_execution_frame(initializer));
    assert_eq!(storage.previous_execution_activation(child), Some(parent));
    assert_eq!(storage.execution_activation_count(), 2);
    assert!(storage.release_execution_frame(child));
    assert!(!storage.execution_activation_is_live(child));
    let next = storage.reserve_execution_frame().unwrap();
    assert!(next.0 > child.0);
    assert_eq!(storage.execution_frame_owner(next), None);
}

#[test]
fn activation_rejects_reused_and_out_of_order_reservations() {
    let mut storage = VariableStorage::new();
    let earlier = storage.reserve_execution_frame().unwrap();
    let later = storage.reserve_execution_frame().unwrap();
    storage.activate_execution_frame(later).unwrap();
    storage.set_execution_frame_owner(later, 12, None).unwrap();
    assert_eq!(
        storage.activate_execution_frame(later),
        Err(RuntimeError::InvalidExecutionState)
    );
    assert_eq!(
        storage.activate_execution_frame(earlier),
        Err(RuntimeError::InvalidExecutionState)
    );
    assert_eq!(
        storage.set_execution_frame_owner(earlier, 10, None),
        Err(RuntimeError::InvalidExecutionState)
    );
    assert!(!storage.execution_activation_is_live(earlier));
    assert!(storage.execution_activation_is_live(later));
    assert_eq!(storage.previous_execution_activation(earlier), None);
    assert_eq!(
        storage.set_execution_frame_owner(later, 13, None),
        Err(RuntimeError::InvalidExecutionState)
    );

    let unsuspended = storage.reserve_execution_frame().unwrap();
    let mut nonempty = vec![Value::Int(9)];
    assert_eq!(
        storage.resume_execution_frame(unsuspended, &mut nonempty),
        Err(RuntimeError::InvalidExecutionState)
    );
    assert!(!storage.execution_activation_is_live(unsuspended));
    assert_eq!(nonempty, vec![Value::Int(9)]);
}

#[test]
fn activation_record_charge_matches_native_layout_and_clone_preserves_registry() {
    let requested_capacity = 5;
    let record_bytes = core::mem::size_of::<(FrameId, ExecutionActivation)>();
    assert_eq!(
        VariableStorage::execution_frame_reservation_charge(requested_capacity),
        requested_capacity.checked_mul(record_bytes)
    );

    let mut storage = VariableStorage::new();
    storage
        .reserve_execution_frames(requested_capacity)
        .unwrap();
    let id = storage.reserve_execution_frame().unwrap();
    storage.activate_execution_frame(id).unwrap();
    storage
        .set_execution_frame_owner(id, 23, Some(InstanceId(7)))
        .unwrap();
    let mut locals = vec![Value::Int(41)];
    storage.suspend_execution_frame(id, &mut locals).unwrap();

    let clone = storage.clone();
    assert_eq!(
        clone.execution_frame_capacity(),
        storage.execution_frame_capacity()
    );
    assert!(clone.execution_activation_is_live(id));
    assert_eq!(
        clone.execution_frame_owner(id),
        storage.execution_frame_owner(id)
    );
    assert_eq!(clone.local_slot(id, 0), Some(&Value::Int(41)));
    std::println!(
        "execution_activation_record_bytes={record_bytes}; reserved_capacity={}",
        clone.execution_frame_capacity()
    );
}

#[test]
fn reserved_frame_capacity_refusal_is_call_stack_overflow_and_issues_no_identity() {
    let mut storage = VariableStorage::new();
    storage.reserve_execution_frames(1).unwrap();
    let first = storage.reserve_execution_frame().unwrap();
    // The counter advances past the one admitted identity (reserve_execution_frame, memory/execution_frames.rs:58-73).
    assert_eq!(storage.next_frame_id, first.0 + 1);
    let next_after_first = storage.next_frame_id;

    // The capacity check runs before the identity is issued, so the counter is unchanged.
    let error = storage.reserve_execution_frame().unwrap_err();
    assert_eq!(
        error,
        crate::vm::VmTrap::CallStackOverflow.into_runtime_error()
    );
    assert_eq!(
        error.stable_code(),
        crate::error::StableErrorCode::VmCallStackOverflow
    );
    assert_eq!(storage.next_frame_id, next_after_first);
    assert_eq!(storage.execution_frame_count(), 1);
}
