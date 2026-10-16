//! Live storage bounds preserve admitted calls, staging and terminal cleanup.
use super::*;
use crate::error::StableErrorCode;

#[test]
fn dynamic_string_index_charges_the_text_scan_before_extending_the_reference() {
    let prepared = PreparedModule::from_bytes(
        include_bytes!("../../../../trust-runtime/tests/fixtures/portability/f401/gpio.stbc"),
        crate::vm::PreparationLimits::default(),
    )
    .unwrap();
    let mut data = EngineState::construct_data(&prepared, 0, &services::LOGICAL_ONLY).unwrap();
    data.storage
        .set_global("probe_depth", Value::String("abcdefgh".into()));
    let reference = data.storage.ref_for_global("probe_depth").unwrap();
    data.resources.construction_bytes.set(0);
    data.resources.work_budget.reset(4);
    let state = EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY);
    let error = crate::vm::dispatch_refs::dynamic_ref_index(
        &state,
        &crate::vm::FrameStack::default(),
        reference,
        8,
    )
    .unwrap_err()
    .into_runtime_error();
    assert!(matches!(
        error,
        RuntimeError::ExecutionLimit {
            kind: crate::error::ExecutionLimitKind::Work,
            requested: 8,
            limit: 4,
            ..
        }
    ));
    assert_eq!(state.data.resources.construction_bytes.get(), 0);
    assert_eq!(
        state.data.storage.get_global("probe_depth"),
        Some(&Value::String("abcdefgh".into()))
    );
}

#[test]
fn dynamic_multidimensional_reference_growth_is_charged_before_allocation() {
    let prepared = PreparedModule::from_bytes(
        include_bytes!("../../../../trust-runtime/tests/fixtures/portability/f401/gpio.stbc"),
        crate::vm::PreparationLimits::default(),
    )
    .unwrap();
    let mut data = EngineState::construct_data(&prepared, 0, &services::LOGICAL_ONLY).unwrap();
    // Isolate owned reference growth; full admitted dynamic execution is covered
    // separately by portable_register_parity and the source-free fixture.
    data.storage.set_global(
        "probe_depth",
        Value::Array(alloc::boxed::Box::new(
            crate::value::ArrayValue::from_canonical_parts(
                vec![Value::Int(7); 4],
                vec![(1, 2), (1, 2)],
            ),
        )),
    );
    let reference = data.storage.ref_for_global("probe_depth").unwrap();
    data.resources.construction_bytes.set(0);
    data.resources.work_budget.reset(100);
    let state = EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY);
    let frames = crate::vm::FrameStack::default();
    // TIME-08: the new segment and its subscript cost one unit each, with their own bytes.
    let partial =
        crate::vm::dispatch_refs::dynamic_ref_index(&state, &frames, reference, 1).unwrap();
    let first = core::mem::size_of::<crate::value::RefSegment>() + core::mem::size_of::<i64>();
    assert_eq!(state.data.resources.construction_bytes.get(), first);
    assert_eq!(state.data.resources.work_budget.remaining(), 98);
    let pointer = partial.path.as_ptr();
    let complete =
        crate::vm::dispatch_refs::dynamic_ref_index(&state, &frames, partial, 2).unwrap();
    assert_eq!(complete.path.as_ptr(), pointer);
    assert_eq!(
        complete.path,
        vec![crate::value::RefSegment::Index(vec![1, 2])]
    );
    // TIME-08: the trailing subscript costs one unit and one i64, not a copy of the earlier one.
    assert_eq!(
        state.data.resources.construction_bytes.get(),
        first + core::mem::size_of::<i64>()
    );
    assert_eq!(state.data.resources.work_budget.remaining(), 97);

    let mut partial = complete;
    let crate::value::RefSegment::Index(indices) = &mut partial.path[0] else {
        panic!("index")
    };
    *indices = vec![1];
    state
        .data
        .resources
        .construction_bytes
        .set(prepared.limits.max_construction_bytes);
    let error = crate::vm::dispatch_refs::dynamic_ref_index(&state, &frames, partial, 2)
        .unwrap_err()
        .into_runtime_error();
    assert!(matches!(error, RuntimeError::ExecutionLimit {
        kind: crate::error::ExecutionLimitKind::AllocationBytes, requested, ..
    } if requested == core::mem::size_of::<i64>()));
    assert_eq!(
        state.data.resources.construction_bytes.get(),
        prepared.limits.max_construction_bytes
    );
}

