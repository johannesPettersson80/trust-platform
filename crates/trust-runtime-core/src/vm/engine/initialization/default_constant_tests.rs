//! Owning clone seams enforce exact demand before copying or publishing identities.
use super::*;
use crate::bytecode::{TypeEntry, TypeKind};
use crate::vm::PreparationLimits;

fn fixture() -> PreparedModule {
    PreparedModule::from_bytes(
        include_bytes!(
            "../../../../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc"
        ),
        PreparationLimits::default(),
    )
    .unwrap()
}

fn direct_scalar_default_fixture() -> (
    PreparedModule,
    alloc::boxed::Box<EngineData>,
    alloc::boxed::Box<EngineData>,
    usize,
    u32,
) {
    let prepared = fixture();
    let actual_data = EngineState::construct_data(&prepared, 0, &services::LOGICAL_ONLY).unwrap();
    let expected_data = EngineState::construct_data(&prepared, 0, &services::LOGICAL_ONLY).unwrap();
    let initializer_id = prepared
        .initializers
        .entries
        .iter()
        .position(|entry| {
            entry.phase == crate::bytecode::InitializationPhase::Return
                && entry.declaration_idx.is_some()
                && entry.stage == crate::bytecode::InitializationStage::Default
                && entry.code_length == 0
                && entry.body_kind == crate::bytecode::InitializerBodyKind::Action
                && entry.context_initializer_idx.is_none()
                && entry.recipe_type_id.is_none()
                && entry.recipe_member_idx.is_none()
                && entry
                    .declaration_idx
                    .and_then(|index| prepared.layout.entries.get(index as usize))
                    .is_some_and(|declaration| declaration.default_const_idx.is_none())
                && prepared
                    .vm
                    .ref_type(entry.result_ref_idx)
                    .is_some_and(|type_id| direct_scalar_intrinsic_default(&prepared.vm, type_id))
        })
        .unwrap();
    let type_id = prepared
        .vm
        .ref_type(prepared.initializers.entries[initializer_id].result_ref_idx)
        .unwrap();
    (
        prepared,
        actual_data,
        expected_data,
        initializer_id,
        type_id,
    )
}

#[test]
fn direct_intrinsic_scalar_default_preserves_constructor_work_without_staging() {
    let (prepared, mut actual_data, mut expected_data, initializer_id, type_id) =
        direct_scalar_default_fixture();

    let before_frame = actual_data.storage.reserve_execution_frame().unwrap();
    actual_data.storage.release_execution_frame(before_frame);
    {
        let mut actual = EngineState::borrow(&mut actual_data, &prepared, &services::LOGICAL_ONLY);
        actual.data.resources.constructed_values = 0;
        actual.data.resources.work_budget.reset(128);
        let value = actual
            .evaluate_initializer(initializer_id as u32, None, None, 0)
            .unwrap();
        assert_eq!(value, Value::Int(0));
        assert_eq!(actual.data.resources.constructed_values, 1);
        assert!(actual.data.construction.initializers.is_empty());
        assert_eq!(actual.data.storage.execution_activation_count(), 0);
        assert_eq!(actual.data.storage.execution_frame_count(), 0);
        let after_frame = actual.data.storage.reserve_execution_frame().unwrap();
        assert_eq!(after_frame, crate::memory::FrameId(before_frame.0 + 1));
        actual.data.storage.release_execution_frame(after_frame);
    }
    // TIME-08: only the constructed scalar node (CONSTRUCT) is charged; the initializer
    // entry itself is bookkeeping and no longer charges work.
    let actual_remaining = actual_data.resources.work_budget.remaining();

    {
        let mut expected =
            EngineState::borrow(&mut expected_data, &prepared, &services::LOGICAL_ONLY);
        expected.data.resources.constructed_values = 0;
        expected.data.resources.work_budget.reset(128);
        let prepared_module = expected.prepared;
        assert_eq!(
            construct_intrinsic_value(&prepared_module.vm.types, &mut expected, type_id).unwrap(),
            Value::Int(0)
        );
        assert_eq!(expected.data.resources.constructed_values, 1);
        assert_eq!(
            expected.data.resources.work_budget.remaining(),
            actual_remaining
        );
    }
}

