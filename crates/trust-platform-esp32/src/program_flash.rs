//! The C6's program slots (specification 34 §5.1) and the one owner of the flash that they share
//! with the checkpoint slots (specification 34 §10.1).
//!
//! The chip has one flash. [`FlashOwner`] owns it once and hands out each region as a handle over
//! that one flash: [`ProgramSlots`] reaches the two program slots, the checkpoint log's
//! [`SlotFlash`] reaches the checkpoint slots, and
//! [`FlashOwner::read_application`] reads the installed application where it begins. Each handle
//! refuses every access outside its own region before the flash sees it, so a program write can
//! never reach a checkpoint slot, and a checkpoint write never reaches a program slot.

use alloc::rc::Rc;
use core::cell::RefCell;

use trust_runtime_core::ports::ProgramFlash;
use trust_runtime_core::retain::checkpoint::{CheckpointError, FlashError, FlashGeometry, Slot};

use crate::checkpoint::{RawFlash, SlotFlash};
use crate::profile::{FLASH_ERASE_BYTES, FLASH_PROGRAM_BYTES};

/// The bytes of one program slot: 3,056 KiB, which is 764 erase units (`application_a` of
/// `chips/esp32c6.toml`).
pub const SLOT_BYTES: u32 = 3_056 * 1024;

/// The flash offset of slot A, the chip file's `application_a`. The installed application begins
/// here, so the boot reads slot A until it selects the newest committed slot itself.
pub const SLOT_A_OFFSET: u32 = 0x20_0000;

/// The flash offset of slot B, the chip file's `application_b`, which follows slot A.
pub const SLOT_B_OFFSET: u32 = 0x4F_C000;

/// The flash offset of a program slot.
const fn slot_base(slot: Slot) -> u32 {
    match slot {
        Slot::A => SLOT_A_OFFSET,
        Slot::B => SLOT_B_OFFSET,
    }
}

/// The absolute address of the `len` bytes at `offset` in `slot`, or `None` when the offset or the
/// length is not a whole number of program units, or when the bytes do not lie inside the slot
/// (specification 34 §5.1: an install writes whole units inside its slot).
fn access_address(slot: Slot, offset: u32, len: usize) -> Option<u32> {
    let len = u32::try_from(len).ok()?;
    let whole_units =
        offset.is_multiple_of(FLASH_PROGRAM_BYTES) && len.is_multiple_of(FLASH_PROGRAM_BYTES);
    let inside = offset.checked_add(len)? <= SLOT_BYTES;
    if whole_units && inside {
        Some(slot_base(slot) + offset)
    } else {
        None
    }
}

/// The absolute address of the erase unit that starts at `offset` in `slot`, or `None` when
/// `offset` is not the first byte of one of the slot's erase units (specification 34 §5.1: an
/// erase clears one whole erase unit).
fn erase_address(slot: Slot, offset: u32) -> Option<u32> {
    if offset.is_multiple_of(FLASH_ERASE_BYTES) && offset < SLOT_BYTES {
        Some(slot_base(slot) + offset)
    } else {
        None
    }
}

/// The two program slots over a [`RawFlash`], as the program store drives them through
/// [`ProgramFlash`] (specification 34 §5.1). The geometry is the C6's: 4-byte program units, 4 KiB
/// erase units and slots of [`SLOT_BYTES`].
pub struct ProgramSlots<R> {
    raw: R,
    geometry: FlashGeometry,
}

impl<R> ProgramSlots<R> {
    /// The program slots over `raw`.
    ///
    /// # Errors
    ///
    /// [`CheckpointError::Geometry`] when the flash geometry cannot hold a slot.
    pub fn new(raw: R) -> Result<Self, CheckpointError> {
        let geometry = FlashGeometry::new(FLASH_PROGRAM_BYTES, FLASH_ERASE_BYTES, SLOT_BYTES)?;
        Ok(Self { raw, geometry })
    }
}

impl<R: RawFlash> ProgramFlash for ProgramSlots<R> {
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

/// One flash, shared by the handles that [`FlashOwner`] hands out. Each access borrows the flash
/// for its own call only, so no handle holds it between two calls.
pub struct SharedFlash<R> {
    flash: Rc<RefCell<R>>,
}

impl<R> Clone for SharedFlash<R> {
    fn clone(&self) -> Self {
        Self {
            flash: Rc::clone(&self.flash),
        }
    }
}

// A call never runs inside another call on the same flash, so the borrow always succeeds. Should
// that ever change, the access reports the flash error instead of panicking.
impl<R: RawFlash> RawFlash for SharedFlash<R> {
    fn read_at(&mut self, address: u32, bytes: &mut [u8]) -> Result<(), FlashError> {
        let mut flash = self.flash.try_borrow_mut().map_err(|_| FlashError::Read)?;
        flash.read_at(address, bytes)
    }

    fn program_at(&mut self, address: u32, bytes: &[u8]) -> Result<(), FlashError> {
        let mut flash = self
            .flash
            .try_borrow_mut()
            .map_err(|_| FlashError::Program)?;
        flash.program_at(address, bytes)
    }

    fn erase_at(&mut self, address: u32) -> Result<(), FlashError> {
        let mut flash = self.flash.try_borrow_mut().map_err(|_| FlashError::Erase)?;
        flash.erase_at(address)
    }
}

/// The chip's one flash, owned once and split between its regions (specification 34 §5.1 and
/// §10.1): the two program slots, the two checkpoint slots and the installed application, which
/// begins at slot A. The program slots and the checkpoint slots are each handed out once; the boot
/// read is available at any time, so the boot can read the installed application after both are
/// taken.
pub struct FlashOwner<R> {
    flash: SharedFlash<R>,
    program_taken: bool,
    checkpoint_taken: bool,
}

impl<R: RawFlash> FlashOwner<R> {
    /// The owner of `raw`, with no region handed out yet.
    #[must_use]
    pub fn new(raw: R) -> Self {
        Self {
            flash: SharedFlash {
                flash: Rc::new(RefCell::new(raw)),
            },
            program_taken: false,
            checkpoint_taken: false,
        }
    }

    /// The program slots, handed out once: the second call answers `None`.
    ///
    /// # Errors
    ///
    /// [`CheckpointError::Geometry`] when the flash geometry cannot hold a slot.
    pub fn take_program(
        &mut self,
    ) -> Result<Option<ProgramSlots<SharedFlash<R>>>, CheckpointError> {
        if self.program_taken {
            return Ok(None);
        }
        let program = ProgramSlots::new(self.flash.clone())?;
        self.program_taken = true;
        Ok(Some(program))
    }

    /// The checkpoint slots, handed out once: the second call answers `None`.
    ///
    /// # Errors
    ///
    /// [`CheckpointError::Geometry`] when the flash geometry cannot hold the checkpoint log.
    pub fn take_checkpoint(
        &mut self,
    ) -> Result<Option<SlotFlash<SharedFlash<R>>>, CheckpointError> {
        if self.checkpoint_taken {
            return Ok(None);
        }
        let checkpoint = SlotFlash::new(self.flash.clone())?;
        self.checkpoint_taken = true;
        Ok(Some(checkpoint))
    }

    /// Reads the first `buffer.len()` bytes of the installed application, which begin at slot A.
    /// The boot reads them until it selects the newest committed slot itself (specification 34
    /// §5.1, boot selection).
    ///
    /// # Errors
    ///
    /// [`FlashError::Read`] when the flash cannot be read.
    pub fn read_application(&mut self, buffer: &mut [u8]) -> Result<(), FlashError> {
        self.flash.read_at(SLOT_A_OFFSET, buffer)
    }
}
