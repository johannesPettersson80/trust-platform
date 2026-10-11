use super::*;
use crate::bytecode::{BytecodeModule, StorageRole};
use alloc::vec;
use smol_str::SmolStr;

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

#[test]
fn grouped_indexes_match_wire_collections_for_every_fixture_owner() {
    let prepared = prepared();
    let mut expected_actions = BTreeMap::<(Option<u32>, u8, u8), Vec<u32>>::new();
    let mut expected_instances = BTreeMap::<u32, Vec<u32>>::new();
    for (id, action) in prepared.initializers.entries.iter().enumerate() {
        if !action.is_action() {
            continue;
        }
        expected_actions
            .entry((
                action.owner_pou_id,
                action.phase as u8,
                action.trigger as u8,
            ))
            .or_default()
            .push(id as u32);
        if action.trigger == InitializationTrigger::Ordinary
            && matches!(
                action.phase,
                InitializationPhase::Instance | InitializationPhase::Static
            )
        {
            if let Some(owner) = action.owner_pou_id {
                expected_instances
                    .entry(prepared.method_owners.get(&owner).copied().unwrap_or(owner))
                    .or_default()
                    .push(id as u32);
            }
        }
    }
    for (key, expected) in expected_actions {
        assert_eq!(
            prepared.indexes.actions.get(&key),
            Some(expected.as_slice())
        );
    }
    for (key, expected) in expected_instances {
        assert_eq!(prepared.instance_actions(key), expected);
    }
    let mut roots = BTreeMap::<(u32, u32), Vec<usize>>::new();
    let mut programs = BTreeMap::<u32, Vec<usize>>::new();
    for (id, root) in prepared.roots.entries.iter().enumerate() {
        if let Some(pou) = root.template_pou_id {
            roots
                .entry((root.declaration_idx, pou))
                .or_default()
                .push(id);
            if root.parent_root_idx.is_none()
                && prepared.layout.entries[root.declaration_idx as usize].role
                    == StorageRole::ProgramRoot
            {
                programs.entry(pou).or_default().push(id);
            }
        }
    }
    for ((declaration, pou), expected) in roots {
        assert_eq!(prepared.root_candidates(declaration, pou), expected);
    }
    for (pou, expected) in programs {
        assert_eq!(prepared.program_roots(pou), expected);
    }
    let mut declarations = BTreeMap::<(u8, Option<u32>), Vec<u32>>::new();
    let mut retained = BTreeMap::<(u8, Option<u32>), Vec<u32>>::new();
    let mut edges = BTreeMap::<u32, Vec<u32>>::new();
    let mut edge_inputs = BTreeMap::<u32, Vec<(SmolStr, SmolStr, bool)>>::new();
    for (id, declaration) in prepared.layout.entries.iter().enumerate() {
        if !declaration.owns_storage() {
            continue;
        }
        let physical = declaration.owner_pou_id.map(|pou| {
            if declaration.owner == StorageOwner::Instance {
                prepared.method_owners.get(&pou).copied().unwrap_or(pou)
            } else {
                pou
            }
        });
        let owner = if declaration.owner == StorageOwner::Global {
            None
        } else {
            physical
        };
        declarations
            .entry((declaration.owner as u8, owner))
            .or_default()
            .push(id as u32);
        if declaration.role == StorageRole::Variable && declaration.is_retained() {
            let owner = if declaration.owner == StorageOwner::Global {
                None
            } else {
                declaration.owner_pou_id
            };
            retained
                .entry((declaration.owner as u8, owner))
                .or_default()
                .push(id as u32);
        }
        if declaration.role == StorageRole::EdgePhase {
            let input_id = declaration.related_declaration_idx.unwrap();
            edges.entry(input_id).or_default().push(id as u32);
            let input = &prepared.layout.entries[input_id as usize];
            edge_inputs.entry(physical.unwrap()).or_default().push((
                prepared.vm.strings[input.name_idx as usize].clone(),
                prepared.vm.strings[declaration.name_idx as usize].clone(),
                declaration.is_rising_edge(),
            ));
        }
    }
    for (key, mut expected) in declarations {
        expected.sort_unstable_by_key(|id| prepared.layout.entries[*id as usize].slot);
        assert_eq!(
            prepared.indexes.declarations.get(&key),
            Some(expected.as_slice())
        );
    }
    for (key, expected) in retained {
        assert_eq!(
            prepared.indexes.retained_declarations.get(&key),
            Some(expected.as_slice())
        );
    }
    for (key, expected) in edges {
        assert_eq!(prepared.edge_declarations(key), expected);
    }
    for (pou, expected) in edge_inputs {
        let actual = prepared
            .edge_inputs(pou)
            .iter()
            .map(|edge| (edge.name.clone(), edge.phase_name.clone(), edge.rising))
            .collect::<Vec<_>>();
        assert_eq!(actual, expected);
    }
}
