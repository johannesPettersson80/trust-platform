//! Immutable lookup plans built once after admission; no runtime layout scans.
use super::*;
use crate::bytecode::{InitializationPhase, InitializationTrigger, StorageOwner};
use crate::vm::call::context::VmEdgeInput;
use core::cmp::Ordering;

mod actions;
mod background;
mod declaration_plan;
mod declarations;
mod frame_plan;
mod groups;
mod keys;
mod references;
pub(super) mod sort;
use declaration_plan::DeclarationPlanIndex;
use frame_plan::FrameActionPlan;
use groups::Groups;
use keys::{ActionKey, DeclarationKey};
#[cfg(test)]
mod charged_lookup_tests;
#[cfg(all(test, feature = "std", feature = "test-support"))]
mod lookup_geometry_tests;
#[cfg(test)]
mod name_work_tests;
#[cfg(test)]
mod ordinal_tests;
#[cfg(test)]
mod tests;

#[derive(Debug, Default)]
pub(super) struct PreparedIndexes {
    members: Vec<(u32, u32, u32)>,
    globals: Vec<(u32, u32)>,
    type_members: Vec<(u32, u32, u32, u32)>,
    named_types: Vec<(u32, u32)>,
    root_owners: Vec<(u32, usize)>,
    root_owner_ordinals: Vec<u32>,
    root_candidates: Groups<(u32, u32), usize>,
    program_roots: Groups<u32, usize>,
    edge_declarations: Groups<u32, u32>,
    declarations: Groups<DeclarationKey, u32>,
    declaration_plan: DeclarationPlanIndex,
    static_ordinals: Vec<Option<usize>>,
    retained_declarations: Groups<DeclarationKey, u32>,
    actions: Groups<ActionKey, u32>,
    frame_action_plans: Vec<(u32, FrameActionPlan)>,
    instance_actions: Groups<u32, u32>,
    resource_actions: Vec<u32>,
    module_statics: Vec<u32>,
    configuration_actions: Vec<u32>,
    explicit_actions: Vec<bool>,
    edges: Groups<u32, VmEdgeInput>,
    max_name_bytes: usize,
    reference_candidates: Groups<(u8, usize), u32>,
    background_programs: Vec<Vec<u32>>,
}

fn compare_names(left: &str, right: &str) -> Ordering {
    left.bytes()
        .map(|byte| byte.to_ascii_uppercase())
        .cmp(right.bytes().map(|byte| byte.to_ascii_uppercase()))
}

// Preparation budgets charge this bound for each build-time sorted search.
// slice::binary_search_by rounds the remaining size upward, then performs one
// final comparison: ceil(log2(count)) + 1 for a nonempty table.
fn lookup_work(count: usize) -> usize {
    if count <= 1 {
        1
    } else {
        (count - 1).ilog2() as usize + 2
    }
}

impl PreparedIndexes {
    pub(super) fn build(
        vm: &VmModule,
        layout: &StorageLayout,
        roots: &ConstructionRoots,
        initializers: &InitializerIndex,
        methods: &BTreeMap<u32, u32>,
        resources: &ResourceMeta,
        budget: &mut PreparationBudget,
    ) -> Result<Self, RuntimeError> {
        let mut result = Self::default();
        result.prepare_named_types(vm, budget)?;
        result.prepare_declarations(vm, layout, roots, methods, budget)?;
        result.prepare_declaration_plan(vm.pou_by_id.len(), layout, methods, budget)?;
        result.prepare_actions(initializers, methods, budget)?;
        result.prepare_frame_action_plans(budget)?;
        result.prepare_references(vm, budget)?;
        result.prepare_background(vm, layout, resources, budget)?;
        Ok(result)
    }

    fn prepare_named_types(
        &mut self,
        vm: &VmModule,
        budget: &mut PreparationBudget,
    ) -> Result<(), RuntimeError> {
        budget.records::<(u32, u32)>(vm.types.entries.len())?;
        self.named_types
            .try_reserve_exact(vm.types.entries.len())
            .map_err(|_| RuntimeError::PreparationLimit)?;
        for (id, entry) in vm.types.entries.iter().enumerate() {
            budget.charge(0, 1)?;
            if let Some(name) = entry.name_idx {
                self.max_name_bytes = self.max_name_bytes.max(vm.strings[name as usize].len());
                self.named_types
                    .push((name, u32::try_from(id).map_err(|_| RuntimeError::Overflow)?));
            }
        }
        sort::sort_by(&mut self.named_types, budget, &mut |a, b, budget| {
            Ok(sort::compare_names_charged(
                &vm.strings[a.0 as usize],
                &vm.strings[b.0 as usize],
                budget,
            )?
            .then(a.1.cmp(&b.1)))
        })?;
        // Preserve the old first-declared lookup if a valid table repeats a name.
        budget.charge(
            0,
            self.named_types
                .len()
                .checked_mul(self.max_name_bytes.max(1))
                .ok_or(RuntimeError::Overflow)?,
        )?;
        self.named_types.dedup_by(|later, earlier| {
            compare_names(
                &vm.strings[later.0 as usize],
                &vm.strings[earlier.0 as usize],
            )
            .is_eq()
        });
        Ok(())
    }
}

