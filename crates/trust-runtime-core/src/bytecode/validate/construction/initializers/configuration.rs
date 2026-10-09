//! Target and lifetime checks for ordered configuration initialization actions.

use super::super::lookups::{primitive, validate_partial_type};
use super::*;

pub(super) fn validate_action(
    tables: &ValidationContext<'_>,
    construction: &ConstructionTables<'_>,
    declaration: Option<&StorageDeclaration>,
    entry: &InitializerEntry,
    budget: &mut ValidationBudget,
) -> Result<u32, BytecodeError> {
    if entry.stage != InitializationStage::Explicit
        || entry.owner_pou_id.is_some()
        || entry.visible_local_count != 0
        || entry.visible_static_count != 0
        || entry.once != InitializationOnce::None
        || entry.trigger != InitializationTrigger::Ordinary
    {
        return Err(RejectionReason::InvalidInitializerRecord.into());
    }
    let result_type = tables
        .ref_type(entry.result_ref_idx, budget)?
        .ok_or(RejectionReason::InvalidInitializerRecord)?;
    if entry.target_kind == InitializationTarget::DirectIo {
        validate_direct_address(
            tables,
            construction,
            declaration,
            entry,
            result_type,
            budget,
        )?;
        return Ok(result_type);
    }
    if entry.target_kind != InitializationTarget::Reference {
        return Err(RejectionReason::InvalidInitializerRecord.into());
    }
    let target_id = entry
        .target_idx
        .ok_or(RejectionReason::InvalidInitializerRecord)?;
    let target = tables
        .ref_table
        .entries
        .get(target_id as usize)
        .ok_or(RejectionReason::InvalidInitializerRecord)?;
    match target.location {
        RefLocation::Global | RefLocation::Retain | RefLocation::Instance => {
            let declaration = declaration.ok_or(RejectionReason::InvalidInitializerRecord)?;
            if declaration.owner == StorageOwner::Frame
                || declaration.flags & 9 != 0
                || matches!(
                    declaration.role,
                    StorageRole::External
                        | StorageRole::Scratch
                        | StorageRole::Return
                        | StorageRole::NativeState
                        | StorageRole::EdgePhase
                )
            {
                return Err(RejectionReason::InvalidInitializerRecord.into());
            }
            if let Some(base_id) = declaration.ref_idx {
                let base = tables
                    .ref_table
                    .entries
                    .get(base_id as usize)
                    .ok_or(RejectionReason::InvalidInitializerRecord)?;
                if target.location != base.location
                    || target.owner_id != base.owner_id
                    || target.offset != base.offset
                    || !target.segments.starts_with(&base.segments)
                {
                    return Err(RejectionReason::InvalidInitializerRecord.into());
                }
            } else if declaration.owner != StorageOwner::Instance
                || target.location != RefLocation::Instance
                || target.offset != declaration.slot
            {
                return Err(RejectionReason::InvalidInitializerRecord.into());
            }
        }
        _ => return Err(RejectionReason::InvalidInitializerRecord.into()),
    }
    let resolved = construction
        .paths
        .declaration_for_reference(target, budget)?
        .ok_or(RejectionReason::InvalidInitializerRecord)?;
    if Some(resolved as u32) != entry.declaration_idx {
        return Err(RejectionReason::InvalidInitializerRecord.into());
    }
    let base_type = construction.layout.entries[resolved]
        .type_id
        .ok_or(RejectionReason::InvalidInitializerRecord)?;
    let target_type =
        construction
            .paths
            .select_path(tables, base_type, &target.segments, budget, |id| {
                let member = &construction.layout.entries[id];
                if member.flags & 9 != 0
                    || matches!(
                        member.role,
                        StorageRole::NativeState | StorageRole::EdgePhase | StorageRole::External
                    )
                {
                    return Err(RejectionReason::InvalidInitializerRecord.into());
                }
                Ok(())
            })?;
    if tables.ref_type(target_id, budget)? != Some(target_type) {
        return Err(RejectionReason::InvalidInitializerRecord.into());
    }
    validate_partial_type(
        tables,
        target_type,
        result_type,
        entry.partial_kind,
        entry.partial_index,
        budget,
    )?;
    Ok(result_type)
}

fn validate_direct_address(
    tables: &ValidationContext<'_>,
    construction: &ConstructionTables<'_>,
    declaration: Option<&StorageDeclaration>,
    entry: &InitializerEntry,
    result_type: u32,
    budget: &mut ValidationBudget,
) -> Result<(), BytecodeError> {
    if declaration.is_some() || entry.partial_kind != 0 || entry.partial_index != 0 {
        return Err(RejectionReason::InvalidInitializerRecord.into());
    }
    let text = entry
        .target_idx
        .and_then(|id| tables.strings.entries.get(id as usize))
        .ok_or(RejectionReason::InvalidInitializerRecord)?;
    budget.temporary(|budget| {
        // The shared parser owns a String, component slices and a numeric path.
        // Charge a conservative linear peak before it allocates any of them.
        budget.work(text.len())?;
        budget.storage(
            text.len()
                .checked_mul(32)
                .ok_or(RejectionReason::ValidationStorageLimit)?,
        )?;
        let address = crate::io_address::IoAddress::parse(text)
            .map_err(|_| RejectionReason::InvalidInitializerRecord)?;
        if address.wildcard {
            return Err(RejectionReason::InvalidInitializerRecord.into());
        }
        let Some(end) = address
            .flat_byte_end()
            .map_err(|_| RejectionReason::InvalidInitializerRecord)?
        else {
            return Ok(());
        };
        let size = match address.area {
            crate::memory::IoArea::Input => construction.resource.inputs_size,
            crate::memory::IoArea::Output => construction.resource.outputs_size,
            crate::memory::IoArea::Memory => construction.resource.memory_size,
        };
        if end > size {
            return Err(RejectionReason::InvalidInitializerRecord.into());
        }
        use crate::io_address::IoSize;
        let expected = match address.size {
            IoSize::Bit => 1,
            IoSize::Byte => 2,
            IoSize::Word => 3,
            IoSize::DWord => 4,
            IoSize::LWord => 5,
            IoSize::Bytes(_) => return Err(RejectionReason::InvalidInitializerRecord.into()),
        };
        if primitive(tables, result_type, budget)? != expected {
            return Err(RejectionReason::InvalidInitializerRecord.into());
        }
        Ok(())
    })
}
