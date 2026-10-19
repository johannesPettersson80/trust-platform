//! The C6's program slots and the one owner of its flash (specification 34 §5.1 and §10.1). The flash
//! under the regions is a fake: a recording one shows exactly the operations that reached the flash,
//! and a byte-holding one keeps what was programmed. Each expected value is worked out by hand from the
//! specification and the chip file.

use std::cell::RefCell;
use std::rc::Rc;

use trust_chips::Catalog;
use trust_platform_esp32::checkpoint::RawFlash;
use trust_platform_esp32::program_flash::{
    FlashOwner, ProgramSlots, SLOT_A_OFFSET, SLOT_BYTES, SLOT_B_OFFSET,
};
use trust_runtime_core::ports::ProgramFlash;
use trust_runtime_core::retain::checkpoint::{CheckpointFlash, FlashError, Slot};

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
            Self::Erase { address } => (address, 4_096),
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

/// The program slots over a recording fake, and the record of the operations that reached it.
fn program_slots() -> (ProgramSlots<RecordingFlash>, Rc<RefCell<Vec<Op>>>) {
    let ops = Rc::new(RefCell::new(Vec::new()));
    let slots = ProgramSlots::new(RecordingFlash {
        ops: Rc::clone(&ops),
    })
    .expect("the C6's geometry holds a program slot");
    (slots, ops)
}

/// The flash of the bench's 8 MB module, from flash address 0, holding the bytes it was given.
const FLASH_BYTES: usize = 0x80_0000;
/// The erase unit of the C6's flash.
const ERASE_UNIT: usize = 4_096;

/// A flash that keeps its bytes: it is erased to 0xFF, a program clears bits, and an erase sets one
/// 4 KiB unit back to 0xFF.
struct RamFlash {
    bytes: Vec<u8>,
}

impl RamFlash {
    fn erased() -> Self {
        Self {
            bytes: vec![0xFF; FLASH_BYTES],
        }
    }
}

impl RawFlash for RamFlash {
    fn read_at(&mut self, address: u32, bytes: &mut [u8]) -> Result<(), FlashError> {
        let start = usize::try_from(address).map_err(|_| FlashError::Read)?;
        let stored = self
            .bytes
            .get(start..start + bytes.len())
            .ok_or(FlashError::Read)?;
        bytes.copy_from_slice(stored);
        Ok(())
    }

    fn program_at(&mut self, address: u32, bytes: &[u8]) -> Result<(), FlashError> {
        let start = usize::try_from(address).map_err(|_| FlashError::Program)?;
        let stored = self
            .bytes
            .get_mut(start..start + bytes.len())
            .ok_or(FlashError::Program)?;
        for (cell, byte) in stored.iter_mut().zip(bytes) {
            *cell &= byte;
        }
        Ok(())
    }

    fn erase_at(&mut self, address: u32) -> Result<(), FlashError> {
        let start = usize::try_from(address).map_err(|_| FlashError::Erase)?;
        let unit = self
            .bytes
            .get_mut(start..start + ERASE_UNIT)
            .ok_or(FlashError::Erase)?;
        unit.fill(0xFF);
        Ok(())
    }
}

/// An access that runs past its slot is refused before the flash sees it (specification 34 §5.1:
/// the store writes whole units inside its slot). Slot A ends at flash address 0x4F_C000, where slot
/// B begins, and slot B ends at 0x7F_8000, where checkpoint slot A begins. A write that ran past
/// either end would land in the other program slot or in a checkpoint slot, and an erase at the end
/// of slot B would clear checkpoint slot A's first unit. Each such access is refused and reaches
/// nothing, and the last unit of each slot is accepted. Catches a bound one unit too far, which
/// would program the first word after a slot.
#[test]
fn a_slot_refuses_an_access_outside_its_region() {
    let (mut slots, ops) = program_slots();
    // The last 4-byte unit of slot A is at offset 3,129,340. An 8-byte write there runs into slot B.
    assert_eq!(
        slots.program(Slot::A, SLOT_BYTES - 4, &[0; 8]),
        Err(FlashError::Program)
    );
    // A write that starts at the end of slot A starts in slot B.
    assert_eq!(
        slots.program(Slot::A, SLOT_BYTES, &[0; 4]),
        Err(FlashError::Program)
    );
    // An 8-byte write at the last unit of slot B runs into checkpoint slot A at 0x7F_8000.
    assert_eq!(
        slots.program(Slot::B, SLOT_BYTES - 4, &[0; 8]),
        Err(FlashError::Program)
    );
    // The erase unit at the end of slot B is checkpoint slot A's first unit.
    assert_eq!(slots.erase(Slot::B, SLOT_BYTES), Err(FlashError::Erase));
    assert_eq!(
        slots.read(Slot::A, SLOT_BYTES, &mut [0; 4]),
        Err(FlashError::Read)
    );
    assert!(ops.borrow().is_empty(), "no flash operation ran");

    // The last unit of slot A is at 0x4F_BFFC, and that of slot B at 0x7F_7FFC.
    assert_eq!(slots.program(Slot::A, SLOT_BYTES - 4, &[0; 4]), Ok(()));
    assert_eq!(slots.program(Slot::B, SLOT_BYTES - 4, &[0; 4]), Ok(()));
    let reached: Vec<(u32, u64)> = ops.borrow().iter().map(Op::span).collect();
    assert_eq!(reached, [(0x4F_BFFC, 4), (0x7F_7FFC, 4)]);
}