impl PreparedModule {
    pub(crate) fn named_type(&self, name: &str) -> Option<u32> {
        let at = self
            .indexes
            .named_types
            .binary_search_by(|entry| compare_names(&self.vm.strings[entry.0 as usize], name))
            .ok()?;
        Some(self.indexes.named_types[at].1)
    }
    pub(crate) fn background_programs(&self, resource: usize) -> &[u32] {
        self.indexes
            .background_programs
            .get(resource)
            .map_or(&[], Vec::as_slice)
    }
    // Dense globals and owner-slot tables are indexed directly; only the sparse
    // owner or slot fallbacks search. Lookups consume no execution work (TIME-08).
    pub(crate) fn declaration(
        &self,
        owner: StorageOwner,
        pou: Option<u32>,
        slot: usize,
    ) -> Option<(u32, &crate::bytecode::StorageDeclaration)> {
        let wire_slot = u32::try_from(slot).ok()?;
        // Source producers place dense globals first. Admission guarantees a
        // unique owning global per slot; reordered records and external aliases
        // continue through the general index below.
        if owner == StorageOwner::Global
            && let Some(declaration) = self.layout.entries.get(slot)
            && declaration.owner == StorageOwner::Global
            && declaration.owns_storage()
            && declaration.slot == wire_slot
        {
            #[cfg(all(feature = "std", feature = "test-support"))]
            crate::vm::diagnostic_timing::record_index_target_geometry(
                crate::vm::diagnostic_timing::IndexLookupPath::DenseGlobal,
                self.layout.entries.len(),
                0,
            );
            return Some((wire_slot, declaration));
        }
        #[cfg(all(feature = "std", feature = "test-support"))]
        crate::vm::diagnostic_timing::record_index_target_geometry(
            crate::vm::diagnostic_timing::IndexLookupPath::MissingOwner,
            0,
            0,
        );
        let range = self.indexes.declaration_plan.range(owner, pou)?;
        let declarations = self.indexes.declarations.slice(&range);
        if let Some(&id) = declarations.get(slot)
            && let Some(declaration) = self.layout.entries.get(id as usize)
            && declaration.slot == wire_slot
        {
            #[cfg(all(feature = "std", feature = "test-support"))]
            crate::vm::diagnostic_timing::record_index_target_geometry(
                crate::vm::diagnostic_timing::IndexLookupPath::DenseOwnerSlot,
                declarations.len(),
                0,
            );
            return Some((id, declaration));
        }
        #[cfg(all(feature = "std", feature = "test-support"))]
        let mut comparisons = 0;
        let position = declarations.binary_search_by_key(&wire_slot, |id| {
            #[cfg(all(feature = "std", feature = "test-support"))]
            {
                comparisons += 1;
            }
            self.layout.entries[*id as usize].slot
        });
        #[cfg(all(feature = "std", feature = "test-support"))]
        crate::vm::diagnostic_timing::record_index_target_geometry(
            crate::vm::diagnostic_timing::IndexLookupPath::SparseOwnerSlot,
            declarations.len(),
            comparisons,
        );
        let at = position.ok()?;
        let id = declarations[at];
        Some((id, &self.layout.entries[id as usize]))
    }
    pub(crate) fn member(
        &self,
        pou: u32,
        name: &str,
    ) -> Option<&crate::bytecode::StorageDeclaration> {
        let at = self
            .indexes
            .members
            .binary_search_by(|entry| {
                entry
                    .0
                    .cmp(&pou)
                    .then_with(|| compare_names(&self.vm.strings[entry.1 as usize], name))
            })
            .ok()?;
        Some(&self.layout.entries[self.indexes.members[at].2 as usize])
    }
    pub(crate) fn type_member(&self, ty: u32, name: &str) -> Option<u32> {
        self.type_member_position(ty, name)
            .map(|(field_type, _)| field_type)
    }
    pub(crate) fn type_member_position(&self, ty: u32, name: &str) -> Option<(u32, usize)> {
        let at = self
            .indexes
            .type_members
            .binary_search_by(|entry| {
                entry
                    .0
                    .cmp(&ty)
                    .then_with(|| compare_names(&self.vm.strings[entry.1 as usize], name))
            })
            .ok()?;
        let (_, _, field_type, ordinal) = self.indexes.type_members[at];
        Some((field_type, ordinal as usize))
    }
    pub(crate) fn global(&self, name: &str) -> Option<(u32, &crate::bytecode::StorageDeclaration)> {
        let at = self
            .indexes
            .globals
            .binary_search_by(|entry| compare_names(&self.vm.strings[entry.0 as usize], name))
            .ok()?;
        let id = self.indexes.globals[at].1;
        Some((id, &self.layout.entries[id as usize]))
    }
    // The root-owner index alone bounds this search, including absent owners.
    pub(crate) fn root_for_instance_owner(
        &self,
        owner: u32,
    ) -> Option<(usize, &crate::bytecode::ConstructionRoot)> {
        let direct = usize::try_from(owner)
            .ok()
            .and_then(|index| self.indexes.root_owner_ordinals.get(index))
            .copied()
            .filter(|ordinal| *ordinal != 0)
            .and_then(|ordinal| usize::try_from(ordinal - 1).ok())
            .and_then(|index| {
                self.indexes.root_owners.get(index).filter(|entry| {
                    entry.0 == owner
                        && (index == 0 || self.indexes.root_owners[index - 1].0 != owner)
                        && self
                            .indexes
                            .root_owners
                            .get(index + 1)
                            .is_none_or(|next| next.0 != owner)
                        && self
                            .roots
                            .entries
                            .get(entry.1)
                            .is_some_and(|root| root.instance_owner_id == Some(owner))
                })
            });
        if let Some(entry) = direct {
            #[cfg(all(feature = "std", feature = "test-support"))]
            crate::vm::diagnostic_timing::record_index_target_geometry(
                crate::vm::diagnostic_timing::IndexLookupPath::RootOwner,
                self.indexes.root_owners.len(),
                0,
            );
            let id = entry.1;
            return self.roots.entries.get(id).map(|root| (id, root));
        }
        #[cfg(all(feature = "std", feature = "test-support"))]
        let mut comparisons = 0;
        let position = self
            .indexes
            .root_owners
            .binary_search_by_key(&owner, |entry| {
                #[cfg(all(feature = "std", feature = "test-support"))]
                {
                    comparisons += 1;
                }
                entry.0
            });
        #[cfg(all(feature = "std", feature = "test-support"))]
        crate::vm::diagnostic_timing::record_index_target_geometry(
            crate::vm::diagnostic_timing::IndexLookupPath::RootOwner,
            self.indexes.root_owners.len(),
            comparisons,
        );
        let at = position.ok()?;
        let id = self.indexes.root_owners[at].1;
        Some((id, &self.roots.entries[id]))
    }
    pub(crate) fn root_candidates(&self, declaration: u32, pou: u32) -> &[usize] {
        self.indexes
            .root_candidates
            .get(&(declaration, pou))
            .unwrap_or(&[])
    }
    pub(crate) fn program_roots(&self, pou: u32) -> &[usize] {
        self.indexes.program_roots.get(&pou).unwrap_or(&[])
    }
    pub(crate) fn edge_declarations(&self, input: u32) -> &[u32] {
        self.indexes.edge_declarations.get(&input).unwrap_or(&[])
    }
    pub(crate) fn declarations(&self, owner: StorageOwner, pou: Option<u32>) -> &[u32] {
        self.indexes
            .declarations
            .get(&DeclarationKey::new(owner as u8, pou))
            .unwrap_or(&[])
    }
    pub(crate) fn retained_declarations(&self, owner: StorageOwner, pou: Option<u32>) -> &[u32] {
        self.indexes
            .retained_declarations
            .get(&DeclarationKey::new(owner as u8, pou))
            .unwrap_or(&[])
    }
    pub(crate) fn static_ordinal(&self, declaration: u32) -> Option<usize> {
        self.indexes
            .static_ordinals
            .get(declaration as usize)
            .copied()
            .flatten()
    }
    #[cfg(test)]
    pub(crate) fn actions(
        &self,
        owner: Option<u32>,
        phase: InitializationPhase,
        trigger: InitializationTrigger,
    ) -> &[u32] {
        self.indexes
            .actions
            .get(&ActionKey::new(owner, phase as u8, trigger as u8))
            .unwrap_or(&[])
    }
    pub(crate) fn frame_action_plan_index(&self, owner: u32) -> Option<usize> {
        self.indexes
            .frame_action_plans
            .binary_search_by_key(&owner, |entry| entry.0)
            .ok()
    }
    pub(crate) fn frame_actions_at(
        &self,
        plan_index: usize,
        phase: InitializationPhase,
        trigger: InitializationTrigger,
    ) -> &[u32] {
        let Some((_, plan)) = self.indexes.frame_action_plans.get(plan_index) else {
            return &[];
        };
        let range = plan.range(phase, trigger);
        self.indexes.actions.slice(&range)
    }
    pub(crate) fn instance_actions(&self, pou: u32) -> &[u32] {
        self.indexes.instance_actions.get(&pou).unwrap_or(&[])
    }
    pub(crate) fn resource_actions(&self) -> &[u32] {
        &self.indexes.resource_actions
    }
    pub(crate) fn module_static_actions(&self) -> &[u32] {
        &self.indexes.module_statics
    }
    pub(crate) fn configuration_actions(&self) -> &[u32] {
        &self.indexes.configuration_actions
    }
    pub(crate) fn has_explicit_action(&self, id: u32) -> bool {
        self.indexes
            .explicit_actions
            .get(id as usize)
            .copied()
            .unwrap_or(false)
    }
    pub(crate) fn edge_inputs(&self, pou: u32) -> &[VmEdgeInput] {
        self.indexes.edges.get(&pou).unwrap_or(&[])
    }
}