#[test]
fn direct_intrinsic_scalar_default_checks_depth_before_reserving_staging() {
    let (mut prepared, mut data, _, initializer_id, _) = direct_scalar_default_fixture();
    prepared.limits.max_call_depth = 0;
    let before_frame = data.storage.reserve_execution_frame().unwrap();
    data.storage.release_execution_frame(before_frame);
    let mut state = EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY);
    state.data.resources.constructed_values = 0;
    state.data.resources.work_budget.reset(32);
    assert_eq!(
        state.evaluate_initializer(initializer_id as u32, None, None, 0),
        Err(crate::vm::VmTrap::CallStackOverflow.into_runtime_error())
    );
    // TIME-08: the depth check refuses before any charge; the entry is bookkeeping.
    assert_eq!(state.data.resources.work_budget.remaining(), 32);
    assert_eq!(state.data.resources.constructed_values, 0);
    assert!(state.data.construction.initializers.is_empty());
    assert_eq!(state.data.storage.execution_activation_count(), 0);
    assert_eq!(state.data.storage.execution_frame_count(), 0);
    let after_frame = state.data.storage.reserve_execution_frame().unwrap();
    assert_eq!(after_frame, crate::memory::FrameId(before_frame.0 + 1));
    state.data.storage.release_execution_frame(after_frame);
}

#[test]
fn intrinsic_scalar_default_type_guard_excludes_text_null_and_wrappers() {
    let mut module = fixture().vm;
    for prim_id in 1..=23 {
        let type_id = module.types.entries.len() as u32;
        module.types.entries.push(TypeEntry {
            kind: TypeKind::Primitive,
            name_idx: None,
            data: TypeData::Primitive {
                prim_id,
                max_length: 0,
            },
        });
        assert!(direct_scalar_intrinsic_default(&module, type_id));
    }
    for prim_id in [24, 25, 0x0100, 28] {
        let type_id = module.types.entries.len() as u32;
        module.types.entries.push(TypeEntry {
            kind: TypeKind::Primitive,
            name_idx: None,
            data: TypeData::Primitive {
                prim_id,
                max_length: 0,
            },
        });
        assert!(!direct_scalar_intrinsic_default(&module, type_id));
    }
    for prim_id in [26, 27] {
        let type_id = module.types.entries.len() as u32;
        module.types.entries.push(TypeEntry {
            kind: TypeKind::Primitive,
            name_idx: None,
            data: TypeData::Primitive {
                prim_id,
                max_length: 0,
            },
        });
        assert!(direct_scalar_intrinsic_default(&module, type_id));
    }
    let bounded_text = module.types.entries.len() as u32;
    module.types.entries.push(TypeEntry {
        kind: TypeKind::Primitive,
        name_idx: None,
        data: TypeData::Primitive {
            prim_id: 24,
            max_length: 1,
        },
    });
    assert!(!direct_scalar_intrinsic_default(&module, bounded_text));
    let wrapped = module.types.entries.len() as u32;
    module.types.entries.push(TypeEntry {
        kind: TypeKind::Alias,
        name_idx: None,
        data: TypeData::Alias { target_type_id: 0 },
    });
    assert!(!direct_scalar_intrinsic_default(&module, wrapped));
}

#[test]
fn constant_default_charges_exact_clone_demand_and_cleans_rejected_staging() {
    let mut prepared = fixture();
    let mut data = EngineState::construct_data(&prepared, 0, &services::LOGICAL_ONLY).unwrap();
    // Exercise the internal zero-body clone seam without another authoring engine.
    // The separate bytecode_source_free test admits an actual WSTRING container.
    let id = prepared
        .initializers
        .entries
        .iter()
        .position(|entry| {
            entry.declaration_idx.is_some()
                && entry.stage == crate::bytecode::InitializationStage::Default
        })
        .unwrap();
    let declaration = prepared.initializers.entries[id].declaration_idx.unwrap() as usize;
    let constant = prepared.vm.consts.len() as u32;
    let payload = Value::WString("A".repeat(8192));
    prepared.vm.consts.push(payload.clone());
    prepared.layout.entries[declaration].default_const_idx = Some(constant);
    prepared.initializers.entries[id].code_length = 0;
    let demand = core::mem::size_of::<Value>() + 8192;
    prepared.limits.max_construction_bytes = demand - 1;
    {
        let mut state = EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY);
        state.data.resources.construction_bytes.set(0);
        state
            .data
            .resources
            .work_budget
            .reset(prepared.limits.max_work);
        assert!(matches!(
            state.evaluate_initializer(id as u32, None, None, 0),
            Err(RuntimeError::ExecutionLimit {
                kind: crate::error::ExecutionLimitKind::AllocationBytes,
                ..
            })
        ));
        assert_eq!(state.data.resources.construction_bytes.get(), 0);
        assert!(state.data.construction.initializers.is_empty());
        assert_eq!(state.data.storage.execution_activation_count(), 0);
        assert_eq!(state.data.storage.execution_frame_count(), 0);
    }
    let before = data.storage.reserve_execution_frame().unwrap();
    data.storage.release_execution_frame(before);
    prepared.limits.max_construction_bytes = demand;
    let mut state = EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY);
    state.data.resources.construction_bytes.set(0);
    state
        .data
        .resources
        .work_budget
        .reset(prepared.limits.max_work);
    assert_eq!(
        state
            .evaluate_initializer(id as u32, None, None, 0)
            .unwrap(),
        payload
    );
    assert_eq!(state.data.resources.construction_bytes.get(), demand);
    assert!(state.data.construction.initializers.is_empty());
    assert_eq!(state.data.storage.execution_activation_count(), 0);
    assert_eq!(state.data.storage.execution_frame_count(), 0);
    let after = state.data.storage.reserve_execution_frame().unwrap();
    assert_eq!(after, crate::memory::FrameId(before.0 + 1));
    state.data.storage.release_execution_frame(after);
}