/// The store reads and programs whole 4-byte program units, at multiples of the unit (specification
/// 34 §5.1). A program at offset 2, a 3-byte program, a read at offset 1 and a 3-byte read are refused
/// and reach nothing. A whole unit at offset 4 of slot A is programmed at flash address 0x20_0004.
/// Catches a driver that programs a partial unit, which leaves a word that is neither the old record
/// nor the new one.
#[test]
fn a_program_or_read_that_is_not_whole_units_is_refused() {
    let (mut slots, ops) = program_slots();
    assert_eq!(slots.program(Slot::A, 2, &[0; 4]), Err(FlashError::Program));
    assert_eq!(slots.program(Slot::A, 0, &[0; 3]), Err(FlashError::Program));
    assert_eq!(slots.read(Slot::B, 1, &mut [0; 4]), Err(FlashError::Read));
    assert_eq!(slots.read(Slot::B, 0, &mut [0; 3]), Err(FlashError::Read));
    assert!(ops.borrow().is_empty(), "no flash operation ran");

    assert_eq!(slots.program(Slot::A, 4, &[0; 4]), Ok(()));
    assert_eq!(
        *ops.borrow(),
        [Op::Program {
            address: 0x20_0004,
            len: 4
        }]
    );
}

/// An erase clears one whole 4 KiB erase unit that starts at its first byte (specification 34 §5.1).
/// An erase 0x800 into slot A is refused and erases nothing. The unit at offset 0x3000 of slot B is
/// erased at flash address 0x4F_F000 (0x4F_C000 + 0x3000), and the first unit of slot A at 0x20_0000.
/// Catches an erase that starts mid-unit, which clears a unit other than the one addressed, and an
/// erase that takes its base from the other slot.
#[test]
fn an_erase_starts_at_the_first_byte_of_a_4_kib_unit() {
    let (mut slots, ops) = program_slots();
    assert_eq!(slots.erase(Slot::A, 0x800), Err(FlashError::Erase));
    assert!(ops.borrow().is_empty(), "no flash operation ran");

    assert_eq!(slots.erase(Slot::B, 0x3000), Ok(()));
    assert_eq!(slots.erase(Slot::A, 0), Ok(()));
    assert_eq!(
        *ops.borrow(),
        [
            Op::Erase { address: 0x4F_F000 },
            Op::Erase { address: 0x20_0000 },
        ]
    );
}

/// The program slots are the chip file's `application_a` and `application_b`: 3,056 KiB each, at
/// 0x20_0000 and 0x4F_C000 (specification 34 Appendix H.2). The slot is 764 erase units of 4 KiB,
/// and 3,056 x 1,024 = 3,129,344 bytes. Catches slot numbers copied from the whole application
/// region (6,112 KiB) or from the checkpoint slots, and a slot B that starts where slot A does not end.
#[test]
fn slot_numbers_match_the_chip_file() {
    let file = Catalog::embedded()
        .expect("the chip files are valid")
        .chip("esp32c6")
        .cloned()
        .expect("the ESP32-C6 has a chip file");
    let a = file
        .layout
        .application_a
        .expect("the ESP32-C6 names slot A");
    let b = file
        .layout
        .application_b
        .expect("the ESP32-C6 names slot B");
    assert_eq!((a.start, a.kib), (0x20_0000, 3_056));
    assert_eq!((b.start, b.kib), (0x4F_C000, 3_056));
    assert_eq!(a.bytes(), 3_129_344);
    assert_eq!(u64::from(b.start), a.end());

    assert_eq!(
        (SLOT_A_OFFSET, SLOT_B_OFFSET, SLOT_BYTES),
        (0x20_0000, 0x4F_C000, 3_129_344)
    );
    let (slots, _ops) = program_slots();
    let geometry = slots.geometry();
    assert_eq!(
        (
            geometry.program_unit(),
            geometry.erase_unit(),
            geometry.slot_len()
        ),
        (4, 4_096, 3_129_344)
    );
    assert_eq!(geometry.erase_units(), 764);
}

