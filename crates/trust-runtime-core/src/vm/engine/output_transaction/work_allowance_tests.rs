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
    // A DINT global the tests journal, declared beside the cell fixture's own.
    let prepared = crate::memory::cell_access::fixture::prepared_with_globals(&[(
        "journal_probe",
        crate::memory::cell_access::fixture::primitive(8, 0),
    )]);
    let mut data = EngineState::construct_data(&prepared, 0, &services::LOGICAL_ONLY).unwrap();
    let reference = {
        let mut state = EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY);
        crate::vm::engine::test_store::set_global(&mut state, "journal_probe", Value::DInt(7));
        crate::memory::view::StoreAccess::ref_for_global(&state, "journal_probe").unwrap()
    };
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
        crate::vm::engine::test_store::global(&state, "journal_probe"),
        Some(Value::DInt(70))
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
        crate::vm::engine::test_store::global(&state, "journal_probe"),
        Some(Value::DInt(7))
    );
    assert!(state.data.resources.output_journal.is_none());
}
