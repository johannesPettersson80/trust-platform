//! The C6's checkpoint slots as a checkpoint flash (specification 34 §10.1 and D.2).
//!
//! The chip keeps its retain checkpoints in the two slots of its chip file, `checkpoint_a` and
//! `checkpoint_b`. [`SlotFlash`] gives the checkpoint log those slots at the slots' own offsets. It
//! refuses every access the flash cannot take before the flash sees it: a read or a program that is
//! not on whole 4-byte units or runs past its slot, and an erase that does not start at one of the
//! slot's 4 KiB erase units. The flash itself is a [`RawFlash`] at absolute addresses, which the
//! hardware adapter implements over `esp-storage`, so these rules are host-testable.

use trust_runtime_core::retain::checkpoint::{
    CheckpointError, CheckpointFlash, FlashError, FlashGeometry, Slot,
};

use crate::profile::{
    CHECKPOINT_A_OFFSET, CHECKPOINT_B_OFFSET, CHECKPOINT_SLOT_BYTES, FLASH_ERASE_BYTES,
    FLASH_PROGRAM_BYTES,
};

/// The flash of the chip at absolute addresses, as the hardware adapter reads, programs and erases
/// it. Each access is whole program units inside the flash's own bounds; [`SlotFlash`] makes sure
/// of that before it calls.
pub trait RawFlash {
    /// Reads `bytes.len()` bytes from `address`.
    ///
    /// # Errors
    ///
    /// [`FlashError::Read`] when the flash reports the read as failed.
    fn read_at(&mut self, address: u32, bytes: &mut [u8]) -> Result<(), FlashError>;

    /// Programs `bytes` at `address`: a bit that is 0 in `bytes` becomes 0, and no bit becomes 1.
    ///
    /// # Errors
    ///
    /// [`FlashError::Program`] when the flash reports the program as failed.
    fn program_at(&mut self, address: u32, bytes: &[u8]) -> Result<(), FlashError>;

    /// Erases the 4 KiB erase unit that starts at `address`: every byte of it becomes 0xFF.
    ///
    /// # Errors
    ///
    /// [`FlashError::Erase`] when the flash reports the erase as failed.
    fn erase_at(&mut self, address: u32) -> Result<(), FlashError>;
}

/// The flash offset of a slot.
const fn slot_base(slot: Slot) -> u32 {
    match slot {
        Slot::A => CHECKPOINT_A_OFFSET,
        Slot::B => CHECKPOINT_B_OFFSET,
    }
}

/// The absolute address of the `len` bytes at `offset` in `slot`, or `None` when the offset or the
/// length is not a whole number of program units, or when the bytes do not lie inside the slot
/// (specification 34 §10.1: a program writes whole units at multiples of the unit, inside the slot).
fn access_address(slot: Slot, offset: u32, len: usize) -> Option<u32> {
    let len = u32::try_from(len).ok()?;
    let whole_units =
        offset.is_multiple_of(FLASH_PROGRAM_BYTES) && len.is_multiple_of(FLASH_PROGRAM_BYTES);
    let inside = offset.checked_add(len)? <= CHECKPOINT_SLOT_BYTES;
    if whole_units && inside {
        Some(slot_base(slot) + offset)
    } else {
        None
    }
}

/// The absolute address of the erase unit that starts at `offset` in `slot`, or `None` when
/// `offset` is not the first byte of one of the slot's erase units (specification 34 §10.1: an erase
/// clears one whole erase unit).
fn erase_address(slot: Slot, offset: u32) -> Option<u32> {
    if offset.is_multiple_of(FLASH_ERASE_BYTES) && offset < CHECKPOINT_SLOT_BYTES {
        Some(slot_base(slot) + offset)
    } else {
        None
    }
}

/// A chip's checkpoint flash: the two slots of its chip file over a [`RawFlash`] (specification 34
/// §10.1). The checkpoint log drives it through [`CheckpointFlash`].
pub struct SlotFlash<R> {
    raw: R,
    geometry: FlashGeometry,
}

impl<R> SlotFlash<R> {
    /// The checkpoint flash over `raw`, with the chip's geometry: 4-byte program units, 4 KiB erase
    /// units and 16 KiB slots (specification 34 §10.1).
    ///
    /// # Errors
    ///
    /// [`CheckpointError::Geometry`] when the geometry cannot hold the log.
    pub fn new(raw: R) -> Result<Self, CheckpointError> {
        let geometry = FlashGeometry::new(
            FLASH_PROGRAM_BYTES,
            FLASH_ERASE_BYTES,
            CHECKPOINT_SLOT_BYTES,
        )?;
        Ok(Self { raw, geometry })
    }
}

impl<R: RawFlash> CheckpointFlash for SlotFlash<R> {
    fn geometry(&self) -> FlashGeometry {
        self.geometry
    }

    fn read(&mut self, slot: Slot, offset: u32, bytes: &mut [u8]) -> Result<(), FlashError> {
        let address = access_address(slot, offset, bytes.len()).ok_or(FlashError::Read)?;
        self.raw.read_at(address, bytes)
    }

    fn program(&mut self, slot: Slot, offset: u32, bytes: &[u8]) -> Result<(), FlashError> {
        let address = access_address(slot, offset, bytes.len()).ok_or(FlashError::Program)?;
        self.raw.program_at(address, bytes)
    }

    fn erase(&mut self, slot: Slot, offset: u32) -> Result<(), FlashError> {
        let address = erase_address(slot, offset).ok_or(FlashError::Erase)?;
        self.raw.erase_at(address)
    }
}
