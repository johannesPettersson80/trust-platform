//! The C6's checkpoint slots (specification 34 §10.1 and D.2): the chip file's two slots, the access
//! rules of the slot flash and the platform's checkpoint store. The flash under the slots is a
//! recording fake, so each test sees exactly the flash operations that the slot layer let through.

use std::cell::RefCell;
use std::rc::Rc;

use smol_str::SmolStr;
use trust_chips::{Catalog, ChipFile};
use trust_platform_esp32::checkpoint::{RawFlash, SlotFlash};
use trust_platform_esp32::profile::{
    CHECKPOINT_A_OFFSET, CHECKPOINT_B_OFFSET, CHECKPOINT_SLOT_BYTES, FLASH_ERASE_BYTES,
    FLASH_PROGRAM_BYTES,
};
use trust_platform_esp32::Platform;
use trust_runtime_core::deployment::ControlMode;
use trust_runtime_core::ports::CheckpointStorage;
use trust_runtime_core::retain::checkpoint::{CheckpointFlash, FlashError, Slot};
use trust_runtime_core::watchdog::RetainMode;
use trust_runtime_services::capabilities::{LastReset, ResetCauseName};
use trust_runtime_services::runner::EngineLiveValues;
use trust_runtime_services::status::StatusSettings;

/// One flash operation that reached the recording fake: a read or a program of `len` bytes at
/// `address`, or the erase of the unit at `address`.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Op {
    Read { address: u32, len: u64 },
    Program { address: u32, len: u64 },
    Erase { address: u32 },
}

impl Op {
    /// The flash address the operation starts at and the number of bytes it covers.
    fn span(&self) -> (u32, u64) {
        match *self {
            Self::Read { address, len } | Self::Program { address, len } => (address, len),
            Self::Erase { address } => (address, u64::from(FLASH_ERASE_BYTES)),
        }
    }
}

/// A flash that records every operation and succeeds, so a test sees what reached the flash.
struct RecordingFlash {
    ops: Rc<RefCell<Vec<Op>>>,
}

impl RawFlash for RecordingFlash {
    fn read_at(&mut self, address: u32, bytes: &mut [u8]) -> Result<(), FlashError> {
        let len = u64::try_from(bytes.len()).expect("a length fits in 64 bits");
        self.ops.borrow_mut().push(Op::Read { address, len });
        Ok(())
    }

    fn program_at(&mut self, address: u32, bytes: &[u8]) -> Result<(), FlashError> {
        let len = u64::try_from(bytes.len()).expect("a length fits in 64 bits");
        self.ops.borrow_mut().push(Op::Program { address, len });
        Ok(())
    }

    fn erase_at(&mut self, address: u32) -> Result<(), FlashError> {
        self.ops.borrow_mut().push(Op::Erase { address });
        Ok(())
    }
}

/// A slot flash over a recording fake, and the record of the operations that reached the fake.
fn slot_flash() -> (SlotFlash<RecordingFlash>, Rc<RefCell<Vec<Op>>>) {
    let ops = Rc::new(RefCell::new(Vec::new()));
    let flash = SlotFlash::new(RecordingFlash {
        ops: Rc::clone(&ops),
    })
    .expect("the C6's geometry holds the checkpoint log");
    (flash, ops)
}

fn chip_file() -> ChipFile {
    Catalog::embedded()
        .expect("the chip files are valid")
        .chip("esp32c6")
        .cloned()
        .expect("the ESP32-C6 has a chip file")
}

/// A platform for a record with the retain mode `retain` and the checkpoint flash `flash`, over no
/// watchdog.
fn platform_with(retain: RetainMode, flash: Box<dyn CheckpointFlash>) -> Platform<()> {
    Platform::new(
        (),
        SmolStr::new_static("Controller"),
        StatusSettings {
            execution_backend: "vm",
            execution_backend_source: "default",
            simulation_enabled: false,
            simulation_time_scale: 1,
            simulation_mode: SmolStr::new_static("production"),
            simulation_warning: SmolStr::default(),
        },
        false,
        ControlMode::Production,
        EngineLiveValues::empty(),
        LastReset {
            cause: ResetCauseName::PowerOn,
            text: SmolStr::default(),
            boot_count: 1,
        },
    )
    .with_checkpoint(retain, flash)
}

