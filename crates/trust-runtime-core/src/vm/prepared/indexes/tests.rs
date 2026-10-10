use super::*;
use crate::bytecode::{BytecodeModule, StorageRole};
use alloc::vec;

fn prepared() -> PreparedModule {
    PreparedModule::from_bytes(
        include_bytes!(
            "../../../../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc"
        ),
        PreparationLimits::default(),
    )
    .unwrap()
}

#[test]
fn physical_indexes_keep_case_insensitive_identity_and_ignore_external_aliases() {
    let mut prepared = prepared();
    let (root_id, root) = prepared.global("pLaNt").unwrap();
    let pou = root.owner_pou_id.unwrap();
    let real = prepared.member(pou, "ACTIVATIONS").unwrap().clone();
    let mut alias = real.clone();
    alias.role = StorageRole::External;
    alias.slot = u32::MAX;
    alias.flags = 1;
    prepared.layout.entries.push(alias);
    let mut budget = PreparationBudget::new(PreparationLimits::default());
    prepared.indexes = PreparedIndexes::build(
        &prepared.vm,
        &prepared.layout,
        &prepared.roots,
        &prepared.initializers,
        &prepared.method_owners,
        &mut budget,
    )
    .unwrap();
    assert_eq!(prepared.global("PLANT").unwrap().0, root_id);
    assert_eq!(prepared.member(pou, "activations"), Some(&real));
    assert!(prepared
        .declaration(StorageOwner::Instance, Some(pou), u32::MAX as usize)
        .is_none());
    let phase = &prepared.edge_inputs(*prepared.vm.function_block_ids.get("COUNTER").unwrap())[0];
    assert!(phase.rising);
    assert_eq!(phase.name.as_str(), "pulse");
}

#[test]
fn prepared_lifecycle_lists_preserve_wire_order_and_explicit_once_completion() {
    let prepared = prepared();
    for ids in prepared
        .indexes
        .actions
        .values()
        .chain(prepared.indexes.instance_actions.values())
    {
        assert!(ids.windows(2).all(|pair| pair[0] < pair[1]));
    }
    for (id, root) in prepared.roots.entries.iter().enumerate() {
        if let Some(owner) = root.instance_owner_id {
            assert_eq!(
                prepared.root_for_instance_owner(owner).map(|entry| entry.0),
                Some(id)
            );
        }
    }
    let bytes = include_bytes!(
        "../../../../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc"
    );
    let module = BytecodeModule::decode(bytes).unwrap();
    let usage = PreparedModule::from_decoded(&module, PreparationLimits::default())
        .unwrap()
        .preparation_usage();
    let limits = PreparationLimits {
        max_preparation_bytes: usage.bytes - 1,
        ..Default::default()
    };
    assert!(PreparedModule::from_decoded(&module, limits).is_err());
}

#[test]
fn retained_groups_preserve_wire_order_instead_of_physical_slot_order() {
    let mut prepared = prepared();
    let pou = prepared.global("Plant").unwrap().1.owner_pou_id.unwrap();
    let ids = prepared
        .layout
        .entries
        .iter()
        .enumerate()
        .filter(|(_, declaration)| {
            declaration.owner == StorageOwner::Instance
                && declaration.owner_pou_id == Some(pou)
                && declaration.role == StorageRole::Variable
        })
        .map(|(id, _)| id)
        .take(2)
        .collect::<Vec<_>>();
    assert_eq!(ids.len(), 2);
    prepared.layout.entries[ids[0]].retain = 1;
    prepared.layout.entries[ids[1]].retain = 1;
    prepared.layout.entries.swap(ids[0], ids[1]);
    let mut budget = PreparationBudget::new(PreparationLimits::default());
    prepared.indexes = PreparedIndexes::build(
        &prepared.vm,
        &prepared.layout,
        &prepared.roots,
        &prepared.initializers,
        &prepared.method_owners,
        &mut budget,
    )
    .unwrap();
    let selected = |id: &&u32| ids.contains(&(**id as usize));
    let retained = prepared
        .retained_declarations(StorageOwner::Instance, Some(pou))
        .iter()
        .filter(selected)
        .copied()
        .collect::<Vec<_>>();
    let physical = prepared
        .declarations(StorageOwner::Instance, Some(pou))
        .iter()
        .filter(selected)
        .copied()
        .collect::<Vec<_>>();
    assert_eq!(retained, vec![ids[0] as u32, ids[1] as u32]);
    assert_eq!(physical, vec![ids[1] as u32, ids[0] as u32]);
}
