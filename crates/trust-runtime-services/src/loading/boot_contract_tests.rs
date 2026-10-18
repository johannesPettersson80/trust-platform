//! The boot of the program store (specification 34 §5.1, DEPLOY-03): which container a device starts,
//! what it hands over, and when it refuses. The flash is the fake program flash of
//! `tests/program_store.rs`, which has the C6's units. Every expected value is written out here from
//! the specification, not taken from the code under test.

use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;

use trust_runtime_core::deployment::sha256;
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::ports::ProgramStore;
use trust_runtime_core::watchdog::RetainMode;

use super::install::{begin, check, InstallError, Progress};
use super::select::{boot, SelectError};
use crate::installation::{AdmissionError, InstallationError};
use crate::tests::program_store::{
    committed_image, device, packed_container, transfer_of, FlashProbe, PROGRAM, SLOT_BYTES,
};

/// Memory enough for any container of these tests.
const ROOM: usize = 1024 * 1024;

/// A flash whose slot A holds `first` from its first byte and whose slot B holds `second` from its
/// first byte, when there is one. The rest of each slot is erased.
fn flash_with(first: &[u8], second: Option<&[u8]>) -> FlashProbe {
    let slot = |image: Option<&[u8]>| {
        let mut bytes = vec![0xff_u8; SLOT_BYTES as usize];
        if let Some(image) = image {
            bytes[..image.len()].copy_from_slice(image);
        }
        bytes
    };
    FlashProbe::from_slots([slot(Some(first)), slot(second)])
}

/// Installs `container` to its commit point, one step at a time, and returns the sequence it took.
fn install(store: &mut ProgramStore, container: Vec<u8>) -> u64 {
    let mut pending = begin(store, container, &device(), |_| Ok(())).expect("the install begins");
    loop {
        match pending.step(store).expect("the step runs") {
            Progress::Pending(next) => pending = next,
            Progress::Committed(committed) => return committed.sequence,
        }
    }
}

#[test]
fn the_highest_committed_sequence_boots_and_a_packed_container_counts_as_sequence_0() {
    // Specification 34 §5.1 (the program store): the committed container with the highest install
    // sequence is the installed program, in whichever slot it is, and a `TRSTA001` container, which
    // the bench packer writes and which has no sequence, counts as committed with sequence 0 until
    // S4-7 removes it. Wrong behaviour caught: a boot that takes the first slot or the slot written
    // last; a packed container that is not recognised, so a bench device boots nothing; a packed
    // container that outranks a committed install; an install over a packed container that takes
    // its sequence from the wrong number.
    let flash = flash_with(&packed_container(), None);
    let booted = boot(&mut flash.store(), &device(), ROOM).expect("a packed container boots");
    assert_eq!(booted.sequence, 0, "a packed container has sequence 0");
    assert_eq!(
        booted.program, PROGRAM,
        "the boot hands over the program alone"
    );
    assert_eq!(
        booted.record.application.program_sha256,
        sha256(PROGRAM),
        "and the record the container holds"
    );

    // An install made after it takes sequence 1 in the other slot, and then outranks it.
    let mut store = flash.store();
    assert_eq!(install(&mut store, transfer_of(RetainMode::None)), 1);
    let image = committed_image(1, RetainMode::None);
    let slots = flash.slots();
    assert_eq!(
        &slots[1][..image.len()],
        image.as_slice(),
        "the install wrote slot B, since slot A holds the newest committed container"
    );
    assert_eq!(
        boot(&mut store, &device(), ROOM)
            .expect("the install boots")
            .sequence,
        1
    );

    // The highest sequence wins in either slot.
    for (first, second) in [(5, 2), (2, 5)] {
        let flash = flash_with(
            &committed_image(first, RetainMode::None),
            Some(&committed_image(second, RetainMode::None)),
        );
        let booted = boot(&mut flash.store(), &device(), ROOM).expect("a container boots");
        assert_eq!(
            booted.sequence, 5,
            "slot A holds sequence {first} and slot B {second}"
        );
    }

    // A packed container is older than a committed install, also when it is in slot A.
    let flash = flash_with(
        &packed_container(),
        Some(&committed_image(1, RetainMode::None)),
    );
    let booted = boot(&mut flash.store(), &device(), ROOM).expect("a container boots");
    assert_eq!(
        booted.sequence, 1,
        "sequence 1 outranks the packed container"
    );
}