/// The chip file's slots are the slots the adapter uses, and every access the slot flash lets
/// through lands in one of them (specification 34 §10.1 and Appendix H). The slots follow the
/// installed application and end the 8 MiB module. Catches a slot that overlaps the application or
/// the firmware, a slot at another offset or size than the chip file's, and an access that runs
/// past its slot into the flash after it.
#[test]
fn the_checkpoint_slots_are_the_chip_files_and_nothing_else() {
    let file = chip_file();
    let [first, second] = [file.layout.checkpoint_a, file.layout.checkpoint_b];
    assert_eq!(CHECKPOINT_A_OFFSET, first.start);
    assert_eq!(CHECKPOINT_B_OFFSET, second.start);
    assert_eq!(u64::from(CHECKPOINT_SLOT_BYTES), first.bytes());
    assert_eq!(first.bytes(), second.bytes());
    assert_eq!(file.layout.application.end(), u64::from(first.start));
    assert!(file.layout.firmware.end() <= u64::from(file.layout.application.start));
    assert_eq!(second.end(), 8 * 1024 * 1024);

    let regions = [
        (u64::from(first.start), first.end()),
        (u64::from(second.start), second.end()),
    ];
    let (mut flash, ops) = slot_flash();
    for slot in [Slot::A, Slot::B] {
        for unit in 0..CHECKPOINT_SLOT_BYTES / FLASH_PROGRAM_BYTES {
            flash
                .program(slot, unit * FLASH_PROGRAM_BYTES, &[0; 4])
                .expect("a whole unit inside the slot is programmed");
        }
        for sector in 0..CHECKPOINT_SLOT_BYTES / FLASH_ERASE_BYTES {
            flash
                .erase(slot, sector * FLASH_ERASE_BYTES)
                .expect("a sector start inside the slot is erased");
        }
    }
    assert_eq!(
        flash.program(Slot::A, CHECKPOINT_SLOT_BYTES, &[0; 4]),
        Err(FlashError::Program)
    );
    assert_eq!(
        flash.erase(Slot::B, CHECKPOINT_SLOT_BYTES),
        Err(FlashError::Erase)
    );

    let recorded = ops.borrow();
    // 4,096 program units and 4 erase units in each slot; the two refused accesses reached nothing.
    assert_eq!(recorded.len(), 2 * (4_096 + 4));
    for op in recorded.iter() {
        let (address, len) = op.span();
        let end = u64::from(address) + len;
        assert!(
            regions
                .iter()
                .any(|&(start, region_end)| start <= u64::from(address) && end <= region_end),
            "{op:?} lies inside one of the two checkpoint slots"
        );
    }
}

/// A program that is not on whole program units never reaches the flash: a 4-byte program at offset
/// 2, a 3-byte program at offset 0 and a 4-byte read at offset 1 are refused with their flash error
/// and no flash operation runs (specification 34 §10.1: a program writes whole units at multiples of
/// the unit). A whole unit at offset 4 is programmed at flash address 0x7F8004 and read back from it.
/// Catches a driver that programs a partial unit, which leaves a word that is neither the old nor the
/// new record, and a check made after the write.
#[test]
fn an_unaligned_program_is_refused_before_it_reaches_the_flash() {
    let (mut flash, ops) = slot_flash();
    assert_eq!(flash.program(Slot::A, 2, &[0; 4]), Err(FlashError::Program));
    assert_eq!(flash.program(Slot::A, 0, &[0; 3]), Err(FlashError::Program));
    assert_eq!(flash.read(Slot::B, 1, &mut [0; 4]), Err(FlashError::Read));
    assert!(ops.borrow().is_empty(), "no flash operation ran");

    assert_eq!(flash.program(Slot::A, 4, &[0; 4]), Ok(()));
    assert_eq!(flash.read(Slot::A, 4, &mut [0; 4]), Ok(()));
    assert_eq!(
        *ops.borrow(),
        [
            Op::Program {
                address: 0x7F_8004,
                len: 4
            },
            Op::Read {
                address: 0x7F_8004,
                len: 4
            },
        ]
    );
}

/// An erase clears one whole 4 KiB erase unit, so it starts at the unit's first byte (specification
/// 34 §10.1). An erase 2 KiB into the first unit of slot A, and one at the last word of slot B
/// (offset 0x3FFC), are refused with `FlashError::Erase` and erase nothing. The unit at offset 0x3000
/// of slot B is erased at flash address 0x7FF000 (0x7FC000 + 0x3000). Catches an erase that starts in
/// the middle of a unit, which would clear a unit other than the one addressed, and an erase of the
/// wrong slot's unit.
#[test]
fn an_erase_inside_a_sector_is_refused() {
    let (mut flash, ops) = slot_flash();
    assert_eq!(flash.erase(Slot::A, 0x800), Err(FlashError::Erase));
    assert_eq!(flash.erase(Slot::B, 0x3FFC), Err(FlashError::Erase));
    assert!(ops.borrow().is_empty(), "no flash operation ran");

    assert_eq!(flash.erase(Slot::B, 0x3000), Ok(()));
    assert_eq!(*ops.borrow(), [Op::Erase { address: 0x7F_F000 }]);
}

