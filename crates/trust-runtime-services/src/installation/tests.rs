//! The `TRSTA001` and `TRSTA002` containers' checks (specification 34 §5.1).
//!
//! The container bytes are worked out by hand: the program is `[1, 2, 3]` at offset 52, the
//! record is `[0xaa, 0xbb]` at offset 55, the total length is 57, and the CRC-32 values are
//! CRC-32/ISO-HDLC. A `TRSTA002` container of the same payloads starts them at offset 60 and is
//! 65 bytes long.

use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;

use trust_runtime_core::crc32;
use trust_runtime_core::deployment::{
    AdmissionRefusal, ApplicationIdentity, Capabilities, Configuration, Contracts,
    DeploymentRecord, DeploymentRecordError, DeviceDescriptor, NumericContract,
    ProcessImageSummary, ProgramVersion, RequestedLimits, SchedulingProfile, TimingEvidence,
};
use trust_runtime_core::error::RuntimeError;

use super::{
    admit, admit_sequenced, committed_header, decode, decode_sequenced, encode, encode_image,
    encode_sequenced, sequenced_layout, set_sequence, AdmissionError, InstallationError,
    ProgramFormat, SequencedHeader, SequencedLayout,
};

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

/// The chip's descriptor: a computer's maxima with an application slot of `slot_bytes`, running
/// compiled program images (specification 34 §5.1.1, COMPAT-02; specification 44 §10.1).
fn chip(slot_bytes: u64) -> DeviceDescriptor {
    DeviceDescriptor {
        program_versions: &[ProgramVersion::NATIVE_IMAGE],
        application_slot_bytes: Some(slot_bytes),
        ..DeviceDescriptor::hosted()
    }
}

/// A version-1 record that describes `program` as a compiled program image (version 44.1),
/// requests no limit and needs nothing else; `DeploymentRecord::encode` turns it into the record's
/// bytes (specification 34 §5.1.1; specification 44 §10.1).
fn record_of(program: &[u8]) -> DeploymentRecord {
    DeploymentRecord {
        application: ApplicationIdentity {
            program_sha256: trust_runtime_core::deployment::sha256(program),
            program_bytes: u32::try_from(program.len()).expect("a short program"),
            program_version: ProgramVersion::NATIVE_IMAGE,
            program_flags: 0,
            configuration_sha256: [0; 32],
            retain_schema_sha256: [0; 32],
        },
        contracts: Contracts {
            scheduling_profile: SchedulingProfile::COOPERATIVE_RESOURCE_V1,
            numeric_contract: NumericContract::V1,
            timing_evidence: TimingEvidence::UNQUALIFIED,
            capabilities: Capabilities::NONE,
        },
        limits: RequestedLimits {
            max_artifact_bytes: 0,
            max_preparation_bytes: 0,
            max_preparation_work: 0,
            max_validation_scratch_bytes: 0,
            max_validation_work: 0,
            max_construction_values: 0,
            max_construction_bytes: 0,
            max_process_image_bytes: 0,
            max_work: 0,
            max_construction_work: 0,
            max_call_depth: 0,
        },
        process_image: ProcessImageSummary::default(),
        tasks: Vec::new(),
        imports: Vec::new(),
        configuration: Configuration::default(),
    }
}

/// [`record_of`] for an STBC program of the bytecode engine: the record names STBC 3.0.
fn stbc_record_of(program: &[u8]) -> Vec<u8> {
    let mut record = record_of(program);
    record.application.program_version = ProgramVersion::new(3, 0);
    record.encode().expect("the record encodes")
}

