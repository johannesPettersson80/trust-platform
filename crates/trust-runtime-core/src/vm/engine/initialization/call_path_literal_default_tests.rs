//! A literal default takes the same lifetime refusal on its fast path as the declaration commit path.
use super::*;
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

/// Spec 34 §4.3 HOST-04 "Default values keep their type, depth and lifetime checks whichever constructor path
/// produces them": a literal default holding a reference to a dead activation is refused with
/// `ReferenceLifetime` on the literal fast path, and no initializer is left published.
#[test]
fn call_path_literal_default_refuses_a_reference_that_outlives_its_frame() {
    let mut prepared = fixture();
    let mut data = EngineState::construct_data(&prepared, 0, &services::LOGICAL_ONLY).unwrap();
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
    prepared
        .vm
        .consts
        .push(Value::Reference(Some(crate::value::ValueRef {
            location: crate::memory::MemoryLocation::Local(crate::memory::FrameId(u32::MAX)),
            offset: 0,
            path: alloc::vec::Vec::new(),
        })));
    prepared.layout.entries[declaration].default_const_idx = Some(constant);
    prepared.initializers.entries[id].code_length = 0;
    let mut state = EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY);
    state.data.resources.construction_bytes.set(0);
    state
        .data
        .resources
        .work_budget
        .reset(prepared.limits.max_work);
    assert_eq!(
        state.evaluate_initializer(id as u32, None, None, 0),
        Err(RuntimeError::ReferenceLifetime)
    );
    assert!(state.data.construction.initializers.is_empty());
}
