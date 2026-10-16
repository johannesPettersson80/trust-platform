use super::*;
use crate::bytecode::{TypeData, TypeEntry, TypeKind};
use alloc::vec::Vec;
use core::cmp::Ordering;
use smol_str::SmolStr;

#[test]
fn lookup_work_bounds_real_binary_search_comparisons() {
    for (count, expected_calls) in [
        (0, 0),
        (1, 1),
        (2, 2),
        (3, 3),
        (4, 3),
        (5, 4),
        (7, 4),
        (8, 4),
        (9, 5),
        (15, 5),
        (16, 5),
        (17, 6),
        (31, 6),
        (32, 6),
        (33, 7),
        (63, 7),
        (64, 7),
        (65, 8),
    ] {
        let entries = (0..count).map(|index| index * 2).collect::<Vec<_>>();
        for query in 0..=count * 2 {
            let mut calls = 0;
            let result = entries.binary_search_by(|candidate| {
                calls += 1;
                candidate.cmp(&query)
            });
            assert_eq!(calls, expected_calls, "count={count}, query={query}");
            assert!(calls <= lookup_work(count));
            assert_eq!(
                result.ok(),
                entries.iter().position(|candidate| *candidate == query)
            );
        }
        assert_eq!(lookup_work(count), expected_calls.max(1));
    }
}

fn counted_name_search(entries: &[(u32, &str)], owner: u32, query: &str) -> (Option<usize>, usize) {
    let mut calls = 0;
    let mut budget = PreparationBudget::new(PreparationLimits::default());
    let result = entries.binary_search_by(|(candidate_owner, candidate)| {
        calls += 1;
        let ownership = candidate_owner.cmp(&owner);
        if ownership != Ordering::Equal {
            return ownership;
        }
        let actual = compare_names(candidate, query);
        let charged = sort::compare_names_charged(candidate, query, &mut budget).unwrap();
        assert_eq!(actual, charged);
        actual
    });
    (result.ok(), calls)
}

#[test]
fn owner_qualified_name_search_matches_the_linear_result() {
    let names = ["", "A", "Ab", "AlphabetWithALongSuffix", "É", "é", "界"];
    let mut entries = (0..3)
        .flat_map(|owner| names.map(|name| (owner, name)))
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| {
        left.0
            .cmp(&right.0)
            .then_with(|| compare_names(left.1, right.1))
    });
    for count in [0, 1, 2, 3, 4, 7, 8, 9, 15, 16, 17, entries.len()] {
        let selected = &entries[..count];
        for owner in [0, 1, 2, 3, u32::MAX] {
            for query in [
                "",
                "a",
                "AB",
                "alphabet",
                "AlphabetWithALongSuffix",
                "AlphabetWithALongSuffixAndMore",
                "Z",
                "É",
                "é",
                "界",
                "界面",
            ] {
                let (found, calls) = counted_name_search(selected, owner, query);
                assert_eq!(
                    found,
                    selected.iter().position(|(candidate_owner, candidate)| {
                        *candidate_owner == owner && compare_names(candidate, query).is_eq()
                    })
                );
                assert!(calls <= lookup_work(count));
            }
        }
    }
}

#[test]
fn query_result_is_independent_of_an_unrelated_long_admitted_name() {
    let mut prepared = PreparedModule::from_bytes(
        include_bytes!(
            "../../../../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc"
        ),
        PreparationLimits::default(),
    )
    .unwrap();
    let unused_name = prepared.vm.strings.len() as u32;
    prepared.vm.strings.push("UnusedShortName".into());
    let unused_type = prepared.vm.types.entries.len() as u32;
    prepared.vm.types.entries.push(TypeEntry {
        kind: TypeKind::Primitive,
        name_idx: Some(unused_name),
        data: TypeData::Primitive {
            prim_id: 7,
            max_length: 0,
        },
    });
    let rebuild = |prepared: &mut PreparedModule| {
        prepared.indexes = PreparedIndexes::build(
            &prepared.vm,
            &prepared.layout,
            &prepared.roots,
            &prepared.initializers,
            &prepared.method_owners,
            &prepared.resources,
            &mut PreparationBudget::new(PreparationLimits::default()),
        )
        .unwrap();
    };
    rebuild(&mut prepared);
    let (root, declaration) = prepared.global("pLaNt").unwrap();
    let owner = declaration.owner_pou_id.unwrap();
    let member = prepared.member(owner, "ACTIVATIONS").unwrap().clone();
    assert_eq!(prepared.named_type("unusedshortname"), Some(unused_type));

    prepared.vm.strings[unused_name as usize] = SmolStr::new("UnrelatedLongName".repeat(256));
    rebuild(&mut prepared);
    assert_eq!(prepared.global("PLANT").unwrap().0, root);
    assert_eq!(prepared.member(owner, "activations"), Some(&member));
    assert_eq!(
        prepared.named_type(&prepared.vm.strings[unused_name as usize]),
        Some(unused_type)
    );
}

#[test]
fn preparation_lookup_bound_stays_exact_at_the_largest_counts() {
    assert_eq!(lookup_work(usize::MAX), usize::BITS as usize + 1);
    assert_eq!(
        lookup_work(1usize << (usize::BITS - 1)),
        usize::BITS as usize
    );
}
