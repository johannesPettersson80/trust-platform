use super::*;
use crate::vm::call::{bindings::VmWriteTarget, context::CallContext};
use core::mem::size_of;

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

// An empty journal grows to four entries. TIME-08 treats that growth as call-transaction
// bookkeeping (spec 34), so no allocation limit refuses it and it charges no bytes.
const JOURNAL_FIRST_ALLOCATION_BYTES: usize = 4 * size_of::<(Key, Value)>();

#[test]
fn single_output_snapshot_draws_no_allocation_at_any_limit() {
    for limit in [0, JOURNAL_FIRST_ALLOCATION_BYTES] {
        let (mut prepared, mut data, mut frame, reference) = fixture();
        prepared.limits.max_construction_bytes = limit;
        let mut state = EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY);
        let result = state.with_output_transaction(&mut frame, |state, frame| {
            VmWriteTarget::from_reference(&reference).write(state, frame, Value::DInt(70))
        });
        assert!(result.is_ok(), "limit {limit}: {result:?}");
        assert_eq!(state.data.resources.construction_bytes.get(), 0);
        assert_eq!(
            state.data.storage.get_global("journal_probe"),
            Some(&Value::DInt(70))
        );
        assert!(state.data.resources.output_journal.is_none());
    }
}

#[test]
fn repeated_output_keeps_first_snapshot_and_late_failure_rolls_back() {
    let (mut prepared, mut data, mut frame, reference) = fixture();
    prepared.limits.max_construction_bytes = JOURNAL_FIRST_ALLOCATION_BYTES;
    let mut state = EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY);
    let result = state.with_output_transaction(&mut frame, |state, frame| {
        let target = VmWriteTarget::from_reference(&reference);
        target.write(state, frame, Value::DInt(70))?;
        target.write(state, frame, Value::DInt(90))?;
        // TIME-08: the repeated write reuses the first snapshot and draws no allocation.
        assert_eq!(state.data.resources.construction_bytes.get(), 0);
        let journal = state.data.resources.output_journal.as_ref().unwrap();
        assert_eq!(journal.len(), 1);
        assert_eq!(
            journal.get(&(reference.location, reference.offset)),
            Some(&Value::DInt(7))
        );
        Err::<(), _>(VmTrap::Runtime(RuntimeError::DivisionByZero))
    });
    assert!(matches!(
        result,
        Err(VmTrap::Runtime(RuntimeError::DivisionByZero))
    ));
    assert_eq!(
        state.data.storage.get_global("journal_probe"),
        Some(&Value::DInt(7))
    );
    assert!(state.data.resources.output_journal.is_none());
}

#[test]
fn invalid_snapshot_resolves_before_allocation_and_never_inserts() {
    let (mut prepared, mut data, frame, mut reference) = fixture();
    prepared.limits.max_construction_bytes = 0;
    reference.offset = usize::MAX;
    let mut state = EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY);
    state.data.resources.output_journal = Some(SavedDestinations::new());
    // Key resolution is bookkeeping (TIME-08): the refusal is the same with an
    // empty execution allowance, and that allowance is not consumed.
    state.data.resources.work_budget.reset(0);
    assert_eq!(
        state.journal_output_destination(&frame, reference.as_view()),
        Err(RuntimeError::NullReference)
    );
    assert_eq!(state.data.resources.work_budget.remaining(), 0);
    state.data.resources.work_budget.reset(1000);
    assert_eq!(
        state.journal_output_destination(&frame, reference.as_view()),
        Err(RuntimeError::NullReference)
    );
    assert_eq!(
        state.data.resources.output_journal.as_ref().unwrap().len(),
        0
    );
    assert_eq!(state.data.resources.construction_bytes.get(), 0);
    assert_eq!(
        state.data.storage.get_global("journal_probe"),
        Some(&Value::DInt(7))
    );
}
