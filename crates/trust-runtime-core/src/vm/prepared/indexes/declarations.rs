use super::*;
use crate::bytecode::StorageRole;

impl PreparedIndexes {
    pub(super) fn prepare_declarations(
        &mut self,
        vm: &VmModule,
        layout: &StorageLayout,
        roots: &ConstructionRoots,
        methods: &BTreeMap<u32, u32>,
        budget: &mut PreparationBudget,
    ) -> Result<(), RuntimeError> {
        self.prepare_type_members(vm, budget)?;
        let count = layout.entries.len();
        budget.records::<(u8, Option<u32>, u32, u32)>(count)?;
        budget.records::<(u32, u32, u32)>(count)?;
        budget.records::<(u32, u32)>(count)?;
        budget.records::<Option<usize>>(count)?;
        self.slots
            .try_reserve_exact(count)
            .map_err(|_| RuntimeError::Overflow)?;
        self.members
            .try_reserve_exact(count)
            .map_err(|_| RuntimeError::Overflow)?;
        self.globals
            .try_reserve_exact(count)
            .map_err(|_| RuntimeError::Overflow)?;
        self.static_ordinals
            .try_reserve_exact(count)
            .map_err(|_| RuntimeError::Overflow)?;
        self.static_ordinals.resize(count, None);
        let mut ordinals = BTreeMap::<Option<u32>, usize>::new();
        for (id, declaration) in layout.entries.iter().enumerate() {
            budget.charge(0, 1)?;
            if !declaration.owns_storage() {
                continue;
            }
            let id = u32::try_from(id).map_err(|_| RuntimeError::Overflow)?;
            let physical = declaration.owner_pou_id.map(|pou| {
                if declaration.owner == StorageOwner::Instance {
                    methods.get(&pou).copied().unwrap_or(pou)
                } else {
                    pou
                }
            });
            let owner = if declaration.owner == StorageOwner::Global {
                None
            } else {
                physical
            };
            self.slots
                .push((declaration.owner as u8, owner, declaration.slot, id));
            let key = (declaration.owner as u8, owner);
            push_group(&mut self.declarations, key, id, budget)?;
            if declaration.role == StorageRole::Variable && declaration.is_retained() {
                // Snapshot serialization retains original declaration order, unlike
                // the physical-slot ordering used for constructing storage.
                let retained_owner = if declaration.owner == StorageOwner::Global {
                    None
                } else {
                    declaration.owner_pou_id
                };
                push_group(
                    &mut self.retained_declarations,
                    (declaration.owner as u8, retained_owner),
                    id,
                    budget,
                )?;
            }
            let name = &vm.strings[declaration.name_idx as usize];
            self.max_name_bytes = self.max_name_bytes.max(name.len());
            if declaration.owner == StorageOwner::Global {
                self.globals.push((declaration.name_idx, id));
            } else if declaration.owner == StorageOwner::Instance {
                self.members.push((
                    declaration
                        .owner_pou_id
                        .ok_or(RuntimeError::InvalidExecutionState)?,
                    declaration.name_idx,
                    id,
                ));
            }
            if declaration.role == StorageRole::Static {
                budget.charge(0, lookup_work(ordinals.len()))?;
                if !ordinals.contains_key(&declaration.owner_pou_id) {
                    budget.map::<Option<u32>, usize>(1)?;
                }
                let ordinal = ordinals.entry(declaration.owner_pou_id).or_default();
                self.static_ordinals[id as usize] = Some(*ordinal);
                *ordinal += 1;
            }
            if declaration.role == StorageRole::EdgePhase {
                let input_id = declaration
                    .related_declaration_idx
                    .ok_or(RuntimeError::InvalidExecutionState)?;
                push_group(&mut self.edge_declarations, input_id, id, budget)?;
                let input = layout
                    .entries
                    .get(input_id as usize)
                    .ok_or(RuntimeError::InvalidExecutionState)?;
                let plan = VmEdgeInput {
                    name: vm.strings[input.name_idx as usize].clone(),
                    phase_name: name.clone(),
                    rising: declaration.is_rising_edge(),
                };
                push_group(
                    &mut self.edges,
                    physical.ok_or(RuntimeError::InvalidExecutionState)?,
                    plan,
                    budget,
                )?;
            }
        }
        // In-place sorting retains no additional records. Charge comparison work
        // for the bounded index sort, including case-insensitive name bytes.
        budget.charge(
            0,
            count
                .checked_mul(lookup_work(count))
                .and_then(|n| n.checked_mul(self.max_name_bytes.max(1) + 3))
                .ok_or(RuntimeError::Overflow)?,
        )?;
        self.slots.sort_unstable();
        self.members.sort_unstable_by(|a, b| {
            a.0.cmp(&b.0)
                .then_with(|| compare_names(&vm.strings[a.1 as usize], &vm.strings[b.1 as usize]))
        });
        self.globals.sort_unstable_by(|a, b| {
            compare_names(&vm.strings[a.0 as usize], &vm.strings[b.0 as usize])
        });
        for ids in self.declarations.values_mut() {
            ids.sort_unstable_by_key(|id| layout.entries[*id as usize].slot);
        }
        self.prepare_roots(layout, roots, budget)
    }

