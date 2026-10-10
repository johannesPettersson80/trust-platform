use super::*;

pub(super) fn validate_section_entries(
    file_len: usize,
    payload_start: usize,
    entries: &[SectionEntry],
    version: BytecodeVersion,
    budget: &mut DecodeBudget,
) -> Result<(), BytecodeError> {
    budget.charge(0, entries.len())?;
    // The standard currently defines IDs 1..=16. Unknown extension IDs are not
    // deduplicated; retain the STBC 1.x interpretation of later standard IDs.
    let mut standardized_ids = 0u32;
    for entry in entries {
        if SectionId::from_raw(entry.id).is_some()
            && (version.major == 2 || entry.id < SectionId::StorageLayout.as_raw())
        {
            let mask = 1u32 << entry.id;
            if standardized_ids & mask != 0 {
                return Err(BytecodeError::section_diagnostic(
                    SectionDiagnostic::DuplicateSection(entry.id),
                ));
            }
            standardized_ids |= mask;
        }
    }

    // Sort indices rather than cloning the section records. The ordinal makes
    // equal offsets retain wire order, including zero-length section errors.
    let mut sorted = budget.vector::<usize>(entries.len())?;
    sorted.extend(0..entries.len());
    crate::sort::heap_sort::<BytecodeError>(sorted.len(), &mut |operation| {
        use crate::sort::Operation;
        match operation {
            Operation::Charge => {
                budget.charge(0, 1)?;
                Ok(false)
            }
            Operation::Less(left, right) => {
                let left = sorted[left];
                let right = sorted[right];
                Ok((entries[left].offset, left) < (entries[right].offset, right))
            }
            Operation::Swap(left, right) => {
                sorted.swap(left, right);
                Ok(false)
            }
        }
    })?;
    let mut last_end = payload_start;
    for index in sorted {
        let entry = &entries[index];
        if !entry.offset.is_multiple_of(4) {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: u16, length: u32) -> SectionEntry {
        SectionEntry {
            id,
            flags: 0,
            offset: 24,
            length,
        }
    }
    fn check(entries: &[SectionEntry], version: BytecodeVersion) -> Result<(), BytecodeError> {
        validate_section_entries(32, 24, entries, version, &mut DecodeBudget::new(4096, 4096))
    }

    #[test]
    fn shared_section_sort_keeps_equal_offset_wire_order() {
        // A zero-length extension before the nonempty section is legal; after it
        // the same offset overlaps. Stable tie order therefore affects rejection.
        assert!(check(
            &[entry(0x8000, 0), entry(0x8001, 4)],
            BytecodeVersion::SOURCE_FREE
        )
        .is_ok());
        assert_eq!(
            check(
                &[entry(0x8001, 4), entry(0x8000, 0)],
                BytecodeVersion::SOURCE_FREE
            ),
            Err(BytecodeError::SectionOverlap)
        );
    }

    #[test]
    fn section_bitmap_covers_every_known_id_without_rejecting_extensions() {
        for id in 0..=u16::MAX {
            if SectionId::from_raw(id).is_some() {
                assert!(id < 32, "standard ID no longer fits its reviewed bitmap");
                assert!(matches!(
                    check(&[entry(id, 0), entry(id, 0)], BytecodeVersion::SOURCE_FREE),
                    Err(BytecodeError::InvalidSection(_))
                ));
            }
        }
        assert!(check(
            &[entry(0x8000, 0), entry(0x8000, 0)],
            BytecodeVersion::SOURCE_FREE
        )
        .is_ok());
        let legacy = BytecodeVersion { major: 1, minor: 1 };
        assert!(check(
            &[
                entry(SectionId::StorageLayout.as_raw(), 0),
                entry(SectionId::StorageLayout.as_raw(), 0)
            ],
            legacy
        )
        .is_ok());
    }

    #[test]
    fn section_sort_exhaustion_is_charged_before_work() {
        let entries = [entry(0x8000, 0), entry(0x8001, 4)];
        let mut budget = DecodeBudget::new(4096, 4);
        assert_eq!(
            validate_section_entries(32, 24, &entries, BytecodeVersion::SOURCE_FREE, &mut budget),
            Err(BytecodeError::DecodeWorkLimit)
        );
        assert_eq!(budget.stats().work, 4); // uniqueness visits and index reservation
    }
}
