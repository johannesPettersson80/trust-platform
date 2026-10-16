use super::*;
use alloc::vec;

#[test]
fn declaration_owner_direct_table_is_bounded_and_key_checked() {
    let mut prepared = super::tests::prepared();
    assert_eq!(
        prepared.indexes.declaration_plan.owner_index_len_for_test(),
        prepared.vm.pou_by_id.len()
    );
    let pou = prepared.global("Plant").unwrap().1.owner_pou_id.unwrap();
    let owner_index = prepared
        .indexes
        .declaration_plan
        .owner_ordinal_for_test(pou)
        .expect("fixture owner is represented in the declaration plan");
    let direct = prepared
        .indexes
        .declaration_plan
        .range(StorageOwner::Instance, Some(pou));
    assert!(direct.is_some());

    assert!(prepared.indexes.declaration_plan.owners_len_for_test() > 1);
    let wrong_ordinal = if owner_index == 0 { 1 } else { 0 };
    prepared
        .indexes
        .declaration_plan
        .set_owner_ordinal_for_test(pou, u32::try_from(wrong_ordinal + 1).unwrap());
    // A stale cached row falls back to the checked search and returns the same range.
    let recovered = prepared
        .indexes
        .declaration_plan
        .range(StorageOwner::Instance, Some(pou));
    assert_eq!(recovered, direct);
    // An out-of-range POU identity uses the bounded sparse fallback and is absent.
    assert!(prepared
        .indexes
        .declaration_plan
        .range(StorageOwner::Frame, Some(u32::MAX))
        .is_none());
}

#[test]
fn declaration_owner_index_allocation_is_bounded_and_charged() {
    let pou_count = 5;
    let required_bytes = pou_count * core::mem::size_of::<u32>();
    // Empty owners contribute no work; zero-fill plus the global range lookup remain.
    let required_work = pou_count + 1;
    let mut indexes = PreparedIndexes::default();
    let mut budget = PreparationBudget::new(PreparationLimits {
        max_preparation_bytes: required_bytes,
        max_preparation_work: required_work,
        ..PreparationLimits::default()
    });
    indexes
        .prepare_declaration_plan(
            pou_count,
            &crate::bytecode::StorageLayout::default(),
            &BTreeMap::new(),
            &mut budget,
        )
        .unwrap();

    assert_eq!(
        indexes.declaration_plan.owner_index_len_for_test(),
        pou_count
    );
    assert_eq!(indexes.declaration_plan.owner_ordinal_for_test(4), None);
    assert_eq!(budget.usage().bytes, required_bytes);
    assert_eq!(budget.usage().work, required_work);

    for (byte_limit, work_limit) in [
        (required_bytes - 1, usize::MAX),
        (usize::MAX, pou_count - 1),
    ] {
        let mut refused_indexes = PreparedIndexes::default();
        let mut refused = PreparationBudget::new(PreparationLimits {
            max_preparation_bytes: byte_limit,
            max_preparation_work: work_limit,
            ..PreparationLimits::default()
        });
        assert_eq!(
            refused_indexes.prepare_declaration_plan(
                pou_count,
                &crate::bytecode::StorageLayout::default(),
                &BTreeMap::new(),
                &mut refused,
            ),
            Err(RuntimeError::PreparationLimit)
        );
        assert_eq!(
            refused_indexes.declaration_plan.owner_index_len_for_test(),
            0
        );
        assert_eq!(refused_indexes.declaration_plan.owner_capacity(), 0);
    }
}

