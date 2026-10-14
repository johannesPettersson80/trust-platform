//! Literal defaults that hold a reference, an array or a struct decline the direct frame path and charge nothing.
use super::*;
use crate::bytecode::InitializationPhase;
use crate::value::{ArrayValue, StructValue};
use crate::vm::PreparationLimits;

fn fixture() -> PreparedModule {
    PreparedModule::from_bytes(
        include_bytes!(
            "../../../../../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc"
        ),
        PreparationLimits::default(),
    )
    .unwrap()
}

/// The fixture's method return (`Counter.Advance : INT`): a frame-owned default without an executable body.
fn return_default(prepared: &PreparedModule) -> u32 {
    let id = prepared
        .initializers
        .entries
        .iter()
        .position(|entry| {
            entry.phase == InitializationPhase::Return
                && entry.stage == InitializationStage::Default
                && entry.once == InitializationOnce::None
                && entry.code_length == 0
                && entry
                    .declaration_idx
                    .and_then(|index| prepared.layout.entries.get(index as usize))
                    .is_some_and(|declaration| {
                        declaration.owner == StorageOwner::Frame
                            && declaration.default_const_idx.is_none()
                    })
        })
        .expect("fixture method return default");
    id as u32
}

fn frame_for(prepared: &PreparedModule, id: u32) -> VmFrame {
    let declaration_index = prepared.initializers.entries[id as usize]
        .declaration_idx
        .expect("declaration action") as usize;
    let declaration = &prepared.layout.entries[declaration_index];
    let slots = declaration.slot as usize + 1;
    VmFrame {
        activation: None,
        pou_id: declaration.owner_pou_id,
        return_pc: 0,
        code_start: 0,
        code_end: 0,
        local_ref_start: 0,
        local_ref_count: 0,
        locals: alloc::vec![Value::Null; slots],
        parameter_values_present: alloc::vec![false; slots],
        runtime_instance: None,
        instance_owner: None,
    }
}

/// Spec 34 §4.3 HOST-04 "Work done in place": a frame default whose literal holds a reference, an array or a
/// struct is declined by the direct path, which returns `false` and charges no work, constructed values or
/// frame slots.
#[test]
fn call_path_reference_array_and_struct_literal_defaults_decline_the_direct_path() {
    let mut prepared = fixture();
    let id = return_default(&prepared);
    let declaration = prepared.initializers.entries[id as usize]
        .declaration_idx
        .expect("declaration action") as usize;
    prepared.initializers.entries[id as usize].code_length = 0;
    let literals = [
        Value::Reference(Some(crate::value::ValueRef {
            location: crate::memory::MemoryLocation::Local(crate::memory::FrameId(u32::MAX)),
            offset: 0,
            path: alloc::vec::Vec::new(),
        })),
        Value::Array(alloc::boxed::Box::new(ArrayValue::from_canonical_parts(
            alloc::vec![Value::Int(1)],
            alloc::vec![(0, 0)],
        ))),
        Value::Struct(alloc::sync::Arc::new(StructValue::from_canonical_parts(
            smol_str::SmolStr::new_static("Pair"),
            Default::default(),
        ))),
    ];
    for literal in literals {
        let constant = prepared.vm.consts.len() as u32;
        prepared.vm.consts.push(literal);
        prepared.layout.entries[declaration].default_const_idx = Some(constant);
        let mut data = EngineState::construct_data(&prepared, 0, &services::LOGICAL_ONLY).unwrap();
        let mut state = EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY);
        state.data.resources.constructed_values = 0;
        state.data.resources.work_budget.reset(64);
        let mut frame = frame_for(&prepared, id);
        assert_eq!(
            state.apply_direct_frame_default(id, &mut frame, 0),
            Ok(false),
            "literal default {constant} takes the declaration action"
        );
        assert_eq!(state.data.resources.work_budget.remaining(), 64);
        assert_eq!(state.data.resources.constructed_values, 0);
        assert!(frame.locals.iter().all(|value| *value == Value::Null));
    }
}
