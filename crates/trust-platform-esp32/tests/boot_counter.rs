//! The C6's boot counter (specification 34 PORT-05 and Appendix D.2): the boot number of the transfer
//! ids is a count that rises by one at every boot and lives in two flash sectors, so it survives a loss
//! of power. The flash under it is a RAM flash that keeps its bytes when a test drops the counter, as
//! the chip's flash keeps them when power goes; a torn write stops a program part-way. Every expected
//! value is worked out by hand from the record layout of the specification (the count and its
//! complement, 8 bytes, 512 records in each 4 KiB sector).

use std::cell::RefCell;
use std::rc::Rc;

use trust_chips::Catalog;
use trust_platform_esp32::boot_counter::{
    BootCounter, BootCounterError, SECTOR_A_OFFSET, SECTOR_BYTES, SECTOR_B_OFFSET,
};
use trust_platform_esp32::checkpoint::RawFlash;
use trust_platform_esp32::profile::transfer_session;
use trust_runtime_core::deployment::sha256;
use trust_runtime_core::retain::checkpoint::FlashError;

/// The flash from address 0 to the end of the counter's sectors (0xB000), erased.
const FLASH_BYTES: usize = 0xB000;

/// What the next program does: it writes everything, or it writes only the first `bytes` and fails, as
/// a power loss stops it.
#[derive(Clone, Copy)]
enum Program {
    Whole,
    Torn { bytes: usize },
}

struct FlashState {
    bytes: Vec<u8>,
    program: Program,
    erases: Vec<u32>,
    /// The next erase fails before it clears anything.
    fail_erase: bool,
}

/// A flash that keeps its bytes: erased to 0xFF, a program clears bits, an erase sets one 4 KiB unit.
/// The test keeps a clone, so the flash outlives a counter that is dropped.
#[derive(Clone)]
struct RamFlash(Rc<RefCell<FlashState>>);

impl RamFlash {
    fn erased() -> Self {
        Self(Rc::new(RefCell::new(FlashState {
            bytes: vec![0xFF; FLASH_BYTES],
            program: Program::Whole,
            erases: Vec::new(),
            fail_erase: false,
        })))
    }

    /// The next program writes its first `bytes` bytes and fails.
    fn tear_next_program_after(&self, bytes: usize) {
        self.0.borrow_mut().program = Program::Torn { bytes };
    }

    /// The next erase fails and clears nothing.
    fn fail_next_erase(&self) {
        self.0.borrow_mut().fail_erase = true;
    }

    fn erases(&self) -> Vec<u32> {
        self.0.borrow().erases.clone()
    }

    /// Writes the record of `count` at record `index` of the sector at `sector`, as the counter does.
    fn put(&self, sector: u32, index: usize, count: u32) {
        let mut state = self.0.borrow_mut();
        let at = sector as usize + index * 8;
        state.bytes[at..at + 4].copy_from_slice(&count.to_le_bytes());
        state.bytes[at + 4..at + 8].copy_from_slice(&(!count).to_le_bytes());
    }
}

impl RawFlash for RamFlash {
    fn read_at(&mut self, address: u32, bytes: &mut [u8]) -> Result<(), FlashError> {
        let state = self.0.borrow();
        let start = address as usize;
        bytes.copy_from_slice(&state.bytes[start..start + bytes.len()]);
        Ok(())
    }

    fn program_at(&mut self, address: u32, bytes: &[u8]) -> Result<(), FlashError> {
        let mut state = self.0.borrow_mut();
        let start = address as usize;
        let (written, outcome) = match state.program {
            Program::Whole => (bytes.len(), Ok(())),
            Program::Torn { bytes: kept } => (kept.min(bytes.len()), Err(FlashError::Program)),
        };
        state.program = Program::Whole;
        for (cell, byte) in state.bytes[start..start + written].iter_mut().zip(bytes) {
            *cell &= byte;
        }
        outcome
    }

    fn erase_at(&mut self, address: u32) -> Result<(), FlashError> {
        let mut state = self.0.borrow_mut();
        if std::mem::take(&mut state.fail_erase) {
            return Err(FlashError::Erase);
        }
        state.erases.push(address);
        let start = address as usize;
        state.bytes[start..start + SECTOR_BYTES as usize].fill(0xFF);
        Ok(())
    }
}

/// A power loss and the next power-on: the counter is dropped and a new one reads the same flash.
fn power_cycle(flash: &RamFlash) -> BootCounter<RamFlash> {
    BootCounter::new(flash.clone())
}

