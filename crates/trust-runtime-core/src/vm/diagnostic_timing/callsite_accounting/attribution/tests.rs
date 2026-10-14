use super::super::{
    finish_callsite_accounting_log, observe_index_lookup, observe_index_lookup_result,
    record_copy_admission, record_index_owner_geometry, record_index_owner_query,
    record_index_target_geometry, start_callsite_accounting_log, IndexLookupOperation,
    IndexLookupPath, RECORDER,
};
use super::*;
use crate::{
    error::RuntimeError,
    value::{ArrayValue, Value},
};
use alloc::{boxed::Box, vec};

fn array() -> Value {
    Value::Array(Box::new(ArrayValue::from_canonical_parts(
        vec![Value::Int(1), Value::Int(2)],
        vec![(0, 1)],
    )))
}

#[test]
fn numeric_consumers_reconcile_coarse_array_admissions_and_restore_nested_errors() {
    start_callsite_accounting_log();
    let capacities = RECORDER.with(|r| {
        let r = r.borrow();
        (r.owner_queries.capacity(), r.array_consumers.capacity())
    });
    let location = Location::caller();
    let value = array();
    {
        let _native = native(17);
        let _call = consumer(CopyConsumerKind::UserCall, Some(42));
        let _formal = parameter(1, 0, false);
        let _read = read(CopyReadKind::DirectStorage);
        record_copy_admission(location, &value, 64, 64, CopyAdmissionOutcome::Admitted);
        let failure = {
            let _native = native(18);
            let _call = consumer(CopyConsumerKind::FunctionBlock, Some(7));
            let _formal = parameter(0, 2, false);
            let _read = read(CopyReadKind::Reference);
            record_copy_admission(location, &value, 64, 0, CopyAdmissionOutcome::WorkRefused);
            Err::<(), _>(RuntimeError::Overflow)
        };
        assert_eq!(failure, Err(RuntimeError::Overflow));
        assert_eq!(CONSUMER.with(Cell::get).unwrap().symbol_idx, Some(17));
        record_copy_admission(location, &value, 64, 64, CopyAdmissionOutcome::Admitted);
    }
    assert!(CONSUMER.with(Cell::get).is_none());
    record_copy_admission(location, &value, 64, 64, CopyAdmissionOutcome::Admitted);
    assert_eq!(
        RECORDER.with(|r| {
            let r = r.borrow();
            (r.owner_queries.capacity(), r.array_consumers.capacity())
        }),
        capacities
    );
    let snapshot = finish_callsite_accounting_log();
    assert!(snapshot.is_valid());
    assert_eq!(snapshot.copy_len, 1);
    let coarse = snapshot.copies[0];
    assert_eq!(
        (coarse.calls, coarse.admitted, coarse.work_refused),
        (4, 3, 1)
    );
    assert_eq!(
        (
            coarse.shape_len_sum,
            coarse.estimated_bytes,
            coarse.admitted_bytes
        ),
        (8, 256, 192)
    );
    assert_eq!(snapshot.array_consumers.len(), 3);
    assert_eq!(
        snapshot
            .array_consumers
            .iter()
            .map(|r| r.copy.calls)
            .sum::<u64>(),
        coarse.calls
    );
    assert_eq!(
        snapshot
            .array_consumers
            .iter()
            .map(|r| r.copy.shape_len_sum)
            .sum::<u64>(),
        coarse.shape_len_sum
    );
    assert_eq!(
        snapshot
            .array_consumers
            .iter()
            .map(|r| r.copy.admitted_bytes)
            .sum::<u64>(),
        coarse.admitted_bytes
    );
    assert_eq!(
        snapshot
            .array_consumers
            .iter()
            .map(|r| r.copy.work_refused)
            .sum::<u64>(),
        coarse.work_refused
    );
    let outer = snapshot
        .array_consumers
        .iter()
        .find(|r| r.consumer.pou_id == Some(42))
        .unwrap();
    assert_eq!(
        (outer.consumer.formal, outer.consumer.role, outer.copy.calls),
        (Some(1), CopyParameterRole::Input, 2)
    );
    assert!(snapshot
        .array_consumers
        .iter()
        .any(|r| r.consumer.kind == CopyConsumerKind::Unattributed));
}

