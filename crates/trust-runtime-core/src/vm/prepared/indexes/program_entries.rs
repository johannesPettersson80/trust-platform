//! The programs a cycle calls, found once (checklist P1-6; specification 34 PERF-05).
//!
//! A task's program list and a resource's background programs name each program by the string
//! index of its name. Preparation finds, for each such index, the global of that name when it is a
//! program root, and the construction root that reserves the program's instance, so a cycle reaches
//! a program and its instance without comparing names or searching the construction roots.

use super::{lookup_work, sort, PreparedIndexes};
use crate::bytecode::{ConstructionRoots, ResourceMeta, StorageLayout, StorageRole};
use crate::error::RuntimeError;
use crate::vm::names::compare_names;
use crate::vm::prepared::PreparationBudget;
use crate::vm::{module::VmModule, PreparedModule};
use alloc::vec::Vec;

/// A program root that a cycle calls: its declaration, and the construction root that reserves its
/// instance, `None` when no construction root does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ProgramEntry {
    pub(crate) declaration: u32,
    pub(crate) root: Option<u32>,
}

crate::memory::figure::holds_no_heap!(ProgramEntry);

impl PreparedIndexes {
    /// Find the program root of every name a task or a resource's background calls; the globals,
    /// the program roots and the background programs are prepared before. A name that names no
    /// program root has no entry, and the cycle refuses it when it calls it, as it did by name.
    pub(super) fn prepare_program_entries(
        &mut self,
        vm: &VmModule,
        layout: &StorageLayout,
        roots: &ConstructionRoots,
        resources: &ResourceMeta,
        budget: &mut PreparationBudget,
    ) -> Result<(), RuntimeError> {
        let scheduled = resources.resources.iter().flat_map(|resource| {
            resource
                .tasks
                .iter()
                .flat_map(|task| task.program_name_idx.iter().copied())
        });
        let background = self.background_programs.iter().flatten().copied();
        let count = scheduled
            .clone()
            .count()
            .checked_add(background.clone().count())
            .ok_or(RuntimeError::Overflow)?;
        budget.records::<(u32, ProgramEntry)>(count)?;
        let mut entries = Vec::new();
        entries
            .try_reserve_exact(count)
            .map_err(|_| RuntimeError::PreparationLimit)?;
        for name_idx in scheduled.chain(background) {
            let Some(name) = vm.strings.get(name_idx as usize) else {
                continue;
            };
            budget.charge(
                0,
                lookup_work(self.globals.len())
                    .checked_mul(self.max_name_bytes.max(name.len()).max(1))
                    .ok_or(RuntimeError::Overflow)?,
            )?;
            let Ok(at) = self
                .globals
                .binary_search_by(|entry| compare_names(&vm.strings[entry.0 as usize], name))
            else {
                continue;
            };
            let declaration = self.globals[at].1;
            let entry = &layout.entries[declaration as usize];
            if entry.role != StorageRole::ProgramRoot {
                continue;
            }
            // The first of the program's roots that reserves this declaration's instance, as the
            // store finds a program root's instance (`CellReader::program_root`).
            let candidates = entry
                .owner_pou_id
                .and_then(|pou| self.program_roots.get(&pou))
                .unwrap_or(&[]);
            budget.charge(0, candidates.len().max(1))?;
            let root = candidates
                .iter()
                .copied()
                .find(|&index| {
                    roots.entries.get(index).map(|root| root.declaration_idx) == Some(declaration)
                })
                .map(u32::try_from)
                .transpose()
                .map_err(|_| RuntimeError::Overflow)?;
            entries.push((name_idx, ProgramEntry { declaration, root }));
        }
        sort::sort_by(&mut entries, budget, &mut |a, b, _| Ok(a.0.cmp(&b.0)))?;
        budget.charge(0, entries.len())?;
        entries.dedup_by_key(|entry| entry.0);
        self.program_entries = entries;
        Ok(())
    }
}

