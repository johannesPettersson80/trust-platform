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
        self.declarations.finish(budget)?;
        self.retained_declarations.finish(budget)?;
        self.edge_declarations.finish(budget)?;
        self.edges.finish(budget)?;
        sort::sort_by(&mut self.slots, budget, &mut |a, b, _| Ok(a.cmp(b)))?;
        sort::sort_by(&mut self.members, budget, &mut |a, b, budget| {
            let owner = a.0.cmp(&b.0);
            if owner.is_eq() {
                sort::compare_names_charged(
                    &vm.strings[a.1 as usize],
                    &vm.strings[b.1 as usize],
                    budget,
                )
            } else {
                Ok(owner)
            }
        })?;
        sort::sort_by(&mut self.globals, budget, &mut |a, b, budget| {
            sort::compare_names_charged(
                &vm.strings[a.0 as usize],
                &vm.strings[b.0 as usize],
                budget,
            )
        })?;
        self.declarations.sort_values(budget, |a, b| {
            layout.entries[*a as usize]
                .slot
                .cmp(&layout.entries[*b as usize].slot)
        })?;
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
        sort::sort_by(&mut self.root_owners, budget, &mut |a, b, _| Ok(a.cmp(b)))?;
        self.root_candidates.finish(budget)?;
        self.program_roots.finish(budget)?;
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
        sort::sort_by(&mut self.type_members, budget, &mut |a, b, budget| {
            let owner = a.0.cmp(&b.0);
            if owner.is_eq() {
                sort::compare_names_charged(
                    &vm.strings[a.1 as usize],
                    &vm.strings[b.1 as usize],
                    budget,
                )
            } else {
                Ok(owner)
            }
        })?;
        Ok(())
    }
}

/// Append to the bounded build buffer; finish sorts keys without repeated shifts.
pub(super) fn push_group<K: Copy + Ord, V>(
    map: &mut Groups<K, V>,
    key: K,
    value: V,
    budget: &mut PreparationBudget,
) -> Result<(), RuntimeError> {
    map.push(key, value, budget)
}
