//! The `TRSTA001` container's checks (specification 34 §5.1).
//!
//! The container bytes are worked out by hand: the program is `[1, 2, 3]` at offset 52, the
//! record is `[0xaa, 0xbb]` at offset 55, the total length is 57, and the CRC-32 values are
//! CRC-32/ISO-HDLC.

use alloc::vec;
use alloc::vec::Vec;

use trust_runtime_core::bytecode::BytecodeVersion;
use trust_runtime_core::crc32;
use trust_runtime_core::deployment::{
    AdmissionRefusal, ApplicationIdentity, Capabilities, Configuration, Contracts,
    DeploymentRecord, DeploymentRecordError, DeviceDescriptor, NumericContract,
    ProcessImageSummary, RequestedLimits, SchedulingProfile, TimingEvidence,
};
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::vm::PreparationLimits;

use super::{admit, decode, encode, AdmissionError, InstallationError};

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

/// The installed program's bytes: twelve of them. Admission checks only the program's length and
/// SHA-256, so any bytes serve.
const PROGRAM: &[u8] = b"TRSTB002-prg";

/// The chip's descriptor: a computer's maxima with an application slot of `slot_bytes`
/// (specification 34 §5.1.1, COMPAT-02).
fn chip(slot_bytes: u64) -> DeviceDescriptor {
    DeviceDescriptor {
        application_slot_bytes: Some(slot_bytes),
        ..DeviceDescriptor::hosted()
    }
}

/// A version-1 record that describes `program`, requests the default limits and needs nothing
/// else; `DeploymentRecord::encode` turns it into the record's bytes (specification 34 §5.1.1).
fn record_of(program: &[u8]) -> DeploymentRecord {
    DeploymentRecord {
        application: ApplicationIdentity {
            program_sha256: trust_runtime_core::deployment::sha256(program),
            program_bytes: u32::try_from(program.len()).expect("a short program"),
            bytecode_version: BytecodeVersion::SOURCE_FREE,
            bytecode_flags: 0,
            configuration_sha256: [0; 32],
            retain_schema_sha256: [0; 32],
        },
        contracts: Contracts {
            scheduling_profile: SchedulingProfile::COOPERATIVE_RESOURCE_V1,
            numeric_contract: NumericContract::V1,
            timing_evidence: TimingEvidence::UNQUALIFIED,
            capabilities: Capabilities::NONE,
        },
        limits: RequestedLimits::from_preparation(&PreparationLimits::default()),
        process_image: ProcessImageSummary::default(),
        tasks: Vec::new(),
        imports: Vec::new(),
        configuration: Configuration::default(),
    }
}

#[test]
fn an_installed_container_is_admitted_for_the_chip() {
    // Specification 34 §5.1: a chip reads its installed program and record from the one container
    // in its `application` region; §5.1.1 admits it before any state exists. Catches a chip that
    // returns the wrong payload, drops the decoded record or grants other limits.
    let record = record_of(PROGRAM);
    let record_bytes = record.encode().expect("the record encodes");
    let container = encode(PROGRAM, &record_bytes).expect("the container encodes");
    let admitted = admit(&container, &chip(6112 * 1024)).expect("the chip admits it");
    assert_eq!(admitted.program, PROGRAM);
    assert_eq!(admitted.record, record);
    assert_eq!(admitted.limits, PreparationLimits::default());
}

#[test]
fn a_container_of_another_program_is_refused_before_any_state() {
    // Specification 34 §5.1.1, admission step 1: the program must have the length and the SHA-256
    // of the record, refused with `ProgramMismatch` before anything is constructed. Catches a
    // length-only check (the same-length program) and admitting whatever the container holds.
    let record = record_of(PROGRAM);
    let record_bytes = record.encode().expect("the record encodes");
    let mut same_length = PROGRAM.to_vec();
    same_length[0] = b'S';
    let mut longer = PROGRAM.to_vec();
    longer.push(b'!');
    for other in [same_length, longer] {
        let container = encode(&other, &record_bytes).expect("the container encodes");
        assert_eq!(
            admit(&container, &chip(6112 * 1024)),
            Err(AdmissionError::Deployment(RuntimeError::DeploymentRefused(
                AdmissionRefusal::ProgramMismatch
            )))
        );
    }
}

#[test]
fn a_program_too_large_for_the_slot_is_refused_with_its_numbers() {
    // Specification 34 §5.1.1, admission step 8: the program and its record must fit the
    // application slot, refused with the bytes needed and the bytes available. Catches a slot
    // check that is off by one or that drops the numbers.
    let record = record_of(PROGRAM);
    let record_bytes = record.encode().expect("the record encodes");
    // The record is 338 bytes: header 12, seven section headers 56, the seven sections
    // 108 + 8 + 88 + 16 + 4 + 4 + 38, and the CRC 4 (specification 34 §5.1.1).
    assert_eq!(record_bytes.len(), 338);
    // The program is 12 bytes, so the pair needs 350 bytes of the slot; one byte less refuses.
    let needed = u64::try_from(PROGRAM.len() + record_bytes.len()).expect("a small pair");
    assert_eq!(needed, 350);
    let container = encode(PROGRAM, &record_bytes).expect("the container encodes");
    assert_eq!(
        admit(&container, &chip(needed - 1)),
        Err(AdmissionError::Deployment(RuntimeError::DeploymentRefused(
            AdmissionRefusal::ApplicationSlot {
                needed,
                available: needed - 1,
            }
        )))
    );
    // The exact size is admitted, with the program the container holds.
    let admitted = admit(&container, &chip(needed)).expect("the exact fit is admitted");
    assert_eq!(admitted.program, PROGRAM);
}

#[test]
fn an_erased_region_holds_no_program() {
    // Specification 34 §5.1: a region whose first eight bytes are not the magic, an erased region
    // of `0xFF` in particular, holds no installed program; admission reports the container's
    // `NoProgram` rather than a program.
    assert_eq!(
        admit(&[0xff; 64], &chip(6112 * 1024)),
        Err(AdmissionError::Container(InstallationError::NoProgram))
    );
}

#[test]
fn a_container_whose_record_is_unreadable_is_refused_by_the_record_step() {
    // Specification 34 §5.1: the container's two payloads are checked before either is exposed;
    // a program with a record that is not a record is refused by the record's reader before
    // admission, as `RuntimeError::DeploymentRecordInvalid` on a running runtime.
    let container = encode(PROGRAM, b"not a record").expect("the container encodes");
    assert_eq!(
        admit(&container, &chip(6112 * 1024)),
        Err(AdmissionError::Record(DeploymentRecordError::Magic))
    );
}
