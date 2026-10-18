//! The C6's transfer session (specification 34, PORT-05): its boot number comes from the boot counter
//! that the reset record keeps across a reset, and its limits are the program slot and the frame. The
//! counter is the fake store of the host test below, in ordinary memory that outlives a simulated
//! reset, as the C6's persistent RTC fast memory outlives a real one. Every expected value is worked
//! out by hand from the specification.

use trust_platform_esp32::profile::transfer_session;
use trust_platform_esp32::program_flash::SLOT_BYTES;
use trust_platform_memory::reset::{note_boot, ResetStore, RECORD_WORDS};
use trust_runtime_core::deployment::sha256;

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
/// allocated, and the C6's maximum is one program slot, 3,056 KiB = 3,129,344 bytes, since a larger
/// container can never be installed (§5.1). One byte more is refused with the numbers; a container as
/// large as the slot would be taken if the heap could hold it, which a host cannot say, so the test
/// stops at the refusal. Catches a maximum that is the whole 6,112 KiB region, and an off-by-one bound.
#[test]
fn the_c6_session_takes_an_upload_of_one_slot_at_most() {
    assert_eq!(SLOT_BYTES, 3_129_344);
    let mut session = transfer_session(1);
    let refused = session
        .begin(3_129_345, sha256(b"hello"))
        .expect_err("an upload longer than a slot is refused");
    assert_eq!(
        refused.to_string(),
        "transfer too large: 3129345, the device takes at most 3129344"
    );
}
