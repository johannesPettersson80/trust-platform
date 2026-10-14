use super::*;
use crate::memory::InstanceId;
use crate::vm::PreparationLimits;
use alloc::vec;

fn prepared() -> PreparedModule {
    PreparedModule::from_bytes(
        include_bytes!(
            "../../../../../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc"
        ),
        PreparationLimits::default(),
    )
    .unwrap()
}

fn declaration(prepared: &PreparedModule, owner: StorageOwner) -> u32 {
    prepared
        .layout
        .entries
        .iter()
        .position(|entry| {
            entry.owner == owner
                && entry.owns_storage()
                && (owner != StorageOwner::Instance || entry.role == StorageRole::Variable)
        })
        .and_then(|index| u32::try_from(index).ok())
        .expect("fixture has an owning variable declaration")
}

fn frame(slot: usize, initial: Value) -> VmFrame {
    VmFrame {
        activation: None,
        pou_id: None,
        return_pc: 0,
        code_start: 0,
        code_end: 0,
        local_ref_start: 0,
        local_ref_count: 0,
        locals: vec![initial; slot + 1],
        parameter_values_present: Vec::new(),
        runtime_instance: None,
        instance_owner: None,
    }
}

fn state_data(prepared: &PreparedModule) -> alloc::boxed::Box<super::super::super::EngineData> {
    EngineState::construct_data(prepared, 0, &super::super::super::services::LOGICAL_ONLY).unwrap()
}

#[test]
fn frame_commit_validates_name_index_before_charging_or_mutating() {
    let mut prepared = prepared();
    let declaration = declaration(&prepared, StorageOwner::Frame);
    let slot = prepared.layout.entries[declaration as usize].slot as usize;
    let mut data = state_data(&prepared);
    prepared.layout.entries[declaration as usize].name_idx = u32::MAX;
    let mut state = EngineState::borrow(
        &mut data,
        &prepared,
        &super::super::super::services::LOGICAL_ONLY,
    );
    state.data.resources.work_budget.reset(17);
    let mut frame = frame(slot, Value::DInt(-1));

    assert_eq!(
        state.commit_declaration(declaration, None, Some(&mut frame), Value::DInt(7),),
        Err(RuntimeError::InvalidExecutionState)
    );
    assert_eq!(state.data.resources.work_budget.remaining(), 17);
    assert_eq!(frame.locals[slot], Value::DInt(-1));
    assert!(!state
        .data
        .construction
        .initialized_declarations
        .contains(&(declaration, None))
        .unwrap());
}

#[test]
fn frame_commits_publish_values_without_persistent_markers_or_marker_allocation() {
    let mut prepared = prepared();
    let declaration = declaration(&prepared, StorageOwner::Frame);
    let slot = prepared.layout.entries[declaration as usize].slot as usize;
    let mut data = state_data(&prepared);
    let before = data.construction.initialized_declarations.len();
    prepared.limits.max_construction_bytes = 0;
    data.resources.construction_bytes.set(0);
    let mut state = EngineState::borrow(
        &mut data,
        &prepared,
        &super::super::super::services::LOGICAL_ONLY,
    );
    let mut frame = frame(slot, Value::Int(-1));
    // A method invocation can carry an instance identity, but its local
    // declaration still has activation-local visibility and no instance marker.
    for (instance, value) in [(None, 7), (Some(InstanceId(777)), 11), (None, 13)] {
        state.data.resources.work_budget.reset(1);
        state
            .commit_declaration(declaration, instance, Some(&mut frame), Value::Int(value))
            .unwrap();
        assert_eq!(frame.locals[slot], Value::Int(value));
        // TIME-08: the scalar lifetime check and frame publication charge no work.
        assert_eq!(state.data.resources.work_budget.remaining(), 1);
        assert_eq!(state.data.resources.construction_bytes.get(), 0);
        assert_eq!(
            state.data.construction.initialized_declarations.len(),
            before
        );
        assert!(!state
            .data
            .construction
            .initialized_declarations
            .contains(&(declaration, instance))
            .unwrap());
    }
}

