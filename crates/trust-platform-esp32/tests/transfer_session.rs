//! The C6's transfer session (specification 34, PORT-05): its boot number comes from the boot counter
//! that the reset record keeps across a reset, and its limits are the program slot for an upload and
//! the heap for a reply. The counter is the fake store of the host test below, in ordinary memory that
//! outlives a simulated reset, as the C6's persistent RTC fast memory outlives a real one. Every
//! expected value is worked out by hand from the specification.

use trust_platform_esp32::profile::{transfer_session, HEAP_CAPACITY_BYTES};
use trust_platform_esp32::program_flash::SLOT_BYTES;
use trust_platform_memory::reset::{note_boot, ResetStore, RECORD_WORDS};
use trust_runtime_core::deployment::sha256;
use trust_runtime_services::link::DEVICE_FRAME_LEN;
use trust_runtime_services::transfer::{ReplyParts, TransferError};

/// The reset memory of a chip in ordinary RAM: the record words and the boot counter, which a
/// simulated reset leaves alone.
struct SurvivingMemory {
    record: [u32; RECORD_WORDS],
    boot_count: u32,
}

impl ResetStore for SurvivingMemory {
    fn record_words(&self) -> [u32; RECORD_WORDS] {
        self.record
    }

    fn set_record_words(&mut self, words: [u32; RECORD_WORDS]) {
        self.record = words;
    }

    fn boot_count(&self) -> u32 {
        self.boot_count
    }

    fn set_boot_count(&mut self, count: u32) {
        self.boot_count = count;
    }
}

/// The first transfer id of the session of the boot that `memory` counts now: the firmware counts the
/// boot with `note_boot` and builds the session with the count.
fn first_transfer_id(memory: &mut SurvivingMemory) -> String {
    let mut session = transfer_session(note_boot(memory));
    session
        .begin(5, sha256(b"hello"))
        .expect("an upload of five bytes begins")
        .to_string()
}

/// Specification 34, PORT-05: a transfer id is 16 hexadecimal digits, the boot number in the high 32
/// bits and a sequence within the boot in the low 32 bits, and an id never repeats across boots. The
/// boot number is the boot counter of the reset record, which rises by one at every reset and survives
/// it (the C6 keeps it in persistent RTC fast memory). The first boot counts 1 and its first id is
/// `0000000100000001`; after a reset the counter is 2 and the first id is `0000000200000001`; after a
/// third, `0000000300000001`. Catches a session built with a boot number that does not rise, which
/// would let an id of a transfer lost in a reset name the new boot's transfer, and a boot number put
/// in the low half of the id.
#[test]
fn transfer_ids_differ_after_a_reset() {
    let mut memory = SurvivingMemory {
        record: [0; RECORD_WORDS],
        boot_count: 0,
    };
    assert_eq!(first_transfer_id(&mut memory), "0000000100000001");
    assert_eq!(first_transfer_id(&mut memory), "0000000200000001");
    assert_eq!(first_transfer_id(&mut memory), "0000000300000001");
}

/// Specification 34, PORT-05: an upload larger than the device's maximum is refused before anything is
/// allocated, and the C6's maximum is one program slot, 2,064 KiB = 2,113,536 bytes, since a larger
/// container can never be installed (§5.1, D.2). One byte more is refused with the numbers; a
/// container as large as the slot would be taken if the heap could hold it, which a host cannot say,
/// so the test stops at the refusal. Catches a maximum that is the whole 4,128 KiB region, the slot of
/// the layout before the second firmware slot, and an off-by-one bound.
#[test]
fn the_c6_session_takes_an_upload_of_one_slot_at_most() {
    assert_eq!(SLOT_BYTES, 2_113_536);
    let mut session = transfer_session(1);
    let refused = session
        .begin(2_113_537, sha256(b"hello"))
        .expect_err("an upload longer than a slot is refused");
    assert_eq!(
        refused.to_string(),
        "transfer too large: 2113537, the device takes at most 2113536"
    );
}