#[test]
fn equal_length_reference_paths_charge_the_same_whatever_their_capacity() {
    let prepared = PreparedModule::from_bytes(
        include_bytes!("../../../../trust-runtime/tests/fixtures/portability/f401/gpio.stbc"),
        crate::vm::PreparationLimits::default(),
    )
    .unwrap();
    let mut data = EngineState::construct_data(&prepared, 0, &services::LOGICAL_ONLY).unwrap();
    data.storage.set_global(
        "probe_depth",
        Value::Array(alloc::boxed::Box::new(
            crate::value::ArrayValue::from_canonical_parts(
                vec![Value::Int(7); 4],
                vec![(1, 2), (1, 2)],
            ),
        )),
    );
    let reference = data.storage.ref_for_global("probe_depth").unwrap();
    data.resources.construction_bytes.set(0);
    data.resources.work_budget.reset(100);
    let state = EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY);
    let frames = crate::vm::FrameStack::default();
    let segment = core::mem::size_of::<crate::value::RefSegment>();
    let subscript = core::mem::size_of::<i64>();

    // Two empty paths with different starting capacities each add a segment and its subscript.
    for capacity in [0, 8] {
        let mut root = reference.clone();
        root.path = alloc::vec::Vec::with_capacity(capacity);
        state.data.resources.construction_bytes.set(0);
        state.data.resources.work_budget.reset(100);
        let next = crate::vm::dispatch_refs::dynamic_ref_index(&state, &frames, root, 1).unwrap();
        assert_eq!(next.path, vec![crate::value::RefSegment::Index(vec![1])]);
        assert_eq!(state.data.resources.work_budget.remaining(), 98);
        assert_eq!(
            state.data.resources.construction_bytes.get(),
            segment + subscript
        );
    }

    // Two one-segment paths with different subscript capacities each add one subscript.
    for capacity in [1, 4] {
        let mut indices = alloc::vec::Vec::with_capacity(capacity);
        indices.push(1);
        let mut partial = reference.clone();
        partial.path = vec![crate::value::RefSegment::Index(indices)];
        state.data.resources.construction_bytes.set(0);
        state.data.resources.work_budget.reset(100);
        let next =
            crate::vm::dispatch_refs::dynamic_ref_index(&state, &frames, partial, 2).unwrap();
        assert_eq!(next.path, vec![crate::value::RefSegment::Index(vec![1, 2])]);
        assert_eq!(state.data.resources.work_budget.remaining(), 99);
        assert_eq!(state.data.resources.construction_bytes.get(), subscript);
    }
}

#[cfg(feature = "register-ir")]
#[test]
fn borrowed_dynamic_field_materialization_charges_the_new_owned_path() {
    let prepared = PreparedModule::from_bytes(
        include_bytes!("../../../../trust-runtime/tests/fixtures/portability/f401/gpio.stbc"),
        crate::vm::PreparationLimits::default(),
    )
    .unwrap();
    let mut data = EngineState::construct_data(&prepared, 0, &services::LOGICAL_ONLY).unwrap();
    data.storage.set_global(
        "probe_depth",
        Value::Struct(alloc::sync::Arc::new(
            crate::value::StructValue::from_canonical_parts(
                "Record".into(),
                crate::collections::OrderedMap::from_iter([("field".into(), Value::Int(7))]),
            ),
        )),
    );
    let reference = data.storage.ref_for_global("probe_depth").unwrap();
    data.resources.construction_bytes.set(0);
    let state = EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY);
    let frames = crate::vm::FrameStack::default();
    let next = crate::vm::dispatch_refs::dynamic_ref_field_borrowed(
        &state,
        &frames,
        &reference,
        "field".into(),
    )
    .unwrap();
    assert_eq!(
        next.path,
        vec![crate::value::RefSegment::Field("field".into())]
    );
    assert_eq!(
        state.data.resources.construction_bytes.get(),
        core::mem::size_of::<crate::value::RefSegment>()
    );
    assert!(reference.path.is_empty());
    state
        .data
        .resources
        .construction_bytes
        .set(prepared.limits.max_construction_bytes);
    let error = crate::vm::dispatch_refs::dynamic_ref_field_borrowed(
        &state,
        &frames,
        &reference,
        "field".into(),
    )
    .unwrap_err()
    .into_runtime_error();
    assert!(matches!(
        error,
        RuntimeError::ExecutionLimit {
            kind: crate::error::ExecutionLimitKind::AllocationBytes,
            ..
        }
    ));
    assert!(reference.path.is_empty());
}

