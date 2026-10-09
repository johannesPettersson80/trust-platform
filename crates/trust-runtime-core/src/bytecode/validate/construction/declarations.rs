//! Declaration roles, dense storage and reference/type coverage.

use super::*;
use crate::bytecode::RefEntry;

pub(super) fn validate_declarations(
    tables: &ValidationContext<'_>,
    layout: &StorageLayout,
    budget: &mut ValidationBudget,
) -> Result<(), BytecodeError> {
    let mut slots = Vec::new();
    let mut names = Vec::new();
    let mut static_names = Vec::new();
    let mut declared_refs = Vec::new();
    let mut scratch = Vec::new();
    for (position, decl) in layout.entries.iter().enumerate() {
        budget.work(1)?;
        ensure_string_index(tables.strings, decl.name_idx)?;
        match (decl.role, decl.source_name_idx) {
            (StorageRole::Static, Some(name)) => {
                ensure_string_index(tables.strings, name)?;
                budget.push(
                    &mut static_names,
                    (
                        decl.owner_pou_id,
                        tables.strings.entries[name as usize].as_str(),
                    ),
                )?;
            }
            (StorageRole::Static, None) | (_, Some(_)) => {
                return Err(RejectionReason::InvalidConstructionRecord.into())
            }
            _ => {}
        }
        let owner = validate_role(tables, decl, budget)?;
        if decl.role != StorageRole::External {
            let storage_owner = if decl.owner == StorageOwner::Global {
                None
            } else if decl.owner == StorageOwner::Instance
                && owner.is_some_and(|pou| pou.kind == PouKind::Method)
            {
                owner.and_then(|pou| pou.owner_pou_id)
            } else {
                decl.owner_pou_id
            };
            budget.push(
                &mut slots,
                (decl.owner as u8, storage_owner, decl.slot, position),
            )?;
            budget.push(
                &mut names,
                (
                    decl.owner as u8,
                    storage_owner,
                    tables.strings.entries[decl.name_idx as usize].as_str(),
                    position,
                ),
            )?;
        }
        if let Some(reference) = validate_binding(tables, decl, owner, budget)? {
            budget.push(
                &mut declared_refs,
                (
                    reference.location as u8,
                    reference.owner_id,
                    reference.offset,
                ),
            )?;
            if decl.role == StorageRole::Scratch {
                let end = owner
                    .ok_or(RejectionReason::InvalidConstructionRecord)?
                    .local_ref_count;
                budget.push(&mut scratch, (reference.owner_id, decl.slot, end))?;
            }
        }
    }
    validate_keys(layout, &mut slots, &mut names, &mut static_names, budget)?;
    validate_coverage(tables, layout, &mut declared_refs, &mut scratch, budget)
}

fn scratch_contains(
    ranges: &[(u32, u32, u32)],
    reference: &crate::bytecode::RefEntry,
    budget: &mut ValidationBudget,
) -> Result<bool, BytecodeError> {
    let at = budget.lower_bound(ranges, |range, _| {
        Ok((range.0, range.1) <= (reference.owner_id, reference.offset))
    })?;
    Ok(at
        .checked_sub(1)
        .and_then(|at| ranges.get(at))
        .is_some_and(|range| range.0 == reference.owner_id && reference.offset < range.2))
}

