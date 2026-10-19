//! The check of an install on a chip (specification 34 §11.2, decision G13, and §11.3): the program is
//! prepared first with the limits the heap allows now, and then its warm start is tried with the newest
//! checkpoint. The container is the A4 program (6,412 bytes), whose program `Plant` has the retained
//! DINT `activations`. Every expected value is written out here from the specification: the numbers
//! of the refusal are the program's length, the room and the reserve given to the check.

use alloc::format;
use alloc::vec::Vec;

use smol_str::SmolStr;
use trust_runtime_core::bytecode::{BytecodeModule, ValidationLimits};
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::retain::RetainSnapshot;
use trust_runtime_core::value::Value;
use trust_runtime_core::watchdog::RetainMode;

use super::{check_install, newest_checkpoint_payload_len};
use crate::installation::admit_sequenced;
use crate::runner::engine::DeviceMemory;
use crate::tests::program_store::{committed_image, device, CheckpointProbe};

/// The memory of the device: a 16 KiB region, as the fake program flash's slots, and the 144 KiB heap.
const MEMORY: DeviceMemory = DeviceMemory {
    region_bytes: 16 * 1024,
    heap_bytes: 144 * 1024,
};
/// Room for any program of these tests.
const ROOM: usize = 1 << 20;
/// The bytes the check keeps in reserve.
const RESERVE: usize = 20_000;

fn checkpoint_of(value: Value) -> CheckpointProbe {
    let mut snapshot = RetainSnapshot::default();
    snapshot.insert("@program/Plant/activations", value);
    CheckpointProbe::with_checkpoint(&snapshot)
}

#[test]
fn an_install_the_heap_cannot_hold_is_refused_as_insufficient_memory_with_its_numbers() {
    // Specification 34 §11.2 and §11.3: the check prepares the program with the limits of the heap's
    // room less the reserve, so a program that cannot fit is refused cleanly, before the shared dry
    // run that prepares it with the limits of its record. With 30,000 bytes free and 20,000 kept in
    // reserve, 10,000 remain, which allow no call level, so the program of 6,412 bytes is refused with
    // the numbers. With the heap's room the same program passes. Wrong behaviour caught: a check that
    // prepares the program with the hosted limits of its record, which on a chip's heap resets the chip
    // instead of refusing; numbers that are left out of the refusal.
    let container = committed_image(1, RetainMode::None);
    let admitted = admit_sequenced(&container, &device())
        .expect("the container is admitted")
        .admitted;
    assert_eq!(admitted.program.len(), 6_412);

    let refused = check_install(
        &mut CheckpointProbe::erased(),
        &admitted,
        MEMORY,
        30_000,
        RESERVE,
    );
    assert_eq!(
        refused,
        Err(RuntimeError::ControlError(SmolStr::new(
            "insufficient memory: a program of 6412 bytes needs more than the 10000 bytes of heap \
             that remain for it (30000 bytes are free, 20000 are kept in reserve)"
        )))
    );

    let fits = check_install(
        &mut CheckpointProbe::erased(),
        &admitted,
        MEMORY,
        ROOM,
        RESERVE,
    );
    assert_eq!(fits, Ok(()));
}

#[test]
fn the_retained_values_are_still_tried_after_the_memory_check() {
    // Specification 11 §6.7.1 and 34 §11.2 (decision G13): once the program fits, its warm start is
    // tried with the newest checkpoint as before, so the errors of the retained values are the warm
    // start's own and are not worded as memory. The checkpoint holds `activations` as a BOOL, which a
    // DINT cannot take (type mismatch); the same program with an INT, which migrates, passes; a program
    // that keeps no retained values passes whatever the checkpoint holds. Wrong behaviour caught: a
    // memory check that replaces the retained-value check, or one that words its errors as memory.
    let keeps = committed_image(1, RetainMode::File);
    let admitted = admit_sequenced(&keeps, &device())
        .expect("the container is admitted")
        .admitted;
    let refused = check_install(
        &mut checkpoint_of(Value::Bool(true)),
        &admitted,
        MEMORY,
        ROOM,
        RESERVE,
    );
    assert_eq!(
        refused,
        Err(RuntimeError::RetainMigration(
            "invalid retained value for '@program/Plant/activations': type mismatch".into()
        )),
        "the warm start's own error"
    );

    let migrates = check_install(
        &mut checkpoint_of(Value::Int(7)),
        &admitted,
        MEMORY,
        ROOM,
        RESERVE,
    );
    assert_eq!(migrates, Ok(()));

    let none = committed_image(1, RetainMode::None);
    let admitted = admit_sequenced(&none, &device())
        .expect("the container is admitted")
        .admitted;
    let ignores = check_install(
        &mut checkpoint_of(Value::Bool(true)),
        &admitted,
        MEMORY,
        ROOM,
        RESERVE,
    );
    assert_eq!(ignores, Ok(()));
}

