//! Exact prepared lookup results for dense, reordered, sparse, fallback, absent and root branches.
use super::*;
use crate::bytecode::StorageRole;

fn fixture(dense_globals: bool) -> PreparedModule {
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
    let mut reordered = global.clone();
    reordered.slot = 2;
    let mut external = global.clone();
    external.role = StorageRole::External;
    let mut dense = template;
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
    prepared.layout.entries = if dense_globals {
        alloc::vec![global, reordered, external, dense, sparse, collided]
    } else {
        alloc::vec![reordered, global, external, dense, sparse, collided]
    };
    let mut indexes = PreparedIndexes::default();
    let mut budget = PreparationBudget::new(PreparationLimits::default());
    for (id, declaration) in prepared.layout.entries.iter().enumerate() {
        if declaration.owns_storage() {
            indexes
                .declarations
                .push(
                    DeclarationKey::new(declaration.owner as u8, declaration.owner_pou_id),
                    id as u32,
                    &mut budget,
                )
                .unwrap();
        }
    }
    indexes.declarations.finish(&mut budget).unwrap();
    indexes
        .declarations
        .sort_values(&mut budget, |a, b| {
            prepared.layout.entries[*a as usize]
                .slot
                .cmp(&prepared.layout.entries[*b as usize].slot)
        })
        .unwrap();
    indexes
        .prepare_declaration_plan(
            prepared.vm.pou_by_id.len(),
            &prepared.layout,
            &BTreeMap::new(),
            &mut budget,
        )
        .unwrap();
    indexes.root_owners = alloc::vec![(3, 0), (7, 0), (9, 0)];
    assert!(
        indexes.declaration_plan.owner_ordinal_for_test(7).is_some(),
        "the high-ID owner must be cached"
    );
    assert!(
        indexes
            .declaration_plan
            .owner_ordinal_for_test(fallback_pou)
            .is_none(),
        "the larger colliding owner must use fallback"
    );
    prepared.indexes = indexes;
    prepared
}

#[test]
fn declaration_matches_dense_reordered_sparse_external_and_absent_queries() {
    let prepared = fixture(false);
    let fallback_pou = 7u32
        .checked_add(u32::try_from(prepared.vm.pou_by_id.len()).unwrap())
        .unwrap();
    for (owner, pou, slot, expected) in [
        (StorageOwner::Global, None, 0, Some(1)),
        (StorageOwner::Global, None, 2, Some(0)),
        (StorageOwner::Global, None, 1, None),
        (StorageOwner::Frame, Some(0), 0, Some(3)),
        (StorageOwner::Frame, Some(7), 2, Some(4)),
        (StorageOwner::Frame, Some(7), 1, None),
        (StorageOwner::Frame, Some(fallback_pou), 2, Some(5)),
        (StorageOwner::Frame, Some(fallback_pou), 1, None),
        (StorageOwner::Frame, Some(8), 2, None),
        (StorageOwner::Frame, None, 0, None),
    ] {
        let actual = prepared.declaration(owner, pou, slot).map(|(id, _)| id);
        assert_eq!(
            actual, expected,
            "owner={owner:?}, pou={pou:?}, slot={slot}"
        );
    }
    // Dense globals take the first path when the admitted records are ordered.
    let prepared = fixture(true);
    assert_eq!(
        prepared
            .declaration(StorageOwner::Global, None, 0)
            .map(|(id, _)| id),
        Some(0)
    );
}

#[test]
fn root_uses_its_actual_index_and_missing_or_empty_owner_is_absent() {
    let mut prepared = fixture(false);
    // Owners 3 and 7 resolve through the sorted root-owner index; u32::MAX is absent.
    for (owner, expected) in [(3, Some(0)), (7, Some(0)), (u32::MAX, None)] {
        assert_eq!(
            prepared.root_for_instance_owner(owner).map(|r| r.0),
            expected,
            "owner={owner}"
        );
    }
    prepared.indexes.root_owners.clear();
    assert!(prepared.root_for_instance_owner(7).is_none());
    prepared.indexes.declaration_plan = Default::default();
    assert!(
        prepared
            .declaration(StorageOwner::Frame, Some(7), 2)
            .is_none(),
        "an empty owner index is absent"
    );
}

#[test]
fn invalid_wire_slot_is_absent_without_a_search() {
    let prepared = fixture(false);
    assert!(prepared
        .declaration(StorageOwner::Global, None, usize::MAX)
        .is_none());
}

