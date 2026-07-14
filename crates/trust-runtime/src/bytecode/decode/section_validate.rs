fn validate_section_entries(
    file_len: usize,
    entries: &[SectionEntry],
) -> Result<(), BytecodeError> {
    let mut standardized_ids = [false; 13];
    for entry in entries {
        if SectionId::from_raw(entry.id).is_some() {
            let index = entry.id as usize;
            if standardized_ids[index] {
                return Err(BytecodeError::InvalidSection(
                    format!("duplicate standardized section id 0x{:04X}", entry.id).into(),
                ));
            }
            standardized_ids[index] = true;
        }
    }

    let mut sorted = entries.to_vec();
    sorted.sort_by_key(|entry| entry.offset);
    let mut last_end = 0usize;
    for entry in sorted {
        if entry.offset % 4 != 0 {
            return Err(BytecodeError::SectionAlignment);
        }
        let start = entry.offset as usize;
        let end = start + entry.length as usize;
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