#[test]
fn an_install_check_refuses_a_validation_limit_as_insufficient_memory_with_its_numbers() {
    // Specification 34 §11.2 and §11.3: the check of an install words the validator's limit as it words
    // every other limit of the heap that remains. The analysis of the A4 program peaks at a number of
    // bytes that the test measures; a heap of three halves of that peak less one leaves a scratch
    // limit just below it (two thirds of the heap), so the check meets the validator's limit and not
    // the preparation limit. Wrong behaviour caught: the install refused as `invalid bytecode
    // 'invalid section data: validation analysis storage limit exceeded'`, which names no number.
    let container = committed_image(1, RetainMode::None);
    let admitted = admit_sequenced(&container, &device())
        .expect("the container is admitted")
        .admitted;
    let peak = BytecodeModule::decode(admitted.program)
        .expect("the A4 program decodes")
        .validated_with_limits(ValidationLimits::default())
        .expect("the A4 program validates")
        .stats()
        .peak_scratch_bytes;
    assert!(
        peak > 12_288,
        "the analysis of the A4 program peaks at {peak}"
    );
    let remains = 3 * (peak - 1) / 2;
    let room = remains + RESERVE;

    let refused = check_install(
        &mut CheckpointProbe::erased(),
        &admitted,
        MEMORY,
        room,
        RESERVE,
    );
    assert_eq!(
        refused,
        Err(RuntimeError::ControlError(SmolStr::new(format!(
            "insufficient memory: a program of 6412 bytes needs more than the {remains} bytes of \
             heap that remain for it ({room} bytes are free, {RESERVE} are kept in reserve)"
        ))))
    );
}

#[test]
fn the_newest_checkpoints_payload_length_is_read_from_its_header() {
    // Specification 34 §10.1 and §11.3: the length that a change keeps room for is the payload of the
    // newest committed record, which its header gives. A log with no committed record has nothing to
    // read, so 0. A checkpoint of one DINT is a payload of the length the codec writes for it, and
    // reading the length programs and erases nothing. A damaged record header cannot say how long
    // the payload is, and the answer is the error that a warm start meets. Wrong behaviour caught: a
    // length of 0 for a log that holds a checkpoint; the largest payload for the newest one; a
    // damaged header that reads as an empty log.
    assert_eq!(
        newest_checkpoint_payload_len(&mut CheckpointProbe::erased()),
        Ok(0)
    );

    let mut snapshot = RetainSnapshot::default();
    snapshot.insert("@program/Plant/activations", Value::DInt(7));
    let mut payload = Vec::new();
    snapshot
        .encode_snapshot_payload_into(&mut payload)
        .expect("the snapshot encodes");
    let mut checkpoints = CheckpointProbe::with_checkpoint(&snapshot);
    assert_eq!(
        newest_checkpoint_payload_len(&mut checkpoints),
        Ok(payload.len())
    );
    assert!(payload.len() < 64, "a payload of one value is short");
    assert_eq!(checkpoints.writes(), 0, "the slots are only read");

    // The first record of slot A starts after the 16-byte slot header; byte 48 of its header is the
    // low byte of the payload length, so a flipped bit there fails the header's check while the
    // commit marker after the header still reads as written.
    checkpoints.damage(16 + 48);
    assert!(matches!(
        newest_checkpoint_payload_len(&mut checkpoints),
        Err(RuntimeError::RetainCorruption(_))
    ));
}