#[test]
fn frame_commit_still_rejects_stale_references_before_publication() {
    let prepared = prepared();
    let declaration = declaration(&prepared, StorageOwner::Frame);
    let slot = prepared.layout.entries[declaration as usize].slot as usize;
    let mut data = state_data(&prepared);
    let mut state = EngineState::borrow(
        &mut data,
        &prepared,
        &super::super::super::services::LOGICAL_ONLY,
    );
    state.data.resources.work_budget.reset(100);
    let mut frame = frame(slot, Value::Int(-1));
    let invalid = Value::Reference(Some(crate::value::ValueRef {
        location: crate::memory::MemoryLocation::Local(crate::memory::FrameId(u32::MAX)),
        offset: 0,
        path: Vec::new(),
    }));
    let marks = state.data.construction.initialized_declarations.len();
    assert_eq!(
        state.commit_declaration(declaration, None, Some(&mut frame), invalid),
        Err(RuntimeError::ReferenceLifetime)
    );
    assert_eq!(frame.locals[slot], Value::Int(-1));
    assert_eq!(
        state.data.construction.initialized_declarations.len(),
        marks
    );
}

#[test]
fn frame_commit_validates_slot_without_consuming_work() {
    let prepared = prepared();
    let declaration = declaration(&prepared, StorageOwner::Frame);
    let mut data = state_data(&prepared);
    let mut state = EngineState::borrow(
        &mut data,
        &prepared,
        &super::super::super::services::LOGICAL_ONLY,
    );
    // TIME-08: a scalar commit and its frame bookkeeping charge nothing, so an
    // exhausted allowance does not preempt the slot validation.
    state.data.resources.work_budget.reset(0);
    assert_eq!(
        state.commit_declaration(declaration, None, None, Value::Int(1)),
        Err(RuntimeError::InvalidExecutionState)
    );
    assert_eq!(state.data.resources.work_budget.remaining(), 0);
    state.data.resources.work_budget.reset(1);
    assert_eq!(
        state.commit_declaration(declaration, None, None, Value::Int(1)),
        Err(RuntimeError::InvalidExecutionState)
    );
    let mut frame = frame(0, Value::Int(-1));
    frame.locals.clear();
    state.data.resources.work_budget.reset(1);
    assert_eq!(
        state.commit_declaration(declaration, None, Some(&mut frame), Value::Int(1)),
        Err(RuntimeError::InvalidExecutionState)
    );
    assert!(frame.locals.is_empty());
}

#[test]
fn owned_graph_membership_admission_covers_non_power_of_two_searches() {
    let prepared = prepared();
    let mut data = state_data(&prepared);
    let state = EngineState::borrow(
        &mut data,
        &prepared,
        &super::super::super::services::LOGICAL_ONLY,
    );
    let target = InstanceId(u32::MAX);
    let limit = prepared.limits.max_construction_bytes;

    for count in [3usize, 5] {
        let mut visited = super::super::super::InstanceSet::default();
        for id in 0..u32::try_from(count).unwrap() {
            assert!(visited.insert(InstanceId(id)).unwrap());
        }

        // TIME-08: the membership search is bookkeeping and charges no work. The
        // insertion's allocation admission is the refusal, and it precedes the insert.
        state.data.resources.construction_bytes.set(limit);
        state.data.resources.work_budget.reset(1);
        assert_eq!(
            state.collect_owned_instance_graph(&Value::Instance(target), 0, &mut visited),
            Err(RuntimeError::ExecutionLimit {
                kind: crate::error::ExecutionLimitKind::AllocationBytes,
                used: limit,
                requested: 8 * core::mem::size_of::<InstanceId>(),
                limit,
            }),
            "visited length={count}"
        );
        assert_eq!(state.data.resources.work_budget.remaining(), 1);
        assert_eq!(visited.len(), count);
        assert!(!visited.contains(&target));
    }
}

#[test]
fn failed_instance_storage_write_does_not_publish_its_initialized_mark() {
    let prepared = prepared();
    let declaration = declaration(&prepared, StorageOwner::Instance);
    let instance = InstanceId(u32::MAX);
    let mut data = state_data(&prepared);
    let mut state = EngineState::borrow(
        &mut data,
        &prepared,
        &super::super::super::services::LOGICAL_ONLY,
    );
    state
        .data
        .resources
        .work_budget
        .reset(prepared.limits.max_work);
    let before = state.data.construction.initialized_declarations.len();

    assert_eq!(
        state.commit_declaration(declaration, Some(instance), None, Value::DInt(7)),
        Err(RuntimeError::InvalidExecutionState)
    );
    assert_eq!(
        state.data.construction.initialized_declarations.len(),
        before
    );
    assert!(!state
        .data
        .construction
        .initialized_declarations
        .contains(&(declaration, Some(instance)))
        .unwrap());
    assert!(state.data.storage.get_instance(instance).is_none());
}

