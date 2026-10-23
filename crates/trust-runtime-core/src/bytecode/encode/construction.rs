use super::*;

fn optional(value: Option<u32>) -> u32 {
    value.unwrap_or(u32::MAX)
}

pub(super) fn encode_construction(
    data: &SectionData,
    out: &mut Buffer,
) -> Result<(), BytecodeError> {
    match data {
        SectionData::StorageLayout(table) => {
            out.extend_from_slice(&encoded_count(table.entries.len())?.to_le_bytes())?;
            for entry in &table.entries {
                out.extend_from_slice(&[
                    entry.owner as u8,
                    entry.role as u8,
                    entry.retain,
                    entry.flags,
                ])?;
                for field in [
                    optional(entry.owner_pou_id),
                    entry.name_idx,
                    optional(entry.type_id),
                    entry.slot,
                    optional(entry.ref_idx),
                    optional(entry.default_const_idx),
                    entry.construction_nodes,
                    optional(entry.related_declaration_idx),
                    optional(entry.source_name_idx),
                ] {
                    out.extend_from_slice(&field.to_le_bytes())?;
                }
            }
        }
        SectionData::ConstructionRoots(table) => {
            out.extend_from_slice(&encoded_count(table.entries.len())?.to_le_bytes())?;
            for entry in &table.entries {
                for field in [
                    entry.declaration_idx,
                    optional(entry.binding_ref_idx),
                    optional(entry.instance_owner_id),
                    optional(entry.parent_root_idx),
                    optional(entry.template_pou_id),
                    entry.flags,
                ] {
                    out.extend_from_slice(&field.to_le_bytes())?;
                }
            }
        }
        SectionData::Initializers(table) => {
            out.extend_from_slice(&encoded_count(table.entries.len())?.to_le_bytes())?;
            for entry in &table.entries {
                for field in [
                    optional(entry.declaration_idx),
                    optional(entry.owner_pou_id),
                    entry.result_ref_idx,
                    entry.code_offset,
                    entry.code_length,
                    entry.visible_local_count,
                    entry.visible_static_count,
                ] {
                    out.extend_from_slice(&field.to_le_bytes())?;
                }
                out.extend_from_slice(&[
                    entry.phase as u8,
                    entry.once as u8,
                    entry.stage as u8,
                    entry.trigger as u8,
                ])?;
                out.extend_from_slice(&optional(entry.target_idx).to_le_bytes())?;
                out.extend_from_slice(&[entry.partial_kind, entry.target_kind as u8])?;
                out.extend_from_slice(&entry.target_reserved)?;
                out.extend_from_slice(&entry.partial_index.to_le_bytes())?;
                for field in [
                    entry.context_initializer_idx,
                    entry.recipe_type_id,
                    entry.recipe_member_idx,
                ] {
                    out.extend_from_slice(&optional(field).to_le_bytes())?;
                }
                out.extend_from_slice(&[entry.body_kind as u8])?;
                out.extend_from_slice(&entry.recipe_reserved)?;
            }
        }
        SectionData::AccessBindings(table) => {
            out.extend_from_slice(&encoded_count(table.entries.len())?.to_le_bytes())?;
            for entry in &table.entries {
                for field in [entry.name_idx, entry.type_id, entry.ref_idx] {
                    out.extend_from_slice(&field.to_le_bytes())?;
                }
                out.extend_from_slice(&[entry.partial_kind, entry.flags])?;
                out.extend_from_slice(&entry.reserved.to_le_bytes())?;
                out.extend_from_slice(&entry.partial_index.to_le_bytes())?;
            }
        }
        _ => return Err(crate::bytecode::RejectionReason::InvalidConstructionRecord.into()),
    }
    Ok(())
}
