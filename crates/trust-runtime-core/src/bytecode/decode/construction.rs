use super::*;
use crate::bytecode::{
    AccessBindingEntry, AccessBindings, ConstructionRoot, ConstructionRoots, InitializationOnce,
    InitializationPhase, InitializationStage, InitializationTarget, InitializationTrigger,
    InitializerBodyKind, InitializerEntry, InitializerIndex, StorageDeclaration, StorageLayout,
    StorageOwner, StorageRole, BYTECODE_MAX_CONSTRUCTION_RECORDS,
};

pub(super) fn decode_construction(
    kind: SectionId,
    payload: &[u8],
    budget: &mut DecodeBudget,
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
            let mut entries = budget.vector(count)?;
            for _ in 0..count {
                let record = Record::<40>::read(&mut reader)?;
                entries.push(StorageDeclaration {
                    owner: StorageOwner::from_raw(record.0[0]).ok_or_else(invalid)?,
                    role: StorageRole::from_raw(record.0[1]).ok_or_else(invalid)?,
                    retain: record.0[2],
                    flags: record.0[3],
                    owner_pou_id: record.optional::<4>(),
                    name_idx: record.word::<8>(),
                    type_id: record.optional::<12>(),
                    slot: record.word::<16>(),
                    ref_idx: record.optional::<20>(),
                    default_const_idx: record.optional::<24>(),
                    construction_nodes: record.word::<28>(),
                    related_declaration_idx: record.optional::<32>(),
                    source_name_idx: record.optional::<36>(),
                });
            }
            SectionData::StorageLayout(StorageLayout { entries })
        }
        SectionId::ConstructionRoots => {
            let mut entries = budget.vector(count)?;
            for _ in 0..count {
                let record = Record::<24>::read(&mut reader)?;
                entries.push(ConstructionRoot {
                    declaration_idx: record.word::<0>(),
                    binding_ref_idx: record.optional::<4>(),
                    instance_owner_id: record.optional::<8>(),
                    parent_root_idx: record.optional::<12>(),
                    template_pou_id: record.optional::<16>(),
                    flags: record.word::<20>(),
                });
            }
            SectionData::ConstructionRoots(ConstructionRoots { entries })
        }
        SectionId::Initializers => {
            let mut entries = budget.vector(count)?;
            for _ in 0..count {
                let record = Record::<60>::read(&mut reader)?;
                entries.push(InitializerEntry {
                    declaration_idx: record.optional::<0>(),
                    owner_pou_id: record.optional::<4>(),
                    result_ref_idx: record.word::<8>(),
                    code_offset: record.word::<12>(),
                    code_length: record.word::<16>(),
                    visible_local_count: record.word::<20>(),
                    visible_static_count: record.word::<24>(),
                    phase: InitializationPhase::from_raw(record.0[28]).ok_or_else(invalid)?,
                    once: InitializationOnce::from_raw(record.0[29]).ok_or_else(invalid)?,
                    stage: InitializationStage::from_raw(record.0[30]).ok_or_else(invalid)?,
                    trigger: InitializationTrigger::from_raw(record.0[31]).ok_or_else(invalid)?,
                    target_idx: record.optional::<32>(),
                    partial_kind: record.0[36],
                    target_kind: InitializationTarget::from_raw(record.0[37])
                        .ok_or_else(invalid)?,
                    target_reserved: [record.0[38], record.0[39]],
                    partial_index: record.word::<40>(),
                    context_initializer_idx: record.optional::<44>(),
                    recipe_type_id: record.optional::<48>(),
                    recipe_member_idx: record.optional::<52>(),
                    body_kind: InitializerBodyKind::from_raw(record.0[56]).ok_or_else(invalid)?,
                    recipe_reserved: [record.0[57], record.0[58], record.0[59]],
                });
            }
            SectionData::Initializers(InitializerIndex { entries })
        }
        SectionId::AccessBindings => {
            let mut entries = budget.vector(count)?;
            for _ in 0..count {
                let record = Record::<20>::read(&mut reader)?;
                entries.push(AccessBindingEntry {
                    name_idx: record.word::<0>(),
                    type_id: record.word::<4>(),
                    ref_idx: record.word::<8>(),
                    partial_kind: record.0[12],
                    flags: record.0[13],
                    reserved: u16::from_le_bytes([record.0[14], record.0[15]]),
                    partial_index: record.word::<16>(),
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

#[cfg(test)]
mod tests;