#[test]
fn reference_address_clones_are_charged_before_copying_a_static_path() {
    let prepared = PreparedModule::from_bytes(
        include_bytes!(
            "../../../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc"
        ),
        crate::vm::PreparationLimits::default(),
    )
    .unwrap();
    let mut module = prepared.vm.clone();
    let path = vec![
        crate::value::RefSegment::Field("counter".into()),
        crate::value::RefSegment::Field("history".into()),
        crate::value::RefSegment::Index(vec![1]),
    ];
    let index = module.refs.len() as u32;
    module.refs.push(crate::vm::module::VmRef::Global {
        offset: 0,
        path: path.clone(),
    });
    module.ref_types.push(None);
    let mut data = EngineState::construct_data(&prepared, 0, &services::LOGICAL_ONLY).unwrap();
    let state = EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY);
    state.data.resources.construction_bytes.set(0);
    state.data.resources.work_budget.reset(64);
    let reference = crate::vm::dispatch_refs::load_ref_addr(
        &state,
        &module,
        &crate::vm::FrameStack::default(),
        index,
    )
    .unwrap();
    assert_eq!(reference.path, path);
    let bytes = 3 * core::mem::size_of::<crate::value::RefSegment>() + core::mem::size_of::<i64>();
    assert_eq!(state.data.resources.construction_bytes.get(), bytes);
    // TIME-08 COPY: materializing the static address copies 3 path segments and
    // 1 index = 4 units; the REF instruction's own unit is charged by dispatch.
    assert_eq!(state.data.resources.work_budget.remaining(), 60);
    state
        .data
        .resources
        .construction_bytes
        .set(prepared.limits.max_construction_bytes);
    assert_eq!(
        crate::vm::dispatch_refs::load_ref_addr(
            &state,
            &module,
            &crate::vm::FrameStack::default(),
            index
        )
        .unwrap_err()
        .into_runtime_error()
        .stable_code(),
        StableErrorCode::RuntimeExecutionTimeout,
    );
}

#[test]
fn prepared_ready_buffer_is_reused_and_cleared_between_resource_cycles() {
    let prepared = PreparedModule::from_bytes(
        include_bytes!(
            "../../../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc"
        ),
        crate::vm::PreparationLimits::default(),
    )
    .unwrap();
    let mut data = EngineState::construct_data(&prepared, 0, &services::LOGICAL_ONLY).unwrap();
    let mut state = EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY);
    let pointer = state.data.resources.ready_tasks.as_ptr();
    let capacity = state.data.resources.ready_tasks.capacity();
    assert!(capacity >= state.data.resource.tasks.len());
    for ms in (0..=1000).step_by(10) {
        state.execute_cycle(Duration::from_millis(ms)).unwrap();
        assert_eq!(state.data.resources.ready_tasks.as_ptr(), pointer);
        assert_eq!(state.data.resources.ready_tasks.capacity(), capacity);
        assert!(state.data.resources.ready_tasks.is_empty());
    }
    assert_eq!(state.data.tasks[0].overrun_count, 0);
}

#[test]
fn failed_cycle_returns_the_prepared_ready_buffer_before_latching_fault() {
    struct FailingStore;
    impl ExecutionServices for FailingStore {
        fn deadline_exceeded(&self) -> bool {
            false
        }
        fn has_wall_clock(&self) -> bool {
            false
        }
        fn has_retain_store(&self) -> bool {
            true
        }
        fn current_dt(&self) -> Result<crate::value::DateTimeValue, RuntimeError> {
            Err(RuntimeError::UndefinedFunction("CURRENT_DT".into()))
        }
        fn save_retain_snapshot(
            &self,
            _: &crate::retain::RetainSnapshot,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::RetainStore("injected store failure".into()))
        }
    }
    let prepared = PreparedModule::from_bytes(
        include_bytes!(
            "../../../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc"
        ),
        crate::vm::PreparationLimits::default(),
    )
    .unwrap();
    let mut data = EngineState::construct_data(&prepared, 0, &FailingStore).unwrap();
    let mut state = EngineState::borrow(&mut data, &prepared, &FailingStore);
    let pointer = state.data.resources.ready_tasks.as_ptr();
    let capacity = state.data.resources.ready_tasks.capacity();
    let outputs = state.outputs().to_vec();
    assert!(matches!(
        state.execute_cycle(Duration::from_millis(30)),
        Err(RuntimeError::RetainStore(_))
    ));
    assert_eq!(state.data.resources.ready_tasks.as_ptr(), pointer);
    assert_eq!(state.data.resources.ready_tasks.capacity(), capacity);
    assert!(state.data.resources.ready_tasks.is_empty());
    assert_eq!(state.outputs(), outputs);
    assert_eq!(
        state.execute_cycle(Duration::from_millis(50)),
        Err(RuntimeError::ResourceFaulted)
    );
}