#[test]
fn a_damaged_newest_container_refuses_the_boot() {
    // Specification 34 §5.1 and DEPLOY-03 (decision Q12): the newest committed container fails its
    // payload check, so the boot is refused with `no installed program: <reason>`, and the older
    // container in the other slot is not started in its place. Byte 60 is the first byte of the
    // program payload of a `TRSTA002` container. Wrong behaviour caught: a boot that falls back to the
    // other slot; a refusal without its reason; a reason that repeats `no installed program`.
    let mut newest = committed_image(2, RetainMode::None);
    newest[60] ^= 0x01;
    let flash = flash_with(
        &committed_image(1, RetainMode::None),
        Some(newest.as_slice()),
    );
    let error = boot(&mut flash.store(), &device(), ROOM)
        .expect_err("the damaged newest container refuses the boot");
    assert_eq!(
        error,
        SelectError::Refused(AdmissionError::Container(
            InstallationError::PayloadChecksum(1)
        ))
    );
    assert_eq!(
        error.to_string(),
        "no installed program: the CRC-32 of entry kind 1 does not match"
    );
    assert_eq!(
        error.reason().to_string(),
        "the CRC-32 of entry kind 1 does not match",
        "a device without a program names this reason when it refuses a restart"
    );

    // A damaged packed container is a damaged committed container as well.
    let mut packed = packed_container();
    packed[52] ^= 0x01;
    let flash = flash_with(&packed, None);
    let error = boot(&mut flash.store(), &device(), ROOM)
        .expect_err("the damaged packed container refuses the boot");
    assert_eq!(
        error.to_string(),
        "no installed program: the CRC-32 of entry kind 1 does not match"
    );
}

#[test]
fn a_container_longer_than_the_room_refuses_the_boot_with_its_numbers() {
    // Specification 34 §5.1: a container longer than the memory the boot has for it refuses the boot
    // before anything is read, with the container's length and the room. The container is 60 header
    // bytes and its two payloads. Wrong behaviour caught: a bound that is one byte off; a boot that
    // reads a container the memory cannot hold.
    let image = committed_image(1, RetainMode::None);
    let flash = flash_with(&image, None);
    let room = image.len() - 1;
    let error = boot(&mut flash.store(), &device(), room)
        .expect_err("a container one byte longer than the room is refused");
    assert_eq!(
        error,
        SelectError::NoRoom {
            needed: image.len(),
            room
        }
    );
    assert_eq!(
        error.to_string(),
        alloc::format!(
            "no installed program: the installed container needs {} bytes, more than the heap's {room} bytes",
            image.len()
        )
    );
    let reads = flash.ops().len();
    assert_eq!(reads, 2, "only the two slot headers were read");
    assert!(
        boot(&mut flash.store(), &device(), image.len()).is_ok(),
        "a container as long as the room boots"
    );
}

#[test]
fn an_erased_store_refuses_the_boot_and_names_both_containers() {
    // Specification 34 §5.1: an erased store holds no installed program, and the reason names the two
    // magics a slot may hold. Wrong behaviour caught: erased flash read as a container.
    let flash = FlashProbe::erased();
    let error = boot(&mut flash.store(), &device(), ROOM).expect_err("an erased store has none");
    assert_eq!(error, SelectError::NoContainer);
    assert_eq!(
        error.to_string(),
        "no installed program: no TRSTA001 or TRSTA002 container in either slot"
    );
}

#[test]
fn a_check_that_could_not_run_is_not_a_failure_to_restore_retained_values() {
    // Specification 11 §6.7.1 and 34 §11.2 (decision G13): a retained-value error of the check is the
    // retained values that cannot be restored, with the warm start's own error; any other error of the
    // check means that it could not run, for example because the new program could not be built, and it
    // keeps its own text. Wrong behaviour caught: a candidate that cannot be built reported as retained
    // values that cannot be restored.
    let container = transfer_of(RetainMode::None);
    let restore = check(&container, &device(), |_| {
        Err(RuntimeError::RetainMigration(
            "invalid retained value for 'r'".into(),
        ))
    })
    .expect_err("the warm start refuses");
    assert!(matches!(restore, InstallError::Retain(_)));
    assert_eq!(
        restore.to_string(),
        "the retained values cannot be restored: retain migration error 'invalid retained value for 'r''"
    );

    let build = check(&container, &device(), |_| {
        Err(RuntimeError::PreparationLimit)
    })
    .expect_err("the program cannot be built");
    assert!(matches!(build, InstallError::RetainCheck(_)));
    assert_eq!(
        build.to_string(),
        "the retained-value check could not run: preparation resource limit exceeded"
    );
}