#[test]
fn numeric_owner_queries_distinguish_ids_and_unperformed_refused_searches() {
    start_callsite_accounting_log();
    for (pou, operation) in [
        (7, IndexLookupOperation::WritableDeclaration),
        (7000, IndexLookupOperation::WritableDeclaration),
        (7, IndexLookupOperation::InstanceRoot),
    ] {
        observe_index_lookup(operation, || {
            record_index_owner_query(pou);
            record_index_owner_geometry(true, 3, 3);
            record_index_target_geometry(IndexLookupPath::DenseOwnerSlot, 2, 0);
            Some(())
        });
    }
    let failure = observe_index_lookup_result(IndexLookupOperation::WritableDeclaration, || {
        record_index_owner_query(u32::MAX);
        Err::<Option<()>, _>(RuntimeError::Overflow)
    });
    assert_eq!(failure, Err(RuntimeError::Overflow));
    let snapshot = finish_callsite_accounting_log();
    assert!(snapshot.is_valid());
    assert_eq!(snapshot.owner_queries.len(), 4);
    assert_eq!(
        snapshot
            .owner_queries
            .iter()
            .map(|r| r.comparisons.sum)
            .sum::<u64>(),
        9
    );
    assert_eq!(
        snapshot.owner_queries.iter().map(|r| r.calls).sum::<u64>(),
        snapshot.index_lookups[..snapshot.index_lookup_len]
            .iter()
            .map(|r| r.calls)
            .sum::<u64>()
    );
    assert_eq!(
        snapshot
            .owner_queries
            .iter()
            .filter(|r| r.pou_id == 7)
            .count(),
        2
    );
    let refused = snapshot
        .owner_queries
        .iter()
        .find(|r| r.pou_id == u32::MAX)
        .unwrap();
    assert_eq!(
        (
            refused.comparisons.sum,
            refused.binary_search,
            refused.found
        ),
        (0, false, 0)
    );
}

#[test]
fn identity_capacity_and_counter_loss_invalidates_the_existing_snapshot() {
    start_callsite_accounting_log();
    let site = CallSite {
        file: "synthetic.rs",
        line: 1,
        column: 1,
    };
    RECORDER.with(|r| {
        let mut r = r.borrow_mut();
        for pou_id in 0..=ATTRIBUTION_RECORD_CAPACITY as u32 {
            r.record_owner_query(OwnerQueryEvent {
                site,
                operation: IndexLookupOperation::WritableDeclaration,
                pou_id,
                binary_search: true,
                path: IndexLookupPath::DenseOwnerSlot,
                found: true,
                table_len: 3,
                comparisons: 3,
            });
        }
    });
    let snapshot = finish_callsite_accounting_log();
    assert_eq!(snapshot.owner_queries.len(), ATTRIBUTION_RECORD_CAPACITY);
    assert_eq!(snapshot.overflow_count, 1);
    assert!(!snapshot.is_valid());

    start_callsite_accounting_log();
    let value = array();
    let location = Location::caller();
    for formal in 0..=ATTRIBUTION_RECORD_CAPACITY {
        let _formal = parameter(formal, 0, false);
        record_copy_admission(location, &value, 64, 64, CopyAdmissionOutcome::Admitted);
    }
    let snapshot = finish_callsite_accounting_log();
    assert_eq!(snapshot.array_consumers.len(), ATTRIBUTION_RECORD_CAPACITY);
    assert_eq!(snapshot.overflow_count, 1);
    assert!(!snapshot.is_valid());

    start_callsite_accounting_log();
    record_copy_admission(location, &value, 64, 64, CopyAdmissionOutcome::Admitted);
    RECORDER.with(|r| r.borrow_mut().array_consumers[0].copy.calls = u64::MAX);
    record_copy_admission(location, &value, 64, 64, CopyAdmissionOutcome::Admitted);
    assert!(!finish_callsite_accounting_log().is_valid());

    start_callsite_accounting_log();
    RECORDER.with(|r| {
        let _busy = r.borrow_mut();
        record_copy_admission(location, &value, 64, 64, CopyAdmissionOutcome::Admitted);
    });
    let snapshot = finish_callsite_accounting_log();
    assert!(snapshot.recorder_busy > 0);
    assert!(!snapshot.is_valid());
}

