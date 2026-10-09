//! Source-free I/O bindings must fit the declared persistent process image.

use super::*;

pub(super) fn validate_bindings(
    module: &BytecodeModuleView<'_>,
    tables: &ValidationContext<'_>,
    construction: &ConstructionTables<'_>,
    budget: &mut ValidationBudget,
) -> Result<(), BytecodeError> {
    let Some(SectionData::IoMap(map)) = module.section(SectionId::IoMap) else {
        return Err(RejectionReason::IncompleteConstructionMetadata.into());
    };
    for binding in &map.bindings {
        budget.work(1)?;
        let reference = tables
            .ref_table
            .entries
            .get(binding.ref_idx as usize)
            .ok_or(RejectionReason::InvalidConstructionRecord)?;
        if !matches!(
            reference.location,
            RefLocation::Global | RefLocation::Retain | RefLocation::Instance
        ) {
            return Err(RejectionReason::InvalidConstructionRecord.into());
        }
        let text = tables
            .strings
            .entries
            .get(binding.address_str_idx as usize)
            .ok_or(RejectionReason::InvalidConstructionRecord)?;
        budget.temporary(|budget| {
            budget.work(text.len())?;
            budget.storage(
                text.len()
                    .checked_mul(32)
                    .ok_or(RejectionReason::ValidationStorageLimit)?,
            )?;
            let address = crate::io_address::IoAddress::parse(text)
                .map_err(|_| RejectionReason::InvalidConstructionRecord)?;
            let Some(mut end) = address
                .flat_byte_end()
                .map_err(|_| RejectionReason::InvalidConstructionRecord)?
            else {
                return Ok(());
            };
            if let Some(ty) = binding.type_id {
                let width = leaf_width(tables, ty, budget)?;
                end = end.max(
                    address
                        .byte
                        .checked_add(width)
                        .ok_or(RejectionReason::InvalidConstructionRecord)?,
                );
            }
            let size = match address.area {
                crate::memory::IoArea::Input => construction.resource.inputs_size,
                crate::memory::IoArea::Output => construction.resource.outputs_size,
                crate::memory::IoArea::Memory => construction.resource.memory_size,
            };
            if end > size {
                return Err(RejectionReason::InvalidConstructionRecord.into());
            }
            Ok(())
        })?;
    }
    Ok(())
}

fn leaf_width(
    tables: &ValidationContext<'_>,
    mut ty: u32,
    budget: &mut ValidationBudget,
) -> Result<u32, BytecodeError> {
    for _ in 0..=crate::bytecode::BYTECODE_MAX_CONST_NESTING {
        budget.work(1)?;
        let entry = tables
            .types
            .entries
            .get(ty as usize)
            .ok_or(RejectionReason::InvalidConstructionRecord)?;
        match &entry.data {
            TypeData::Alias { target_type_id } => ty = *target_type_id,
            TypeData::Enum { base_type_id, .. } | TypeData::Subrange { base_type_id, .. } => {
                ty = *base_type_id
            }
            TypeData::Primitive {
                prim_id,
                max_length,
            } => {
                let width = crate::vm::sizeof_primitive_type(*prim_id, *max_length)
                    .map_err(|_| RejectionReason::InvalidConstructionRecord)?;
                return u32::try_from(width)
                    .map_err(|_| RejectionReason::InvalidConstructionRecord.into());
            }
            _ => return Err(RejectionReason::InvalidConstructionRecord.into()),
        }
    }
    Err(RejectionReason::TypeReferenceRecursionOverflow.into())
}
