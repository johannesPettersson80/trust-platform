//! Compact owner-to-range plans for admitted storage declarations.
use super::*;
use crate::bytecode::StorageOwner;
use alloc::vec::Vec;
use core::ops::Range;

#[derive(Debug, Default)]
pub(super) struct DeclarationPlanIndex {
    global: DeclarationRange,
    owners: Vec<PouDeclarationPlan>,
    owner_ordinals: Vec<u32>,
}

#[derive(Debug, Default)]
struct PouDeclarationPlan {
    pou_id: u32,
    frame: DeclarationRange,
    instance: DeclarationRange,
}

#[derive(Debug, Default)]
struct DeclarationRange {
    start: u32,
    end: u32,
}

impl DeclarationRange {
    fn from_range(range: Range<usize>) -> Result<Self, RuntimeError> {
        Ok(Self {
            start: u32::try_from(range.start).map_err(|_| RuntimeError::Overflow)?,
            end: u32::try_from(range.end).map_err(|_| RuntimeError::Overflow)?,
        })
    }

    fn as_range(&self) -> Range<usize> {
        self.start as usize..self.end as usize
    }

    fn is_empty(&self) -> bool {
        self.start == self.end
    }
}

impl DeclarationPlanIndex {
    // Count-bounded buckets preserve every previous in-range hit: ascending
    // owners below the table length claim their unique bucket before larger IDs.
    fn owner_bucket(&self, pou: u32) -> Option<usize> {
        let identity = usize::try_from(pou).ok()?;
        let count = self.owner_ordinals.len();
        (count != 0).then(|| identity % count)
    }

    #[cfg(test)]
    pub(super) fn owner_capacity(&self) -> usize {
        self.owners.capacity()
    }

    #[cfg(test)]
    pub(super) fn owner_ordinal_for_test(&self, pou: u32) -> Option<usize> {
        self.owner_bucket(pou)
            .and_then(|bucket| self.owner_ordinals.get(bucket))
            .copied()
            .filter(|ordinal| *ordinal != 0)
            .and_then(|ordinal| usize::try_from(ordinal - 1).ok())
            .filter(|index| {
                self.owners
                    .get(*index)
                    .is_some_and(|plan| plan.pou_id == pou)
            })
    }

    #[cfg(test)]
    pub(super) fn owner_index_len_for_test(&self) -> usize {
        self.owner_ordinals.len()
    }

    #[cfg(test)]
    pub(super) fn owners_len_for_test(&self) -> usize {
        self.owners.len()
    }

    #[cfg(test)]
    pub(super) fn set_owner_ordinal_for_test(&mut self, pou: u32, ordinal: u32) {
        if let Some(bucket) = self.owner_bucket(pou)
            && let Some(slot) = self.owner_ordinals.get_mut(bucket)
        {
            *slot = ordinal;
        }
    }

    pub(super) fn range(&self, owner: StorageOwner, pou: Option<u32>) -> Option<Range<usize>> {
        if owner == StorageOwner::Global {
            return (!self.global.is_empty()).then(|| self.global.as_range());
        }
        let pou = pou?;
        #[cfg(all(feature = "std", feature = "test-support"))]
        crate::vm::diagnostic_timing::record_index_owner_query(pou);
        let direct = self
            .owner_bucket(pou)
            .and_then(|bucket| self.owner_ordinals.get(bucket))
            .copied()
            .filter(|ordinal| *ordinal != 0)
            .and_then(|ordinal| usize::try_from(ordinal - 1).ok())
            .and_then(|index| self.owners.get(index))
            .filter(|plan| plan.pou_id == pou);
        #[cfg(all(feature = "std", feature = "test-support"))]
        if direct.is_some() {
            crate::vm::diagnostic_timing::record_index_owner_geometry(false, self.owners.len(), 0);
        }
        let plan = if let Some(plan) = direct {
            Some(plan)
        } else {
            #[cfg(all(feature = "std", feature = "test-support"))]
            let mut comparisons = 0;
            let position = self.owners.binary_search_by_key(&pou, |plan| {
                #[cfg(all(feature = "std", feature = "test-support"))]
                {
                    comparisons += 1;
                }
                plan.pou_id
            });
            #[cfg(all(feature = "std", feature = "test-support"))]
            crate::vm::diagnostic_timing::record_index_owner_geometry(
                true,
                self.owners.len(),
                comparisons,
            );
            position.ok().map(|index| &self.owners[index])
        };
        let plan = plan?;
        let range = match owner {
            StorageOwner::Frame => &plan.frame,
            StorageOwner::Instance => &plan.instance,
            StorageOwner::Global => &self.global,
        };
        (!range.is_empty()).then(|| range.as_range())
    }
}