#[test]
fn four_deep_gpio_call_with_primitive_defaults_fits_admitted_storage() {
    let prepared = PreparedModule::from_bytes(
        include_bytes!("../../../../trust-runtime/tests/fixtures/portability/f401/gpio.stbc"),
        crate::vm::PreparationLimits {
            max_call_depth: 4,
            ..Default::default()
        },
    )
    .unwrap();
    let mut state_data =
        EngineState::construct_data(&prepared, 0, &services::LOGICAL_ONLY).unwrap();
    let mut state = EngineState::borrow(&mut state_data, &prepared, &services::LOGICAL_ONLY);
    let activation_capacity = state.data.storage.execution_frame_capacity();
    assert!(activation_capacity >= 8);
    let Some(Value::Instance(plant)) = state.data.storage.get_global("Plant") else {
        panic!("root");
    };
    let plant = *plant;
    state.data.storage.set_global("probe_depth", Value::Int(4));
    for scan in 1..=32 {
        state
            .execute_cycle(Duration::from_millis(scan * 10))
            .unwrap();
        assert_eq!(
            state.data.storage.get_instance_var(plant, "depth_result"),
            Some(&Value::Int(4))
        );
        assert_eq!(state.data.storage.execution_frame_count(), 0);
        assert_eq!(state.data.storage.execution_activation_count(), 0);
        assert_eq!(
            state.data.storage.execution_frame_capacity(),
            activation_capacity
        );
    }
    assert_eq!(state.data.storage.execution_frame_count(), 0);
    assert_eq!(state.data.storage.execution_activation_count(), 0);
}

#[test]
fn executable_initializer_fails_cleanly_when_staging_frame_capacity_is_full() {
    let prepared = PreparedModule::from_bytes(
        include_bytes!(
            "../../../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc"
        ),
        crate::vm::PreparationLimits::default(),
    )
    .unwrap();
    let mut data = EngineState::construct_data(&prepared, 0, &services::LOGICAL_ONLY).unwrap();
    let mut state = EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY);
    let Some(Value::Instance(plant)) = state.data.storage.get_global("Plant") else {
        panic!("fixture root");
    };
    let plant = *plant;
    let (initializer, name) = prepared
        .initializers
        .entries
        .iter()
        .enumerate()
        .find_map(|(id, entry)| {
            if entry.stage != crate::bytecode::InitializationStage::Explicit
                || entry.code_length == 0
            {
                return None;
            }
            let declaration = entry.declaration_idx?;
            let declaration = prepared.layout.entries.get(declaration as usize)?;
            let name = prepared.vm.strings.get(declaration.name_idx as usize)?;
            if !name.eq_ignore_ascii_case("enabled") {
                return None;
            }
            Some((u32::try_from(id).ok()?, name.clone()))
        })
        .expect("fixture executable BOOL initializer");
    let original = state
        .data
        .storage
        .get_instance_var(plant, name.as_str())
        .cloned();

    assert_eq!(state.data.storage.execution_frame_count(), 0);
    state.data.storage.reserve_execution_frames(1).unwrap();
    let occupied = state.data.storage.reserve_execution_frame().unwrap();
    let error = state
        .evaluate_initializer(initializer, None, Some(plant), 0)
        .unwrap_err();
    assert_eq!(error.stable_code(), StableErrorCode::VmCallStackOverflow);
    assert_eq!(state.data.storage.execution_frame_count(), 1);
    assert!(state.data.storage.execution_frame_is_live(occupied));
    assert_eq!(state.data.storage.execution_activation_count(), 0);
    assert!(state.data.construction.initializers.is_empty());
    assert!(state.data.construction.frames.is_empty());
    assert!(state.data.construction.types.is_empty());
    assert_eq!(
        state.data.storage.get_instance_var(plant, name.as_str()),
        original.as_ref()
    );
    assert!(state.data.storage.release_execution_frame(occupied));
    assert_eq!(state.data.storage.execution_frame_count(), 0);
    let next = state.data.storage.reserve_execution_frame().unwrap();
    assert_eq!(next, crate::memory::FrameId(occupied.0 + 1));
    state.data.storage.release_execution_frame(next);
}

