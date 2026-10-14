use super::*;

#[test]
fn initializer_snapshot_refusal_skips_owned_clone_and_retires_instances() {
    let prepared = PreparedModule::from_bytes(
        include_bytes!(
            "../../../../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc"
        ),
        crate::vm::PreparationLimits::default(),
    )
    .unwrap();
    let services = &super::super::services::LOGICAL_ONLY;
    let mut data = EngineState::construct_data(&prepared, 0, services).unwrap();
    let mut state = EngineState::borrow(&mut data, &prepared, services);
    let result = state.data.storage.reserve_execution_frame().unwrap();
    state.data.storage.activate_execution_frame(result).unwrap();
    let mut slot = vec![Value::DInt(7)];
    state
        .data
        .storage
        .suspend_execution_frame(result, &mut slot)
        .unwrap();
    let pou = *prepared.vm.function_block_ids.get("COUNTER").unwrap();
    let temporary = state.reserve_instance(pou, Some(result)).unwrap();
    let entry = prepared.initializers.entries[0].clone();
    state
        .data
        .construction
        .initializers
        .push(ActiveInitializer {
            id: 0,
            context: entry.context_initializer_idx.unwrap_or(0),
            entry,
            result,
            lexical_frame: None,
            instance: None,
            depth: 0,
            writable_instances: InstanceSet::default(),
        });

    // Retirement and the snapshot's bookkeeping consume no work (TIME-08), so the
    // refusal comes from the snapshot's allocation admission, which precedes the clone.
    let limit = prepared.limits.max_construction_bytes;
    state.data.resources.construction_bytes.set(limit);
    state.data.resources.work_budget.reset(2);
    let snapshot_copies = state.data.lifetimes.owned_instances.snapshot_copy_count();
    let error = state
        .finish_initializer(Ok(()), result, None, None, Vec::new())
        .unwrap_err();

    assert!(matches!(
        error,
        RuntimeError::ExecutionLimit {
            kind: crate::error::ExecutionLimitKind::AllocationBytes,
            ..
        }
    ));
    assert_eq!(state.data.resources.work_budget.remaining(), 2);
    assert_eq!(
        state.data.lifetimes.owned_instances.snapshot_copy_count(),
        snapshot_copies,
        "a terminal cleanup refusal must not clone the rollback ownership list"
    );
    assert_eq!(state.data.resources.construction_bytes.get(), limit);
    assert!(state.data.storage.get_instance(temporary).is_none());
    assert!(!state
        .data
        .lifetimes
        .instance_lifetimes
        .contains_key(&temporary));
    assert_eq!(state.data.storage.execution_frame_count(), 0);
    assert!(state.data.construction.initializers.is_empty());
}