#[test]
fn member_default_preflight_cannot_strand_an_instance_or_lifetime_metadata() {
    let mut prepared = fixture();
    let mut data = EngineState::construct_data(&prepared, 0, &services::LOGICAL_ONLY).unwrap();
    let declaration = prepared
        .layout
        .entries
        .iter()
        .position(|entry| entry.owner == StorageOwner::Instance && entry.type_id.is_some())
        .unwrap();
    let pou = prepared.layout.entries[declaration].owner_pou_id.unwrap();
    let name = prepared.vm.strings[prepared.layout.entries[declaration].name_idx as usize].clone();
    let constant = prepared.vm.consts.len() as u32;
    prepared.vm.consts.push(Value::WString("A".repeat(8192)));
    prepared.layout.entries[declaration].default_const_idx = Some(constant);
    let instances = data.storage.instances().len();
    let templates = data.construction.instance_templates.len();
    let lifetimes = data.lifetimes.instance_lifetimes.len();
    {
        let mut state = EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY);
        // TIME-08: the instance node takes the one unit; the first member preflight
        // (a CONSTRUCT node) refuses before any instance is issued.
        state.data.resources.work_budget.reset(1);
        assert!(matches!(
            state.reserve_instance(pou, None),
            Err(RuntimeError::ExecutionLimit {
                kind: crate::error::ExecutionLimitKind::Work,
                ..
            })
        ));
        assert_eq!(state.data.storage.instances().len(), instances);
        assert_eq!(state.data.construction.instance_templates.len(), templates);
        assert_eq!(state.data.lifetimes.instance_lifetimes.len(), lifetimes);
        assert_eq!(state.data.storage.execution_activation_count(), 0);
    }
    prepared.limits.max_construction_bytes = core::mem::size_of::<Value>() + 8192 - 1;
    {
        let mut state = EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY);
        state.data.resources.construction_bytes.set(0);
        state
            .data
            .resources
            .work_budget
            .reset(prepared.limits.max_work);
        assert!(matches!(
            state.reserve_instance(pou, None),
            Err(RuntimeError::ExecutionLimit {
                kind: crate::error::ExecutionLimitKind::AllocationBytes,
                ..
            })
        ));
        assert_eq!(state.data.storage.instances().len(), instances);
        assert_eq!(state.data.construction.instance_templates.len(), templates);
        assert_eq!(state.data.lifetimes.instance_lifetimes.len(), lifetimes);
        assert_eq!(state.data.storage.execution_activation_count(), 0);
    }
    prepared.limits.max_construction_bytes = 32768;
    let mut state = EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY);
    state.data.resources.construction_bytes.set(0);
    state
        .data
        .resources
        .work_budget
        .reset(prepared.limits.max_work);
    let instance = state.reserve_instance(pou, None).unwrap();
    assert_eq!(
        state
            .data
            .storage
            .get_instance(instance)
            .unwrap()
            .variables
            .get(&name),
        Some(&Value::WString("A".repeat(8192)))
    );
    assert!(state.data.resources.construction_bytes.get() >= core::mem::size_of::<Value>() + 8192);
}