#[test]
fn disabled_scopes_leave_state_and_original_result_unchanged() {
    let _ = finish_callsite_accounting_log();
    assert!(CONSUMER.with(Cell::get).is_none());
    let result = {
        let _native = native(9);
        let _formal = parameter(0, 2, false);
        Err::<(), _>(RuntimeError::TypeMismatch)
    };
    assert_eq!(result, Err(RuntimeError::TypeMismatch));
    assert!(CONSUMER.with(Cell::get).is_none());
    assert!(finish_callsite_accounting_log().array_consumers.is_empty());
}

#[test]
fn finish_drains_coarse_and_attributed_records_and_failure_counters_together() {
    start_callsite_accounting_log();
    super::super::record_work_charge(
        Location::caller(),
        1,
        1,
        super::super::WorkChargeOutcome::Charged,
    );
    observe_index_lookup(IndexLookupOperation::WritableDeclaration, || {
        record_index_owner_query(7);
        record_index_owner_geometry(true, 3, 3);
        record_index_target_geometry(IndexLookupPath::DenseOwnerSlot, 2, 0);
        Some(())
    });
    record_copy_admission(
        Location::caller(),
        &array(),
        64,
        64,
        CopyAdmissionOutcome::Admitted,
    );
    let first = finish_callsite_accounting_log();
    assert!(first.is_valid());
    assert_eq!(
        (first.work_len, first.copy_len, first.index_lookup_len),
        (1, 1, 1)
    );
    assert_eq!(
        (first.owner_queries.len(), first.array_consumers.len()),
        (1, 1)
    );
    assert_eq!(first.copies[0].calls, first.array_consumers[0].copy.calls);
    assert_eq!(first.index_lookups[0].calls, first.owner_queries[0].calls);
    let second = finish_callsite_accounting_log();
    assert!(second.is_valid());
    assert_eq!(
        (
            second.work_len,
            second.copy_len,
            second.lifecycle_len,
            second.index_lookup_len
        ),
        (0, 0, 0, 0)
    );
    assert!(second.owner_queries.is_empty());
    assert!(second.array_consumers.is_empty());

    start_callsite_accounting_log();
    super::super::note_busy();
    assert_eq!(finish_callsite_accounting_log().recorder_busy, 1);
    assert!(finish_callsite_accounting_log().is_valid());
}

#[test]
fn busy_finish_preserves_unreturned_records_for_one_successful_drain() {
    start_callsite_accounting_log();
    super::super::record_work_charge(
        Location::caller(),
        1,
        1,
        super::super::WorkChargeOutcome::Charged,
    );
    record_copy_admission(
        Location::caller(),
        &array(),
        64,
        64,
        CopyAdmissionOutcome::Admitted,
    );
    super::super::note_busy();
    RECORDER.with(|recorder| {
        let _borrow = recorder.borrow_mut();
        let busy = finish_callsite_accounting_log();
        assert!(!busy.is_valid());
        assert_eq!(busy.recorder_busy, 2);
        assert_eq!((busy.work_len, busy.copy_len), (0, 0));
        assert!(busy.array_consumers.is_empty());
    });
    let recovered = finish_callsite_accounting_log();
    assert!(recovered.is_valid());
    assert_eq!((recovered.work_len, recovered.copy_len), (1, 1));
    assert_eq!(recovered.array_consumers.len(), 1);
    assert_eq!(
        recovered.copies[0].calls,
        recovered.array_consumers[0].copy.calls
    );
    let empty = finish_callsite_accounting_log();
    assert!(empty.is_valid());
    assert_eq!((empty.work_len, empty.copy_len), (0, 0));
    assert!(empty.array_consumers.is_empty());
}
