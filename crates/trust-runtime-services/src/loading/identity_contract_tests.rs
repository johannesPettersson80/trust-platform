//! The identities `program.active` reports (specification 34 §11.2), read from the program store in
//! bounded reads. The flash is the fake program flash of `tests/program_store.rs`, which has the C6's
//! units and records every operation. Every expected value is written out here from the specification
//! and the container layout of §5.1 with a program image (specification 44 §10.1: the image at the
//! first multiple of 64 bytes after the 60-byte header), not taken from the code under test.

use alloc::vec;
use alloc::vec::Vec;

use trust_runtime_core::deployment::sha256;
use trust_runtime_core::retain::checkpoint::{FlashError, Slot};
use trust_runtime_core::watchdog::RetainMode;

use super::identity::{identities, IdentityError};
use super::select::ScanError;
use crate::installation::encode_image;
use crate::ports::{ProgramIdentities, ProgramIdentity};
use crate::tests::program_store::{
    committed_image, program, record_bytes, FlashOp, FlashProbe, SLOT_BYTES,
};

/// A flash whose slot A holds `first` from its first byte, when there is one, and whose slot B holds
/// `second` from its first byte, when there is one. The rest of each slot is erased.
fn flash_of(first: Option<&[u8]>, second: Option<&[u8]>) -> FlashProbe {
    let slot = |image: Option<&[u8]>| {
        let mut bytes = vec![0xff_u8; SLOT_BYTES as usize];
        if let Some(image) = image {
            bytes[..image.len()].copy_from_slice(image);
        }
        bytes
    };
    FlashProbe::from_slots([slot(first), slot(second)])
}

/// The identity of a container with install sequence `sequence` whose program is [`program`].
fn identity(sequence: u64) -> ProgramIdentity {
    ProgramIdentity {
        sequence,
        program_sha256: sha256(program()),
        bytes: program().len(),
    }
}

#[test]
fn an_erased_store_reports_no_program() {
    // Specification 34 §11.2: each member is `null` when the store holds no such program. Wrong
    // behaviour caught: an identity made up for a slot that holds no committed container.
    let flash = flash_of(None, None);
    assert_eq!(
        identities(&mut flash.store()),
        Ok(ProgramIdentities {
            active: None,
            previous: None,
        })
    );
}

#[test]
fn a_store_with_one_container_reports_it_as_active_and_no_previous_program() {
    // Specification 34 §11.2: the committed container with the highest sequence is the selected
    // program, wherever it is, and `previous` is `null` while the store holds no other. Slot B holds
    // sequence 7. Wrong behaviour caught: an active container that must be in slot A; a previous
    // program that is the same container again.
    let flash = flash_of(None, Some(&committed_image(7, RetainMode::None)));
    assert_eq!(
        identities(&mut flash.store()),
        Ok(ProgramIdentities {
            active: Some(identity(7)),
            previous: None,
        })
    );
}

#[test]
fn the_identity_comes_from_the_header_and_the_record_in_reads_of_one_step() {
    // Specification 34 §11.2 and §5.1: a chip reads the header of each container (60 bytes), then the
    // record in reads of at most the step budget (256 bytes), and never a byte of the program. The
    // image starts at 64 and is 6,412 bytes long, so its record starts at 6,476, a multiple of the
    // 4-byte program unit. Wrong behaviour caught: a read of the whole container (one read of
    // thousands of bytes), a read that crosses into the program, a record read in one piece.
    let flash = flash_of(
        Some(&committed_image(2, RetainMode::None)),
        Some(&committed_image(5, RetainMode::None)),
    );
    let reported = identities(&mut flash.store()).expect("the identities are read");
    assert_eq!(reported.active, Some(identity(5)));
    assert_eq!(reported.previous, Some(identity(2)));

    let mut record_reads = 0;
    for operation in flash.ops() {
        let FlashOp::Read { offset, len, .. } = operation else {
            panic!("reading identities writes nothing: {operation:?}");
        };
        assert!(len <= 256, "a read of {len} bytes is above the step budget");
        if offset >= 6_476 {
            record_reads += 1;
        } else {
            assert_eq!(
                (offset, len),
                (0, 60),
                "a read before the record is a header read"
            );
        }
    }
    assert!(
        record_reads >= 2,
        "the record of each container was read ({record_reads} reads)"
    );
}

