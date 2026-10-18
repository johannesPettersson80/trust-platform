//! The STM32U5A9's reset store over the TAMP backup registers (specification 34, D.5; design §1.5): the boot
//! counter and the reset record in the F407's register numbers, checked through a fake register file.

use trust_platform_memory::reset::{ResetCause, ResetRecord, RECORD_WORDS};
use trust_platform_stm32u5::reset::{BackupRegisters, BackupStore};

/// The 32 backup registers of the TAMP block, held in memory.
#[derive(Debug, Default)]
struct FakeBackup {
    registers: [u32; 32],
}

impl BackupRegisters for FakeBackup {
    fn read(&self, number: usize) -> u32 {
        self.registers[number]
    }

    fn write(&mut self, number: usize, value: u32) {
        self.registers[number] = value;
    }
}

#[test]
fn the_boot_counter_rises_by_one_per_boot() {
    // Design §1.5 and the F407's layout: the boot counter is BKP14R. Each boot raises it by one, and a
    // record written after a boot carries the count of that boot, and reads back once. Catches a counter in
    // another register, a count that does not rise by one, a record stamped before the boot is counted, and
    // a record that is reported on every boot.
    let mut store = BackupStore::new(FakeBackup::default());
    assert_eq!(store.note_boot(), 1);
    assert_eq!(store.registers().registers[14], 1);
    assert_eq!(store.note_boot(), 2);
    assert_eq!(store.registers().registers[14], 2);

    store.write_reset_record(ResetCause::Watchdog, "scan");
    let record = store
        .take_reset_record()
        .expect("a written record reads back");
    assert_eq!(record.boot_count(), 2);
    assert_eq!(store.take_reset_record(), None);
}

#[test]
fn the_reset_record_lives_in_backup_registers_two_to_thirteen() {
    // Design §1.5 and the F407's layout: the record's twelve words are BKP2R to BKP13R. BKP0R and BKP1R keep
    // the cookie and BKP14R the boot counter, so the record must leave them as they are. Catches a record that
    // starts at the cookie, one that overflows into the counter, and one that shifts a word by one register.
    let mut store = BackupStore::new(FakeBackup::default());
    store.write_reset_record(ResetCause::Panic, "main.rs:42");
    let registers = store.registers().registers;

    assert_eq!(registers[0], 0);
    assert_eq!(registers[1], 0);
    assert_eq!(registers[14], 0);
    let words: [u32; RECORD_WORDS] = registers[2..14]
        .try_into()
        .expect("twelve words from BKP2R to BKP13R");
    let record = ResetRecord::decode(words).expect("the record is in BKP2R to BKP13R");
    assert_eq!(record.cause(), ResetCause::Panic);
    assert_eq!(record.text(), &b"main.rs:42"[..]);
}
