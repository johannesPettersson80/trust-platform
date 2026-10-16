//! STBC 3.0 header integrity: the whole-file checksum covers every header field,
//! while 1.x and 2.0 keep the section-table rule.

use trust_runtime_core::bytecode::{
    BytecodeError, BytecodeModule, BytecodeVersion, HEADER_FLAG_CRC32,
    HEADER_FLAG_LEGACY_TEMPORAL32,
};
use trust_runtime_core::crc32;

const PROGRAM_V2: &[u8] =
    include_bytes!("../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc");

/// The checksum stored at header offsets 20..24.
fn stored_checksum(bytes: &[u8]) -> u32 {
    u32::from_le_bytes(bytes[20..24].try_into().unwrap())
}

/// The 3.0 rule: the whole file with the four checksum bytes read as zero.
fn whole_file_checksum(bytes: &[u8]) -> u32 {
    let zero_checksum = [0u8; 4];
    crc32::checksum_parts(&[&bytes[..20], &zero_checksum[..], &bytes[24..]])
}

/// The 1.x and 2.0 rule: the section table and payloads from the table offset.
fn section_checksum(bytes: &[u8]) -> u32 {
    let section_table_off = u32::from_le_bytes(bytes[16..20].try_into().unwrap()) as usize;
    crc32::checksum(&bytes[section_table_off..])
}

/// A 3.0 artifact built from the saved 2.0 program, as temporal_storage_abi.rs does.
fn source_free_artifact() -> Vec<u8> {
    let mut module = BytecodeModule::decode(PROGRAM_V2).unwrap();
    module.version = BytecodeVersion::SOURCE_FREE;
    module.flags = HEADER_FLAG_CRC32;
    module.encode().unwrap()
}

#[test]
fn saved_two_point_zero_fixture_still_decodes_unchanged() {
    let module = BytecodeModule::decode(PROGRAM_V2).unwrap();
    assert_eq!(module.version, BytecodeVersion::SOURCE_FREE_V2);
    assert_eq!(module.encode().unwrap(), PROGRAM_V2);
}

#[test]
fn three_point_zero_artifact_round_trips_with_a_whole_file_checksum() {
    let bytes = source_free_artifact();
    let decoded = BytecodeModule::decode(&bytes).unwrap();
    assert_eq!(decoded.version, BytecodeVersion::SOURCE_FREE);
    assert_eq!(decoded.flags, HEADER_FLAG_CRC32);
    assert_eq!(decoded.encode().unwrap(), bytes);

    let mut zeroed = bytes.clone();
    zeroed[20..24].fill(0);
    assert_eq!(stored_checksum(&bytes), crc32::checksum(&zeroed));
    assert_eq!(stored_checksum(&bytes), whole_file_checksum(&bytes));
}

#[test]
fn relabelling_three_point_zero_to_an_older_major_fails_the_checksum() {
    let bytes = source_free_artifact();
    for (major, minor) in [(1u16, 1u16), (2, 0)] {
        let mut relabelled = bytes.clone();
        relabelled[4..6].copy_from_slice(&major.to_le_bytes());
        relabelled[6..8].copy_from_slice(&minor.to_le_bytes());
        assert_eq!(
            BytecodeModule::decode(&relabelled),
            Err(BytecodeError::InvalidChecksum {
                expected: stored_checksum(&bytes),
                actual: section_checksum(&relabelled),
            })
        );
    }
}

#[test]
fn relabelling_the_two_point_zero_fixture_to_three_point_zero_fails_the_checksum() {
    let mut relabelled = PROGRAM_V2.to_vec();
    relabelled[4..6].copy_from_slice(&3u16.to_le_bytes());
    relabelled[6..8].copy_from_slice(&0u16.to_le_bytes());
    assert_eq!(
        BytecodeModule::decode(&relabelled),
        Err(BytecodeError::InvalidChecksum {
            expected: stored_checksum(PROGRAM_V2),
            actual: whole_file_checksum(&relabelled),
        })
    );
}

#[test]
fn toggling_the_legacy_temporal_flag_fails_the_checksum() {
    let bytes = source_free_artifact();
    let mut toggled = bytes.clone();
    let flags = u32::from_le_bytes(toggled[8..12].try_into().unwrap());
    toggled[8..12].copy_from_slice(&(flags | HEADER_FLAG_LEGACY_TEMPORAL32).to_le_bytes());
    assert_eq!(
        BytecodeModule::decode(&toggled),
        Err(BytecodeError::InvalidChecksum {
            expected: stored_checksum(&bytes),
            actual: whole_file_checksum(&toggled),
        })
    );
}

#[test]
fn clearing_the_checksum_flag_is_refused_by_the_flag_rule() {
    let mut unchecked = source_free_artifact();
    unchecked[8..12].fill(0);
    assert!(matches!(
        BytecodeModule::decode(&unchecked),
        Err(BytecodeError::InvalidHeader(_))
    ));
}

#[test]
fn changed_section_count_and_table_offset_fail_the_checksum() {
    let bytes = source_free_artifact();
    let stored = stored_checksum(&bytes);
    let section_count = u16::from_le_bytes(bytes[14..16].try_into().unwrap());
    for changed in [section_count - 1, section_count + 1] {
        let mut counted = bytes.clone();
        counted[14..16].copy_from_slice(&changed.to_le_bytes());
        assert_eq!(
            BytecodeModule::decode(&counted),
            Err(BytecodeError::InvalidChecksum {
                expected: stored,
                actual: whole_file_checksum(&counted),
            })
        );
    }
    // Offset 28 is aligned, past the header and inside the file, so only the checksum fails.
    let mut moved = bytes.clone();
    moved[16..20].copy_from_slice(&28u32.to_le_bytes());
    assert_eq!(
        BytecodeModule::decode(&moved),
        Err(BytecodeError::InvalidChecksum {
            expected: stored,
            actual: whole_file_checksum(&moved),
        })
    );
}

#[test]
fn changed_header_size_is_refused_by_the_structural_rule_before_the_checksum() {
    // A header size of 28 leaves the section table at offset 24, before the header end.
    let mut resized = source_free_artifact();
    resized[12..14].copy_from_slice(&28u16.to_le_bytes());
    assert!(matches!(
        BytecodeModule::decode(&resized),
        Err(BytecodeError::InvalidHeader(_))
    ));
}
