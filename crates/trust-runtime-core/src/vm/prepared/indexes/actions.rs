use super::declarations::push_group;
use super::*;
use crate::bytecode::{InitializationOnce, InitializationStage};

impl PreparedIndexes {
    pub(super) fn prepare_actions(
        &mut self,
        initializers: &InitializerIndex,
        methods: &BTreeMap<u32, u32>,
        budget: &mut PreparationBudget,
    ) -> Result<(), RuntimeError> {
        let count = initializers.entries.len();
        budget.records::<bool>(count)?;
        self.explicit_actions
            .try_reserve_exact(count)
            .map_err(|_| RuntimeError::Overflow)?;
        self.explicit_actions.resize(count, false);
        let mut explicit = Vec::new();
        budget.records::<(u32, u8, u8)>(count)?;
        explicit
            .try_reserve_exact(count)
            .map_err(|_| RuntimeError::Overflow)?;
        for entry in &initializers.entries {
            budget.charge(0, 1)?;
            if entry.is_action() && entry.stage == InitializationStage::Explicit {
                if let Some(declaration) = entry.declaration_idx {
                    explicit.push((declaration, entry.phase as u8, entry.trigger as u8));
                }
            }
        }
        sort::sort_by(&mut explicit, budget, &mut |a, b, _| Ok(a.cmp(b)))?;
        for (id, entry) in initializers.entries.iter().enumerate() {
            budget.charge(0, lookup_work(explicit.len()))?;
            if !entry.is_action() {
                continue;
            }
            self.explicit_actions[id] = entry.declaration_idx.is_some_and(|declaration| {
                explicit
                    .binary_search(&(declaration, entry.phase as u8, entry.trigger as u8))
                    .is_ok()
            });
            let id = u32::try_from(id).map_err(|_| RuntimeError::Overflow)?;
            push_group(
                &mut self.actions,
                (entry.owner_pou_id, entry.phase as u8, entry.trigger as u8),
                id,
                budget,
            )?;
            if entry.phase == InitializationPhase::Configuration {
                push_id(&mut self.configuration_actions, id, budget)?;
            }
            if entry.trigger != InitializationTrigger::Ordinary {
                continue;
            }
            if entry.phase == InitializationPhase::Resource {
                push_id(&mut self.resource_actions, id, budget)?;
            }
            if entry.phase == InitializationPhase::Static
                && entry.once == InitializationOnce::Module
            {
                push_id(&mut self.module_statics, id, budget)?;
            }
            if matches!(
                entry.phase,
                InitializationPhase::Instance | InitializationPhase::Static
            ) {
                if let Some(owner) = entry.owner_pou_id {
                    let physical = methods.get(&owner).copied().unwrap_or(owner);
                    push_group(&mut self.instance_actions, physical, id, budget)?;
                }
            }
        }
        self.actions.finish(budget)?;
        self.instance_actions.finish(budget)?;
        Ok(())
    }
}
fn push_id(
    ids: &mut Vec<u32>,
    id: u32,
    budget: &mut PreparationBudget,
) -> Result<(), RuntimeError> {
    if ids.len() == ids.capacity() {
        let extra = ids.capacity().max(1);
        budget.records::<u32>(extra)?;
        ids.try_reserve_exact(extra)
            .map_err(|_| RuntimeError::Overflow)?;
    }
    ids.push(id);
    Ok(())
}
