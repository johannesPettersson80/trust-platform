use alloc::collections::BTreeMap;
use alloc::rc::Rc;
use alloc::vec::Vec;
use core::cell::RefCell;

use super::*;
use crate::firmware_slots::tests::test_image;
use crate::firmware_slots::{FIRMWARE_A_OFFSET, FIRMWARE_B_OFFSET, PROJECT, SELECT_OFFSET};

/// A flash that holds bytes in 4 KiB sectors, erased to 0xFF, where a program only clears bits, as
/// NOR flash does.
#[derive(Clone, Default)]
struct Memory(Rc<RefCell<BTreeMap<u32, Vec<u8>>>>);

impl Memory {
    fn byte(&self, address: u32) -> u8 {
        self.0
            .borrow()
            .get(&(address / 4_096))
            .map_or(0xFF, |sector| sector[(address % 4_096) as usize])
    }

    fn write(&self, address: u32, bytes: &[u8]) {
        let mut sectors = self.0.borrow_mut();
        for (at, byte) in (address..).zip(bytes) {
            let sector = sectors
                .entry(at / 4_096)
                .or_insert_with(|| alloc::vec![0xFF; 4_096]);
            sector[(at % 4_096) as usize] &= *byte;
        }
    }
}

impl RawFlash for Memory {
    fn read_at(&mut self, address: u32, bytes: &mut [u8]) -> Result<(), FlashError> {
        for (at, byte) in (address..).zip(bytes.iter_mut()) {
            *byte = self.byte(at);
        }
        Ok(())
    }

    fn program_at(&mut self, address: u32, bytes: &[u8]) -> Result<(), FlashError> {
        self.write(address, bytes);
        Ok(())
    }

    fn erase_at(&mut self, address: u32) -> Result<(), FlashError> {
        self.0.borrow_mut().remove(&(address / 4_096));
        Ok(())
    }
}

/// What the boot loader does at a boot (ESP-IDF v5.5.1 with rollback): a `PendingVerify` entry
/// becomes `Aborted`, and the active entry, when `New`, becomes `PendingVerify`; it returns the slot it
/// boots.
fn boot_loader(memory: &Memory) -> Slot {
    let mut selection = Selection::new(memory.clone());
    let mut entries = selection.entries();
    for (index, slot) in entries.iter_mut().enumerate() {
        if let Some(entry) = slot.filter(|entry| entry.state == ImageState::PendingVerify) {
            let aborted = SelectEntry {
                state: ImageState::Aborted,
                ..entry
            };
            selection
                .write(index, aborted)
                .expect("the boot loader writes");
            *slot = Some(aborted);
        }
    }
    if let Some(index) = crate::firmware_slots::active_entry(&entries)
        && let Some(entry) = entries[index].filter(|entry| entry.state == ImageState::New)
    {
        selection
            .write(
                index,
                SelectEntry {
                    state: ImageState::PendingVerify,
                    ..entry
                },
            )
            .expect("the boot loader writes");
    }
    crate::firmware_slots::selected_slot(&Selection::new(memory.clone()).entries())
}

/// An update to slot B is selected on trial, the boot loader boots it, and the new firmware confirms
/// itself, so the boot loader keeps booting it; `capabilities` then reports it confirmed. Catches a
/// firmware that never confirms itself (rolled back at the next reset) and a selection written over
/// the running firmware's own entry.
#[test]
fn an_update_runs_on_trial_until_its_boot_confirms_it() {
    let memory = Memory::default();
    let mut running_a = C6Firmware::new(memory.clone(), Slot::A);
    running_a
        .activate(Slot::B)
        .expect("the selection is written");
    assert_eq!(boot_loader(&memory), Slot::B);

    let mut running_b = C6Firmware::new(memory.clone(), Slot::B);
    let (selection, facts) = running_b.boot("0.24.77");
    assert!(selection.trial);
    assert_eq!(
        facts,
        FirmwareFacts {
            slot: Slot::B,
            version: "0.24.77".into(),
            confirmed: false,
            rolled_back: None,
        }
    );
    running_b
        .confirm(&selection)
        .expect("the confirmation is written");
    assert_eq!(
        boot_loader(&memory),
        Slot::B,
        "a confirmed firmware is booted again"
    );
    let (_, facts) = running_b.boot("0.24.77");
    assert!(facts.confirmed);
}

/// An update that is reset before it confirms itself is rolled back: the boot loader aborts it and
/// boots slot A again, whose firmware reports the version it rolled back from slot B's descriptor.
/// Catches a rollback that goes unreported.
#[test]
fn an_update_reset_before_it_confirms_is_rolled_back_and_reported() {
    let memory = Memory::default();
    memory.write(FIRMWARE_B_OFFSET, &test_image(13, PROJECT, "0.24.77"));
    let mut running_a = C6Firmware::new(memory.clone(), Slot::A);
    running_a
        .activate(Slot::B)
        .expect("the selection is written");
    assert_eq!(boot_loader(&memory), Slot::B);
    // The new firmware hangs before its boot is done: the watchdog resets the chip.
    assert_eq!(boot_loader(&memory), Slot::A);
    let mut back_on_a = C6Firmware::new(memory.clone(), Slot::A);
    let (selection, facts) = back_on_a.boot("0.24.76");
    assert!(!selection.trial);
    assert_eq!(facts.rolled_back.as_deref(), Some("0.24.77"));
    assert!(facts.confirmed);
}

/// The chip checks an uploaded image where it lies in slot B with its first 60 bytes from the
/// session, and reads the version from its descriptor. Catches a check that reads the kept bytes from
/// the erased slot, which would refuse every upload.
#[test]
fn the_check_reads_the_kept_bytes_from_the_session() {
    let memory = Memory::default();
    let image = test_image(13, PROJECT, "0.24.77");
    memory.write(FIRMWARE_B_OFFSET + 60, &image[60..]);
    let mut slots = FirmwareSlots::new(memory.clone()).expect("the slots");
    let mut chip = C6Firmware::new(memory.clone(), Slot::A);
    let checked = chip
        .check(&mut slots, Slot::B, &image[..60], image.len())
        .expect("the image passes");
    assert_eq!(checked.version, "0.24.77");
    let refused = chip.check(&mut slots, Slot::B, &[0; 60], image.len());
    assert_eq!(
        refused.err().as_deref(),
        Some("the firmware image does not begin with the ESP image magic 0xE9")
    );
}

/// The slots are `ota_0` at 0x10000 and `ota_1` at 0x200000, and the selection is `otadata` at
/// 0xB000: a write of the firmware slots reaches no other address. Catches a slot offset that would
/// overwrite the running firmware or the program store.
#[test]
fn the_slots_and_the_selection_lie_where_the_partition_table_puts_them() {
    let memory = Memory::default();
    let mut slots = FirmwareSlots::new(memory.clone()).expect("the slots");
    slots.program(Slot::A, 0, &[0x00; 4]).expect("a program");
    slots.program(Slot::B, 4, &[0x00; 4]).expect("a program");
    assert_eq!(memory.byte(FIRMWARE_A_OFFSET), 0x00);
    assert_eq!(memory.byte(FIRMWARE_B_OFFSET + 4), 0x00);
    assert!(slots
        .program(Slot::A, FIRMWARE_SLOT_BYTES, &[0; 4])
        .is_err());
    let mut chip = C6Firmware::new(memory.clone(), Slot::A);
    chip.activate(Slot::B).expect("the selection is written");
    assert_eq!(memory.byte(SELECT_OFFSET), 2, "entry 0 holds sequence 2");
}
