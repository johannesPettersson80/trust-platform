//! TIME-08: the rollback journal and its saved value are call-transaction bookkeeping and
//! draw neither work nor cumulative allocation, whether the transaction commits or rolls back.
use super::*;
use crate::vm::call::{bindings::VmWriteTarget, context::CallContext};

fn fixture() -> (
    PreparedModule,
    alloc::boxed::Box<EngineData>,
    VmFrame,
    ValueRef,
) {
    let prepared = PreparedModule::from_bytes(
        include_bytes!(
            "../../../../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc"
        ),
        crate::vm::PreparationLimits::default(),
    )
    .unwrap();
    let mut data = EngineState::construct_data(&prepared, 0, &services::LOGICAL_ONLY).unwrap();
    data.storage.set_global("journal_probe", Value::DInt(7));
    let reference = data.storage.ref_for_global("journal_probe").unwrap();
    data.resources.construction_bytes.set(0);
    let frame = VmFrame {
        activation: None,
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
    (prepared, data, frame, reference)
}

#[test]
fn committed_output_snapshot_draws_no_work() {
    let (prepared, mut data, mut frame, reference) = fixture();
    let mut state = EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY);
    // A scalar write has no copy work, and the journal saves its old value without work
    // or allocation.
    state.data.resources.work_budget.reset(0);
    let result = state.with_output_transaction(&mut frame, |state, frame| {
        VmWriteTarget::from_reference(&reference).write(state, frame, Value::DInt(70))
    });
    assert!(result.is_ok());
    assert_eq!(state.data.resources.work_budget.remaining(), 0);
    assert_eq!(state.data.resources.construction_bytes.get(), 0);
    assert_eq!(
        state.data.storage.get_global("journal_probe"),
        Some(&Value::DInt(70))
    );
    assert!(state.data.resources.output_journal.is_none());
}

#[test]
fn rolled_back_output_snapshot_restores_its_value_without_work() {
    let (prepared, mut data, mut frame, reference) = fixture();
    let mut state = EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY);
    state.data.resources.work_budget.reset(0);
    let result = state.with_output_transaction(&mut frame, |state, frame| {
        VmWriteTarget::from_reference(&reference).write(state, frame, Value::DInt(70))?;
        Err::<(), _>(VmTrap::Runtime(RuntimeError::DivisionByZero))
    });
    assert!(matches!(
        result,
        Err(VmTrap::Runtime(RuntimeError::DivisionByZero))
    ));
    assert_eq!(state.data.resources.work_budget.remaining(), 0);
    assert_eq!(state.data.resources.construction_bytes.get(), 0);
    assert_eq!(
        state.data.storage.get_global("journal_probe"),
        Some(&Value::DInt(7))
    );
    assert!(state.data.resources.output_journal.is_none());
}
