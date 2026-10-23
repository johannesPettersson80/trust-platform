//! Background program selection is immutable resource metadata, prepared once.

use super::{compare_names, lookup_work, sort, PreparedIndexes};
use crate::bytecode::{ResourceMeta, StorageLayout, StorageRole};
use crate::error::RuntimeError;
use crate::vm::{module::VmModule, prepared::PreparationBudget};
use alloc::vec::Vec;

impl PreparedIndexes {
    pub(super) fn prepare_background(
        &mut self,
        vm: &VmModule,
        layout: &StorageLayout,
        resources: &ResourceMeta,
        budget: &mut PreparationBudget,
    ) -> Result<(), RuntimeError> {
        let mut roots = Vec::new();
        budget.records::<u32>(layout.entries.len())?;
        roots
            .try_reserve_exact(layout.entries.len())
            .map_err(|_| RuntimeError::PreparationLimit)?;
        for declaration in &layout.entries {
            budget.charge(0, 1)?;
            if declaration.role == StorageRole::ProgramRoot {
                roots.push(declaration.name_idx);
            }
        }
        budget.records::<Vec<u32>>(resources.resources.len())?;
        self.background_programs
            .try_reserve_exact(resources.resources.len())
            .map_err(|_| RuntimeError::PreparationLimit)?;
        for resource in &resources.resources {
            let scheduled_count = resource.tasks.iter().try_fold(0usize, |count, task| {
                count
                    .checked_add(task.program_name_idx.len())
                    .ok_or(RuntimeError::Overflow)
            })?;
            budget.records::<u32>(scheduled_count)?;
            let mut scheduled = Vec::new();
            scheduled
                .try_reserve_exact(scheduled_count)
                .map_err(|_| RuntimeError::PreparationLimit)?;
            for task in &resource.tasks {
                budget.charge(0, task.program_name_idx.len())?;
                scheduled.extend_from_slice(&task.program_name_idx);
            }
            sort::sort_by(&mut scheduled, budget, &mut |left, right, budget| {
                sort::compare_names_charged(
                    &vm.strings[*left as usize],
                    &vm.strings[*right as usize],
                    budget,
                )
            })?;
            budget.records::<u32>(roots.len())?;
            let mut background = Vec::new();
            background
                .try_reserve_exact(roots.len())
                .map_err(|_| RuntimeError::PreparationLimit)?;
            for &root in &roots {
                let name = &vm.strings[root as usize];
                budget.charge(
                    0,
                    lookup_work(scheduled.len())
                        .checked_mul(self.max_name_bytes.max(name.len()).max(1))
                        .ok_or(RuntimeError::Overflow)?,
                )?;
                if scheduled
                    .binary_search_by(|index| compare_names(&vm.strings[*index as usize], name))
                    .is_err()
                {
                    background.push(root);
                }
            }
            // Keep declaration order: the scheduled-name sort is lookup-only.
            self.background_programs.push(background);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vm::{PreparationLimits, PreparedModule};
    use alloc::vec;
    use smol_str::SmolStr;

    #[test]
    fn prepared_background_order_ignores_unrelated_declarations_and_scheduled_name_case() {
        let mut prepared = PreparedModule::from_bytes(
            include_bytes!(
                "../../../../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc"
            ),
            PreparationLimits::default(),
        )
        .unwrap();
        let template = prepared
            .layout
            .entries
            .iter()
            .find(|entry| entry.role == StorageRole::ProgramRoot)
            .unwrap()
            .clone();
        let base = prepared.vm.strings.len() as u32;
        prepared
            .vm
            .strings
            .extend(["First", "Scheduled", "Last", "sChEdUlEd"].map(SmolStr::from));
        let mut layout = prepared.layout.clone();
        layout.entries.clear();
        for offset in 0..3 {
            let mut root = template.clone();
            root.name_idx = base + offset;
            layout.entries.push(root);
            let mut unrelated = template.clone();
            unrelated.role = StorageRole::Variable;
            layout.entries.extend(core::iter::repeat_n(unrelated, 4096));
        }
        let mut resources = prepared.resources.clone();
        let task = resources.resources[0].tasks[0].clone();
        resources.resources[0].tasks = vec![crate::bytecode::TaskEntry {
            program_name_idx: vec![base + 3],
            ..task
        }];
        let mut indexes = PreparedIndexes {
            max_name_bytes: 9,
            ..Default::default()
        };
        indexes
            .prepare_background(
                &prepared.vm,
                &layout,
                &resources,
                &mut PreparationBudget::new(PreparationLimits::default()),
            )
            .unwrap();
        assert_eq!(indexes.background_programs, vec![vec![base, base + 2]]);

        // A new resource candidate computes its own list; it cannot retain the
        // previous installation's scheduled/background classification.
        resources.resources[0].tasks.clear();
        let mut replacement = PreparedIndexes {
            max_name_bytes: 9,
            ..Default::default()
        };
        replacement
            .prepare_background(
                &prepared.vm,
                &layout,
                &resources,
                &mut PreparationBudget::new(PreparationLimits::default()),
            )
            .unwrap();
        assert_eq!(
            replacement.background_programs,
            vec![vec![base, base + 1, base + 2]]
        );
    }
}
