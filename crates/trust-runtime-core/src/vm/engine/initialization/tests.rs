//! Defense-in-depth execution checks after ordinary wire admission.
use super::*;
use crate::bytecode::InitializationStage;
use crate::vm::{module::VmRef, PreparationLimits};

#[test]
fn forged_dynamic_initializer_store_cannot_modify_global_storage() {
    let bytes = include_bytes!(
        "../../../../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc"
    );
    let prepared = PreparedModule::from_bytes(bytes, PreparationLimits::default()).unwrap();
    let mut forged = PreparedModule::from_bytes(bytes, PreparationLimits::default()).unwrap();
    let mut state = EngineState::new(&prepared, 0, &super::super::services::LOGICAL_ONLY).unwrap();
    state
        .storage
        .set_global("staging_escape_probe", Value::DInt(7));
    let destination = state
        .storage
        .ref_for_global("staging_escape_probe")
        .unwrap();
    let reference = forged.vm.refs.len() as u32;
    forged.vm.refs.push(VmRef::Global {
        offset: destination.offset,
        path: Vec::new(),
    });
    let constant = forged.vm.consts.len() as u32;
    forged.vm.consts.push(Value::DInt(99));
    let start = forged.vm.code.len() as u32;
    // LOAD_REF_ADDR(global), LOAD_CONST(99), STORE_DEREF. This test deliberately
    // corrupts private, already-admitted metadata; it grants no public bypass of
    // static validation and proves the runtime boundary independently of it.
    forged.vm.code.push(0x22);
    forged.vm.code.extend_from_slice(&reference.to_le_bytes());
    forged.vm.code.push(0x10);
    forged.vm.code.extend_from_slice(&constant.to_le_bytes());
    forged.vm.code.push(0x33);
    let entry = &mut forged.initializers.entries[0];
    entry.code_offset = start;
    entry.code_length = 11;
    entry.stage = InitializationStage::Default;
    state.prepared = &forged;
    assert_eq!(
        state.evaluate_initializer(0, None, None, 0),
        Err(RuntimeError::StagingViolation)
    );
    assert_eq!(
        state.storage.get_global("staging_escape_probe"),
        Some(&Value::DInt(7))
    );
    assert!(
        state.construction.initializers.is_empty(),
        "failed initializer cleans its staging frame"
    );
    assert_eq!(state.storage.execution_frame_count(), 0);
    assert!(state.lifetimes.live_activations.is_empty());
    assert!(state.construction.frames.is_empty());
    assert!(state.construction.types.is_empty());
    // A trap must leave no staged slot or context that affects the next action.
    state.prepared = &prepared;
    let Some(Value::Instance(plant)) = state.storage.get_global("Plant") else {
        panic!("fixture root");
    };
    let plant = *plant;
    assert_eq!(
        state.evaluate_initializer(enabled_initializer(&prepared), None, Some(plant), 0),
        Ok(Value::Bool(true))
    );
    assert_eq!(state.storage.execution_frame_count(), 0);
    assert_eq!(
        state.storage.get_global("staging_escape_probe"),
        Some(&Value::DInt(7))
    );
}

fn enabled_initializer(prepared: &PreparedModule) -> u32 {
    prepared
        .initializers
        .entries
        .iter()
        .position(|entry| {
            entry.stage == InitializationStage::Explicit
                && entry
                    .declaration_idx
                    .and_then(|index| prepared.layout.entries.get(index as usize))
                    .and_then(|declaration| prepared.vm.strings.get(declaration.name_idx as usize))
                    .is_some_and(|name| name.eq_ignore_ascii_case("enabled"))
        })
        .map(|index| u32::try_from(index).unwrap())
        .expect("fixture explicit BOOL initializer")
}

#[test]
fn deepest_admitted_initializer_reuses_staging_and_rejects_the_next_depth() {
    let prepared = PreparedModule::from_bytes(
        include_bytes!(
            "../../../../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc"
        ),
        PreparationLimits {
            max_call_depth: 4,
            ..Default::default()
        },
    )
    .unwrap();
    let mut state = EngineState::new(&prepared, 0, &super::super::services::LOGICAL_ONLY).unwrap();
    let Some(Value::Instance(plant)) = state.storage.get_global("Plant") else {
        panic!("fixture root");
    };
    let plant = *plant;
    let initializer = enabled_initializer(&prepared);
    let before = state.storage.instances().len();
    for _ in 0..3 {
        assert_eq!(
            state.evaluate_initializer(initializer, None, Some(plant), 3),
            Ok(Value::Bool(true))
        );
        assert!(state.construction.initializers.is_empty());
        assert!(state.construction.frames.is_empty());
        assert!(state.construction.types.is_empty());
        assert!(state.lifetimes.live_activations.is_empty());
        assert_eq!(state.storage.execution_frame_count(), 0);
        assert_eq!(state.storage.instances().len(), before);
    }
    assert_eq!(
        state.evaluate_initializer(initializer, None, Some(plant), 4),
        Err(crate::vm::VmTrap::CallStackOverflow.into_runtime_error())
    );
    assert_eq!(state.storage.execution_frame_count(), 0);
    assert!(state.construction.initializers.is_empty());
    assert_eq!(
        state.evaluate_initializer(initializer, None, Some(plant), 3),
        Ok(Value::Bool(true))
    );
    assert_eq!(state.storage.execution_frame_count(), 0);
}
