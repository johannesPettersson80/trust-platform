use super::*;

pub(super) fn validate_section_entries(
    file_len: usize,
    payload_start: usize,
    entries: &[SectionEntry],
    version: BytecodeVersion,
    budget: &mut DecodeBudget,
) -> Result<(), BytecodeError> {
    budget.charge(
        entries
            .len()
            .checked_mul(core::mem::size_of::<u16>() + 4 * core::mem::size_of::<usize>())
            .ok_or_else(|| BytecodeError::InvalidHeader("decoder allocation overflow".into()))?,
        entries.len(),
    )?;
    let mut standardized_ids = alloc::collections::BTreeSet::new();
    for entry in entries {
        if SectionId::from_raw(entry.id).is_some()
            && (version.major == 2 || entry.id < SectionId::StorageLayout.as_raw())
            && !standardized_ids.insert(entry.id)
        {
            return Err(BytecodeError::InvalidSection(
                format!("duplicate standardized section id 0x{:04X}", entry.id).into(),
            ));
        }
    }

    let mut sorted = budget.copy(entries)?;
    budget.charge(
        entries
            .len()
            .checked_mul(core::mem::size_of::<SectionEntry>())
            .ok_or_else(|| BytecodeError::InvalidHeader("decoder allocation overflow".into()))?,
        entries
            .len()
            .checked_mul(entries.len().max(1).ilog2() as usize + 1)
            .ok_or_else(|| BytecodeError::InvalidHeader("decoder work overflow".into()))?,
    )?;
    sorted.sort_by_key(|entry| entry.offset);
    let mut last_end = payload_start;
    for entry in sorted {
        if entry.offset % 4 != 0 {
            return Err(BytecodeError::SectionAlignment);
        }
        let start = entry.offset as usize;
        let end = start
            .checked_add(entry.length as usize)
            .ok_or(BytecodeError::SectionOutOfBounds)?;
        if end > file_len {
            return Err(BytecodeError::SectionOutOfBounds);
        }
        if start < last_end {
            return Err(BytecodeError::SectionOverlap);
        }
        last_end = end;
    }
    Ok(())
}
