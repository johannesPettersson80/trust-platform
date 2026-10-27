//! The STM32U5A9's checkpoint flash (specification 34 §10.1 and D.5; design §1.1 and §4.4): the bank
//! and page of an address, the quad-word rule, the chip file's checkpoint slots, the slot's flash
//! rules, and the checkpoint log over a simulated flash that programs each quad-word once, as the
//! chip's ECC requires.

use trust_chips::ChipFile;
use trust_platform_stm32u5::flash::{
    is_quad_word, page_of, Bank, Fault, RawFlash, SlotFlash, BANK_BYTES, CHECKPOINT_A,
    CHECKPOINT_B, CHECKPOINT_SLOT_BYTES, FLASH_BASE, FLASH_END, PAGES_PER_BANK, PAGE_BYTES,
};
use trust_runtime_core::retain::checkpoint::{
    CheckpointFlash, CheckpointIdentity, CheckpointLog, CheckpointPersister, FlashError, Progress,
    Slot,
};

fn chip_file() -> ChipFile {
    ChipFile::parse("stm32u5a9", include_str!("../../../chips/stm32u5a9.toml"))
        .expect("the STM32U5A9 chip file is valid")
}

/// The bytes the simulation holds: from the first checkpoint slot to the end of the flash.
const SIM_BYTES: usize = (FLASH_END - CHECKPOINT_A) as usize;

/// The flash of the two checkpoint slots as a simulation (specification 34 §10.1). An erase sets a
/// page to 0xFF, a program only clears bits, and a quad-word may be programmed once between two
/// erases of its page, as the U5's ECC requires: a second program is reported as PROGERR.
struct SimFlash {
    bytes: Vec<u8>,
    programmed: Vec<bool>,
}

impl SimFlash {
    fn erased() -> Self {
        Self {
            bytes: vec![0xFF; SIM_BYTES],
            programmed: vec![false; SIM_BYTES / 16],
        }
    }
}

impl RawFlash for SimFlash {
    fn read_at(&self, address: u32, bytes: &mut [u8]) {
        let start = (address - CHECKPOINT_A) as usize;
        bytes.copy_from_slice(&self.bytes[start..start + bytes.len()]);
    }

    fn program_quad_word(&mut self, address: u32, bytes: &[u8; 16]) -> Result<(), Fault> {
        let start = (address - CHECKPOINT_A) as usize;
        let unit = start / 16;
        if self.programmed[unit] {
            return Err(Fault::Flag("PROGERR"));
        }
        for (cell, value) in self.bytes[start..start + 16].iter_mut().zip(bytes) {
            *cell &= *value;
        }
        self.programmed[unit] = true;
        Ok(())
    }

    fn erase_page(&mut self, bank: Bank, page: u32) -> Result<(), Fault> {
        if bank != Bank::Two || page >= PAGES_PER_BANK {
            return Err(Fault::Address);
        }
        let address = FLASH_BASE + BANK_BYTES + page * PAGE_BYTES;
        let start = address.checked_sub(CHECKPOINT_A).ok_or(Fault::Address)? as usize;
        let end = start + PAGE_BYTES as usize;
        if end > SIM_BYTES {
            return Err(Fault::Address);
        }
        self.bytes[start..end].fill(0xFF);
        self.programmed[start / 16..end / 16].fill(false);
        Ok(())
    }
}

fn slot_flash() -> SlotFlash<SimFlash> {
    SlotFlash::new(SimFlash::erased()).expect("the chip's geometry holds the checkpoint log")
}

/// Writes a checkpoint of `payload` for `identity` through the log, all steps at once, and returns
/// its sequence.
fn append(flash: &mut SlotFlash<SimFlash>, identity: CheckpointIdentity, payload: &[u8]) -> u64 {
    CheckpointLog::open(flash)
        .and_then(|mut log| log.append(identity, payload))
        .expect("the checkpoint commits")
        .sequence
}

/// The newest committed checkpoint as the persister's recovery reads it: its sequence and payload.
fn recover(
    flash: &mut SlotFlash<SimFlash>,
    identity: CheckpointIdentity,
    len: usize,
) -> (u64, Vec<u8>) {
    let log = CheckpointLog::open(flash).expect("the slots read");
    let mut persister =
        CheckpointPersister::new(log, identity, len).expect("room for the checkpoint buffer");
    let mut payload = vec![0; len];
    let record = persister
        .recover(&mut payload)
        .expect("the newest checkpoint loads")
        .expect("a checkpoint is committed");
    (record.sequence, payload)
}

