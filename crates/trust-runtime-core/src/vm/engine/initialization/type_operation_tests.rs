use super::*;
use crate::vm::{engine::services::LOGICAL_ONLY, PreparationLimits, PreparedModule};

fn fixture() -> PreparedModule {
    PreparedModule::from_bytes(
        include_bytes!("../../../../../trust-runtime/tests/fixtures/portability/f401/gpio.stbc"),
        PreparationLimits::default(),
    )
    .unwrap()
}

#[test]
fn active_type_masks_preserve_operation_identity_and_unwind() {
    let prepared = fixture();
    assert!(prepared.vm.types.entries.len() >= 2);
    let mut data = EngineState::construct_data(&prepared, 0, &LOGICAL_ONLY).unwrap();
    let mut state = EngineState::borrow(&mut data, &prepared, &LOGICAL_ONLY);
    for (ty, op) in [
        (0, ValueOperation::Default),
        (0, ValueOperation::Coerce),
        (1, ValueOperation::Default),
        (1, ValueOperation::Apply),
    ] {
        state.enter_type(ty, op).unwrap();
    }
    let active = state.data.construction.type_operations.clone();
    let capacity = state.data.construction.types.capacity();
    let error = state.enter_type(0, ValueOperation::Default).unwrap_err();
    assert_eq!(error, invalid_bytecode("cyclic typed default construction"));
    assert_eq!(state.data.construction.types.len(), 4);
    assert_eq!(state.data.construction.types.capacity(), capacity);
    assert_eq!(state.data.construction.type_operations, active);
    state.leave_type();
    state.enter_type(1, ValueOperation::Apply).unwrap();
    for _ in 0..4 {
        state.leave_type();
    }
    assert!(state.data.construction.types.is_empty());
    assert!(state
        .data
        .construction
        .type_operations
        .iter()
        .all(|bits| *bits == 0));
    state.enter_type(0, ValueOperation::Default).unwrap();
    state.leave_type();
}

#[test]
fn type_mask_entry_refusals_do_not_publish_membership() {
    let mut prepared = fixture();
    let mut data = EngineState::construct_data(&prepared, 0, &LOGICAL_ONLY).unwrap();
    prepared.limits.max_construction_bytes = 0;
    data.resources.construction_bytes.set(0);
    let mut state = EngineState::borrow(&mut data, &prepared, &LOGICAL_ONLY);
    for work in [0, 1, 2] {
        state.data.resources.work_budget.reset(work);
        let error = state.enter_type(0, ValueOperation::Default).unwrap_err();
        // TIME-08: entry bookkeeping consumes no work, so the allocation refuses at
        // every allowance and the allowance is left untouched.
        assert!(matches!(
            error,
            RuntimeError::ExecutionLimit {
                kind: crate::error::ExecutionLimitKind::AllocationBytes,
                ..
            }
        ));
        assert_eq!(state.data.resources.work_budget.remaining(), work);
        assert!(state.data.construction.types.is_empty());
        assert!(state
            .data
            .construction
            .type_operations
            .iter()
            .all(|bits| *bits == 0));
    }
}

#[test]
fn type_membership_consumes_no_work_and_rejects_invalid_ordinals() {
    let prepared = fixture();
    let mut data = EngineState::construct_data(&prepared, 0, &LOGICAL_ONLY).unwrap();
    let mut state = EngineState::borrow(&mut data, &prepared, &LOGICAL_ONLY);
    state.enter_type(0, ValueOperation::Default).unwrap();
    state.enter_type(0, ValueOperation::Coerce).unwrap();
    state.data.resources.work_budget.reset(2);
    state.enter_type(1, ValueOperation::Apply).unwrap();
    // TIME-08: the type-operation mask and ordinal checks are bookkeeping, not work.
    assert_eq!(state.data.resources.work_budget.remaining(), 2);
    state.data.resources.work_budget.reset(2);
    assert_eq!(
        state.enter_type(u32::MAX, ValueOperation::Default),
        Err(RuntimeError::TypeMismatch)
    );
    assert_eq!(state.data.construction.types.len(), 3);
    for _ in 0..3 {
        state.leave_type();
    }
    assert!(state
        .data
        .construction
        .type_operations
        .iter()
        .all(|bits| *bits == 0));
}

#[test]
fn type_mask_capacity_follows_the_admitted_type_table() {
    let prepared = fixture();
    let data = EngineState::construct_data(&prepared, 0, &LOGICAL_ONLY).unwrap();
    assert_eq!(
        data.construction.type_operations.len(),
        prepared.vm.types.entries.len().div_ceil(2)
    );
    assert!(data.construction.types.is_empty());
    assert!(data
        .construction
        .type_operations
        .iter()
        .all(|bits| *bits == 0));
}