#[test]
fn marker_allocation_refusals_precede_instance_publication() {
    let mut prepared = prepared();
    let declaration = prepared
        .layout
        .entries
        .iter()
        .position(|entry| {
            entry.owner == StorageOwner::Instance
                && entry.role == StorageRole::Variable
                && prepared.vm.strings[entry.name_idx as usize] == "value"
        })
        .unwrap() as u32;
    let owner = prepared.layout.entries[declaration as usize]
        .owner_pou_id
        .unwrap();
    let mut data = state_data(&prepared);
    let instance = *data
        .storage
        .instances()
        .iter()
        .find(|(id, _)| data.construction.instance_templates.get(id) == Some(&owner))
        .unwrap()
        .0;
    let previous = data
        .storage
        .get_instance_var(instance, "value")
        .unwrap()
        .clone();
    // Resource construction has already populated/reserved marker storage.
    // Isolate the first insertion so this test actually reaches growth admission.
    data.construction.initialized_declarations = Default::default();
    let (bytes, _) = data
        .construction
        .initialized_declarations
        .insertion_demand((declaration, Some(instance)))
        .unwrap();
    let marker_capacity_before = data
        .construction
        .initialized_declarations
        .capacity_state_for_test();
    assert!(bytes > 0, "the tested insertion must require allocation");
    let mut state = EngineState::borrow(
        &mut data,
        &prepared,
        &super::super::super::services::LOGICAL_ONLY,
    );

    // Bookkeeping charges no work, so the marker's allocation admission refuses
    // first and the one-unit allowance is left untouched.
    let limit = prepared.limits.max_construction_bytes;
    state.data.resources.construction_bytes.set(limit);
    state.data.resources.work_budget.reset(1);
    assert!(matches!(
        state.commit_declaration(declaration, Some(instance), None, Value::Int(7),),
        Err(RuntimeError::ExecutionLimit {
            kind: crate::error::ExecutionLimitKind::AllocationBytes,
            ..
        })
    ));
    assert_eq!(state.data.resources.work_budget.remaining(), 1);
    assert_eq!(state.data.resources.construction_bytes.get(), limit);
    assert_eq!(
        state
            .data
            .construction
            .initialized_declarations
            .capacity_state_for_test(),
        marker_capacity_before
    );
    assert_eq!(
        state.data.storage.get_instance_var(instance, "value"),
        Some(&previous)
    );
    assert!(!state
        .data
        .construction
        .initialized_declarations
        .contains(&(declaration, Some(instance)))
        .unwrap());

    prepared.limits.max_construction_bytes = 0;
    data.resources.construction_bytes.set(0);
    let mut state = EngineState::borrow(
        &mut data,
        &prepared,
        &super::super::super::services::LOGICAL_ONLY,
    );
    state.data.resources.work_budget.reset(1);
    assert!(matches!(
        state.commit_declaration(declaration, Some(instance), None, Value::Int(11),),
        Err(RuntimeError::ExecutionLimit {
            kind: crate::error::ExecutionLimitKind::AllocationBytes,
            ..
        })
    ));
    assert_eq!(state.data.resources.work_budget.remaining(), 1);
    assert_eq!(state.data.resources.construction_bytes.get(), 0);
    assert_eq!(
        state
            .data
            .construction
            .initialized_declarations
            .capacity_state_for_test(),
        marker_capacity_before
    );
    assert_eq!(
        state.data.storage.get_instance_var(instance, "value"),
        Some(&previous)
    );
    assert!(!state
        .data
        .construction
        .initialized_declarations
        .contains(&(declaration, Some(instance)))
        .unwrap());
}