    fn prepare_roots(
        &mut self,
        layout: &StorageLayout,
        roots: &ConstructionRoots,
        budget: &mut PreparationBudget,
    ) -> Result<(), RuntimeError> {
        budget.records::<(u32, usize)>(roots.entries.len())?;
        self.root_owners
            .try_reserve_exact(roots.entries.len())
            .map_err(|_| RuntimeError::Overflow)?;
        for (id, root) in roots.entries.iter().enumerate() {
            budget.charge(0, 1)?;
            if let Some(owner) = root.instance_owner_id {
                self.root_owners.push((owner, id));
            }
            if let Some(pou) = root.template_pou_id {
                push_group(
                    &mut self.root_candidates,
                    (root.declaration_idx, pou),
                    id,
                    budget,
                )?;
                if root.parent_root_idx.is_none()
                    && layout.entries[root.declaration_idx as usize].role
                        == StorageRole::ProgramRoot
                {
                    push_group(&mut self.program_roots, pou, id, budget)?;
                }
            }
        }
        budget.charge(
            0,
            roots
                .entries
                .len()
                .checked_mul(lookup_work(roots.entries.len()))
                .ok_or(RuntimeError::Overflow)?,
        )?;
        self.root_owners.sort_unstable();
        Ok(())
    }

    fn prepare_type_members(
        &mut self,
        vm: &VmModule,
        budget: &mut PreparationBudget,
    ) -> Result<(), RuntimeError> {
        let mut count = 0usize;
        for entry in &vm.types.entries {
            budget.charge(0, 1)?;
            if let crate::bytecode::TypeData::Struct { fields }
            | crate::bytecode::TypeData::Union { fields } = &entry.data
            {
                count = count
                    .checked_add(fields.len())
                    .ok_or(RuntimeError::Overflow)?;
            }
        }
        budget.records::<(u32, u32, u32)>(count)?;
        self.type_members
            .try_reserve_exact(count)
            .map_err(|_| RuntimeError::Overflow)?;
        for (id, entry) in vm.types.entries.iter().enumerate() {
            if let crate::bytecode::TypeData::Struct { fields }
            | crate::bytecode::TypeData::Union { fields } = &entry.data
            {
                for field in fields {
                    self.max_name_bytes = self
                        .max_name_bytes
                        .max(vm.strings[field.name_idx as usize].len());
                    self.type_members
                        .push((id as u32, field.name_idx, field.type_id));
                }
            }
        }
        budget.charge(
            0,
            count
                .checked_mul(lookup_work(count))
                .and_then(|n| n.checked_mul(self.max_name_bytes.max(1)))
                .ok_or(RuntimeError::Overflow)?,
        )?;
        self.type_members.sort_unstable_by(|a, b| {
            a.0.cmp(&b.0)
                .then_with(|| compare_names(&vm.strings[a.1 as usize], &vm.strings[b.1 as usize]))
        });
        Ok(())
    }
}

/// Charge map insertion and geometric vector capacity before either can allocate.
pub(super) fn push_group<K: Ord, V>(
    map: &mut BTreeMap<K, Vec<V>>,
    key: K,
    value: V,
    budget: &mut PreparationBudget,
) -> Result<(), RuntimeError> {
    budget.charge(0, lookup_work(map.len()))?;
    if !map.contains_key(&key) {
        budget.map::<K, Vec<V>>(1)?;
    }
    let values = map.entry(key).or_default();
    if values.len() == values.capacity() {
        let extra = values.capacity().max(1);
        budget.records::<V>(extra)?;
        values
            .try_reserve_exact(extra)
            .map_err(|_| RuntimeError::Overflow)?;
    }
    values.push(value);
    Ok(())
}
