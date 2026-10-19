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
use trust_runtime_core::retain::checkpoint::Slot;
use trust_runtime_core::watchdog::RetainMode;

use super::install::{begin, check, InstallError, Progress};
use super::select::{boot, previous, SelectError};
use crate::installation::{AdmissionError, InstallationError};
use crate::tests::program_store::{
    committed_image, device, packed_container, transfer_of, FlashProbe, PROGRAM, SLOT_BYTES,
};

/// Memory enough for any container of these tests.
const ROOM: usize = 1024 * 1024;

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

/// A flash whose slot A holds `first` from its first byte and whose slot B holds `second` from its
/// first byte, when there is one. The rest of each slot is erased.
fn flash_with(first: &[u8], second: Option<&[u8]>) -> FlashProbe {
    flash_of(Some(first), second)
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
fn the_highest_committed_sequence_boots_in_either_slot() {
    // Specification 34 §5.1 (the program store): the committed container with the highest install
    // sequence is the installed program, in whichever slot it is. A first flash holds sequence 1 in
    // slot A, and the install made after it takes sequence 2 in slot B and then outranks it. Wrong
    // behaviour caught: a boot that takes the first slot or the slot written last; an install over a
    // first flash that writes the slot the running program is in, or takes its sequence from the wrong
    // number.
    let flash = flash_with(&committed_image(1, RetainMode::None), None);
    let booted = boot(&mut flash.store(), &device(), ROOM).expect("a first flash boots");
    assert_eq!(booted.sequence, 1, "a first flash has sequence 1");
    assert_eq!(
        booted.program, PROGRAM,
        "the boot hands over the program alone"
    );
    assert_eq!(
        booted.record.application.program_sha256,
        sha256(PROGRAM),
        "and the record the container holds"
    );

    let mut store = flash.store();
    assert_eq!(install(&mut store, transfer_of(RetainMode::None)), 2);
    let image = committed_image(2, RetainMode::None);
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
        2
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
fn an_erased_store_refuses_the_boot_and_names_the_container_it_reads() {
    // Specification 34 §5.1: an erased store holds no installed program, and the reason names the one
    // magic a slot may hold. Wrong behaviour caught: erased flash read as a container; a reason that
    // still offers `TRSTA001`.
    let flash = FlashProbe::erased();
    let error = boot(&mut flash.store(), &device(), ROOM).expect_err("an erased store has none");
    assert_eq!(error, SelectError::NoContainer);
    assert_eq!(
        error.to_string(),
        "no installed program: no TRSTA002 container in either slot"
    );
}

#[test]
fn a_trsta001_container_is_not_committed() {
    // Specification 34 §5.1 (the program store): only a `TRSTA002` container is committed. A slot whose
    // first eight bytes read `TRSTA001` holds a version-1 container, which is not committed: the boot
    // does not start it and names its slot, whatever else the slot holds; it takes no part in the
    // order; and the next install overwrites it like an erased slot, in slot A with sequence 1. Wrong
    // behaviour caught: a boot that still reads version 1 (0.36 counted it as sequence 0); a refusal
    // that does not say which slot holds the container; a version-1 container counted as the older of
    // two, or one that blocks the install from slot A.
    let version_one = packed_container();
    let reason =
        "slot A holds a TRSTA001 container, which is not committed (only TRSTA002 is read)";

    // In slot A, intact or with a damaged program, the boot is refused and names slot A.
    let mut damaged = version_one.clone();
    damaged[52] ^= 0x01;
    for container in [&version_one, &damaged] {
        let flash = flash_with(container, None);
        let error = boot(&mut flash.store(), &device(), ROOM)
            .expect_err("a TRSTA001 container is not committed");
        assert_eq!(error, SelectError::VersionOne(Slot::A));
        assert_eq!(
            error.to_string(),
            alloc::format!("no installed program: {reason}")
        );
        assert_eq!(error.reason().to_string(), reason);
    }

    // In slot B the reason names slot B, and in both the first one is named.
    let flash = flash_of(None, Some(&version_one));
    let error = boot(&mut flash.store(), &device(), ROOM)
        .expect_err("a TRSTA001 container is not committed");
    assert_eq!(
        error.to_string(),
        "no installed program: slot B holds a TRSTA001 container, which is not committed (only TRSTA002 is read)"
    );
    let flash = flash_with(&version_one, Some(&version_one));
    assert_eq!(
        boot(&mut flash.store(), &device(), ROOM).expect_err("neither is committed"),
        SelectError::VersionOne(Slot::A)
    );

    // Beside a committed container it takes no part in the order, in either slot: the committed
    // container boots, and there is no older program.
    for (first, second) in [
        (version_one.clone(), committed_image(1, RetainMode::None)),
        (committed_image(1, RetainMode::None), version_one.clone()),
    ] {
        let flash = flash_with(&first, Some(&second));
        let mut store = flash.store();
        let booted = boot(&mut store, &device(), ROOM).expect("the committed container boots");
        assert_eq!(booted.sequence, 1);
        assert_eq!(
            previous(&mut store, &device()),
            Ok(None),
            "a version-1 container is not the program before"
        );
    }

    // The next install takes slot A and sequence 1, as in an erased store, and boots.
    let flash = flash_with(&version_one, None);
    let mut store = flash.store();
    assert_eq!(install(&mut store, transfer_of(RetainMode::None)), 1);
    let image = committed_image(1, RetainMode::None);
    assert_eq!(
        &flash.slots()[0][..image.len()],
        image.as_slice(),
        "the install overwrote the version-1 container in slot A"
    );
    assert_eq!(
        boot(&mut store, &device(), ROOM)
            .expect("the install boots")
            .sequence,
        1
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
