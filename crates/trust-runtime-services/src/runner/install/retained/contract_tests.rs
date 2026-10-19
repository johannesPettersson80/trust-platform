//! The check of an install on a chip (specification 34 §11.2, decision G13, and §11.3): the program is
//! prepared first with the limits the heap allows now, and then its warm start is tried with the newest
//! checkpoint. The container is the A4 program (6,412 bytes), whose program `Plant` has the retained
//! DINT `activations`. Every expected value is written out here from the specification: the numbers
//! of the refusal are the program's length, the room and the reserve given to the check.

use smol_str::SmolStr;
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::retain::RetainSnapshot;
use trust_runtime_core::value::Value;
use trust_runtime_core::watchdog::RetainMode;

use super::check_install;
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
