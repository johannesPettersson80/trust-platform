//! Immutable lookup plans built once after admission; no runtime layout scans.
use super::*;
use crate::bytecode::{InitializationPhase, InitializationTrigger, StorageOwner};
use crate::vm::call::context::VmEdgeInput;
use core::cmp::Ordering;

mod actions;
mod declarations;
mod groups;
pub(super) mod sort;
use groups::Groups;
#[cfg(test)]
mod tests;

#[derive(Debug, Default)]
pub(super) struct PreparedIndexes {
    slots: Vec<(u8, Option<u32>, u32, u32)>,
    members: Vec<(u32, u32, u32)>,
    globals: Vec<(u32, u32)>,
    type_members: Vec<(u32, u32, u32)>,
    root_owners: Vec<(u32, usize)>,
    root_candidates: Groups<(u32, u32), usize>,
    program_roots: Groups<u32, usize>,
    edge_declarations: Groups<u32, u32>,
    declarations: Groups<(u8, Option<u32>), u32>,
    static_ordinals: Vec<Option<usize>>,
    retained_declarations: Groups<(u8, Option<u32>), u32>,
    actions: Groups<(Option<u32>, u8, u8), u32>,
    instance_actions: Groups<u32, u32>,
    resource_actions: Vec<u32>,
    module_statics: Vec<u32>,
    configuration_actions: Vec<u32>,
    explicit_actions: Vec<bool>,
    edges: Groups<u32, VmEdgeInput>,
    max_name_bytes: usize,
}

fn compare_names(left: &str, right: &str) -> Ordering {
    left.bytes()
        .map(|byte| byte.to_ascii_uppercase())
        .cmp(right.bytes().map(|byte| byte.to_ascii_uppercase()))
}
fn lookup_work(count: usize) -> usize {
    count.max(1).ilog2() as usize + 1
}

impl PreparedIndexes {
    pub(super) fn build(
        vm: &VmModule,
        layout: &StorageLayout,
        roots: &ConstructionRoots,
        initializers: &InitializerIndex,
        methods: &BTreeMap<u32, u32>,
        budget: &mut PreparationBudget,
    ) -> Result<Self, RuntimeError> {
        let mut result = Self::default();
        result.prepare_declarations(vm, layout, roots, methods, budget)?;
        result.prepare_actions(initializers, methods, budget)?;
        Ok(result)
    }
}

impl PreparedModule {
    pub(crate) fn lookup_work(&self) -> usize {
        lookup_work(
            self.layout
                .entries
                .len()
                .max(self.roots.entries.len())
                .max(self.indexes.type_members.len())
                .max(self.initializers.entries.len()),
        )
    }
    pub(crate) fn name_lookup_work(&self, name: &str) -> usize {
        self.lookup_work()
            .saturating_mul(self.indexes.max_name_bytes.max(name.len()).max(1))
    }
    pub(crate) fn declaration(
        &self,
        owner: StorageOwner,
        pou: Option<u32>,
        slot: usize,
    ) -> Option<(u32, &crate::bytecode::StorageDeclaration)> {
        let key = (
            owner as u8,
            if owner == StorageOwner::Global {
                None
            } else {
                pou
            },
            u32::try_from(slot).ok()?,
        );
        let at = self
            .indexes
            .slots
            .binary_search_by_key(&key, |entry| (entry.0, entry.1, entry.2))
            .ok()?;
        let id = self.indexes.slots[at].3;
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
        Some(self.indexes.type_members[at].2)
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
    pub(crate) fn root_for_instance_owner(
        &self,
        owner: u32,
    ) -> Option<(usize, &crate::bytecode::ConstructionRoot)> {
        let at = self
            .indexes
            .root_owners
            .binary_search_by_key(&owner, |entry| entry.0)
            .ok()?;
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
            .get(&(owner as u8, pou))
            .unwrap_or(&[])
    }
    pub(crate) fn retained_declarations(&self, owner: StorageOwner, pou: Option<u32>) -> &[u32] {
        self.indexes
            .retained_declarations
            .get(&(owner as u8, pou))
            .unwrap_or(&[])
    }
    pub(crate) fn static_ordinal(&self, declaration: u32) -> Option<usize> {
        self.indexes
            .static_ordinals
            .get(declaration as usize)
            .copied()
            .flatten()
    }
    pub(crate) fn actions(
        &self,
        owner: Option<u32>,
        phase: InitializationPhase,
        trigger: InitializationTrigger,
    ) -> &[u32] {
        self.indexes
            .actions
            .get(&(owner, phase as u8, trigger as u8))
            .unwrap_or(&[])
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