#[test]
fn native_state_mark_is_admitted_before_publication() {
    let mut prepared = prepared();
    let pou = *prepared
        .vm
        .function_block_ids
        .get("COUNTER")
        .expect("fixture Counter FB");
    let declarations = prepared
        .declarations(StorageOwner::Instance, Some(pou))
        .to_vec();
    let declaration = *declarations.first().expect("Counter instance slot");
    for &id in &declarations {
        let entry = &mut prepared.layout.entries[id as usize];
        if id == declaration {
            entry.role = StorageRole::NativeState;
            entry.owner_pou_id = Some(pou);
        } else if matches!(
            entry.role,
            StorageRole::NativeState | StorageRole::EdgePhase
        ) {
            entry.role = StorageRole::Variable;
        }
    }

    let mut data = state_data(&prepared);
    let instance = *data
        .storage
        .instances()
        .iter()
        .find(|(id, _)| data.construction.instance_templates.get(id) == Some(&pou))
        .expect("constructed Counter instance")
        .0;
    data.construction.initialized_declarations = Default::default();
    data.construction.initialized_instances = Default::default();
    prepared.limits.max_construction_bytes = 0;
    data.resources.construction_bytes.set(0);

    let key = (declaration, Some(instance));
    let (bytes, _) = data
        .construction
        .initialized_declarations
        .insertion_demand(key)
        .unwrap();
    let marker_capacity_before = data
        .construction
        .initialized_declarations
        .capacity_state_for_test();
    assert!(bytes > 0, "first sparse word requires capacity");
    assert!(
        declarations.contains(&declaration),
        "selected declaration is in the owner range"
    );

    let mut state = EngineState::borrow(
        &mut data,
        &prepared,
        &super::super::super::services::LOGICAL_ONLY,
    );
    let marker = key;
    let field_name = prepared.vm.strings
        [prepared.layout.entries[declaration as usize].name_idx as usize]
        .clone();
    let before_value = state
        .data
        .storage
        .get_instance_var(instance, &field_name)
        .cloned();
    let instance_count = state.data.storage.instances().len();
    // TIME-08: the native marker's admission is an allocation. An exhausted work
    // allowance neither refuses it earlier nor is consumed by it.
    state.data.resources.work_budget.reset(0);
    assert!(matches!(
        state.initialize_instance(instance, false),
        Err(RuntimeError::ExecutionLimit {
            kind: crate::error::ExecutionLimitKind::AllocationBytes,
            used: 0,
            limit: 0,
            ..
        })
    ));
    assert_eq!(state.data.resources.work_budget.remaining(), 0);
    assert!(!state
        .data
        .construction
        .initialized_declarations
        .contains(&marker)
        .unwrap());
    assert_eq!(
        state
            .data
            .construction
            .initialized_declarations
            .capacity_state_for_test(),
        marker_capacity_before
    );
    assert!(!state
        .data
        .construction
        .initialized_instances
        .contains(&instance));
    assert_eq!(state.data.storage.instances().len(), instance_count);
    assert_eq!(
        state.data.storage.get_instance_var(instance, &field_name),
        before_value.as_ref()
    );

    state
        .data
        .resources
        .work_budget
        .reset(prepared.limits.max_work);
    assert!(matches!(
        state.initialize_instance(instance, false),
        Err(RuntimeError::ExecutionLimit {
            kind: crate::error::ExecutionLimitKind::AllocationBytes,
            ..
        })
    ));
    assert_eq!(state.data.resources.construction_bytes.get(), 0);
    assert!(!state
        .data
        .construction
        .initialized_declarations
        .contains(&marker)
        .unwrap());
    assert!(!state
        .data
        .construction
        .initialized_instances
        .contains(&instance));
    assert_eq!(state.data.storage.instances().len(), instance_count);
    assert_eq!(
        state.data.storage.get_instance_var(instance, &field_name),
        before_value.as_ref()
    );
    assert_eq!(
        state
            .data
            .construction
            .initialized_declarations
            .capacity_state_for_test(),
        marker_capacity_before
    );
}

