use super::*;
use crate::bytecode::{
    AccessBindingEntry, AccessBindings, ConstructionRoot, ConstructionRoots, InitializationOnce,
    InitializationPhase, InitializationStage, InitializationTarget, InitializationTrigger,
    InitializerBodyKind, InitializerEntry, InitializerIndex, StorageDeclaration, StorageLayout,
    StorageOwner, StorageRole, BYTECODE_MAX_CONSTRUCTION_RECORDS,
};

fn optional(reader: &mut BytecodeReader<'_>) -> Result<Option<u32>, BytecodeError> {
    let value = reader.read_u32()?;
    Ok((value != u32::MAX).then_some(value))
}

pub(super) fn decode_construction(
    kind: SectionId,
    payload: &[u8],
) -> Result<SectionData, BytecodeError> {
    let mut reader = BytecodeReader::new(payload);
    let width = match kind {
        SectionId::ConstructionRoots => 24,
        SectionId::Initializers => 60,
        SectionId::AccessBindings => 20,
        _ => 40,
    };
    let count = read_bounded_count_with_limit(
        &mut reader,
        width,
        BYTECODE_MAX_CONSTRUCTION_RECORDS,
        "construction metadata",
    )?;
    let invalid = || BytecodeError::from(RejectionReason::InvalidConstructionRecord);
    let data = match kind {
        SectionId::StorageLayout => {
            let mut entries = Vec::new();
            entries.try_reserve_exact(count).map_err(|_| invalid())?;
            for _ in 0..count {
                entries.push(StorageDeclaration {
                    owner: StorageOwner::from_raw(reader.read_u8()?).ok_or_else(invalid)?,
                    role: StorageRole::from_raw(reader.read_u8()?).ok_or_else(invalid)?,
                    retain: reader.read_u8()?,
                    flags: reader.read_u8()?,
                    owner_pou_id: optional(&mut reader)?,
                    name_idx: reader.read_u32()?,
                    type_id: optional(&mut reader)?,
                    slot: reader.read_u32()?,
                    ref_idx: optional(&mut reader)?,
                    default_const_idx: optional(&mut reader)?,
                    construction_nodes: reader.read_u32()?,
                    related_declaration_idx: optional(&mut reader)?,
                    source_name_idx: optional(&mut reader)?,
                });
            }
            SectionData::StorageLayout(StorageLayout { entries })
        }
        SectionId::ConstructionRoots => {
            let mut entries = Vec::new();
            entries.try_reserve_exact(count).map_err(|_| invalid())?;
            for _ in 0..count {
                entries.push(ConstructionRoot {
                    declaration_idx: reader.read_u32()?,
                    binding_ref_idx: optional(&mut reader)?,
                    instance_owner_id: optional(&mut reader)?,
                    parent_root_idx: optional(&mut reader)?,
                    template_pou_id: optional(&mut reader)?,
                    flags: reader.read_u32()?,
                });
            }
            SectionData::ConstructionRoots(ConstructionRoots { entries })
        }
        SectionId::Initializers => {
            let mut entries = Vec::new();
            entries.try_reserve_exact(count).map_err(|_| invalid())?;
            for _ in 0..count {
                entries.push(InitializerEntry {
                    declaration_idx: optional(&mut reader)?,
                    owner_pou_id: optional(&mut reader)?,
                    result_ref_idx: reader.read_u32()?,
                    code_offset: reader.read_u32()?,
                    code_length: reader.read_u32()?,
                    visible_local_count: reader.read_u32()?,
                    visible_static_count: reader.read_u32()?,
                    phase: InitializationPhase::from_raw(reader.read_u8()?).ok_or_else(invalid)?,
                    once: InitializationOnce::from_raw(reader.read_u8()?).ok_or_else(invalid)?,
                    stage: InitializationStage::from_raw(reader.read_u8()?).ok_or_else(invalid)?,
                    trigger: InitializationTrigger::from_raw(reader.read_u8()?)
                        .ok_or_else(invalid)?,
                    target_idx: optional(&mut reader)?,
                    partial_kind: reader.read_u8()?,
                    target_kind: InitializationTarget::from_raw(reader.read_u8()?)
                        .ok_or_else(invalid)?,
                    target_reserved: [reader.read_u8()?, reader.read_u8()?],
                    partial_index: reader.read_u32()?,
                    context_initializer_idx: optional(&mut reader)?,
                    recipe_type_id: optional(&mut reader)?,
                    recipe_member_idx: optional(&mut reader)?,
                    body_kind: InitializerBodyKind::from_raw(reader.read_u8()?)
                        .ok_or_else(invalid)?,
                    recipe_reserved: [reader.read_u8()?, reader.read_u8()?, reader.read_u8()?],
                });
            }
            SectionData::Initializers(InitializerIndex { entries })
        }
        SectionId::AccessBindings => {
            let mut entries = Vec::new();
            entries.try_reserve_exact(count).map_err(|_| invalid())?;
            for _ in 0..count {
                entries.push(AccessBindingEntry {
                    name_idx: reader.read_u32()?,
                    type_id: reader.read_u32()?,
                    ref_idx: reader.read_u32()?,
                    partial_kind: reader.read_u8()?,
                    flags: reader.read_u8()?,
                    reserved: reader.read_u16()?,
                    partial_index: reader.read_u32()?,
                });
            }
            SectionData::AccessBindings(AccessBindings { entries })
        }
        _ => return Err(invalid()),
    };
    if reader.remaining() != 0 {
        return Err(invalid());
    }
    Ok(data)
}