/// Specification 34 PORT-05: the boot number is a count that rises by one at every boot and survives a
/// loss of power, so an id never repeats across boots. A flash that never counted starts at 1; each
/// power-on reads the flash again and counts the next number, 2, 3, 4, and the first transfer id of the
/// boot with count `n` is `n` in the high 32 bits and sequence 1 in the low 32 bits (16 hexadecimal
/// digits). Catches a count kept in memory that a power loss clears (the RTC counter this replaces,
/// which started at 1 again), a count that does not rise, and a boot number put in the low half of the
/// id.
#[test]
fn the_boot_counter_survives_a_power_loss() {
    let flash = RamFlash::erased();
    let mut ids = Vec::new();
    for expected in 1_u32..=4 {
        let mut counter = power_cycle(&flash);
        let count = counter.count_boot().expect("the boot is counted");
        assert_eq!(
            count,
            expected,
            "boot {expected} after {} power losses",
            expected - 1
        );
        let mut session = transfer_session(count);
        ids.push(
            session
                .begin(5, sha256(b"hello"))
                .expect("an upload of five bytes begins")
                .to_string(),
        );
        // The counter is dropped here: the power is lost.
    }
    assert_eq!(
        ids,
        [
            "0000000100000001",
            "0000000200000001",
            "0000000300000001",
            "0000000400000001"
        ],
        "no id repeats across the four boots"
    );
}

/// A write that power ends part-way never loses the count before it, and never lets a count be handed
/// out twice: a boot counts only after its record reads back, so the count of a torn write was not
/// handed out, and the next boot counts above every whole record. The first boot counts 1. The second
/// boot's write is torn after 2 bytes, after 4 bytes (the count is programmed, its complement is not)
/// or after 6 bytes, and fails, so that boot is not counted. After 2 or 4 bytes the record's two words
/// disagree, so it reads as no record, the highest count is 1, and the next boot writes 2 after the
/// torn record. After 6 bytes the complement `!2 = 0xFFFF_FFFD`, little-endian `FD FF FF FF`, has its
/// last two bytes equal to erased flash, so the record is whole: the next boot finds 2 and counts 3.
/// The counts handed out are 1, then 2 or 3, then one more each time. Catches a torn record that is
/// taken for a count that was never handed out and then handed out again, a count that falls back to
/// 1, and a record that is taken whole without its complement.
#[test]
fn a_write_that_power_ends_leaves_the_previous_count() {
    for (torn_after, next) in [(2_usize, 2_u32), (4, 2), (6, 3)] {
        let flash = RamFlash::erased();
        assert_eq!(power_cycle(&flash).count_boot(), Ok(1));
        flash.tear_next_program_after(torn_after);
        assert_eq!(
            power_cycle(&flash).count_boot(),
            Err(BootCounterError::Flash(FlashError::Program)),
            "the write torn after {torn_after} bytes is not counted"
        );
        assert_eq!(
            power_cycle(&flash).count_boot(),
            Ok(next),
            "the boot after a write torn after {torn_after} bytes counts {next}"
        );
        assert_eq!(power_cycle(&flash).count_boot(), Ok(next + 1));
    }
}

/// A sector holds 512 records of 8 bytes (4,096 / 8). The 513th boot finds sector A full and writes its
/// record at the start of sector B after erasing it; the 1,025th finds both full and erases sector A,
/// whose counts are all older, and writes at its start. The counts stay 1, 2, 3 ... without a gap or a
/// repeat across the switch, and each erase is of the sector that holds only older counts. A power loss
/// after the erase and before the write (the write is torn after no byte here) leaves the full sector
/// with its highest count, so the next boot counts the same number, 513, without loss. Catches a count
/// that restarts at a sector switch, a switch that erases the sector holding the newest count, and a
/// lost count between the erase and the write.
#[test]
fn a_full_sector_continues_in_the_other_one_without_losing_a_count() {
    let flash = RamFlash::erased();
    for expected in 1_u32..=512 {
        assert_eq!(power_cycle(&flash).count_boot(), Ok(expected));
    }
    assert_eq!(flash.erases(), Vec::<u32>::new(), "512 boots need no erase");

    // The 513th boot: the erase of sector B goes, then the write fails as power ends.
    flash.tear_next_program_after(0);
    assert_eq!(
        power_cycle(&flash).count_boot(),
        Err(BootCounterError::Flash(FlashError::Program))
    );
    assert_eq!(flash.erases(), [SECTOR_B_OFFSET]);
    assert_eq!(
        power_cycle(&flash).count_boot(),
        Ok(513),
        "the count of the failed boot is counted by the next one; nothing was lost"
    );
    for expected in 514_u32..=1_024 {
        assert_eq!(power_cycle(&flash).count_boot(), Ok(expected));
    }
    assert_eq!(
        flash.erases(),
        [SECTOR_B_OFFSET, SECTOR_B_OFFSET],
        "the second erase of sector B was the retry of the 513th boot"
    );
    // Both sectors are full now: 512 counts in A, and 512 (513 to 1,024) in B.
    assert_eq!(power_cycle(&flash).count_boot(), Ok(1_025));
    assert_eq!(
        flash.erases(),
        [SECTOR_B_OFFSET, SECTOR_B_OFFSET, SECTOR_A_OFFSET],
        "the 1,025th boot erases sector A, which holds only counts below 513"
    );
    assert_eq!(power_cycle(&flash).count_boot(), Ok(1_026));
}