#[cfg(all(feature = "std", feature = "test-support"))]
#[test]
fn instrumented_lookups_run_their_searches_to_completion() {
    use crate::vm::diagnostic_timing::{
        finish_callsite_accounting_log, observe_index_lookup, start_callsite_accounting_log,
        IndexLookupOperation,
    };
    let prepared = fixture(false);
    let fallback_pou = 7u32
        .checked_add(u32::try_from(prepared.vm.pou_by_id.len()).unwrap())
        .unwrap();
    let owner_count = prepared.indexes.declaration_plan.owners_len_for_test();
    // The owner search performs lookup_work(owner_count) comparisons, then one slot comparison.
    start_callsite_accounting_log();
    let result = observe_index_lookup(IndexLookupOperation::WritableDeclaration, || {
        prepared.declaration(StorageOwner::Frame, Some(fallback_pou), 2)
    });
    let snapshot = finish_callsite_accounting_log();
    assert_eq!(result.map(|(id, _)| id), Some(5));
    assert!(snapshot.is_valid());
    assert_eq!(snapshot.index_lookup_len, 1);
    assert!(snapshot.index_lookups[0].owner_binary_search);
    assert_eq!(
        snapshot.index_lookups[0].owner_comparisons.sum,
        lookup_work(owner_count) as u64
    );
    assert_eq!(snapshot.index_lookups[0].target_comparisons.sum, 1);

    // The high-ID cache hit uses the direct owner row, then one sparse-slot comparison.
    start_callsite_accounting_log();
    let result = observe_index_lookup(IndexLookupOperation::WritableDeclaration, || {
        prepared.declaration(StorageOwner::Frame, Some(7), 2)
    });
    let snapshot = finish_callsite_accounting_log();
    assert_eq!(result.map(|(id, _)| id), Some(4));
    assert!(snapshot.is_valid());
    assert_eq!(snapshot.index_lookup_len, 1);
    assert!(!snapshot.index_lookups[0].owner_binary_search);
    assert_eq!(snapshot.index_lookups[0].owner_comparisons.sum, 0);
    assert_eq!(snapshot.index_lookups[0].target_comparisons.sum, 1);

    // Three root owners make the sorted root search perform lookup_work(3) comparisons.
    start_callsite_accounting_log();
    let result = observe_index_lookup(IndexLookupOperation::InstanceRoot, || {
        prepared.root_for_instance_owner(7)
    });
    let snapshot = finish_callsite_accounting_log();
    assert_eq!(result.map(|(id, _)| id), Some(0));
    assert!(snapshot.is_valid());
    assert_eq!(snapshot.index_lookups[0].target_comparisons.sum, 3);
}

#[test]
fn sparse_owner_and_slot_resolve_without_a_former_work_bound() {
    let mut prepared = fixture(false);
    let template = prepared
        .layout
        .entries
        .iter()
        .find(|entry| entry.owner == StorageOwner::Frame)
        .unwrap()
        .clone();
    prepared.layout.entries.clear();
    for ordinal in 0..68 {
        let mut declaration = template.clone();
        declaration.owner_pou_id = Some(ordinal * 2 + 1);
        declaration.slot = 0;
        prepared.layout.entries.push(declaration);
    }
    for slot in 1..11 {
        let mut declaration = template.clone();
        declaration.owner_pou_id = Some(135);
        declaration.slot = slot * 2;
        prepared.layout.entries.push(declaration);
    }
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
        .declarations
        .sort_values(&mut budget, |a, b| {
            prepared.layout.entries[*a as usize]
                .slot
                .cmp(&prepared.layout.entries[*b as usize].slot)
        })
        .unwrap();
    indexes
        .prepare_declaration_plan(
            prepared.vm.pou_by_id.len(),
            &prepared.layout,
            &BTreeMap::new(),
            &mut budget,
        )
        .unwrap();
    prepared.indexes = indexes;
    // Owner 135 holds slot 0 at id 67 (ordinal 67) and slots 2..=20 at ids 68..=77.
    assert_eq!(
        prepared
            .declaration(StorageOwner::Frame, Some(135), 2)
            .map(|(id, _)| id),
        Some(68)
    );
    assert_eq!(
        prepared
            .declaration(StorageOwner::Frame, Some(135), 20)
            .map(|(id, _)| id),
        Some(77)
    );
    assert!(prepared
        .declaration(StorageOwner::Frame, Some(135), 19)
        .is_none());
}

#[test]
fn candidate_groups_return_sparse_aliases_and_ignore_unrelated_descriptors() {
    use crate::memory::{FrameId, InstanceId, MemoryLocation};
    use crate::vm::module::VmRef;
    let mut prepared = fixture(false);
    prepared.vm.refs = alloc::vec![
        VmRef::Global {
            offset: 2,
            path: alloc::vec![]
        },
        VmRef::Global {
            offset: 99,
            path: alloc::vec![]
        },
        VmRef::Instance {
            owner_instance_id: 0,
            offset: 0,
            path: alloc::vec![]
        },
    ];
    let mut budget = PreparationBudget::new(PreparationLimits::default());
    prepared.indexes.reference_candidates = Default::default();
    prepared
        .indexes
        .prepare_references(&prepared.vm, &mut budget)
        .unwrap();
    for (location, offset, expected) in [
        (MemoryLocation::Global, 2, alloc::vec![0]),
        (MemoryLocation::Global, 3, alloc::vec![]),
        (MemoryLocation::Global, usize::MAX, alloc::vec![]),
        (
            MemoryLocation::Instance(InstanceId(u32::MAX)),
            0,
            alloc::vec![2],
        ),
        (MemoryLocation::Local(FrameId(0)), 0, alloc::vec![]),
    ] {
        let actual = prepared.dynamic_reference_candidates(location, offset);
        assert_eq!(actual, expected.as_slice());
    }
    // More descriptors in existing groups and unrelated local descriptors do
    // not change the sparse candidate groups. The immutable index is rebuilt once.
    for _ in 0..100 {
        prepared.vm.refs.push(prepared.vm.refs[0].clone());
        prepared.vm.refs.push(VmRef::Local {
            owner_frame_id: 0,
            offset: 0,
            path: alloc::vec![],
        });
    }
    prepared.indexes.reference_candidates = Default::default();
    prepared
        .indexes
        .prepare_references(&prepared.vm, &mut budget)
        .unwrap();
    let candidates = prepared.dynamic_reference_candidates(MemoryLocation::Global, 2);
    assert_eq!(candidates.len(), 101);
    assert!(prepared
        .dynamic_reference_candidates(MemoryLocation::Global, 3)
        .is_empty());
    prepared.indexes.reference_candidates = Default::default();
    assert!(prepared
        .dynamic_reference_candidates(MemoryLocation::Global, 2)
        .is_empty());
}