#[test]
fn an_installed_container_is_admitted_for_the_chip() {
    // Specification 34 §5.1: a chip reads its installed program and record from the one container
    // in its `application` region; §5.1.1 admits it before any state exists. Catches a chip that
    // returns the wrong payload or drops the decoded record.
    let record = record_of(PROGRAM);
    let record_bytes = record.encode().expect("the record encodes");
    let container = encode_image(1, PROGRAM, &record_bytes).expect("the container encodes");
    let admitted = admit_sequenced(&container, &chip(6112 * 1024))
        .expect("the chip admits it")
        .admitted;
    assert_eq!(admitted.format, ProgramFormat::Image);
    assert_eq!(admitted.program, PROGRAM);
    assert_eq!(admitted.record, record);
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
        let container = encode_image(1, &other, &record_bytes).expect("the container encodes");
        assert_eq!(
            admit_sequenced(&container, &chip(6112 * 1024)),
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
    // The record is 352 bytes: header 12, eight section headers 64, the eight sections
    // 108 + 8 + 88 + 16 + 4 + 4 + 42 + 2 (section 8, the parameters of no driver), and the CRC 4
    // (specification 34 §5.1.1, version 2).
    assert_eq!(record_bytes.len(), 352);
    // The program is 12 bytes, so the pair needs 364 bytes of the slot; one byte less refuses.
    let needed = u64::try_from(PROGRAM.len() + record_bytes.len()).expect("a small pair");
    assert_eq!(needed, 364);
    let container = encode_image(1, PROGRAM, &record_bytes).expect("the container encodes");
    assert_eq!(
        admit_sequenced(&container, &chip(needed - 1)),
        Err(AdmissionError::Deployment(RuntimeError::DeploymentRefused(
            AdmissionRefusal::ApplicationSlot {
                needed,
                available: needed - 1,
            }
        )))
    );
    // The exact size is admitted, with the program the container holds.
    let admitted = admit_sequenced(&container, &chip(needed)).expect("the exact fit is admitted");
    assert_eq!(admitted.admitted.program, PROGRAM);
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

/// A `TRSTA002` container of the STBC program `PROGRAM` and its record, written with the install
/// sequence `sequence`.
fn sequenced_container(sequence: u64) -> Vec<u8> {
    encode_sequenced(sequence, PROGRAM, &stbc_record_of(PROGRAM)).expect("the container encodes")
}

#[test]
fn a_trsta002_container_holds_its_sequence_at_byte_16_and_its_payloads_from_byte_60() {
    // Specification 34 §5.1: the install sequence is the eight bytes at offset 16, the entry count
    // is at 24, and the payloads start at byte 60. Worked out by hand: 60 header bytes, 3 program
    // bytes and 2 record bytes make 65 bytes. Catches a header one field too long or too short.
    let bytes = encode_sequenced(5, &[1, 2, 3], &[0xaa, 0xbb]).expect("the container encodes");
    assert_eq!(bytes.len(), 65);
    assert_eq!(&bytes[0..8], b"TRSTA002");
    assert_eq!(&bytes[16..24], &5u64.to_le_bytes());
    assert_eq!(&bytes[24..28], &2u32.to_le_bytes());
    assert_eq!(&bytes[60..63], &[1, 2, 3]);
    let decoded = decode_sequenced(&bytes).expect("the container decodes");
    assert_eq!(decoded.sequence, 5);
    assert_eq!(decoded.installed.program, &[1, 2, 3]);
    assert_eq!(decoded.installed.record, &[0xaa, 0xbb]);
}

#[test]
fn the_header_crc_covers_the_sequence() {
    // Specification 34 §5.1 and DEPLOY-03: the header CRC-32 covers the install sequence, so a
    // sequence bit that changes fails the header check, and the container is not taken for the one
    // written. Catches a sequence left outside the CRC, which would let a damaged slot select
    // another sequence.
    let mut bytes = sequenced_container(2);
    assert_eq!(
        decode_sequenced(&bytes)
            .expect("the container decodes")
            .sequence,
        2
    );
    bytes[16] ^= 1;
    assert!(matches!(
        decode_sequenced(&bytes),
        Err(InstallationError::HeaderChecksum)
    ));
}

#[test]
fn the_sequence_is_written_under_a_fresh_header_crc() {
    // Specification 34 §5.1: the device gives each commit its sequence, written into the header
    // under a fresh header CRC-32, so the payloads decode as before. Catches a sequence written
    // without its CRC, which the check would refuse as a header fault.
    let mut bytes = sequenced_container(0);
    set_sequence(&mut bytes, 7).expect("the sequence is set");
    let decoded = decode_sequenced(&bytes).expect("the container with sequence 7 decodes");
    assert_eq!(decoded.sequence, 7);
    assert_eq!(decoded.installed.program, PROGRAM);
}

#[test]
fn a_slot_header_is_committed_only_with_its_magic_crc_and_length() {
    // Specification 34 §5.1: a slot is committed when its magic reads TRSTA002, its header CRC-32
    // matches and its total length lies inside the slot. The container is 65 bytes long, so a
    // slot of 64 bytes does not hold it and a slot of 65 bytes does. Catches a length check that is
    // off by one, or a damaged header or a version-1 magic taken for a committed container.
    let bytes = encode_sequenced(5, &[1, 2, 3], &[0xaa, 0xbb]).expect("the container encodes");
    let header = SequencedHeader {
        total_len: 65,
        sequence: 5,
    };
    assert_eq!(committed_header(&bytes[..60], 65), Some(header));
    assert_eq!(committed_header(&bytes[..60], 64), None);
    let mut damaged = bytes.clone();
    damaged[16] ^= 1;
    assert_eq!(committed_header(&damaged[..60], 65), None);
    let version_one = encode(&[0; 10], &[0; 4]).expect("the container encodes");
    assert_eq!(committed_header(&version_one[..60], 1000), None);
}

#[test]
fn a_trsta001_container_still_reads() {
    // Specification 34 §5.1: the boot reads TRSTA002 only, and the version-1 decoder stays for the
    // tests that build an old container, so the version-1 container still decodes to its payloads
    // and the version-2 reader refuses it as no program. Catches a shared layout that moves the version-1 entries, or one that takes the
    // version-1 magic for version 2.
    let installed = decode(&CONTAINER).expect("the hand-built container decodes");
    assert_eq!(installed.program, &[1, 2, 3]);
    assert_eq!(installed.record, &[0xaa, 0xbb]);
    assert!(matches!(
        decode_sequenced(&CONTAINER),
        Err(InstallationError::NoProgram)
    ));
}

#[test]
fn the_layout_of_a_trsta002_container_is_read_from_its_header_alone() {
    // Specification 34 §5.1 and §11.2 (`program.active`): the 60 header bytes name the program's
    // length and the record's place. Worked out by hand for the container above: the program takes
    // bytes 60 to 63 (3 bytes), the record bytes 63 to 65, and the record's CRC-32 is 0x49822c98
    // (the value the version-1 container above carries for the same two bytes). Catches a layout
    // that reads the entries of version 1 (4 bytes too early), one that swaps the two payloads, and
    // a record CRC-32 taken from the program.
    let bytes = encode_sequenced(5, &[1, 2, 3], &[0xaa, 0xbb]).expect("the container encodes");
    assert_eq!(
        sequenced_layout(&bytes[..60], 65),
        Ok(SequencedLayout {
            program_bytes: 3,
            record: 63..65,
            record_crc: 0x4982_2c98,
        })
    );
}

#[test]
fn a_layout_is_refused_for_a_header_that_fails_a_check_of_the_container() {
    // Specification 34 §5.1: the header checks of the whole container hold for the header alone, in
    // their order. A flipped sequence bit fails the header CRC-32; a total length below what the
    // header declares (64 of 65) fails the length check; a version-1 header has no TRSTA002 magic;
    // 59 bytes end inside the header. Catches a layout that trusts a header the container check
    // would refuse.
    let bytes = encode_sequenced(5, &[1, 2, 3], &[0xaa, 0xbb]).expect("the container encodes");
    let mut damaged = bytes.clone();
    damaged[16] ^= 1;
    assert_eq!(
        sequenced_layout(&damaged[..60], 65),
        Err(InstallationError::HeaderChecksum)
    );
    assert_eq!(
        sequenced_layout(&bytes[..60], 64),
        Err(InstallationError::TotalLength)
    );
    assert_eq!(
        sequenced_layout(&CONTAINER[..52], 57),
        Err(InstallationError::NoProgram)
    );
    assert_eq!(
        sequenced_layout(&bytes[..59], 65),
        Err(InstallationError::Truncated)
    );
}

/// The bytes of a stand-in compiled program image. Admission reads only their length and SHA-256.
const IMAGE: &[u8] = b"TRSTI001-image-bytes";

/// [`record_of`] for a compiled program image, as its bytes.
fn image_record_of(image: &[u8]) -> Vec<u8> {
    record_of(image).encode().expect("the record encodes")
}

/// A chip that runs compiled program images.
fn image_chip() -> DeviceDescriptor {
    chip(6112 * 1024)
}

#[test]
fn an_image_lies_in_its_container_at_a_64_byte_boundary() {
    // Specification 44 §10.1 and §5.3: entry kind 3 names a compiled program image, which starts at
    // the first multiple of 64 after the 60-byte header, with zeros between, so that an image read
    // where it lies keeps its sections' alignment; the record follows the image. Worked out by
    // hand: the 20 image bytes lie at 64 to 84 and the record from 84. Catches an image at byte 60,
    // where its code would lose its 64-byte alignment, and a kind the reader takes for STBC.
    let record = image_record_of(IMAGE);
    let bytes = encode_image(5, IMAGE, &record).expect("the container encodes");
    assert_eq!(bytes.len(), 84 + record.len());
    assert_eq!(&bytes[28..32], &3u32.to_le_bytes());
    assert_eq!(&bytes[32..36], &64u32.to_le_bytes());
    assert_eq!(&bytes[36..40], &20u32.to_le_bytes());
    assert_eq!(&bytes[44..48], &2u32.to_le_bytes());
    assert_eq!(&bytes[48..52], &84u32.to_le_bytes());
    assert_eq!(&bytes[60..64], &[0; 4]);
    assert_eq!(&bytes[64..84], IMAGE);
    let decoded = decode_sequenced(&bytes).expect("the container decodes");
    assert_eq!(decoded.sequence, 5);
    assert_eq!(decoded.installed.format, ProgramFormat::Image);
    assert_eq!(decoded.installed.program, IMAGE);
    assert_eq!(decoded.installed.record, record.as_slice());
    let total = u32::try_from(bytes.len()).expect("a short container");
    assert_eq!(
        sequenced_layout(&bytes[..60], total),
        Ok(SequencedLayout {
            program_bytes: 20,
            record: 84..bytes.len(),
            record_crc: crc32::checksum(&record),
        })
    );
    // An STBC container keeps its program at byte 60.
    let stbc = sequenced_container(5);
    let decoded = decode_sequenced(&stbc).expect("the container decodes");
    assert_eq!(decoded.installed.format, ProgramFormat::Bytecode);
    assert_eq!(&stbc[32..36], &60u32.to_le_bytes());
}

#[test]
fn a_container_of_two_programs_is_refused() {
    // Specification 44 §10.1: a container holds one program and its record. An image entry next to
    // an STBC entry is refused by the entry check before any payload is read. Catches a reader that
    // takes either entry for the program.
    let mut bytes = encode_image(1, IMAGE, &image_record_of(IMAGE)).expect("the container encodes");
    bytes[44..48].copy_from_slice(&1u32.to_le_bytes());
    let crc = crc32::checksum(&bytes[12..60]);
    bytes[8..12].copy_from_slice(&crc.to_le_bytes());
    assert!(matches!(
        decode_sequenced(&bytes),
        Err(InstallationError::EntryKind(1))
    ));
}

#[test]
fn an_image_and_its_record_name_the_same_kind_of_program() {
    // Specification 44 §10.1: the record of an image names the compiled-image version, and the
    // record of STBC an STBC version. A container whose record names the other kind describes
    // another program and is refused before any state exists, as a record of another program is
    // (specification 34 §5.1.1 step 1). Catches a device that runs an image as STBC or the reverse.
    let refused = Err(AdmissionError::Deployment(RuntimeError::DeploymentRefused(
        AdmissionRefusal::ProgramMismatch,
    )));
    let stbc_record = stbc_record_of(IMAGE);
    let image_with_stbc_record =
        encode_image(1, IMAGE, &stbc_record).expect("the container encodes");
    assert_eq!(
        admit_sequenced(&image_with_stbc_record, &image_chip()),
        refused
    );
    let stbc_with_image_record =
        encode_sequenced(1, IMAGE, &image_record_of(IMAGE)).expect("the container encodes");
    assert_eq!(
        admit_sequenced(&stbc_with_image_record, &image_chip()),
        refused
    );
    let image = encode_image(1, IMAGE, &image_record_of(IMAGE)).expect("the container encodes");
    let admitted = admit_sequenced(&image, &image_chip()).expect("the image is admitted");
    assert_eq!(admitted.admitted.format, ProgramFormat::Image);
    assert_eq!(admitted.admitted.program, IMAGE);
}

#[test]
fn a_device_that_runs_no_images_refuses_one() {
    // Specification 34 §5.1.1 step 2 with specification 44 §10.1: a device whose descriptor does not
    // name the compiled-image version refuses an image, and says so in words rather than as an STBC
    // version. Catches a refusal that reads "STBC 44.1".
    let image = encode_image(1, IMAGE, &image_record_of(IMAGE)).expect("the container encodes");
    let no_images = DeviceDescriptor {
        program_versions: &[],
        ..chip(6112 * 1024)
    };
    let refusal = admit_sequenced(&image, &no_images).expect_err("the chip runs no image");
    assert_eq!(
        refusal,
        AdmissionError::Deployment(RuntimeError::DeploymentRefused(
            AdmissionRefusal::ProgramVersion {
                major: 44,
                minor: 1,
            }
        ))
    );
    assert_eq!(
        refusal.to_string(),
        "deployment refused: the device does not run compiled program images"
    );
}