#[test]
fn zero_allowance_cleanup_still_retires_owned_instances_and_preserves_promoted_ones() {
    let prepared = PreparedModule::from_bytes(
        include_bytes!(
            "../../../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc"
        ),
        crate::vm::PreparationLimits::default(),
    )
    .unwrap();
    let mut state_data =
        EngineState::construct_data(&prepared, 0, &services::LOGICAL_ONLY).unwrap();
    let mut state = EngineState::borrow(&mut state_data, &prepared, &services::LOGICAL_ONLY);
    let old = state.data.storage.reserve_execution_frame().unwrap();
    let new = state.data.storage.reserve_execution_frame().unwrap();
    state.data.storage.activate_execution_frame(old).unwrap();
    state.data.storage.activate_execution_frame(new).unwrap();
    let pou = *prepared.vm.function_block_ids.get("COUNTER").unwrap();
    let retired = state.reserve_instance(pou, Some(old)).unwrap();
    let promoted = state.reserve_instance(pou, Some(old)).unwrap();
    state
        .promote_constructed_instances(&Value::Instance(promoted), old, Some(new))
        .unwrap();
    state
        .data
        .construction
        .initialized_declarations
        .insert((0, Some(retired)))
        .unwrap();
    state
        .data
        .construction
        .once
        .insert((0, Some(retired)))
        .unwrap();
    state.data.resources.work_budget.reset(0);
    // TIME-08: owned-instance retirement is lifecycle bookkeeping. An empty
    // allowance neither refuses the cleanup nor draws work from it.
    state.remove_owned_instances(old).unwrap();
    assert_eq!(state.data.resources.work_budget.remaining(), 0);
    assert!(state.data.storage.get_instance(retired).is_none());
    assert!(state.data.storage.get_instance(promoted).is_some());
    assert!(!state
        .data
        .lifetimes
        .instance_lifetimes
        .contains_key(&retired));
    assert!(!state
        .data
        .construction
        .initialized_declarations
        .contains(&(0, Some(retired)))
        .unwrap());
    assert!(!state
        .data
        .construction
        .once
        .contains(&(0, Some(retired)))
        .unwrap());
    state
        .data
        .resources
        .work_budget
        .reset(prepared.limits.max_work);
    state.remove_owned_instances(new).unwrap();
    assert!(state.data.storage.get_instance(promoted).is_none());
}

#[test]
fn journal_growth_budget_failure_does_not_publish_snapshot_entries_or_writes() {
    let prepared = PreparedModule::from_bytes(
        include_bytes!("../../../../trust-runtime/tests/fixtures/portability/f401/gpio.stbc"),
        crate::vm::PreparationLimits::default(),
    )
    .unwrap();
    let mut state_data =
        EngineState::construct_data(&prepared, 0, &services::LOGICAL_ONLY).unwrap();
    let state = EngineState::borrow(&mut state_data, &prepared, &services::LOGICAL_ONLY);
    let reference = state.data.storage.ref_for_global("probe_depth").unwrap();
    state.data.resources.output_journal = Some(destinations::SavedDestinations::new());
    state
        .data
        .resources
        .construction_bytes
        .set(prepared.limits.max_construction_bytes);
    let result = state.snapshot_destinations(None, core::iter::once(reference.as_view()));
    assert!(matches!(
        result,
        Err(RuntimeError::ExecutionLimit {
            kind: crate::error::ExecutionLimitKind::AllocationBytes,
            ..
        })
    ));
    assert_eq!(
        state.data.resources.output_journal.as_ref().unwrap().len(),
        0
    );
    assert_eq!(
        state.data.storage.get_global("probe_depth"),
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
    let mut state_data =
        EngineState::construct_data(&prepared, 0, &services::LOGICAL_ONLY).unwrap();
    let mut state = EngineState::borrow(&mut state_data, &prepared, &services::LOGICAL_ONLY);
    let Some(Value::Instance(plant)) = state.data.storage.get_global("Plant") else {
        panic!("root");
    };
    let plant = *plant;
    state.data.images.inputs[0] = 1;
    state
        .data
        .resources
        .construction_bytes
        .set(prepared.limits.max_construction_bytes);
    assert!(matches!(
        state.sample_input_image(),
        Err(RuntimeError::ExecutionLimit {
            kind: crate::error::ExecutionLimitKind::AllocationBytes,
            ..
        })
    ));
    assert_eq!(
        state.data.storage.get_instance_var(plant, "button"),
        Some(&Value::Bool(false))
    );
    state
        .data
        .storage
        .set_instance_var(plant, "led", Value::Bool(true));
    assert!(matches!(
        state.publish_output_image(),
        Err(RuntimeError::ExecutionLimit {
            kind: crate::error::ExecutionLimitKind::AllocationBytes,
            ..
        })
    ));
    assert_eq!(state.outputs(), &[0]);
}
