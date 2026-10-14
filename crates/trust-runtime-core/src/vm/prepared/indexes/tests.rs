use super::*;
use crate::bytecode::{BytecodeModule, StorageRole};
use alloc::vec;
use smol_str::SmolStr;

pub(super) fn prepared() -> PreparedModule {
    PreparedModule::from_bytes(
        include_bytes!(
            "../../../../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc"
        ),
        PreparationLimits::default(),
    )
    .unwrap()
}

#[test]
fn named_types_preserve_first_declared_case_insensitive_lookup() {
    use crate::bytecode::{TypeData, TypeEntry, TypeKind};
    let mut prepared = prepared();
    let name = prepared.vm.strings.len() as u32;
    prepared
        .vm
        .strings
        .extend(["FirstName".into(), "FIRSTNAME".into(), "SecondName".into()]);
    let first = prepared.vm.types.entries.len() as u32;
    for (offset, prim_id) in [(0, 16), (1, 17), (2, 7)] {
        prepared.vm.types.entries.push(TypeEntry {
            kind: TypeKind::Primitive,
            name_idx: Some(name + offset),
            data: TypeData::Primitive {
                prim_id,
                max_length: 0,
            },
        });
    }
    let mut indexes = PreparedIndexes::default();
    indexes
        .prepare_named_types(
            &prepared.vm,
            &mut PreparationBudget::new(PreparationLimits::default()),
        )
        .unwrap();
    prepared.indexes = indexes;
    assert_eq!(prepared.named_type("fIrStNaMe"), Some(first));
    assert_eq!(prepared.named_type("secondname"), Some(first + 2));
    assert_eq!(prepared.named_type("absent"), None);
    let mut refused = PreparedIndexes::default();
    assert_eq!(
        refused.prepare_named_types(
            &prepared.vm,
            &mut PreparationBudget::new(PreparationLimits {
                max_preparation_bytes: 0,
                ..PreparationLimits::default()
            })
        ),
        Err(RuntimeError::PreparationLimit)
    );
    assert!(refused.named_types.is_empty());
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
        &prepared.resources,
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
        &prepared.resources,
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
            && let Some(owner) = action.owner_pou_id
        {
            expected_instances
                .entry(prepared.method_owners.get(&owner).copied().unwrap_or(owner))
                .or_default()
                .push(id as u32);
        }
    }
    for (key, expected) in expected_actions {
        assert_eq!(
            prepared
                .indexes
                .actions
                .get(&ActionKey::new(key.0, key.1, key.2)),
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
            prepared
                .indexes
                .declarations
                .get(&DeclarationKey::new(key.0, key.1)),
            Some(expected.as_slice())
        );
    }
    for (key, expected) in retained {
        assert_eq!(
            prepared
                .indexes
                .retained_declarations
                .get(&DeclarationKey::new(key.0, key.1)),
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

#[test]
fn frame_action_plans_match_wire_groups_for_all_owners_and_empty_phases() {
    let prepared = prepared();
    let relevant_owner = |entry: &crate::bytecode::InitializerEntry| {
        entry.is_action()
            && entry.owner_pou_id.is_some()
            && matches!(
                entry.phase,
                InitializationPhase::Parameter
                    | InitializationPhase::Return
                    | InitializationPhase::Static
                    | InitializationPhase::Frame
            )
    };
    let mut expected_owners = prepared
        .initializers
        .entries
        .iter()
        .filter(|entry| relevant_owner(entry))
        .filter_map(|entry| entry.owner_pou_id)
        .collect::<Vec<_>>();
    expected_owners.sort_unstable();
    expected_owners.dedup();
    assert_eq!(
        prepared
            .indexes
            .frame_action_plans
            .iter()
            .map(|(owner, _)| *owner)
            .collect::<Vec<_>>(),
        expected_owners
    );

    let mut owners = prepared.vm.pou_by_id.keys().copied().collect::<Vec<_>>();
    owners.extend(expected_owners.iter().copied());
    owners.sort_unstable();
    owners.dedup();
    let phases = [
        InitializationPhase::Parameter,
        InitializationPhase::Return,
        InitializationPhase::Static,
        InitializationPhase::Frame,
    ];
    let triggers = [
        InitializationTrigger::Ordinary,
        InitializationTrigger::AfterRestart,
    ];
    for owner in owners {
        let plan = prepared.frame_action_plan_index(owner);
        for phase in phases {
            for trigger in triggers {
                let planned = plan.map_or(&[][..], |index| {
                    prepared.frame_actions_at(index, phase, trigger)
                });
                assert_eq!(
                    planned,
                    prepared.actions(Some(owner), phase, trigger),
                    "owner={owner} phase={phase:?} trigger={trigger:?}"
                );
            }
        }
    }

    let missing_owner = (0..=prepared.vm.pou_by_id.len() as u32)
        .find(|owner| !prepared.vm.pou_by_id.contains_key(owner))
        .unwrap();
    assert!(prepared.frame_action_plan_index(missing_owner).is_none());
    for phase in phases {
        for trigger in triggers {
            assert!(prepared
                .actions(Some(missing_owner), phase, trigger)
                .is_empty());
        }
    }
}

#[test]
fn frame_action_plan_stream_merge_keeps_sparse_owners_and_mixed_triggers() {
    let mut indexes = PreparedIndexes::default();
    let mut budget = PreparationBudget::new(PreparationLimits::default());
    for (owner, phase, trigger, id) in [
        (
            Some(u32::MAX),
            InitializationPhase::Static,
            InitializationTrigger::AfterRestart,
            10,
        ),
        (
            Some(7),
            InitializationPhase::Frame,
            InitializationTrigger::Ordinary,
            4,
        ),
        (
            None,
            InitializationPhase::Parameter,
            InitializationTrigger::Ordinary,
            99,
        ),
        (
            Some(0),
            InitializationPhase::Parameter,
            InitializationTrigger::Ordinary,
            2,
        ),
        (
            Some(u32::MAX),
            InitializationPhase::Return,
            InitializationTrigger::Ordinary,
            11,
        ),
        (
            Some(7),
            InitializationPhase::Static,
            InitializationTrigger::Ordinary,
            3,
        ),
        (
            Some(u32::MAX),
            InitializationPhase::Parameter,
            InitializationTrigger::AfterRestart,
            12,
        ),
    ] {
        indexes
            .actions
            .push(
                ActionKey::new(owner, phase as u8, trigger as u8),
                id,
                &mut budget,
            )
            .unwrap();
    }
    // The ownerless key is not eligible for a frame plan even though it uses a
    // frame-relevant phase.
    indexes.actions.finish(&mut budget).unwrap();
    indexes.prepare_frame_action_plans(&mut budget).unwrap();

    assert_eq!(
        indexes
            .frame_action_plans
            .iter()
            .map(|(owner, _)| *owner)
            .collect::<Vec<_>>(),
        [0, 7, u32::MAX]
    );
    let phases = [
        InitializationPhase::Parameter,
        InitializationPhase::Return,
        InitializationPhase::Static,
        InitializationPhase::Frame,
    ];
    for owner in [0, 7, u32::MAX] {
        let plan_index = indexes
            .frame_action_plans
            .binary_search_by_key(&owner, |entry| entry.0)
            .unwrap();
        let (_, plan) = &indexes.frame_action_plans[plan_index];
        for phase in phases {
            for trigger in [
                InitializationTrigger::Ordinary,
                InitializationTrigger::AfterRestart,
            ] {
                let planned = indexes.actions.slice(&plan.range(phase, trigger));
                let grouped = if matches!(
                    (phase, trigger),
                    (
                        InitializationPhase::Parameter
                            | InitializationPhase::Return
                            | InitializationPhase::Frame,
                        InitializationTrigger::Ordinary
                    ) | (
                        InitializationPhase::Static,
                        InitializationTrigger::Ordinary | InitializationTrigger::AfterRestart
                    )
                ) {
                    indexes
                        .actions
                        .get(&ActionKey::new(Some(owner), phase as u8, trigger as u8))
                        .unwrap_or(&[])
                } else {
                    &[]
                };
                assert_eq!(planned, grouped, "owner={owner} {phase:?} {trigger:?}");
            }
        }
    }
    assert!(indexes
        .frame_action_plans
        .binary_search_by_key(&8, |entry| entry.0)
        .is_err());
}

#[test]
fn frame_action_plan_storage_is_refused_before_its_allocation() {
    let prepared = prepared();
    let mut measure = PreparedIndexes::default();
    let mut measure_budget = PreparationBudget::new(PreparationLimits::default());
    measure
        .prepare_named_types(&prepared.vm, &mut measure_budget)
        .unwrap();
    measure
        .prepare_declarations(
            &prepared.vm,
            &prepared.layout,
            &prepared.roots,
            &prepared.method_owners,
            &mut measure_budget,
        )
        .unwrap();
    measure
        .prepare_actions(
            &prepared.initializers,
            &prepared.method_owners,
            &mut measure_budget,
        )
        .unwrap();
    let prior_bytes = measure_budget.usage().bytes;
    measure
        .prepare_frame_action_plans(&mut measure_budget)
        .unwrap();
    let plan_bytes = measure_budget.usage().bytes - prior_bytes;
    assert!(plan_bytes > 0);
    let max_preparation_bytes = prior_bytes.checked_add(plan_bytes - 1).unwrap();

    let mut limited = PreparedIndexes::default();
    let mut limited_budget = PreparationBudget::new(PreparationLimits {
        max_preparation_bytes,
        ..PreparationLimits::default()
    });
    limited
        .prepare_named_types(&prepared.vm, &mut limited_budget)
        .unwrap();
    limited
        .prepare_declarations(
            &prepared.vm,
            &prepared.layout,
            &prepared.roots,
            &prepared.method_owners,
            &mut limited_budget,
        )
        .unwrap();
    limited
        .prepare_actions(
            &prepared.initializers,
            &prepared.method_owners,
            &mut limited_budget,
        )
        .unwrap();
    assert_eq!(
        limited.prepare_frame_action_plans(&mut limited_budget),
        Err(RuntimeError::PreparationLimit)
    );
    assert!(limited.frame_action_plans.is_empty());
    assert_eq!(limited.frame_action_plans.capacity(), 0);
}

#[test]
fn declaration_lookup_matches_linear_owner_and_slot_resolution() {
    let prepared = prepared();
    for owner in [
        StorageOwner::Global,
        StorageOwner::Instance,
        StorageOwner::Frame,
    ] {
        let mut keys = Vec::<Option<u32>>::new();
        for declaration in &prepared.layout.entries {
            if declaration.owner != owner || !declaration.owns_storage() {
                continue;
            }
            let pou = if owner == StorageOwner::Global {
                None
            } else {
                declaration.owner_pou_id.map(|pou| {
                    if owner == StorageOwner::Instance {
                        prepared.method_owners.get(&pou).copied().unwrap_or(pou)
                    } else {
                        pou
                    }
                })
            };
            if !keys.contains(&pou) {
                keys.push(pou);
            }
        }
        for pou in keys {
            let max_slot = prepared
                .layout
                .entries
                .iter()
                .filter(|declaration| {
                    if declaration.owner != owner || !declaration.owns_storage() {
                        return false;
                    }
                    let physical = if owner == StorageOwner::Global {
                        None
                    } else {
                        declaration.owner_pou_id.map(|pou| {
                            if owner == StorageOwner::Instance {
                                prepared.method_owners.get(&pou).copied().unwrap_or(pou)
                            } else {
                                pou
                            }
                        })
                    };
                    physical == pou
                })
                .map(|declaration| declaration.slot)
                .max()
                .unwrap_or(0);
            for slot in 0..=max_slot.saturating_add(2) as usize {
                let expected = prepared
                    .layout
                    .entries
                    .iter()
                    .enumerate()
                    .find(|(_, declaration)| {
                        if declaration.owner != owner
                            || !declaration.owns_storage()
                            || declaration.slot as usize != slot
                        {
                            return false;
                        }
                        let physical = if owner == StorageOwner::Global {
                            None
                        } else {
                            declaration.owner_pou_id.map(|pou| {
                                if owner == StorageOwner::Instance {
                                    prepared.method_owners.get(&pou).copied().unwrap_or(pou)
                                } else {
                                    pou
                                }
                            })
                        };
                        physical == pou
                    })
                    .map(|(id, _)| id as u32);
                let actual = prepared.declaration(owner, pou, slot).map(|(id, _)| id);
                assert_eq!(actual, expected, "{owner:?} {pou:?} slot {slot}");
            }
        }
    }
    assert!(prepared
        .declaration(StorageOwner::Global, None, usize::MAX)
        .is_none());
    let global_max_slot = prepared
        .layout
        .entries
        .iter()
        .filter(|entry| entry.owner == StorageOwner::Global && entry.owns_storage())
        .map(|entry| entry.slot)
        .max()
        .unwrap_or(0);
    for pou in [None, Some(0), Some(u32::MAX)] {
        for slot in 0..=global_max_slot.saturating_add(1) as usize {
            let expected = prepared
                .layout
                .entries
                .iter()
                .enumerate()
                .find(|(_, entry)| {
                    entry.owner == StorageOwner::Global
                        && entry.owns_storage()
                        && entry.slot as usize == slot
                })
                .map(|(id, _)| id as u32);
            assert_eq!(
                prepared
                    .declaration(StorageOwner::Global, pou, slot)
                    .map(|(id, _)| id),
                expected,
                "global lookup with {pou:?} at slot {slot}"
            );
        }
    }
}

#[test]
fn declaration_lookup_falls_back_for_scratch_gaps_and_normalizes_method_aliases() {
    let mut prepared = prepared();
    let (&method, &owner) = prepared.method_owners.iter().next().unwrap();
    let template = prepared
        .layout
        .entries
        .iter()
        .find(|declaration| {
            declaration.owner == StorageOwner::Instance
                && declaration.owner_pou_id == Some(owner)
                && declaration.role == StorageRole::Variable
        })
        .unwrap()
        .clone();

    let mut root_member = template.clone();
    root_member.slot = 0;
    let mut external = template.clone();
    external.owner_pou_id = Some(method);
    external.role = StorageRole::External;
    external.slot = 1;
    let mut method_member = template.clone();
    method_member.owner_pou_id = Some(method);
    method_member.slot = 1;

    let mut scratch = template.clone();
    scratch.owner = StorageOwner::Frame;
    scratch.owner_pou_id = Some(method);
    scratch.role = StorageRole::Scratch;
    scratch.type_id = None;
    scratch.slot = 0;
    scratch.construction_nodes = 2;
    let mut frame_member = template.clone();
    frame_member.owner = StorageOwner::Frame;
    frame_member.owner_pou_id = Some(method);
    frame_member.slot = 2;
    let mut frame_member_after = frame_member.clone();
    frame_member_after.slot = 3;

    prepared.layout.entries = vec![
        root_member,
        external,
        method_member,
        scratch,
        frame_member,
        frame_member_after,
    ];
    let mut groups = PreparedIndexes::default();
    let mut budget = PreparationBudget::new(PreparationLimits::default());
    for ((owner, pou), id) in [
        ((StorageOwner::Instance as u8, Some(owner)), 0),
        ((StorageOwner::Instance as u8, Some(owner)), 2),
        ((StorageOwner::Frame as u8, Some(method)), 3),
        ((StorageOwner::Frame as u8, Some(method)), 4),
        ((StorageOwner::Frame as u8, Some(method)), 5),
    ] {
        groups
            .declarations
            .push(DeclarationKey::new(owner, pou), id, &mut budget)
            .unwrap();
    }
    groups.declarations.finish(&mut budget).unwrap();
    groups
        .declarations
        .sort_values(&mut budget, |a, b| {
            prepared.layout.entries[*a as usize]
                .slot
                .cmp(&prepared.layout.entries[*b as usize].slot)
        })
        .unwrap();
    groups
        .prepare_declaration_plan(
            prepared.vm.pou_by_id.len(),
            &prepared.layout,
            &prepared.method_owners,
            &mut budget,
        )
        .unwrap();
    prepared.indexes = groups;

    assert_eq!(
        prepared
            .declaration(StorageOwner::Instance, Some(owner), 1)
            .map(|(id, _)| id),
        Some(2),
        "the external alias does not shadow the method-owned storage slot"
    );
    assert!(prepared
        .declaration(StorageOwner::Instance, Some(method), 1)
        .is_none());
    assert_eq!(
        prepared
            .declaration(StorageOwner::Frame, Some(method), 2)
            .map(|(id, _)| id),
        Some(4),
        "the scratch-width gap requires slot search after a dense candidate mismatch"
    );
    assert!(prepared
        .declaration(StorageOwner::Frame, Some(method), 1)
        .is_none());
    assert_eq!(
        prepared
            .declaration(StorageOwner::Frame, Some(method), 3)
            .map(|(id, _)| id),
        Some(5)
    );
}

#[test]
fn declaration_plan_falls_back_for_sparse_reordered_pou_ids() {
    let mut prepared = prepared();
    let template = prepared
        .layout
        .entries
        .iter()
        .find(|declaration| declaration.owns_storage())
        .unwrap()
        .clone();
    let mut low = template.clone();
    low.owner = StorageOwner::Frame;
    low.owner_pou_id = Some(7);
    low.role = StorageRole::Variable;
    low.slot = 2;
    let mut high = low.clone();
    high.owner_pou_id = Some(u32::MAX);
    high.slot = 4;
    prepared.layout.entries = vec![high, low];

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
        .sort_values(&mut budget, |left, right| {
            prepared.layout.entries[*left as usize]
                .slot
                .cmp(&prepared.layout.entries[*right as usize].slot)
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

    for (pou, slot, expected_id) in [(7_u32, 2_usize, 1_u32), (u32::MAX, 4, 0)] {
        let expected_group = prepared
            .indexes
            .declarations
            .get(&DeclarationKey::new(StorageOwner::Frame as u8, Some(pou)))
            .unwrap();
        let expected = expected_group
            .iter()
            .copied()
            .find(|id| prepared.layout.entries[*id as usize].slot as usize == slot);
        let actual = prepared
            .declaration(StorageOwner::Frame, Some(pou), slot)
            .map(|(id, _)| id);
        assert_eq!(actual, expected);
        assert_eq!(actual, Some(expected_id));
    }
    assert!(prepared
        .declaration(StorageOwner::Frame, Some(8), 2)
        .is_none());
}

#[test]
fn declaration_plan_records_are_refused_before_owner_plan_allocation() {
    let prepared = prepared();
    let mut indexes = PreparedIndexes::default();
    let mut budget = PreparationBudget::new(PreparationLimits::default());
    indexes
        .prepare_named_types(&prepared.vm, &mut budget)
        .unwrap();
    indexes
        .prepare_declarations(
            &prepared.vm,
            &prepared.layout,
            &prepared.roots,
            &prepared.method_owners,
            &mut budget,
        )
        .unwrap();
    let owner_count = prepared
        .layout
        .entries
        .iter()
        .filter(|entry| {
            entry.owns_storage()
                && matches!(entry.owner, StorageOwner::Frame | StorageOwner::Instance)
        })
        .count();
    assert!(owner_count > 0);
    let scratch_bytes = owner_count
        .checked_mul(core::mem::size_of::<u32>())
        .unwrap();
    let max_preparation_bytes = budget.usage().bytes.checked_add(scratch_bytes).unwrap();
    let mut limited = PreparationBudget::new(PreparationLimits {
        max_preparation_bytes,
        ..PreparationLimits::default()
    });
    // Repeat the prior setup under the constrained budget so the owner-id
    // scratch vector fits, while the persistent deduplicated plan does not.
    let mut limited_indexes = PreparedIndexes::default();
    limited_indexes
        .prepare_named_types(&prepared.vm, &mut limited)
        .unwrap();
    limited_indexes
        .prepare_declarations(
            &prepared.vm,
            &prepared.layout,
            &prepared.roots,
            &prepared.method_owners,
            &mut limited,
        )
        .unwrap();
    assert_eq!(
        limited_indexes.prepare_declaration_plan(
            prepared.vm.pou_by_id.len(),
            &prepared.layout,
            &prepared.method_owners,
            &mut limited,
        ),
        Err(RuntimeError::PreparationLimit)
    );
    assert_eq!(limited_indexes.declaration_plan.owner_capacity(), 0);
}