impl PreparedIndexes {
    pub(super) fn prepare_declaration_plan(
        &mut self,
        pou_count: usize,
        layout: &StorageLayout,
        methods: &BTreeMap<u32, u32>,
        budget: &mut PreparationBudget,
    ) -> Result<(), RuntimeError> {
        let mut owner_count = 0usize;
        for declaration in &layout.entries {
            budget.charge(0, 1)?;
            if declaration.owns_storage()
                && matches!(
                    declaration.owner,
                    StorageOwner::Frame | StorageOwner::Instance
                )
            {
                owner_count = owner_count.checked_add(1).ok_or(RuntimeError::Overflow)?;
            }
        }

        budget.records::<u32>(owner_count)?;
        let mut owners = Vec::new();
        owners
            .try_reserve_exact(owner_count)
            .map_err(|_| RuntimeError::Overflow)?;
        for declaration in &layout.entries {
            budget.charge(0, 1)?;
            if !declaration.owns_storage()
                || !matches!(
                    declaration.owner,
                    StorageOwner::Frame | StorageOwner::Instance
                )
            {
                continue;
            }
            let mut owner = declaration
                .owner_pou_id
                .ok_or(RuntimeError::InvalidExecutionState)?;
            if declaration.owner == StorageOwner::Instance {
                budget.charge(0, lookup_work(methods.len()))?;
                owner = methods.get(&owner).copied().unwrap_or(owner);
            }
            owners.push(owner);
        }
        sort::sort_by(&mut owners, budget, &mut |left, right, _| {
            Ok(left.cmp(right))
        })?;
        budget.charge(0, owners.len())?;
        owners.dedup();

        budget.records::<PouDeclarationPlan>(owners.len())?;
        self.declaration_plan
            .owners
            .try_reserve_exact(owners.len())
            .map_err(|_| RuntimeError::Overflow)?;
        for pou_id in owners {
            let frame = declaration_range(
                &self.declarations,
                StorageOwner::Frame,
                Some(pou_id),
                budget,
            )?;
            let instance = declaration_range(
                &self.declarations,
                StorageOwner::Instance,
                Some(pou_id),
                budget,
            )?;
            self.declaration_plan.owners.push(PouDeclarationPlan {
                pou_id,
                frame,
                instance,
            });
        }
        budget.charge(0, self.declaration_plan.owners.len())?;
        budget.records::<u32>(pou_count)?;
        self.declaration_plan
            .owner_ordinals
            .try_reserve_exact(pou_count)
            .map_err(|_| RuntimeError::Overflow)?;
        self.declaration_plan.owner_ordinals.resize(pou_count, 0);
        for (index, plan) in self.declaration_plan.owners.iter().enumerate() {
            let Some(bucket) = self.declaration_plan.owner_bucket(plan.pou_id) else {
                continue;
            };
            let ordinal = &mut self.declaration_plan.owner_ordinals[bucket];
            // Sorted owners make the first occupant deterministic. A collision
            // retains the prior owner and the existing search fallback.
            if *ordinal == 0 {
                *ordinal = u32::try_from(index.checked_add(1).ok_or(RuntimeError::Overflow)?)
                    .map_err(|_| RuntimeError::Overflow)?;
            }
        }
        self.declaration_plan.global =
            declaration_range(&self.declarations, StorageOwner::Global, None, budget)?;

        Ok(())
    }
}

fn declaration_range(
    declarations: &Groups<DeclarationKey, u32>,
    owner: StorageOwner,
    pou: Option<u32>,
    budget: &mut PreparationBudget,
) -> Result<DeclarationRange, RuntimeError> {
    budget.charge(0, lookup_work(declarations.group_count()))?;
    declarations
        .range(&DeclarationKey::new(owner as u8, pou))
        .map(DeclarationRange::from_range)
        .transpose()
        .map(|range| range.unwrap_or_default())
}
