use super::*;
use crate::vm::call::{bindings::VmWriteTarget, context::CallContext};

/// Spec 34 §4.3 HOST-04 "Value transfer": two writes that reach one physical slot through two references keep one
/// journal snapshot, the writes land in order, and a later failure restores the slot to its value before the call.
#[test]
fn call_path_aliased_output_writes_keep_one_snapshot_and_restore_after_a_later_failure() {
    let prepared = PreparedModule::from_bytes(
        include_bytes!(
            "../../../../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc"
        ),
        crate::vm::PreparationLimits::default(),
    )
    .unwrap();
    let mut state_data =
        EngineState::construct_data(&prepared, 0, &super::super::services::LOGICAL_ONLY).unwrap();
    let mut state = EngineState::borrow(
        &mut state_data,
        &prepared,
        &super::super::services::LOGICAL_ONLY,
    );
    state
        .data
        .storage
        .set_global("output_probe", Value::DInt(7));
    let first = state.data.storage.ref_for_global("output_probe").unwrap();
    let second = state.data.storage.ref_for_global("output_probe").unwrap();
    let id = state.data.storage.reserve_execution_frame().unwrap();
    let mut frame = VmFrame {
        activation: Some(id),
        pou_id: None,
        return_pc: 0,
        code_start: 0,
        code_end: 0,
        local_ref_start: 0,
        local_ref_count: 0,
        locals: Vec::new(),
        parameter_values_present: Vec::new(),
        runtime_instance: None,
        instance_owner: None,
    };

    let failed = state.with_output_transaction(&mut frame, |state, frame| {
        VmWriteTarget::from_reference(&first).write(state, frame, Value::DInt(70))?;
        VmWriteTarget::from_reference(&second).write(state, frame, Value::DInt(90))?;
        let snapshots = state
            .data
            .resources
            .output_journal
            .as_ref()
            .map_or(0, |journal| journal.len());
        assert_eq!(snapshots, 1, "one snapshot per physical destination");
        assert_eq!(
            state.data.storage.get_global("output_probe"),
            Some(&Value::DInt(90)),
            "the second write lands after the first"
        );
        Err::<(), _>(VmTrap::Runtime(RuntimeError::Overflow))
    });
    assert!(matches!(
        failed,
        Err(VmTrap::Runtime(RuntimeError::Overflow))
    ));
    assert_eq!(
        state.data.storage.get_global("output_probe"),
        Some(&Value::DInt(7)),
        "the first snapshot is restored after the later failure"
    );
    assert!(state.data.resources.output_journal.is_none());
}
