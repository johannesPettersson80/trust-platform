//! Validate per-instance edge-input state without inferring ownership from names.

use super::*;

pub(super) fn validate_edges(
    tables: &ValidationContext<'_>,
    layout: &StorageLayout,
    budget: &mut ValidationBudget,
) -> Result<(), BytecodeError> {
    let mut inputs = Vec::new();
    for declaration in &layout.entries {
        budget.work(1)?;
        if declaration.role != StorageRole::EdgePhase {
            if declaration.flags & 0x30 != 0 || declaration.related_declaration_idx.is_some() {
                return Err(RejectionReason::InvalidConstructionRecord.into());
            }
            continue;
        }
        if declaration.owner != StorageOwner::Instance || !matches!(declaration.flags, 0x10 | 0x20)
        {
            return Err(RejectionReason::InvalidConstructionRecord.into());
        }
        let owner = declaration
            .owner_pou_id
            .map(|id| tables.pou(id, budget))
            .transpose()?
            .flatten()
            .ok_or(RejectionReason::InvalidConstructionRecord)?;
        if !matches!(owner.kind, PouKind::Program | PouKind::FunctionBlock) {
            return Err(RejectionReason::InvalidConstructionRecord.into());
        }
        let input_id = declaration
            .related_declaration_idx
            .ok_or(RejectionReason::InvalidConstructionRecord)?;
        let input = layout
            .entries
            .get(input_id as usize)
            .ok_or(RejectionReason::InvalidConstructionRecord)?;
        if input.owner != declaration.owner
            || input.owner_pou_id != declaration.owner_pou_id
            || input.flags & 0x0E != 2
            || !matches!(input.role, StorageRole::Variable | StorageRole::Parameter)
            || input.retain != declaration.retain
        {
            return Err(RejectionReason::InvalidConstructionRecord.into());
        }
        bool_type(tables, input.type_id, budget)?;
        bool_type(tables, declaration.type_id, budget)?;
        let seed = declaration
            .default_const_idx
            .and_then(|id| tables.const_pool.entries.get(id as usize))
            .ok_or(RejectionReason::InvalidConstructionRecord)?;
        let expected = u8::from(declaration.flags == 0x20);
        if Some(seed.type_id) != declaration.type_id || seed.payload.as_slice() != [expected] {
            return Err(RejectionReason::InvalidConstructionRecord.into());
        }
        budget.push(&mut inputs, input_id)?;
    }
    budget.sort_by(&mut inputs, &mut |a, b, _| Ok(a.cmp(b)))?;
    for pair in inputs.windows(2) {
        budget.work(1)?;
        if pair[0] == pair[1] {
            return Err(RejectionReason::InvalidConstructionRecord.into());
        }
    }
    Ok(())
}

fn bool_type(
    tables: &ValidationContext<'_>,
    id: Option<u32>,
    budget: &mut ValidationBudget,
) -> Result<(), BytecodeError> {
    let mut id = id.ok_or(RejectionReason::InvalidConstructionRecord)?;
    for _ in 0..64 {
        budget.work(1)?;
        match tables
            .types
            .entries
            .get(id as usize)
            .map(|entry| &entry.data)
        {
            Some(TypeData::Primitive { prim_id: 1, .. }) => return Ok(()),
            Some(TypeData::Alias { target_type_id }) => id = *target_type_id,
            _ => return Err(RejectionReason::InvalidConstructionRecord.into()),
        }
    }
    Err(RejectionReason::InvalidConstructionRecord.into())
}
