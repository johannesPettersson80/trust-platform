//! The deployment record of an image (specification 44 §10.1): every field from the image's own
//! tables, so that admission and the loader see what the image holds.

use crate::deployment::ProgramVersion;
use crate::deployment::{
    sha256, Capabilities, DeploymentRecord, NumericContract, ProcessImageSummary,
    SchedulingProfile, TaskSummary, TimingEvidence,
};
use crate::error::RuntimeError;
use crate::native::image::Target;
use crate::native::program::tests::image;

/// The test image: `MAIN` in the task `Fast` of 10 ms and priority 1, no `SINGLE`, its output bit
/// bound to `%QX0.1` in a one-byte output area, no input or marker byte, nothing retained.
fn test_image() -> alloc::vec::Vec<u8> {
    image(0, 100, 64, Target::X86_64)
}

/// The record names the image by its SHA-256, its length and the compiled-image version, and
/// carries the configuration identity it was given. A record that named STBC 3.0, or the hash of
/// another file, would let a device admit the wrong program.
#[test]
fn the_record_names_the_image() {
    let bytes = test_image();
    let configuration = [7u8; 32];
    let record = DeploymentRecord::for_image(&bytes, configuration).expect("the image describes");
    assert_eq!(record.application.program_sha256, sha256(&bytes));
    assert_eq!(record.application.program_bytes as usize, bytes.len());
    assert_eq!(
        record.application.program_version,
        ProgramVersion::NATIVE_IMAGE
    );
    assert_eq!(
        record.application.program_version,
        ProgramVersion::new(44, 1)
    );
    assert_eq!(record.application.program_flags, 0);
    assert_eq!(record.application.configuration_sha256, configuration);
    // Nothing is retained: the identity is SHA-256 of no bytes (FIPS 180-4).
    assert_eq!(
        record.application.retain_schema_sha256[..4],
        [0xe3, 0xb0, 0xc4, 0x42]
    );
}

/// The process image and the tasks are the image's: one output byte, one binding, the task of 10 ms
/// at priority 1 started by its interval; the record requests that one byte of process image and
/// no preparation limit (specification 44 §10.2, steps 1 and 5). A record with the hosted defaults
/// would make a chip claim the wrong lines or schedule the wrong tasks, and one that requested no
/// process image would pass a device's maximum whatever the program binds.
#[test]
fn the_record_carries_the_process_image_and_the_tasks() {
    let record = DeploymentRecord::for_image(&test_image(), [0; 32]).expect("the image describes");
    assert_eq!(
        record.process_image,
        ProcessImageSummary {
            input_bytes: 0,
            output_bytes: 1,
            marker_bytes: 0,
            bindings: 1,
        }
    );
    assert_eq!(
        record.tasks,
        [TaskSummary {
            interval_nanos: 10_000_000,
            priority: 1,
            event: false,
        }]
    );
    assert_eq!(
        record.contracts.scheduling_profile,
        SchedulingProfile::COOPERATIVE_RESOURCE_V1
    );
    assert_eq!(record.contracts.numeric_contract, NumericContract::V1);
    assert_eq!(
        record.contracts.timing_evidence,
        TimingEvidence::UNQUALIFIED
    );
    assert_eq!(record.contracts.capabilities, Capabilities::NONE);
    assert!(record.imports.is_empty());
    // An image is not prepared: it requests no preparation limit, only its process image.
    for (limit, value) in record.limits.values() {
        let expected = u64::from(limit == crate::deployment::LimitKind::ProcessImageBytes);
        assert_eq!(value, expected, "{limit:?}");
    }
    // The record encodes and decodes as every record does.
    let encoded = record.encode().expect("the record encodes");
    assert_eq!(DeploymentRecord::decode(&encoded), Ok(record));
}

/// A damaged image has no record: a byte of the header changed fails the header's CRC.
#[test]
fn a_damaged_image_has_no_record() {
    let mut bytes = test_image();
    bytes[20] ^= 0xFF;
    assert!(matches!(
        DeploymentRecord::for_image(&bytes, [0; 32]),
        Err(RuntimeError::InvalidBundle(_))
    ));
}
