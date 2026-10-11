//! Callable default identity and lexical-context validation.

use super::*;
use crate::bytecode::InitializerBodyKind;

pub(super) fn validate_identity(
    tables: &ValidationContext<'_>,
    index: &InitializerIndex,
    layout: &StorageLayout,
    id: usize,
    entry: &InitializerEntry,
    budget: &mut ValidationBudget,
) -> Result<Option<u32>, BytecodeError> {
    budget.work(1)?;
    let invalid = RejectionReason::InvalidInitializerRecord;
    if entry.recipe_reserved != [0; 3] {
        return Err(invalid.into());
    }
    if entry.body_kind == InitializerBodyKind::Action {
        if entry.recipe_type_id.is_some() || entry.recipe_member_idx.is_some() {
            return Err(invalid.into());
        }
        if let Some(context_id) = entry.context_initializer_idx {
            let context = index.entries.get(context_id as usize).ok_or(invalid)?;
            if context_id as usize >= id
                || context.body_kind != InitializerBodyKind::Action
                || context.context_initializer_idx.is_some()
                || context.recipe_context_key(layout).is_none()
                || context.recipe_context_key(layout) != entry.recipe_context_key(layout)
            {
                return Err(RejectionReason::InitializerVisibility.into());
            }
        }
        return Ok(None);
    }
    let context = entry
        .context_initializer_idx
        .and_then(|id| index.entries.get(id as usize))
        .ok_or(invalid)?;
    if context.body_kind != InitializerBodyKind::Action
        || context.context_initializer_idx.is_some()
        || entry.phase != InitializationPhase::ValueDefault
        || entry.owner_pou_id != context.owner_pou_id
        || entry.trigger != context.trigger
        || entry.declaration_idx.is_some()
        || entry.visible_local_count != 0
        || entry.visible_static_count != 0
        || entry.once != InitializationOnce::None
        || entry.stage != InitializationStage::Default
        || entry.target_kind != InitializationTarget::Declaration
        || entry.target_idx.is_some()
        || entry.partial_kind != 0
        || entry.partial_index != 0
    {
        return Err(invalid.into());
    }
    let type_id = entry.recipe_type_id.ok_or(invalid)?;
    let ty = tables.types.entries.get(type_id as usize).ok_or(invalid)?;
    let result = match entry.body_kind {
        InitializerBodyKind::TypeDefault if entry.recipe_member_idx.is_none() => type_id,
        InitializerBodyKind::MemberDefault => {
            let fields = match &ty.data {
                TypeData::Struct { fields } | TypeData::Union { fields } => fields,
                _ => return Err(invalid.into()),
            };
            fields
                .get(entry.recipe_member_idx.ok_or(invalid)? as usize)
                .ok_or(invalid)?
                .type_id
        }
        _ => return Err(invalid.into()),
    };
    Ok(Some(result))
}

pub(super) fn validate_unique(
    index: &InitializerIndex,
    budget: &mut ValidationBudget,
) -> Result<(), BytecodeError> {
    let mut keys = Vec::new();
    for entry in &index.entries {
        budget.work(1)?;
        if entry.body_kind != InitializerBodyKind::Action {
            budget.push(
                &mut keys,
                (
                    entry.context_initializer_idx,
                    entry.recipe_type_id,
                    entry.recipe_member_idx,
                ),
            )?;
        }
    }
    budget.sort_by(&mut keys, &mut |left, right, _| Ok(left.cmp(right)))?;
    for pair in keys.windows(2) {
        budget.work(1)?;
        if pair[0] == pair[1] {
            return Err(RejectionReason::InvalidInitializerRecord.into());
        }
    }
    Ok(())
}