#[test]
fn a_flash_address_maps_to_its_bank_and_page() {
    // DS13543 §3.4 and design §1.1: bank 1 from 0x0800_0000 and bank 2 from 0x0820_0000, 256 pages of
    // 8 KiB in each bank, so the flash ends at 0x0840_0000.
    assert_eq!(page_of(0x0800_0000), Some((Bank::One, 0)));
    assert_eq!(page_of(0x0800_2000), Some((Bank::One, 1)));
    assert_eq!(page_of(0x081F_FFFF), Some((Bank::One, 255)));
    assert_eq!(page_of(0x0820_0000), Some((Bank::Two, 0)));
    assert_eq!(page_of(0x083F_E000), Some((Bank::Two, 255)));
    assert_eq!(page_of(0x083F_FFFF), Some((Bank::Two, 255)));
    assert_eq!(page_of(0x0840_0000), None);
    assert_eq!(page_of(0x07FF_FFFF), None);
}

#[test]
fn a_quad_word_must_be_16_byte_aligned() {
    // RM0456 programs whole quad-words: the address is a multiple of 16 and all 16 bytes lie in the
    // flash, so the last quad-word ends exactly at 0x0840_0000.
    assert!(is_quad_word(0x0800_0000));
    assert!(is_quad_word(0x0800_0010));
    assert!(!is_quad_word(0x0800_0008));
    assert!(!is_quad_word(0x0800_0004));
    assert!(is_quad_word(0x083F_FFF0));
    assert!(!is_quad_word(0x0840_0000));
    assert!(!is_quad_word(0x07FF_FFF0));
}

#[test]
fn checkpoint_slots_match_the_chip_file() {
    let layout = chip_file().layout;
    // The chip file's [layout] and design §4.4 give the same two slots of 16 KiB each.
    assert_eq!(CHECKPOINT_A, 0x083F_8000);
    assert_eq!(CHECKPOINT_B, 0x083F_C000);
    assert_eq!(layout.checkpoint_a.start, CHECKPOINT_A);
    assert_eq!(layout.checkpoint_b.start, CHECKPOINT_B);
    assert_eq!(
        layout.checkpoint_a.bytes(),
        u64::from(CHECKPOINT_SLOT_BYTES)
    );
    assert_eq!(
        layout.checkpoint_b.bytes(),
        u64::from(CHECKPOINT_SLOT_BYTES)
    );
    // Each slot is two pages of bank 2, pages 252 and 254 of the bank at its first byte.
    assert_eq!(page_of(CHECKPOINT_A), Some((Bank::Two, 252)));
    assert_eq!(page_of(CHECKPOINT_B), Some((Bank::Two, 254)));
}

#[test]
fn a_program_writes_whole_quad_words_inside_its_slot() {
    let mut flash = slot_flash();
    // Whole quad-words at a 16-byte offset are programmed, up to the end of the slot.
    assert_eq!(flash.program(Slot::A, 16, &[0x00; 16]), Ok(()));
    assert_eq!(flash.program(Slot::B, 16_368, &[0x00; 16]), Ok(()));
    // The offset must be a multiple of 16 (RM0456): offset 8 is refused.
    assert_eq!(
        flash.program(Slot::A, 8, &[0x00; 16]),
        Err(FlashError::Program)
    );
    // The length must be whole quad-words: 20 bytes are refused.
    assert_eq!(
        flash.program(Slot::A, 32, &[0x00; 20]),
        Err(FlashError::Program)
    );
    // A program that runs past its slot is refused: 16,368 + 32 is 16,400, past 16,384.
    assert_eq!(
        flash.program(Slot::A, 16_368, &[0x00; 32]),
        Err(FlashError::Program)
    );
}

#[test]
fn a_read_outside_the_slot_is_refused() {
    let mut flash = slot_flash();
    let mut last = [0; 8];
    // The last eight bytes of slot B end at 16,384, the end of the slot.
    assert_eq!(flash.read(Slot::B, 16_376, &mut last), Ok(()));
    // Eight bytes from 16,380 run four bytes past the slot.
    assert_eq!(
        flash.read(Slot::B, 16_380, &mut last),
        Err(FlashError::Read)
    );
    assert_eq!(
        flash.read(Slot::A, 16_384, &mut [0; 1]),
        Err(FlashError::Read)
    );
}