impl PreparedModule {
    /// The program root that a cycle calls by the name at string index `name_idx`.
    pub(crate) fn program_entry(&self, name_idx: u32) -> Option<ProgramEntry> {
        let entries = &self.indexes.program_entries;
        let at = entries
            .binary_search_by_key(&name_idx, |entry| entry.0)
            .ok()?;
        Some(entries[at].1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vm::prepared::PreparationLimits;

    /// The fixture (`program-v2`, `main.st`) declares `PROGRAM Plant WITH Periodic : Main`, so its
    /// one task calls the program root `Plant`, whose instance its construction root reserves.
    /// Three names are added to the task: `pLaNt`, which names the same program because IEC
    /// 61131-3 (§6.1.2) letters in identifiers are not case-sensitive; `Main`, the program's type,
    /// which is no global; and `Absent`, which nothing declares.
    /// Catches: a task that spells its program in another case left without it (the scan would
    /// fail); a type name or an undeclared name taken for a program; a program reached through
    /// another root than the one that reserves its instance.
    #[test]
    fn a_task_reaches_its_program_root_and_the_root_that_reserves_its_instance() {
        let mut prepared = super::super::tests::prepared();
        let plant = prepared.resources.resources[0].tasks[0].program_name_idx[0];
        assert_eq!(&prepared.vm.strings[plant as usize], "Plant");
        let first = prepared.vm.strings.len() as u32;
        for text in ["pLaNt", "Main", "Absent"] {
            prepared.vm.strings.push(text);
        }
        let mut resources = prepared.resources.clone();
        resources.resources[0].tasks[0]
            .program_name_idx
            .extend([first, first + 1, first + 2]);
        let mut indexes = core::mem::take(&mut prepared.indexes);
        indexes
            .prepare_program_entries(
                &prepared.vm,
                &prepared.layout,
                &prepared.roots,
                &resources,
                &mut PreparationBudget::new(PreparationLimits::default()),
            )
            .unwrap();
        prepared.indexes = indexes;

        // Worked out from the fixture's data: the declaration named Plant is a program root, and
        // exactly one construction root holds it.
        let declaration = prepared
            .layout
            .entries
            .iter()
            .position(|entry| &prepared.vm.strings[entry.name_idx as usize] == "Plant")
            .unwrap() as u32;
        assert_eq!(
            prepared.layout.entries[declaration as usize].role,
            StorageRole::ProgramRoot
        );
        let holders: Vec<usize> = prepared
            .roots
            .entries
            .iter()
            .enumerate()
            .filter(|(_, root)| root.declaration_idx == declaration)
            .map(|(index, _)| index)
            .collect();
        assert_eq!(holders.len(), 1);
        let expected = ProgramEntry {
            declaration,
            root: Some(holders[0] as u32),
        };
        assert_eq!(prepared.program_entry(plant), Some(expected));
        assert_eq!(prepared.program_entry(first), Some(expected));
        assert_eq!(prepared.program_entry(first + 1), None);
        assert_eq!(prepared.program_entry(first + 2), None);
    }

    /// The entries are preparation records: a budget without room for them refuses the
    /// preparation before anything is kept.
    /// Catches: an index that grows outside the preparation's byte limit (specification 34
    /// MEM-14).
    #[test]
    fn program_entries_are_refused_by_a_budget_without_room() {
        let prepared = super::super::tests::prepared();
        let mut indexes = PreparedIndexes::default();
        assert_eq!(
            indexes.prepare_program_entries(
                &prepared.vm,
                &prepared.layout,
                &prepared.roots,
                &prepared.resources,
                &mut PreparationBudget::new(PreparationLimits {
                    max_preparation_bytes: 0,
                    ..PreparationLimits::default()
                }),
            ),
            Err(RuntimeError::PreparationLimit)
        );
        assert!(indexes.program_entries.is_empty());
    }
}