#[test]
fn root_owner_index_allocation_is_bounded_and_charged_before_reserve() {
    let root_count = 5;
    let mut indexes = PreparedIndexes {
        root_owners: vec![(0, 0), (3, 1)],
        ..Default::default()
    };
    let required_bytes = root_count * core::mem::size_of::<u32>();
    let mut exact = PreparationBudget::new(PreparationLimits {
        max_preparation_bytes: required_bytes,
        max_preparation_work: root_count + indexes.root_owners.len(),
        ..PreparationLimits::default()
    });
    indexes
        .prepare_root_owner_ordinals(root_count, &mut exact)
        .unwrap();
    assert_eq!(indexes.root_owner_ordinals, vec![1, 0, 0, 2, 0]);
    assert_eq!(exact.usage().bytes, required_bytes);
    assert_eq!(exact.usage().work, root_count + 2);

    let mut refused_indexes = PreparedIndexes {
        root_owners: vec![(0, 0), (3, 1)],
        ..Default::default()
    };
    let mut refused = PreparationBudget::new(PreparationLimits {
        max_preparation_bytes: required_bytes - 1,
        max_preparation_work: usize::MAX,
        ..PreparationLimits::default()
    });
    assert_eq!(
        refused_indexes.prepare_root_owner_ordinals(root_count, &mut refused),
        Err(RuntimeError::PreparationLimit)
    );
    assert!(refused_indexes.root_owner_ordinals.is_empty());
    assert_eq!(refused_indexes.root_owner_ordinals.capacity(), 0);
}

#[test]
fn root_owner_direct_table_is_bounded_and_corruption_uses_binary_search() {
    let mut prepared = super::tests::prepared();
    assert_eq!(
        prepared.indexes.root_owner_ordinals.len(),
        prepared.roots.entries.len()
    );
    let owner = prepared
        .indexes
        .root_owners
        .iter()
        .find(|(owner, _)| {
            (*owner as usize) < prepared.indexes.root_owner_ordinals.len()
                && prepared.indexes.root_owner_ordinals[*owner as usize] != 0
        })
        .map(|(owner, _)| *owner)
        .expect("fixture has an in-range root owner");
    let expected = prepared
        .indexes
        .root_owners
        .binary_search_by_key(&owner, |entry| entry.0)
        .ok()
        .map(|at| prepared.indexes.root_owners[at].1);

    let direct = prepared.root_for_instance_owner(owner).map(|(id, _)| id);
    assert_eq!(direct, expected);

    let owner_index = owner as usize;
    let other = prepared
        .indexes
        .root_owners
        .iter()
        .enumerate()
        .find(|(_, entry)| entry.0 != owner)
        .map(|(index, _)| index)
        .expect("fixture has multiple root owners");
    prepared.indexes.root_owner_ordinals[owner_index] = u32::try_from(other + 1).unwrap();
    // A cached row with the wrong owner falls back to the checked search.
    let recovered = prepared.root_for_instance_owner(owner).map(|(id, _)| id);
    assert_eq!(recovered, expected);
    // An out-of-range wire identity uses the bounded sparse fallback and is absent.
    assert!(prepared.root_for_instance_owner(u32::MAX).is_none());
}

#[test]
fn duplicate_root_owner_keeps_binary_search_when_cache_is_forged() {
    let mut prepared = super::tests::prepared();
    prepared.roots.entries = vec![
        crate::bytecode::ConstructionRoot {
            declaration_idx: 0,
            binding_ref_idx: None,
            instance_owner_id: Some(0),
            parent_root_idx: None,
            template_pou_id: None,
            flags: 0,
        },
        crate::bytecode::ConstructionRoot {
            declaration_idx: 0,
            binding_ref_idx: None,
            instance_owner_id: Some(0),
            parent_root_idx: None,
            template_pou_id: None,
            flags: 0,
        },
    ];
    prepared.indexes.root_owners = vec![(0, 0), (0, 1)];
    prepared.indexes.root_owner_ordinals.clear();
    let root_count = prepared.roots.entries.len();
    prepared
        .indexes
        .prepare_root_owner_ordinals(
            root_count,
            &mut PreparationBudget::new(PreparationLimits::default()),
        )
        .unwrap();
    assert_eq!(prepared.indexes.root_owner_ordinals[0], 0);

    let selected = prepared
        .indexes
        .root_owners
        .binary_search_by_key(&0, |entry| entry.0)
        .unwrap();
    let forged = 1 - selected;
    prepared.indexes.root_owner_ordinals[0] = u32::try_from(forged + 1).unwrap();

    // Duplicate owner keys must bypass even a nonzero cached ordinal.
    let result = prepared.root_for_instance_owner(0).map(|(id, _)| id);
    assert_eq!(result, Some(prepared.indexes.root_owners[selected].1));
}