#[test]
fn an_erase_clears_the_page_that_holds_its_offset() {
    let mut flash = slot_flash();
    flash
        .program(Slot::A, 0, &[0x00; 16])
        .expect("the first page is programmed");
    flash
        .program(Slot::A, 8_192, &[0x00; 16])
        .expect("the second page is programmed");
    // Offset 8,292 lies in the second page, which the erase clears: each slot is two erase units.
    flash.erase(Slot::A, 8_292).expect("the page is erased");
    let mut first = [0xEE; 16];
    flash.read(Slot::A, 0, &mut first).expect("the slot reads");
    assert_eq!(first, [0x00; 16]);
    let mut second = [0xEE; 16];
    flash
        .read(Slot::A, 8_192, &mut second)
        .expect("the slot reads");
    assert_eq!(second, [0xFF; 16]);
    // The erased quad-word takes a program again, because its flag was cleared with the page.
    assert_eq!(flash.program(Slot::A, 8_192, &[0x00; 16]), Ok(()));
    // A slot has no erase unit past its second page.
    assert_eq!(flash.erase(Slot::A, 16_384), Err(FlashError::Erase));
}

#[test]
fn a_16_byte_program_unit_takes_payloads_of_16288_bytes() {
    let geometry = slot_flash().geometry();
    assert_eq!(geometry.program_unit(), 16);
    assert_eq!(geometry.erase_unit(), 8_192);
    assert_eq!(geometry.slot_len(), 16_384);
    // 16,384 bytes less the 16-byte slot header, the 64-byte record header and the 16-byte commit
    // field, which takes the program unit (specification 34 §10.1: the commit field is the larger of
    // 4 bytes and the program unit).
    assert_eq!(geometry.max_payload_len(), 16_288);
    assert_eq!(geometry.record_len(16_288), Some(16_368));
    assert_eq!(geometry.record_len(16_289), None);
}

#[test]
fn the_checkpoint_log_programs_each_quad_word_once() {
    let mut flash = slot_flash();
    let identity = CheckpointIdentity {
        application: [0xA1; 16],
        schema: [0xB2; 16],
    };
    // Each payload is 8,000 bytes, so each record takes 8,080 bytes (64 + 16 + 8,000). Slot A takes
    // two (16 + 2 × 8,080 = 16,176 of its 16,384 bytes). The third record does not fit, so the log
    // erases slot B and writes there.
    let (first, second, third) = ([0x11; 8_000], [0x22; 8_000], [0x33; 8_000]);
    assert_eq!(append(&mut flash, identity, &first), 1);
    assert_eq!(append(&mut flash, identity, &second), 2);
    assert_eq!(append(&mut flash, identity, &third), 3);

    // Specification 34 §10.1: the slot header at offset 0 is `TRCS`, the first record starts at
    // offset 16 with `TRCK`, and its commit field is `DONE` four times at record offset 64.
    let mut magic = [0; 4];
    flash.read(Slot::A, 0, &mut magic).expect("the slot reads");
    assert_eq!(magic, *b"TRCS");
    flash.read(Slot::A, 16, &mut magic).expect("the slot reads");
    assert_eq!(magic, *b"TRCK");
    let mut commit = [0; 16];
    flash
        .read(Slot::A, 80, &mut commit)
        .expect("the slot reads");
    assert_eq!(commit, *b"DONEDONEDONEDONE");
    flash.read(Slot::B, 0, &mut magic).expect("the slot reads");
    assert_eq!(magic, *b"TRCS");

    let (sequence, payload) = recover(&mut flash, identity, 8_000);
    assert_eq!(sequence, 3);
    assert!(
        payload == third,
        "the newest checkpoint is the third payload"
    );

    // A fourth write, interrupted after three steps of one quad-word each, leaves its record
    // uncommitted: its header is only partly programmed. The log is dropped, as a power loss would.
    let fourth = [0x44; 8_000];
    {
        let mut log = CheckpointLog::open(&mut flash).expect("the slots read");
        log.begin(identity, &fourth).expect("the write begins");
        for _ in 0..3 {
            assert_eq!(log.step(&fourth, 16), Ok(Progress::Pending));
        }
    }
    let (sequence, payload) = recover(&mut flash, identity, 8_000);
    assert_eq!(sequence, 3);
    assert!(
        payload == third,
        "the interrupted write leaves the third payload newest"
    );

    // The next checkpoint commits, and recovery loads it.
    let fifth = [0x55; 8_000];
    append(&mut flash, identity, &fifth);
    let (_, payload) = recover(&mut flash, identity, 8_000);
    assert!(payload == fifth, "the fifth payload is the newest");
}