fn validate_role<'a>(
    tables: &ValidationContext<'a>,
    decl: &StorageDeclaration,
    budget: &mut ValidationBudget,
) -> Result<Option<&'a PouEntry>, BytecodeError> {
    if decl.flags & !0x3F != 0 || decl.retain > 3 {
        return Err(RejectionReason::InvalidConstructionRecord.into());
    }
    let owner = decl
        .owner_pou_id
        .map(|id| tables.pou(id, budget))
        .transpose()?
        .flatten();
    if decl.owner_pou_id.is_some() && owner.is_none() {
        return Err(RejectionReason::InvalidConstructionRecord.into());
    }
    if decl.owner != StorageOwner::Global && owner.is_none() {
        return Err(RejectionReason::InvalidConstructionRecord.into());
    }
    if decl.role == StorageRole::ProgramRoot {
        if decl.owner != StorageOwner::Global
            || owner.is_none_or(|pou| pou.kind != PouKind::Program)
            || decl.type_id.is_some()
            || decl.default_const_idx.is_some()
        {
            return Err(RejectionReason::InvalidConstructionRecord.into());
        }
    } else if decl.role == StorageRole::Scratch {
        if decl.owner != StorageOwner::Frame
            || decl.type_id.is_some()
            || decl.default_const_idx.is_some()
            || decl.flags != 0
            || decl.retain != 0
            || decl.construction_nodes == 0
        {
            return Err(RejectionReason::InvalidConstructionRecord.into());
        }
    } else {
        let ty = decl
            .type_id
            .ok_or(RejectionReason::InvalidConstructionRecord)?;
        ensure_type_index(tables.types, ty)?;
        if let Some(value) = decl.default_const_idx {
            ensure_const_index(tables.const_pool, value)?;
            if tables.const_pool.entries[value as usize].type_id != ty {
                return Err(RejectionReason::InvalidConstructionRecord.into());
            }
        }
    }
    if decl.owner == StorageOwner::Instance
        && owner.is_some_and(|p| {
            !matches!(
                p.kind,
                PouKind::Program | PouKind::FunctionBlock | PouKind::Class | PouKind::Method
            )
        })
    {
        return Err(RejectionReason::InvalidConstructionRecord.into());
    }
    if decl.role == StorageRole::NativeState
        && (decl.owner != StorageOwner::Instance
            || owner.is_none_or(|pou| pou.kind != PouKind::FunctionBlock)
            || decl.flags != 0
            || decl.retain != 0
            || decl.default_const_idx.is_some())
    {
        return Err(RejectionReason::InvalidConstructionRecord.into());
    }
    if decl.role == StorageRole::Static
        && (decl.owner == StorageOwner::Frame
            || owner.is_none_or(|p| !matches!(p.kind, PouKind::Function | PouKind::Method)))
    {
        return Err(RejectionReason::InvalidConstructionRecord.into());
    }
    if decl.owner == StorageOwner::Frame && decl.role != StorageRole::External {
        let pou = owner.ok_or(RejectionReason::InvalidConstructionRecord)?;
        if decl.slot >= pou.local_ref_count || decl.retain != 0 {
            return Err(RejectionReason::InvalidConstructionRecord.into());
        }
        let return_slots = u32::from(pou.return_type_id.is_some());
        let parameter_end = return_slots
            .checked_add(pou.params.len() as u32)
            .ok_or(RejectionReason::InvalidConstructionRecord)?;
        let expected_role = if decl.slot < return_slots {
            StorageRole::Return
        } else if decl.slot < parameter_end {
            StorageRole::Parameter
        } else {
            StorageRole::Variable
        };
        if decl.role != expected_role
            && !(expected_role == StorageRole::Variable && decl.role == StorageRole::Scratch)
        {
            return Err(RejectionReason::InvalidConstructionRecord.into());
        }
        if decl.role == StorageRole::Return
            && (decl.slot != 0 || decl.type_id != pou.return_type_id)
        {
            return Err(RejectionReason::ConstructionSignatureMismatch.into());
        }
        if decl.role == StorageRole::Parameter {
            let at = decl
                .slot
                .checked_sub(u32::from(pou.return_type_id.is_some()))
                .ok_or(RejectionReason::ConstructionSignatureMismatch)?;
            if pou.params.get(at as usize).is_none_or(|p| {
                Some(p.type_id) != decl.type_id
                    || p.name_idx != decl.name_idx
                    || decl.flags != (2 << p.direction)
            }) {
                return Err(RejectionReason::ConstructionSignatureMismatch.into());
            }
        }
    }
    Ok(owner)
}

fn validate_keys(
    layout: &StorageLayout,
    slots: &mut [(u8, Option<u32>, u32, usize)],
    names: &mut [(u8, Option<u32>, &str, usize)],
    static_names: &mut [(Option<u32>, &str)],
    budget: &mut ValidationBudget,
) -> Result<(), BytecodeError> {
    budget.sort_by(slots, |a, b, _| Ok(a.cmp(b)))?;
    let mut previous_owner = None;
    let mut next_slot = 0u32;
    for &(kind, owner, slot, declaration) in slots.iter() {
        budget.work(1)?;
        if previous_owner != Some((kind, owner)) {
            previous_owner = Some((kind, owner));
            next_slot = 0;
        }
        if slot != next_slot {
            return Err(RejectionReason::NonDenseStorageSlots.into());
        }
        let record = &layout.entries[declaration];
        let width = if record.role == StorageRole::Scratch {
            record.construction_nodes
        } else {
            1
        };
        next_slot = next_slot
            .checked_add(width)
            .ok_or(RejectionReason::ConstructionDemandOverflow)?;
    }
    budget.sort_by(static_names, |a, b, budget| {
        Ok(a.0.cmp(&b.0).then(budget.compare_names(a.1, b.1)?))
    })?;
    for pair in static_names.windows(2) {
        budget.work(1)?;
        if pair[0].0 == pair[1].0 && budget.compare_names(pair[0].1, pair[1].1)?.is_eq() {
            return Err(RejectionReason::InvalidConstructionRecord.into());
        }
    }
    budget.sort_by(names, |a, b, budget| {
        Ok((a.0, a.1)
            .cmp(&(b.0, b.1))
            .then(budget.compare_names(a.2, b.2)?)
            .then(a.3.cmp(&b.3)))
    })?;
    for pair in slots.windows(2) {
        budget.work(1)?;
        if (pair[0].0, pair[0].1, pair[0].2) == (pair[1].0, pair[1].1, pair[1].2) {
            return Err(RejectionReason::InvalidConstructionRecord.into());
        }
    }
    for pair in names.windows(2) {
        budget.work(1)?;
        if (pair[0].0, pair[0].1) == (pair[1].0, pair[1].1)
            && budget.compare_names(pair[0].2, pair[1].2)?.is_eq()
        {
            return Err(RejectionReason::InvalidConstructionRecord.into());
        }
    }
    Ok(())
}