#[test]
fn a_record_that_starts_inside_a_program_unit_is_read_whole() {
    // Specification 34 §5.1 and specification 44 §10.1: the image starts at byte 64 and the record
    // follows it with no padding, so an image of 3 bytes puts the record at byte 67, inside the 4-byte
    // unit that starts at 64. The flash reads whole units at multiples of the unit, so the read starts
    // at 64 and the bytes before the record are dropped. The record names the SHA-256 of `program()`,
    // and the entry table gives the length 3.
    // Wrong behaviour caught: a read at an unaligned offset (the flash refuses it), a record that
    // keeps the unit's first three bytes, a length taken from the record.
    let record = record_bytes(RetainMode::None);
    let container = encode_image(4, &[1, 2, 3], &record).expect("the container encodes");
    let flash = flash_of(Some(&container), None);
    assert_eq!(
        identities(&mut flash.store()),
        Ok(ProgramIdentities {
            active: Some(ProgramIdentity {
                sequence: 4,
                program_sha256: sha256(program()),
                bytes: 3,
            }),
            previous: None,
        })
    );
}

#[test]
fn a_container_whose_record_is_damaged_reports_null_and_does_not_hide_the_other() {
    // Specification 34 §11.2: a member is `null` when the container's record fails its CRC-32. The
    // record of the active container (sequence 5, slot B) has one bit flipped at byte 6,484 (the
    // record starts at 6,476), while `previous` is whole. Wrong behaviour caught: an identity read
    // from a record that fails its check; an error that hides the other container.
    let mut damaged = committed_image(5, RetainMode::None);
    damaged[6_484] ^= 0x01;
    let flash = flash_of(Some(&committed_image(2, RetainMode::None)), Some(&damaged));
    assert_eq!(
        identities(&mut flash.store()),
        Ok(ProgramIdentities {
            active: None,
            previous: Some(identity(2)),
        })
    );
}

#[test]
fn two_committed_containers_with_one_sequence_are_an_error() {
    // Specification 34 §5.1: neither is the newest, so the identities cannot be told. Wrong behaviour
    // caught: an arbitrary pick between the two.
    let flash = flash_of(
        Some(&committed_image(3, RetainMode::None)),
        Some(&committed_image(3, RetainMode::None)),
    );
    assert_eq!(
        identities(&mut flash.store()),
        Err(IdentityError::Scan(ScanError::SequenceTie(3)))
    );
}

#[test]
fn a_flash_that_cannot_be_read_is_an_error_with_the_failing_read() {
    // Specification 34 §11.2: a store whose flash cannot be read answers with that error. The reads
    // are: 0 and 1 the two headers that order the slots, 2 the active container's header, 3 the first
    // read of its record. Failing read 1 fails the ordering; failing read 3 fails the record. Wrong
    // behaviour caught: a failed read taken for an erased slot, which would report no program.
    for (failing, expected) in [
        (1, IdentityError::Scan(ScanError::Flash(FlashError::Read))),
        (3, IdentityError::Flash(FlashError::Read)),
    ] {
        let flash = flash_of(
            Some(&committed_image(2, RetainMode::None)),
            Some(&committed_image(5, RetainMode::None)),
        );
        flash.fail_at(failing);
        assert_eq!(
            identities(&mut flash.store()),
            Err(expected),
            "read {failing} fails"
        );
        let reads: Vec<FlashOp> = flash.ops();
        assert_eq!(
            reads.len(),
            failing + 1,
            "nothing is read after the failure"
        );
        assert!(matches!(reads[0], FlashOp::Read { slot: Slot::A, .. }));
    }
}