/// The bytes of the constant `name` in the firmware's `memory.rs`, a sum or a difference of
/// products such as `16 * 1024 - 64`.
fn memory_rs_bytes(name: &str) -> usize {
    let source = include_str!("../../../firmware/trust-esp32c6/src/memory.rs");
    let prefix = format!("const {name}: usize = ");
    let line = source
        .lines()
        .find_map(|line| line.strip_prefix(prefix.as_str()))
        .unwrap_or_else(|| panic!("memory.rs declares {name}"));
    let bytes: i64 = line
        .trim_end_matches(';')
        .replace('-', "+-")
        .split('+')
        .map(|term| {
            let (sign, product) = term
                .trim()
                .strip_prefix('-')
                .map_or((1, term), |product| (-1, product));
            sign * product
                .split('*')
                .map(|factor| factor.trim().parse::<i64>().expect("a whole number"))
                .product::<i64>()
        })
        .sum();
    usize::try_from(bytes).expect("a number of bytes")
}

/// A reply of `len` bytes in the parts of 4,096 bytes that a heap with room gives.
fn reply_of(len: usize) -> ReplyParts {
    let mut parts = ReplyParts::new();
    parts
        .try_write(&vec![b'x'; len])
        .expect("a host heap has room for 180,161 bytes");
    parts
}

/// Specification 34, PORT-05 (reply capacity and parts) and Appendix D.2 (long replies): the C6's
/// session holds a reply of up to the capacity of its heap, the three regions that the firmware's
/// `memory.rs` gives the allocator: the main heap of 32 x 1,024 + 64 = 32,832 bytes and the 64 x 1,024
/// = 65,536 bytes that the deleted engine's RAM code took, 98,368 bytes, the 65,536 bytes of reclaimed
/// RAM and the 16 x 1,024 - 128 = 16,256 bytes of LP SRAM, 180,160 bytes in all, which is 44 parts of
/// 4,096, the last one 4,032 bytes. The test reads the figures out of `memory.rs`, so a change of the
/// regions fails here until the profile follows. A reply of 180,160 bytes is held whole, one byte
/// more is refused with `reply longer than 180160 bytes`, and a reply of 16,261 bytes, the 200 events
/// of the C6's store, which is longer than the 8,192-byte frame, is held, where the interim rule
/// refused everything above the frame. Catches a session still held to the frame, a capacity that
/// leaves out a region (the main heap's 64 KiB were left out until 25 October 2026), a figure that
/// drifts from `memory.rs`, and an off-by-one at the capacity.
#[test]
fn the_c6_session_holds_replies_up_to_its_heap() {
    assert_eq!(memory_rs_bytes("HEAP_BYTES"), 32_832);
    assert_eq!(memory_rs_bytes("PROGRAM_EXTRA_HEAP_BYTES"), 65_536);
    assert_eq!(memory_rs_bytes("RECLAIMED_BYTES"), 65_536);
    assert_eq!(memory_rs_bytes("LOW_POWER_BYTES"), 16_256);
    assert_eq!(
        HEAP_CAPACITY_BYTES,
        memory_rs_bytes("HEAP_BYTES")
            + memory_rs_bytes("PROGRAM_EXTRA_HEAP_BYTES")
            + memory_rs_bytes("RECLAIMED_BYTES")
            + memory_rs_bytes("LOW_POWER_BYTES")
    );
    assert_eq!(HEAP_CAPACITY_BYTES, 180_160);
    assert_ne!(HEAP_CAPACITY_BYTES, DEVICE_FRAME_LEN, "not the frame");

    let mut session = transfer_session(1);
    assert_eq!(session.reply_capacity(), 180_160);
    let events = session
        .hold_reply(reply_of(16_261))
        .expect("200 events are longer than the frame and fit the heap");
    assert_eq!(events.bytes, 16_261);
    let whole = session
        .hold_reply(reply_of(180_160))
        .expect("a reply of exactly the heap's capacity is held");
    assert_eq!(whole.bytes, 180_160);
    let refused = session
        .hold_reply(reply_of(180_161))
        .expect_err("one byte more than the heap is refused");
    assert_eq!(refused, TransferError::ReplyTooLong { capacity: 180_160 });
    assert_eq!(refused.to_string(), "reply longer than 180160 bytes");
    // The refusal changed nothing: the reply held before it is still there, and its last whole
    // part, from 180,160 - 4,032 - 4,096 = 172,032, reads.
    let last = session
        .get(whole.transfer, 172_032, 4_096)
        .expect("the held reply still reads");
    assert_eq!(last.data.len(), 4_096);
}
