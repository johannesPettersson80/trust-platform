//! Frozen scalar decoder oracle: exact results, first rejection and budget charges.
use super::*;

fn optional(reader: &mut BytecodeReader<'_>) -> Result<Option<u32>, BytecodeError> {
    let value = reader.read_u32()?;
    Ok((value != u32::MAX).then_some(value))
}

fn scalar_oracle(
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
            let mut entries = budget.vector(count)?;
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
            let mut entries = budget.vector(count)?;
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
            let mut entries = budget.vector(count)?;
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

fn compare(kind: SectionId, payload: &[u8], bytes: usize, work: usize) {
    let mut before = DecodeBudget::new(bytes, work);
    let mut after = DecodeBudget::new(bytes, work);
    assert_eq!(
        decode_construction(kind, payload, &mut after),
        scalar_oracle(kind, payload, &mut before),
        "section {kind:?}, payload {payload:?}, limits {bytes}/{work}"
    );
    assert_eq!(after.stats(), before.stats());
}

#[test]
fn fixed_construction_records_preserve_every_field_and_rejection_boundary() {
    for (kind, width) in [
        (SectionId::StorageLayout, 40),
        (SectionId::ConstructionRoots, 24),
        (SectionId::Initializers, 60),
        (SectionId::AccessBindings, 20),
    ] {
        let mut payload = alloc::vec![0; 4 + width * 2];
        payload[..4].copy_from_slice(&2u32.to_le_bytes());
        // Starting from valid enum discriminants, change every byte through its
        // full domain. Includes all optional sentinels, reserved bytes and kinds.
        for offset in 4..payload.len() {
            for value in 0..=u8::MAX {
                payload[offset] = value;
                compare(kind, &payload, usize::MAX, usize::MAX);
            }
            payload[offset] = 0;
        }
        for byte in [0x7f, 0x80, 0xff] {
            payload[4..].fill(byte);
            for start in [4, 4 + width] {
                match kind {
                    SectionId::StorageLayout => {
                        payload[start] = 0;
                        payload[start + 1] = 0;
                    }
                    SectionId::Initializers => {
                        for offset in [28, 29, 30, 31, 37, 56] {
                            payload[start + offset] = 0;
                        }
                    }
                    _ => {}
                }
            }
            compare(kind, &payload, usize::MAX, usize::MAX);
        }
        for length in 0..payload.len() {
            compare(kind, &payload[..length], usize::MAX, usize::MAX);
        }
        payload.extend_from_slice(&[1, 2, 3]);
        compare(kind, &payload, usize::MAX, usize::MAX);
        for count in [0, 1, 2, 3, u32::MAX] {
            payload[..4].copy_from_slice(&count.to_le_bytes());
            for (bytes, work) in [
                (0, 0),
                (1, usize::MAX),
                (usize::MAX, 1),
                (usize::MAX, usize::MAX),
            ] {
                compare(kind, &payload, bytes, work);
            }
        }
    }
}

#[test]
fn fixed_construction_records_preserve_combined_random_fields_and_limits() {
    let mut seed = 0x91e1_0da5_51e4_9b37u64;
    for (kind, width) in [
        (SectionId::StorageLayout, 40),
        (SectionId::ConstructionRoots, 24),
        (SectionId::Initializers, 60),
        (SectionId::AccessBindings, 20),
    ] {
        let mut payload = alloc::vec![0; 4 + width];
        payload[..4].copy_from_slice(&1u32.to_le_bytes());
        for iteration in 0..2048 {
            for byte in &mut payload[4..] {
                seed ^= seed << 13;
                seed ^= seed >> 7;
                seed ^= seed << 17;
                *byte = seed as u8;
            }
            // Alternate unconstrained malformed discriminants with valid ones
            // so every non-enum field participates in successful decoded values.
            if iteration % 2 == 0 {
                match kind {
                    SectionId::StorageLayout => {
                        payload[4] %= 3;
                        payload[5] %= 9;
                    }
                    SectionId::Initializers => {
                        for offset in [28, 29, 30, 31, 37, 56] {
                            payload[4 + offset] = 0;
                        }
                    }
                    _ => {}
                }
            }
            compare(kind, &payload, usize::MAX, usize::MAX);
            compare(kind, &payload, iteration % 128, iteration % 3);
        }
    }
}
