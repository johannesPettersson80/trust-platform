//! The `TRSTA001` container's checks (specification 34 §5.1).
//!
//! The container bytes are worked out by hand: the program is `[1, 2, 3]` at offset 52, the
//! record is `[0xaa, 0xbb]` at offset 55, the total length is 57, and the CRC-32 values are
//! CRC-32/ISO-HDLC.

use alloc::vec;
use alloc::vec::Vec;

use trust_runtime_core::crc32;

use super::{decode, encode, InstallationError};

/// The program `[1, 2, 3]` and the record `[0xaa, 0xbb]` in a version-1 container, byte for byte.
///
/// The header CRC-32 over bytes 12 to 51 is `0xd08a3f7e`, the program CRC-32 is `0x55bc801d` and
/// the record CRC-32 is `0x49822c98`.
const CONTAINER: [u8; 57] = [
    84, 82, 83, 84, 65, 48, 48, 49, // "TRSTA001"
    126, 63, 138, 208, // header CRC-32
    57, 0, 0, 0, // total length
    2, 0, 0, 0, // entry count
    1, 0, 0, 0, 52, 0, 0, 0, 3, 0, 0, 0, 29, 128, 188, 85, // program.stbc entry
    2, 0, 0, 0, 55, 0, 0, 0, 2, 0, 0, 0, 152, 44, 130, 73, // record entry
    1, 2, 3, // program.stbc
    170, 187, // deployment record
];

/// Recompute the header CRC-32 of a container whose bytes 12 to 51 were changed.
fn fix_header_crc(bytes: &mut [u8]) {
    let crc = crc32::checksum(&bytes[12..52]);
    bytes[8..12].copy_from_slice(&crc.to_le_bytes());
}

#[test]
fn a_packed_container_decodes_to_its_two_payloads() {
    let installed = decode(&CONTAINER).expect("the hand-built container decodes");
    assert_eq!(installed.program, &[1, 2, 3]);
    assert_eq!(installed.record, &[0xaa, 0xbb]);
}

#[test]
fn encode_builds_the_container_that_decode_reads() {
    // The writer must produce the same bytes the reader checks (specification 34 §5.1): the
    // program first, the record second, and the header CRC-32 over bytes 12 to 51.
    let bytes = encode(&[1, 2, 3], &[0xaa, 0xbb]).expect("a small container encodes");
    assert_eq!(bytes, CONTAINER);
    let installed = decode(&bytes).expect("the written container decodes");
    assert_eq!(installed.program, &[1, 2, 3]);
    assert_eq!(installed.record, &[0xaa, 0xbb]);
}

#[test]
fn an_erased_region_is_no_installed_program() {
    assert!(matches!(
        decode(&[0xff; 64]),
        Err(InstallationError::NoProgram)
    ));
    assert!(matches!(decode(&[]), Err(InstallationError::NoProgram)));
}

#[test]
fn every_header_and_entry_fault_is_refused() {
    // The magic: any first eight bytes other than TRSTA001 is no installed program.
    let mut bytes = CONTAINER;
    bytes[7] = b'X';
    assert!(matches!(decode(&bytes), Err(InstallationError::NoProgram)));

    // The header CRC-32.
    let mut bytes = CONTAINER;
    bytes[8] ^= 1;
    assert!(matches!(
        decode(&bytes),
        Err(InstallationError::HeaderChecksum)
    ));

    // The total length above the region, which holds 57 bytes.
    let mut bytes = CONTAINER;
    bytes[12..16].copy_from_slice(&58u32.to_le_bytes());
    fix_header_crc(&mut bytes);
    assert!(matches!(
        decode(&bytes),
        Err(InstallationError::TotalLength)
    ));

    // The entry count, 2.
    let mut bytes = CONTAINER;
    bytes[16..20].copy_from_slice(&3u32.to_le_bytes());
    fix_header_crc(&mut bytes);
    assert!(matches!(
        decode(&bytes),
        Err(InstallationError::EntryCount(3))
    ));

    // A kind twice: the record's entry claims to be the program.
    let mut bytes = CONTAINER;
    bytes[36..40].copy_from_slice(&1u32.to_le_bytes());
    fix_header_crc(&mut bytes);
    assert!(matches!(
        decode(&bytes),
        Err(InstallationError::EntryKind(1))
    ));

    // A range outside the total length: the program's entry claims 100 bytes.
    let mut bytes = CONTAINER;
    bytes[28..32].copy_from_slice(&100u32.to_le_bytes());
    fix_header_crc(&mut bytes);
    assert!(matches!(decode(&bytes), Err(InstallationError::EntryRange)));

    // An overlap: the record starts inside the program.
    let mut bytes = CONTAINER;
    bytes[40..44].copy_from_slice(&53u32.to_le_bytes());
    fix_header_crc(&mut bytes);
    assert!(matches!(
        decode(&bytes),
        Err(InstallationError::EntryOverlap)
    ));

    // A payload CRC-32: the program's first byte changes, so entry 1 is refused.
    let mut bytes = CONTAINER;
    bytes[52] ^= 1;
    assert!(matches!(
        decode(&bytes),
        Err(InstallationError::PayloadChecksum(1))
    ));
}

#[test]
fn a_write_stopped_at_any_byte_is_no_installed_program() {
    // The writer's order (specification 34 §5.1, DEPLOY-03): the payloads, then the entry table,
    // then the header fields, and the magic last.
    let segments = [(52usize, 3usize), (55, 2), (20, 32), (8, 12), (0, 8)];
    let mut order = Vec::new();
    for (offset, length) in segments {
        order.extend(offset..offset + length);
    }
    assert_eq!(order.len(), CONTAINER.len());
    for written in 0..=order.len() {
        let mut region = vec![0xff; 64];
        for &offset in &order[..written] {
            region[offset] = CONTAINER[offset];
        }
        let result = decode(&region);
        if written < order.len() {
            assert!(
                matches!(result, Err(InstallationError::NoProgram)),
                "the write stopped after {written} bytes"
            );
        } else {
            let installed = result.expect("the whole container decodes");
            assert_eq!(installed.program, &[1, 2, 3]);
            assert_eq!(installed.record, &[0xaa, 0xbb]);
        }
    }
}
