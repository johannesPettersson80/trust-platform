//! Live storage bounds preserve admitted calls, staging and terminal cleanup.
use super::*;
use crate::error::StableErrorCode;

#[test]
fn deepest_admitted_call_can_initialize_then_retire_every_storage_frame() {
    let prepared = PreparedModule::from_bytes(
        include_bytes!("../../../../trust-runtime/tests/fixtures/portability/f401/gpio.stbc"),
        crate::vm::PreparationLimits {
            max_call_depth: 4,
            ..Default::default()
        },
    )
    .unwrap();
    let mut state = EngineState::new(&prepared, 0, &services::LOGICAL_ONLY).unwrap();
    let activation_capacity = state.lifetimes.live_activations.capacity();
    assert!(activation_capacity >= 8);
    let Some(Value::Instance(plant)) = state.storage.get_global("Plant") else {
        panic!("root");
    };
    let plant = *plant;
    state.storage.set_global("probe_depth", Value::Int(4));
    for scan in 1..=32 {
        state
            .execute_cycle(Duration::from_millis(scan * 10))
            .unwrap();
        assert_eq!(
            state.storage.get_instance_var(plant, "depth_result"),
            Some(&Value::Int(4))
        );
        assert_eq!(state.storage.execution_frame_count(), 0);
        assert!(state.lifetimes.live_activations.is_empty());
        assert_eq!(
            state.lifetimes.live_activations.capacity(),
            activation_capacity
        );
    }
    // The call-depth bound alone excludes valid staging storage at depth four.
    state.storage.reserve_execution_frames(4).unwrap();
    let error = state.execute_cycle(Duration::from_millis(330)).unwrap_err();
    assert_eq!(error.stable_code(), StableErrorCode::VmCallStackOverflow);
    assert_eq!(state.storage.execution_frame_count(), 0);
    assert!(state.lifetimes.live_activations.is_empty());
}

#[test]
fn exhausted_cleanup_still_retires_owned_instances_and_preserves_promoted_ones() {
    let prepared = PreparedModule::from_bytes(
        include_bytes!(
            "../../../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc"
        ),
        crate::vm::PreparationLimits::default(),
    )
    .unwrap();
    let mut state = EngineState::new(&prepared, 0, &services::LOGICAL_ONLY).unwrap();
    let old = state.storage.reserve_execution_frame().unwrap();
    let new = state.storage.reserve_execution_frame().unwrap();
    state.lifetimes.live_activations.extend([old, new]);
    let pou = *prepared.vm.function_block_ids.get("COUNTER").unwrap();
    let retired = state.reserve_instance(pou, Some(old)).unwrap();
    let promoted = state.reserve_instance(pou, Some(old)).unwrap();
    state
        .promote_constructed_instances(&Value::Instance(promoted), old, Some(new))
        .unwrap();
    state
        .construction
        .initialized_declarations
        .insert((0, Some(retired)))
        .unwrap();
    state.construction.once.insert((0, Some(retired))).unwrap();
    state.resources.work_budget.reset(0);
    assert_eq!(
        state.remove_owned_instances(old).unwrap_err().stable_code(),
        StableErrorCode::RuntimeExecutionTimeout
    );
    assert!(state.storage.get_instance(retired).is_none());
    assert!(state.storage.get_instance(promoted).is_some());
    assert!(!state.lifetimes.instance_lifetimes.contains_key(&retired));
    assert!(!state
        .construction
        .initialized_declarations
        .contains(&(0, Some(retired)))
        .unwrap());
    assert!(!state
        .construction
        .once
        .contains(&(0, Some(retired)))
        .unwrap());
    state.resources.work_budget.reset(prepared.limits.max_work);
    state.remove_owned_instances(new).unwrap();
    assert!(state.storage.get_instance(promoted).is_none());
}

#[test]
fn journal_growth_budget_failure_does_not_publish_snapshot_entries_or_writes() {
    let prepared = PreparedModule::from_bytes(
        include_bytes!("../../../../trust-runtime/tests/fixtures/portability/f401/gpio.stbc"),
        crate::vm::PreparationLimits::default(),
    )
    .unwrap();
    let mut state = EngineState::new(&prepared, 0, &services::LOGICAL_ONLY).unwrap();
    let reference = state.storage.ref_for_global("probe_depth").unwrap();
    state.resources.output_journal = Some(destinations::SavedDestinations::new());
    state
        .resources
        .construction_bytes
        .set(prepared.limits.max_construction_bytes);
    let result = state.snapshot_destinations(None, core::iter::once(reference.as_view()));
    assert!(matches!(result, Err(RuntimeError::ExecutionTimeout)));
    assert_eq!(state.resources.output_journal.as_ref().unwrap().len(), 0);
    assert_eq!(
        state.storage.get_global("probe_depth"),
        Some(&Value::Int(1))
    );
}

#[test]
fn io_staging_capacity_failure_preserves_existing_bound_values_and_outputs() {
    let prepared = PreparedModule::from_bytes(
        include_bytes!("../../../../trust-runtime/tests/fixtures/portability/f401/gpio.stbc"),
        crate::vm::PreparationLimits::default(),
    )
    .unwrap();
    let mut state = EngineState::new(&prepared, 0, &services::LOGICAL_ONLY).unwrap();
    let Some(Value::Instance(plant)) = state.storage.get_global("Plant") else {
        panic!("root");
    };
    let plant = *plant;
    state.images.inputs[0] = 1;
    state
        .resources
        .construction_bytes
        .set(prepared.limits.max_construction_bytes);
    assert_eq!(
        state.sample_input_image(),
        Err(RuntimeError::ExecutionTimeout)
    );
    assert_eq!(
        state.storage.get_instance_var(plant, "button"),
        Some(&Value::Bool(false))
    );
    state
        .storage
        .set_instance_var(plant, "led", Value::Bool(true));
    assert_eq!(
        state.publish_output_image(),
        Err(RuntimeError::ExecutionTimeout)
    );
    assert_eq!(state.outputs(), &[0]);
}