fn validate_binding<'a>(
    tables: &ValidationContext<'a>,
    decl: &StorageDeclaration,
    owner: Option<&PouEntry>,
    budget: &mut ValidationBudget,
) -> Result<Option<&'a RefEntry>, BytecodeError> {
    if let Some(id) = decl.ref_idx {
        ensure_ref_index(tables.ref_table, id)?;
        let reference = &tables.ref_table.entries[id as usize];
        if !reference.segments.is_empty() || reference.location == RefLocation::InitializerResult {
            return Err(RejectionReason::InvalidConstructionRecord.into());
        }
        if decl.role != StorageRole::External {
            if reference.offset != decl.slot {
                return Err(RejectionReason::InvalidConstructionRecord.into());
            }
            match decl.owner {
                StorageOwner::Global
                    if !matches!(
                        reference.location,
                        RefLocation::Global | RefLocation::Retain
                    ) =>
                {
                    return Err(RejectionReason::InvalidConstructionRecord.into())
                }
                StorageOwner::Frame => ensure_pou_ref_operand(
                    tables.ref_table,
                    owner.ok_or(RejectionReason::InvalidConstructionRecord)?,
                    id,
                )?,
                StorageOwner::Instance => {
                    return Err(RejectionReason::InvalidConstructionRecord.into())
                }
                _ => {}
            }
            if decl.owner == StorageOwner::Frame {
                if reference.location != RefLocation::Local {
                    return Err(RejectionReason::InvalidConstructionRecord.into());
                }
                if decl.role != StorageRole::Scratch && tables.ref_type(id, budget)? != decl.type_id
                {
                    return Err(RejectionReason::InvalidConstructionRecord.into());
                }
            }
        }
        Ok(Some(reference))
    } else if decl.owner == StorageOwner::Instance {
        Ok(None)
    } else {
        Err(RejectionReason::InvalidConstructionRecord.into())
    }
}

fn validate_coverage(
    tables: &ValidationContext<'_>,
    layout: &StorageLayout,
    declared_refs: &mut [(u8, u32, u32)],
    scratch: &mut [(u32, u32, u32)],
    budget: &mut ValidationBudget,
) -> Result<(), BytecodeError> {
    budget.sort_by(declared_refs, |a, b, _| Ok(a.cmp(b)))?;
    budget.sort_by(scratch, |a, b, _| Ok(a.cmp(b)))?;
    for pair in scratch.windows(2) {
        budget.work(1)?;
        if pair[0].0 == pair[1].0 && pair[1].1 < pair[0].2 {
            return Err(RejectionReason::InvalidConstructionRecord.into());
        }
    }
    for declaration in &layout.entries {
        if declaration.owner == StorageOwner::Frame
            && !matches!(
                declaration.role,
                StorageRole::External | StorageRole::Scratch
            )
        {
            let reference = &tables.ref_table.entries[declaration
                .ref_idx
                .ok_or(RejectionReason::InvalidConstructionRecord)?
                as usize];
            if scratch_contains(scratch, reference, budget)? {
                return Err(RejectionReason::InvalidConstructionRecord.into());
            }
        }
    }
    for reference in &tables.ref_table.entries {
        budget.work(1)?;
        if matches!(
            reference.location,
            RefLocation::Global | RefLocation::Retain | RefLocation::Local
        ) {
            let key = (
                reference.location as u8,
                reference.owner_id,
                reference.offset,
            );
            if budget
                .search_by(declared_refs, |v, _| Ok(v.cmp(&key)))?
                .is_err()
            {
                // Compiler scratch-bank tails have one declaration for their complete range.
                let covered = reference.location == RefLocation::Local
                    && scratch_contains(scratch, reference, budget)?;
                if !covered {
                    return Err(RejectionReason::IncompleteConstructionMetadata.into());
                }
            }
        }
    }
    let demands = demand::declaration_demands(tables, layout, budget)?;
    for (decl, expected) in layout.entries.iter().zip(demands) {
        budget.work(1)?;
        if decl.construction_nodes != expected {
            return Err(RejectionReason::ConstructionDemandMismatch.into());
        }
    }
    Ok(())
}