// Build the owning declaration index directly, as existing sparse-layout tests do.
fn declaration_cache_fixture(pou_count: usize, owners: &[u32]) -> PreparedIndexes {
    let prepared = super::tests::prepared();
    let template = prepared
        .layout
        .entries
        .iter()
        .find(|entry| entry.owns_storage())
        .unwrap();
    let mut layout = crate::bytecode::StorageLayout::default();
    let mut indexes = PreparedIndexes::default();
    let mut budget = PreparationBudget::new(PreparationLimits::default());
    for &owner in owners {
        let mut entry = template.clone();
        entry.owner = StorageOwner::Frame;
        entry.owner_pou_id = Some(owner);
        entry.role = crate::bytecode::StorageRole::Variable;
        entry.slot = 0;
        let id = u32::try_from(layout.entries.len()).unwrap();
        layout.entries.push(entry);
        indexes
            .declarations
            .push(
                DeclarationKey::new(StorageOwner::Frame as u8, Some(owner)),
                id,
                &mut budget,
            )
            .unwrap();
    }
    indexes.declarations.finish(&mut budget).unwrap();
    indexes
        .prepare_declaration_plan(pou_count, &layout, &BTreeMap::new(), &mut budget)
        .unwrap();
    indexes
}

#[test]
fn declaration_owner_buckets_cache_sparse_and_maximum_wire_ids() {
    let indexes = declaration_cache_fixture(68, &[u32::MAX, 86, 0]);
    assert_eq!(indexes.declaration_plan.owner_index_len_for_test(), 68);
    for pou in [0, 86, u32::MAX] {
        assert!(indexes
            .declaration_plan
            .owner_ordinal_for_test(pou)
            .is_some());
        let range = indexes
            .declaration_plan
            .range(StorageOwner::Frame, Some(pou))
            .unwrap();
        assert_eq!(range.len(), 1);
    }
}

#[test]
fn declaration_owner_collisions_keep_lower_hits_and_resolve_the_fallback() {
    // Input order differs from the ascending owner order used to fill buckets.
    let indexes = declaration_cache_fixture(3, &[4, u32::MAX, 1]);
    assert!(indexes.declaration_plan.owner_ordinal_for_test(1).is_some());
    assert!(indexes.declaration_plan.owner_ordinal_for_test(4).is_none());
    assert!(indexes
        .declaration_plan
        .range(StorageOwner::Frame, Some(1))
        .is_some());
    assert!(indexes
        .declaration_plan
        .range(StorageOwner::Frame, Some(4))
        .is_some());
}

#[test]
fn declaration_owner_empty_cache_and_absent_identity_resolve_by_search() {
    let empty_cache = declaration_cache_fixture(0, &[7]);
    assert_eq!(empty_cache.declaration_plan.owner_index_len_for_test(), 0);
    assert!(empty_cache
        .declaration_plan
        .range(StorageOwner::Frame, Some(7))
        .is_some());

    let no_owners = declaration_cache_fixture(4, &[]);
    assert!(no_owners
        .declaration_plan
        .range(StorageOwner::Frame, Some(u32::MAX))
        .is_none());

    let indexed = declaration_cache_fixture(3, &[1, 4]);
    assert!(indexed
        .declaration_plan
        .range(StorageOwner::Frame, Some(7))
        .is_none());
}

#[test]
fn declaration_owner_corrupt_sparse_bucket_preserves_result() {
    let mut indexes = declaration_cache_fixture(5, &[1, 86, u32::MAX]);
    // 86 collides with 1; the maximum ID uses a different bucket.
    assert!(indexes
        .declaration_plan
        .owner_ordinal_for_test(u32::MAX)
        .is_some());
    let expected = indexes
        .declaration_plan
        .range(StorageOwner::Frame, Some(u32::MAX));
    for forged in [0, 1, u32::MAX] {
        indexes
            .declaration_plan
            .set_owner_ordinal_for_test(u32::MAX, forged);
        let recovered = indexes
            .declaration_plan
            .range(StorageOwner::Frame, Some(u32::MAX));
        assert_eq!(recovered, expected, "forged cached ordinal {forged}");
    }
}