/// A slot is 16 KiB from its start, so an access that starts or runs past its end is refused before
/// it reaches the flash: a 4-byte program at offset 0x4000, an 8-byte program at offset 0x3FFC, a
/// 4-byte read at offset 0x4000 and an erase at offset 0x4000 (specification 34 §10.1; RETAIN-02: no
/// address outside the two slots is written). An access that ends exactly at the slot end, a 4-byte
/// program at offset 0x3FFC of slot B, is accepted at flash address 0x7FFFFC. Catches a bound that is
/// one unit too far, which would program the first word of the flash after slot B.
#[test]
fn an_offset_past_the_slot_end_is_refused() {
    let (mut flash, ops) = slot_flash();
    assert_eq!(
        flash.program(Slot::A, 0x4000, &[0; 4]),
        Err(FlashError::Program)
    );
    assert_eq!(
        flash.program(Slot::B, 0x3FFC, &[0; 8]),
        Err(FlashError::Program)
    );
    assert_eq!(
        flash.read(Slot::A, 0x4000, &mut [0; 4]),
        Err(FlashError::Read)
    );
    assert_eq!(flash.erase(Slot::A, 0x4000), Err(FlashError::Erase));
    assert!(ops.borrow().is_empty(), "no flash operation ran");

    assert_eq!(flash.program(Slot::B, 0x3FFC, &[0; 4]), Ok(()));
    assert_eq!(
        *ops.borrow(),
        [Op::Program {
            address: 0x7F_FFFC,
            len: 4
        }]
    );
}

/// Slot offsets map to flash addresses worked out by hand: slot A starts at 0x7F8000 and slot B at
/// 0x7FC000, so offset 0x3FFC of slot A is 0x7FBFFC and offset 0x3FFC of slot B is 0x7FFFFC, the last
/// word of the flash (0x7FC000 + 0x3FFC). Catches a slot base taken from the other slot and an offset
/// added to the wrong base.
#[test]
fn a_slot_offset_maps_to_its_flash_address_worked_out_by_hand() {
    let (mut flash, ops) = slot_flash();
    for (slot, offset) in [
        (Slot::A, 0),
        (Slot::A, 0x3FFC),
        (Slot::B, 0),
        (Slot::B, 0x3FFC),
    ] {
        assert_eq!(flash.program(slot, offset, &[0; 4]), Ok(()));
    }
    assert_eq!(
        *ops.borrow(),
        [
            Op::Program {
                address: 0x7F_8000,
                len: 4
            },
            Op::Program {
                address: 0x7F_BFFC,
                len: 4
            },
            Op::Program {
                address: 0x7F_C000,
                len: 4
            },
            Op::Program {
                address: 0x7F_FFFC,
                len: 4
            },
        ]
    );
}

/// The slot flash has the chip's units: 4-byte program units, 4 KiB erase units, four erase units in
/// a 16 KiB slot and 16,300 payload bytes for a record in a slot (specification 34 §10.1). Catches a
/// geometry taken from the STM32F407, which erases one 16 KiB sector.
#[test]
fn the_slot_flash_programs_words_and_erases_4_kib_units() {
    let (flash, _ops) = slot_flash();
    let geometry = flash.geometry();
    assert_eq!(
        (
            geometry.program_unit(),
            geometry.erase_unit(),
            geometry.slot_len()
        ),
        (4, 4_096, 16_384)
    );
    assert_eq!(geometry.erase_units(), 4);
    assert_eq!(geometry.max_payload_len(), 16_300);
}

/// The platform gives its checkpoint store once, with a program budget of one 256-byte page, so one
/// persister step between two scans programs at most a page (specification 34 §10). Catches a store
/// handed out twice, which would give two persisters one flash, and a budget of a whole slot, whose
/// erase time is then spent inside a single step.
#[test]
fn the_platform_gives_its_checkpoint_store_once_with_one_page_per_step() {
    let (flash, _ops) = slot_flash();
    let mut platform = platform_with(RetainMode::File, Box::new(flash));
    let store = platform
        .take_checkpoint_store()
        .expect("a file-mode record has a checkpoint store");
    assert_eq!(store.program_budget, 256);
    assert_eq!(store.flash.geometry().slot_len(), 16_384);
    assert!(
        platform.take_checkpoint_store().is_none(),
        "the store is given once"
    );
}

/// A record with the retain mode `none` gives no checkpoint store, and the platform does not keep
/// the flash (specification 34 §10: a `mode = "none"` project saves nothing). Catches a store for a
/// project that asked for no retained values, which would write checkpoints it did not ask for.
#[test]
fn a_record_with_retain_mode_none_gives_no_checkpoint_store() {
    let (flash, ops) = slot_flash();
    let mut platform = platform_with(RetainMode::None, Box::new(flash));
    assert!(platform.take_checkpoint_store().is_none());
    assert!(ops.borrow().is_empty(), "no flash operation ran");
}
