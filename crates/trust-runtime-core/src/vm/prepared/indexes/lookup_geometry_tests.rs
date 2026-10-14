//! Native diagnostics exercise the actual dense and sparse prepared lookup branches.
use super::*;
use crate::bytecode::StorageRole;
use crate::vm::diagnostic_timing::{
    finish_callsite_accounting_log, observe_index_lookup, start_callsite_accounting_log,
    IndexLookupOperation, IndexLookupPath,
};

#[test]
fn actual_lookup_geometry_preserves_dense_fallback_missing_and_root_results() {
    let mut prepared = PreparedModule::from_bytes(
        include_bytes!(
            "../../../../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc"
        ),
        PreparationLimits::default(),
    )
    .unwrap();
    let template = prepared
        .layout
        .entries
        .iter()
        .find(|entry| entry.owns_storage())
        .unwrap()
        .clone();
    let mut global = template.clone();
    global.owner = StorageOwner::Global;
    global.owner_pou_id = None;
    global.role = StorageRole::Variable;
    global.slot = 0;
    let mut dense = template.clone();
    dense.owner = StorageOwner::Frame;
    dense.owner_pou_id = Some(0);
    dense.role = StorageRole::Variable;
    dense.slot = 0;
    let mut sparse = dense.clone();
    sparse.owner_pou_id = Some(7);
    sparse.slot = 2;
    assert!(
        prepared.vm.pou_by_id.len() <= 7,
        "POU 7 remains beyond the direct raw-ID table range"
    );
    let fallback_pou = 7u32
        .checked_add(u32::try_from(prepared.vm.pou_by_id.len()).unwrap())
        .unwrap();
    let mut collided = sparse.clone();
    collided.owner_pou_id = Some(fallback_pou);
    prepared.layout.entries = alloc::vec![global, dense, sparse, collided];
    let mut indexes = PreparedIndexes::default();
    let mut budget = PreparationBudget::new(PreparationLimits::default());
    for (id, declaration) in prepared.layout.entries.iter().enumerate() {
        indexes
            .declarations
            .push(
                DeclarationKey::new(declaration.owner as u8, declaration.owner_pou_id),
                id as u32,
                &mut budget,
            )
            .unwrap();
    }
    indexes.declarations.finish(&mut budget).unwrap();
    indexes
        .prepare_declaration_plan(
            prepared.vm.pou_by_id.len(),
            &prepared.layout,
            &BTreeMap::new(),
            &mut budget,
        )
        .unwrap();
    assert!(!prepared.roots.entries.is_empty());
    indexes.root_owners = alloc::vec![(3, 0), (7, 0), (9, 0)];
    assert!(indexes.declaration_plan.owner_ordinal_for_test(7).is_some());
    assert!(indexes
        .declaration_plan
        .owner_ordinal_for_test(fallback_pou)
        .is_none());
    prepared.indexes = indexes;
    let owner_count = prepared.indexes.declaration_plan.owners_len_for_test();
    // The owner table is nonempty, so the sorted search performs lookup_work(count) comparisons.
    let owner_work = lookup_work(owner_count) as u64;

    start_callsite_accounting_log();
    for (owner, pou, slot, expected) in [
        (StorageOwner::Global, None, 0, Some(0)),
        (StorageOwner::Frame, Some(0), 0, Some(1)),
        (StorageOwner::Frame, Some(7), 2, Some(2)),
        (StorageOwner::Frame, Some(fallback_pou), 2, Some(3)),
        (StorageOwner::Frame, Some(8), 2, None),
    ] {
        let result = observe_index_lookup(IndexLookupOperation::WritableDeclaration, || {
            prepared.declaration(owner, pou, slot)
        });
        assert_eq!(result.map(|(id, _)| id), expected);
    }
    for (owner, expected) in [(7, Some(0)), (u32::MAX, None)] {
        let result = observe_index_lookup(IndexLookupOperation::InstanceRoot, || {
            prepared.root_for_instance_owner(owner)
        });
        assert_eq!(result.map(|(id, _)| id), expected);
    }
    let snapshot = finish_callsite_accounting_log();
    assert!(snapshot.is_valid());
    assert_eq!(snapshot.work_len, 0, "observation adds no work admission");
    let rows = &snapshot.index_lookups[..snapshot.index_lookup_len];
    let global = rows
        .iter()
        .find(|r| r.path == IndexLookupPath::DenseGlobal)
        .unwrap();
    assert_eq!(global.target_table_len.min, prepared.layout.entries.len());
    assert_eq!(
        global.owner_comparisons.sum + global.target_comparisons.sum,
        0
    );
    let dense = rows
        .iter()
        .find(|r| r.path == IndexLookupPath::DenseOwnerSlot)
        .unwrap();
    assert!(!dense.owner_binary_search);
    assert_eq!(dense.owner_table_len.min, owner_count);
    assert_eq!(dense.target_table_len.min, 1);
    assert_eq!(
        dense.owner_comparisons.sum + dense.target_comparisons.sum,
        0
    );
    let sparse = rows
        .iter()
        .find(|r| r.path == IndexLookupPath::SparseOwnerSlot && !r.owner_binary_search)
        .unwrap();
    assert_eq!(sparse.owner_comparisons.sum, 0);
    assert_eq!(sparse.target_comparisons.sum, 1);
    let fallback = rows
        .iter()
        .find(|r| r.path == IndexLookupPath::SparseOwnerSlot && r.owner_binary_search)
        .unwrap();
    assert_eq!(fallback.owner_table_len.min, owner_count);
    assert_eq!(fallback.owner_comparisons.sum, owner_work);
    assert_eq!(fallback.target_comparisons.sum, 1);
    let missing = rows
        .iter()
        .find(|r| r.path == IndexLookupPath::MissingOwner)
        .unwrap();
    assert_eq!(missing.found, 0);
    assert_eq!(missing.owner_comparisons.sum, owner_work);
    let roots = rows
        .iter()
        .find(|r| r.path == IndexLookupPath::RootOwner)
        .unwrap();
    assert_eq!(roots.calls, 2);
    assert_eq!(roots.found, 1);
    assert_eq!(roots.target_table_len.min, 3);
    assert_eq!(roots.target_table_len.max, 3);
    assert_eq!(roots.target_comparisons.sum, 6);
}