/// An erase that fails stops the boot before it writes, and leaves the full sector with its highest
/// count, so the next boot counts the same number. Catches an erase error that is ignored, which would
/// write into a sector that was not erased.
#[test]
fn a_failed_erase_counts_nothing_and_loses_nothing() {
    let flash = RamFlash::erased();
    for expected in 1_u32..=512 {
        assert_eq!(power_cycle(&flash).count_boot(), Ok(expected));
    }
    flash.fail_next_erase();
    assert_eq!(
        power_cycle(&flash).count_boot(),
        Err(BootCounterError::Flash(FlashError::Erase))
    );
    assert_eq!(power_cycle(&flash).count_boot(), Ok(513));
}

/// The count cannot pass `u32::MAX`: a record of 4,294,967,295 is the highest whole record, and the
/// boot after it is refused instead of wrapping to 0, which would repeat an id. Catches a wrapping add.
#[test]
fn the_count_does_not_wrap() {
    let flash = RamFlash::erased();
    flash.put(SECTOR_A_OFFSET, 0, u32::MAX);
    assert_eq!(
        power_cycle(&flash).count_boot(),
        Err(BootCounterError::Exhausted)
    );
}

/// The counter takes the highest whole record of the two sectors, wherever it is: a sector A that holds
/// 9 and a sector B that holds 40 continue from 40 in sector B. A record whose two words disagree is
/// no record: sector A's record 12 beside a complement of 0 is skipped. Catches a counter that reads
/// only sector A, one that trusts a record without its complement, and one that takes the last record
/// of the sector instead of the highest.
#[test]
fn the_highest_whole_record_of_either_sector_is_the_count() {
    let flash = RamFlash::erased();
    flash.put(SECTOR_A_OFFSET, 0, 9);
    flash.put(SECTOR_B_OFFSET, 0, 40);
    // A damaged record in sector A, after the whole one: its complement is erased, not 12's.
    {
        let mut state = flash.0.borrow_mut();
        let at = SECTOR_A_OFFSET as usize + 8;
        state.bytes[at..at + 4].copy_from_slice(&200_u32.to_le_bytes());
        state.bytes[at + 4..at + 8].copy_from_slice(&0_u32.to_le_bytes());
    }
    assert_eq!(power_cycle(&flash).count_boot(), Ok(41));
    assert_eq!(power_cycle(&flash).count_boot(), Ok(42));
}

/// The chip file names the two sectors the counter writes (specification 34 Appendix D.2 and H.2):
/// `boot_counter_a` at `SECTOR_A_OFFSET`, 0x9000, and `boot_counter_b` at `SECTOR_B_OFFSET`, 0xA000,
/// each `SECTOR_BYTES`, 4 KiB, which is one erase unit of the chip's SPI flash. They lie one after
/// the other at the start of the gap between the partition table, which ends at 0x9000, and the
/// firmware, which starts at 0x10000. That no other region overlaps them is the reader's rule
/// (`crates/trust-chips/tests/rules.rs` refuses a file in which one does), and `partitions.csv`
/// declares them as data partitions (the xtask test of the partition table). Catches a counter that
/// writes sectors the chip file does not declare, which a later layout could hand to the firmware or
/// the partition table, and constants that drift from the file.
#[test]
fn the_chip_file_names_the_boot_counter_sectors() {
    let file = Catalog::embedded()
        .expect("the chip files are valid")
        .chip("esp32c6")
        .cloned()
        .expect("the ESP32-C6 has a chip file");
    let sector_a = file
        .layout
        .boot_counter_a
        .expect("the C6 names its first boot counter sector");
    let sector_b = file
        .layout
        .boot_counter_b
        .expect("the C6 names its second boot counter sector");
    assert_eq!(
        (SECTOR_A_OFFSET, SECTOR_B_OFFSET, SECTOR_BYTES),
        (0x9000, 0xA000, 0x1000)
    );
    assert_eq!(
        (sector_a.start, sector_b.start),
        (SECTOR_A_OFFSET, SECTOR_B_OFFSET)
    );
    assert_eq!(
        (sector_a.bytes(), sector_b.bytes()),
        (u64::from(SECTOR_BYTES), u64::from(SECTOR_BYTES))
    );
    let erase_unit = file
        .memory
        .spi_flash
        .expect("the C6 describes its SPI flash")
        .erase_kib;
    assert_eq!(
        u64::from(SECTOR_BYTES),
        u64::from(erase_unit) * 1024,
        "a counter sector is one erase unit"
    );
    assert_eq!(
        file.layout
            .partition_table
            .expect("the C6 names its partition table")
            .end(),
        u64::from(SECTOR_A_OFFSET),
        "the counter starts where the partition table ends"
    );
    assert_eq!(
        sector_a.end(),
        u64::from(SECTOR_B_OFFSET),
        "its second sector follows the first"
    );
    assert!(
        sector_b.end() <= u64::from(file.layout.firmware.start),
        "and ends before the firmware"
    );
}