#[test]
fn lifecycle_mark_allocation_refusal_precedes_global_and_instance_value_writes() {
    for owner in [StorageOwner::Global, StorageOwner::Instance] {
        let prepared = prepared();
        let declaration = declaration(&prepared, owner);
        let entry = &prepared.layout.entries[declaration as usize];
        let name = prepared.vm.strings[entry.name_idx as usize].as_str();
        let mut data = state_data(&prepared);
        let instance = if owner == StorageOwner::Instance {
            let template = entry.owner_pou_id.expect("instance declaration owner");
            Some(
                *data
                    .storage
                    .instances()
                    .iter()
                    .find(|(id, _)| data.construction.instance_templates.get(id) == Some(&template))
                    .expect("constructed declaration owner")
                    .0,
            )
        } else {
            None
        };
        let previous = match owner {
            StorageOwner::Global => data.storage.get_global(name),
            StorageOwner::Instance => data.storage.get_instance_var(instance.unwrap(), name),
            StorageOwner::Frame => unreachable!(),
        }
        .cloned()
        .expect("initialized storage value");
        data.construction.initialized_declarations = Default::default();
        // The empty marker map needs its first allocation; leave the allowance one
        // byte short of it. The instance value's 16-byte visited-set reservation
        // during lifetime validation still fits and precedes marker admission.
        let marker_bytes = data
            .construction
            .initialized_declarations
            .insertion_demand((declaration, instance))
            .unwrap()
            .0;
        assert!(marker_bytes > 16);
        data.resources
            .construction_bytes
            .set(prepared.limits.max_construction_bytes - (marker_bytes - 1));
        let marker_len_before = data.construction.initialized_declarations.len();
        let marker_keys_before = data
            .construction
            .initialized_declarations
            .iter()
            .collect::<alloc::vec::Vec<_>>();
        let marker_capacity_before = data
            .construction
            .initialized_declarations
            .capacity_state_for_test();
        let mut state = EngineState::borrow(
            &mut data,
            &prepared,
            &super::super::super::services::LOGICAL_ONLY,
        );
        state
            .data
            .resources
            .work_budget
            .reset(prepared.limits.max_work);

        assert!(
            matches!(
                state.commit_declaration(declaration, instance, None, previous.clone()),
                Err(RuntimeError::ExecutionLimit {
                    kind: crate::error::ExecutionLimitKind::AllocationBytes,
                    ..
                })
            ),
            "owner={owner:?}"
        );
        let current = match owner {
            StorageOwner::Global => state.data.storage.get_global(name),
            StorageOwner::Instance => state.data.storage.get_instance_var(instance.unwrap(), name),
            StorageOwner::Frame => unreachable!(),
        };
        assert_eq!(current, Some(&previous), "owner={owner:?}");
        assert!(!state
            .data
            .construction
            .initialized_declarations
            .contains(&(declaration, instance))
            .unwrap());
        assert_eq!(
            state.data.construction.initialized_declarations.len(),
            marker_len_before,
            "owner={owner:?}"
        );
        assert_eq!(
            state
                .data
                .construction
                .initialized_declarations
                .iter()
                .collect::<alloc::vec::Vec<_>>(),
            marker_keys_before,
            "owner={owner:?}"
        );
        assert_eq!(
            state
                .data
                .construction
                .initialized_declarations
                .capacity_state_for_test(),
            marker_capacity_before,
            "owner={owner:?}"
        );
    }
}

