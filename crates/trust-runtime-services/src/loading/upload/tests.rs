//! The contract tests of an upload written into the program store (specification 34 §5.1): which
//! units a begin and each part erase, which bytes they program, in steps of at most the step budget,
//! and their read-back. The fake flash has the C6's units (4-byte program units, 4 KiB erase units,
//! 256-byte steps) and slots of 16 KiB. Every expected operation is worked out from the
//! specification's order: erase first, then program, then read back; the first 60 bytes are never
//! written.

use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;

use trust_runtime_core::ports::ProgramStore;
use trust_runtime_core::retain::checkpoint::{FlashError, Slot};

use super::StoreWrite;
use crate::loading::install::InstallError;
use crate::tests::program_store::{FlashOp, FlashProbe, SLOT_BYTES};

/// The payload `0, 1, ..., 250, 0, 1, ...` of `len` bytes.
fn pattern(len: usize) -> Vec<u8> {
    (0..len)
        .map(|index| u8::try_from(index % 251).expect("below 251"))
        .collect()
}

/// A flash whose slot B holds zeros, a damaged slot that a write must erase before it programs.
fn zeroed_b() -> FlashProbe {
    FlashProbe::from_slots([
        vec![0xff_u8; SLOT_BYTES as usize],
        vec![0x00_u8; SLOT_BYTES as usize],
    ])
}

/// Runs `write` to its end with `bytes`, and returns the steps it took.
fn run(
    write: &mut StoreWrite,
    store: &mut ProgramStore,
    bytes: &[u8],
) -> Result<usize, InstallError> {
    for steps in 1..=200 {
        if write.step(store, bytes)? {
            return Ok(steps);
        }
    }
    panic!("the write did not end within 200 steps");
}

/// The programs of `len` bytes from `start` in steps of 256 bytes, each padded to whole 4-byte units,
/// followed by their read-backs.
fn program_then_read(slot: Slot, start: u32, len: u32) -> Vec<FlashOp> {
    let mut chunks = Vec::new();
    let mut at = start;
    while at < start + len {
        let end = (start + len).min(at + 256);
        chunks.push((at, end - at));
        at = end;
    }
    let mut ops: Vec<FlashOp> = chunks
        .iter()
        .map(|&(offset, len)| FlashOp::Program {
            slot,
            offset,
            len: (len as usize).next_multiple_of(4),
        })
        .collect();
    ops.extend(chunks.iter().map(|&(offset, len)| FlashOp::Read {
        slot,
        offset,
        len: (len as usize).next_multiple_of(4),
    }));
    ops
}

#[test]
fn a_begin_erases_the_slots_first_unit_and_nothing_else() {
    // Spec 34 §5.1: `transfer.begin` erases the slot's first erase unit, which holds the magic, so
    // the container the slot held is no longer committed. Catches a begin that erases the whole slot
    // (four steps here, 764 on the C6) or none.
    let probe = zeroed_b();
    let mut store = probe.store();
    let mut write = StoreWrite::begin(&store, Slot::B);
    assert_eq!(run(&mut write, &mut store, &[]), Ok(1));
    assert_eq!(
        probe.ops(),
        vec![FlashOp::Erase {
            slot: Slot::B,
            offset: 0
        }]
    );
    let slots = probe.slots();
    assert!(slots[1][..4_096].iter().all(|byte| *byte == 0xff));
    assert!(slots[1][4_096..].iter().all(|byte| *byte == 0x00));
}

#[test]
fn each_part_erases_its_new_unit_programs_after_the_header_and_reads_back() {
    // Spec 34 §5.1: each part is written at its offset in the install's steps: the erase of each unit
    // the part reaches into and no earlier part reached into (the first unit was erased by begin), the
    // program of its bytes in steps of at most 256 bytes, and their read-back. The upload's first 60
    // bytes, the header, are not written. A 9,000-byte upload has parts at 0, 4,096 and 8,192 (808
    // bytes). Catches a part 0 that writes the header, a part that erases a unit twice or programs
    // before its erase, and a last part padded past its unit.
    let probe = zeroed_b();
    let mut store = probe.store();
    let data = pattern(9_000);
    run(&mut StoreWrite::begin(&store, Slot::B), &mut store, &[]).expect("the begin's erase");

    let before = probe.ops().len();
    let mut first = StoreWrite::part(&store, Slot::B, 0, 4_096);
    assert_eq!(run(&mut first, &mut store, &data[..4_096]), Ok(32));
    assert_eq!(
        probe.ops()[before..],
        program_then_read(Slot::B, 60, 4_036)[..]
    );

    let before = probe.ops().len();
    let mut second = StoreWrite::part(&store, Slot::B, 4_096, 4_096);
    assert_eq!(run(&mut second, &mut store, &data[4_096..8_192]), Ok(33));
    let mut expected = vec![FlashOp::Erase {
        slot: Slot::B,
        offset: 4_096,
    }];
    expected.extend(program_then_read(Slot::B, 4_096, 4_096));
    assert_eq!(probe.ops()[before..], expected[..]);

    let before = probe.ops().len();
    let mut last = StoreWrite::part(&store, Slot::B, 8_192, 808);
    assert_eq!(run(&mut last, &mut store, &data[8_192..]), Ok(9));
    let mut expected = vec![FlashOp::Erase {
        slot: Slot::B,
        offset: 8_192,
    }];
    expected.extend(program_then_read(Slot::B, 8_192, 808));
    assert_eq!(probe.ops()[before..], expected[..]);

    let slot = &probe.slots()[1];
    assert!(
        slot[..60].iter().all(|byte| *byte == 0xff),
        "no header byte"
    );
    assert_eq!(&slot[60..9_000], &data[60..]);
    assert!(slot[9_000..12_288].iter().all(|byte| *byte == 0xff));
    assert!(
        slot[12_288..].iter().all(|byte| *byte == 0x00),
        "unit 3 untouched"
    );
}

#[test]
fn a_byte_that_does_not_read_back_refuses_the_part() {
    // Spec 34 PORT-05, the transfer part bullet: a step that fails refuses the part with `the
    // programmed bytes do not read back as written`. Part 0 here finds its unit not erased (no begin
    // ran), so the zeros stay where the part programs. Catches a write that never reads back.
    let probe = zeroed_b();
    let mut store = probe.store();
    let data = pattern(4_096);
    let mut write = StoreWrite::part(&store, Slot::B, 0, 4_096);
    assert_eq!(
        run(&mut write, &mut store, &data),
        Err(InstallError::Verify)
    );
}

#[test]
fn a_flash_failure_ends_the_write_with_the_stores_error() {
    // Spec 34 PORT-05, the transfer part bullet: a failed step refuses the part with the store's
    // failure, `the program store's flash <erase or program> failed`. The second operation, the first
    // program of part 1, fails. Catches a failure the write steps past.
    let probe = zeroed_b();
    let mut store = probe.store();
    probe.fail_at(1);
    let data = pattern(4_096);
    let mut write = StoreWrite::part(&store, Slot::B, 4_096, 4_096);
    let error = run(&mut write, &mut store, &data).expect_err("the program fails");
    assert_eq!(error, InstallError::Flash(FlashError::Program));
    assert_eq!(
        error.to_string(),
        "the program store's flash program failed"
    );
}
