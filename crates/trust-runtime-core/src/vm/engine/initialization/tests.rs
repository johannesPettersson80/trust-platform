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
}