#[test]
fn initialized_marker_refusal_does_not_publish_prepared_once_marker_or_global_value() {
    let mut prepared = prepared();
    let declaration = declaration(&prepared, StorageOwner::Global);
    let name = prepared.vm.strings[prepared.layout.entries[declaration as usize].name_idx as usize]
        .clone();
    let mut data = state_data(&prepared);
    let previous = data
        .storage
        .get_global(&name)
        .expect("constructed global")
        .clone();
    data.construction.initialized_declarations = Default::default();
    data.construction.once = Default::default();

    // Fill the once map to its capacity with other declarations, so the new once
    // marker needs growth while the empty initialized map needs its first allocation.
    for offset in 1..=64 {
        data.construction
            .once
            .insert((declaration ^ offset, None))
            .unwrap();
        let demand = data
            .construction
            .once
            .insertion_demand((declaration, None))
            .unwrap();
        if demand.0 > 0 {
            break;
        }
    }
    let once_marker = (declaration, None);
    let initialized_marker = (declaration, None);
    let once_demand = data
        .construction
        .once
        .insertion_demand(once_marker)
        .unwrap();
    let initialized_demand = data
        .construction
        .initialized_declarations
        .insertion_demand(initialized_marker)
        .unwrap();
    assert!(once_demand.0 > 0, "the full once map requires growth");
    assert!(initialized_demand.0 > 0);
    let once_keys_before = data
        .construction
        .once
        .iter()
        .collect::<alloc::vec::Vec<_>>();
    let once_capacity_before = data.construction.once.capacity_state_for_test();
    let initialized_capacity_before = data
        .construction
        .initialized_declarations
        .capacity_state_for_test();
    // Marker admission is allocation only (TIME-08): the once marker is admitted, then
    // the initialized marker's allocation refuses. An exhausted allowance changes nothing.
    prepared.limits.max_construction_bytes = once_demand.0;
    data.resources.construction_bytes.set(0);
    {
        let mut state = EngineState::borrow(
            &mut data,
            &prepared,
            &super::super::super::services::LOGICAL_ONLY,
        );
        state.data.resources.work_budget.reset(0);
        assert_eq!(
            state.commit_declaration_with_once(
                declaration,
                None,
                None,
                Value::Int(17),
                Some(once_marker),
            ),
            Err(RuntimeError::ExecutionLimit {
                kind: crate::error::ExecutionLimitKind::AllocationBytes,
                used: once_demand.0,
                requested: initialized_demand.0,
                limit: once_demand.0,
            })
        );
        assert_eq!(state.data.resources.work_budget.remaining(), 0);
        assert_eq!(state.data.resources.construction_bytes.get(), once_demand.0);
        assert_eq!(
            state.data.construction.once.capacity_state_for_test(),
            once_capacity_before
        );
        assert_eq!(
            state
                .data
                .construction
                .initialized_declarations
                .capacity_state_for_test(),
            initialized_capacity_before
        );
        assert_eq!(state.data.storage.get_global(&name), Some(&previous));
    }
    data.resources.construction_bytes.set(0);
    let mut state = EngineState::borrow(
        &mut data,
        &prepared,
        &super::super::super::services::LOGICAL_ONLY,
    );
    state.data.resources.work_budget.reset(1);
    assert_eq!(
        state.commit_declaration_with_once(
            declaration,
            None,
            None,
            Value::Int(17),
            Some(once_marker),
        ),
        Err(RuntimeError::ExecutionLimit {
            kind: crate::error::ExecutionLimitKind::AllocationBytes,
            used: once_demand.0,
            requested: initialized_demand.0,
            limit: once_demand.0,
        })
    );
    assert_eq!(state.data.resources.work_budget.remaining(), 1);
    assert_eq!(state.data.resources.construction_bytes.get(), once_demand.0);
    assert_eq!(state.data.storage.get_global(&name), Some(&previous));
    assert_eq!(
        state
            .data
            .construction
            .once
            .iter()
            .collect::<alloc::vec::Vec<_>>(),
        once_keys_before
    );
    assert_eq!(
        state.data.construction.once.capacity_state_for_test(),
        once_capacity_before
    );
    assert!(state
        .data
        .construction
        .initialized_declarations
        .iter()
        .next()
        .is_none());
    assert_eq!(
        state
            .data
            .construction
            .initialized_declarations
            .capacity_state_for_test(),
        initialized_capacity_before
    );
}

#[test]
fn lifetime_fault_still_precedes_once_marker_admission() {
    let prepared = prepared();
    let declaration = declaration(&prepared, StorageOwner::Global);
    let mut data = state_data(&prepared);
    let name = prepared.vm.strings[prepared.layout.entries[declaration as usize].name_idx as usize]
        .clone();
    let value_before = data
        .storage
        .get_global(&name)
        .expect("constructed global")
        .clone();
    data.construction.initialized_declarations = Default::default();
    data.construction.once = Default::default();
    let once_before = data.construction.once.capacity_state_for_test();
    let initialized_before = data
        .construction
        .initialized_declarations
        .capacity_state_for_test();
    let mut state = EngineState::borrow(
        &mut data,
        &prepared,
        &super::super::super::services::LOGICAL_ONLY,
    );
    state.data.resources.construction_bytes.set(0);
    state.data.resources.work_budget.reset(1);
    let invalid = Value::Reference(Some(crate::value::ValueRef {
        location: crate::memory::MemoryLocation::Local(crate::memory::FrameId(u32::MAX)),
        offset: 0,
        path: Vec::new(),
    }));
    assert_eq!(
        state.commit_declaration_with_once(
            declaration,
            None,
            None,
            invalid,
            Some((declaration, None)),
        ),
        Err(RuntimeError::ReferenceLifetime)
    );
    assert_eq!(state.data.resources.work_budget.remaining(), 0);
    assert_eq!(state.data.resources.construction_bytes.get(), 0);
    assert_eq!(state.data.storage.get_global(&name), Some(&value_before));
    assert_eq!(state.data.construction.once.len(), 0);
    assert_eq!(
        state.data.construction.once.capacity_state_for_test(),
        once_before
    );
    assert_eq!(
        state
            .data
            .construction
            .initialized_declarations
            .capacity_state_for_test(),
        initialized_before
    );
}