/// The boot reads the installed application after the checkpoint slots and the program slots are
/// taken, and it reads what the program slots wrote, because both are the one flash (specification
/// 34 §5.1: an installed container begins with its magic, `TRSTA002`). Catches a region that takes
/// the flash away from the boot read, which would then find no container, and a boot read at the
/// wrong address.
#[test]
fn read_application_still_works_after_the_flash_is_shared() {
    let mut owner = FlashOwner::new(RamFlash::erased());
    let _checkpoint = owner
        .take_checkpoint()
        .expect("the checkpoint slots have a geometry")
        .expect("the checkpoint slots are handed out");
    let mut program = owner
        .take_program()
        .expect("the program slots have a geometry")
        .expect("the program slots are handed out");
    program
        .program(Slot::A, 0, b"TRSTA002")
        .expect("slot A is erased, so the magic programs");

    let mut magic = [0_u8; 8];
    owner
        .read_application(&mut magic)
        .expect("the installed application can be read");
    assert_eq!(&magic, b"TRSTA002");
}

/// The program slots are handed out once, and the checkpoint slots to one writer at a time, and the boot
/// read stays available. The second hand-out of the program slots answers `None`, and so does the second
/// hand-out of the checkpoint slots while the first writer is alive, so two writers never drive the same
/// slots; the boot read succeeds after both were taken. Catches a second hand-out that gives a second
/// writer to the same slots or to the checkpoint log.
#[test]
fn each_region_is_handed_out_once() {
    let mut owner = FlashOwner::new(RamFlash::erased());
    assert!(owner
        .take_program()
        .expect("the program slots have a geometry")
        .is_some());
    assert!(
        owner
            .take_program()
            .expect("the program slots have a geometry")
            .is_none(),
        "the program slots are handed out once"
    );
    let writer = owner
        .take_checkpoint()
        .expect("the checkpoint slots have a geometry")
        .expect("the checkpoint slots are handed out");
    assert!(
        owner
            .take_checkpoint()
            .expect("the checkpoint slots have a geometry")
            .is_none(),
        "the checkpoint slots are handed out to one writer at a time"
    );
    assert_eq!(owner.read_application(&mut [0; 4]), Ok(()));
    drop(writer);
}

/// The program that is loaded in place takes the checkpoint flash of the program that ended (specification
/// 34 STATE-08): the checkpoint slots are handed out again once the writer that holds them is dropped,
/// and the new writer reaches the same flash, so a word the first one wrote at offset 8 of checkpoint
/// slot A (flash address 0x7F_8008) reads back through the second. Catches slots that are lost with the
/// first writer, which would leave the next program without a checkpoint store, and a second writer over
/// another flash.
#[test]
fn the_checkpoint_slots_are_handed_out_again_when_their_writer_is_dropped() {
    let mut owner = FlashOwner::new(RamFlash::erased());
    let mut first = owner
        .take_checkpoint()
        .expect("the checkpoint slots have a geometry")
        .expect("the checkpoint slots are handed out");
    first
        .program(Slot::A, 8, b"TRCK")
        .expect("checkpoint slot A is erased, so the word programs");
    assert!(
        owner
            .take_checkpoint()
            .expect("the checkpoint slots have a geometry")
            .is_none(),
        "no second writer while the first one lives"
    );
    drop(first);

    let mut second = owner
        .take_checkpoint()
        .expect("the checkpoint slots have a geometry")
        .expect("the slots are handed out again once the first writer is gone");
    let mut word = [0_u8; 4];
    second
        .read(Slot::A, 8, &mut word)
        .expect("the second writer reads slot A");
    assert_eq!(&word, b"TRCK");
}

/// The retained-value check of an install reads the newest checkpoint through a second handle on the
/// checkpoint slots (specification 11 §6.7.1, decision G13; specification 34 §10.1), because the
/// writing handle belongs to the installed program's cycle. Both handles are the one flash: a word
/// programmed through the writer at offset 8 of checkpoint slot B, at flash address 0x7F_C008, reads
/// back through the reader, and the reader can be taken again after the writer and the program slots
/// were handed out. Catches a reader over another flash, or at another slot's address, which would
/// read an erased log and let every install past the check.
#[test]
fn the_checkpoint_reader_reads_what_the_checkpoint_handle_wrote() {
    let mut owner = FlashOwner::new(RamFlash::erased());
    let mut writer = owner
        .take_checkpoint()
        .expect("the checkpoint slots have a geometry")
        .expect("the checkpoint slots are handed out");
    let _program = owner
        .take_program()
        .expect("the program slots have a geometry")
        .expect("the program slots are handed out");
    let mut reader = owner
        .checkpoint_reader()
        .expect("the checkpoint slots have a geometry");

    writer
        .program(Slot::B, 8, b"TRCK")
        .expect("checkpoint slot B is erased, so the word programs");
    let mut word = [0_u8; 4];
    reader
        .read(Slot::B, 8, &mut word)
        .expect("the reader reads slot B");
    assert_eq!(&word, b"TRCK");
    assert!(
        owner.checkpoint_reader().is_ok(),
        "a reader can be taken again"
    );
}
