//! The STM32U5A9's flash (specification 34 §10.1 and D.5; design §1.1 and §4.4; RM0456 and ST's
//! `stm32u5xx_hal_flash.c`): the bank and page of an address, the non-secure sequence that erases a
//! page and programs a quad-word, and the chip's two checkpoint slots as a [`CheckpointFlash`].
//!
//! The address rules and the slot rules are host-tested over a simulated flash. The register sequence
//! (`Nvm`) is compiled for the chip only and proved on the bench. The firmware runs from bank 1 while
//! a checkpoint erases or programs bank 2, which the chip's read-while-write allows (design §4.4), so a
//! checkpoint write never stalls the scan.

use trust_runtime_core::retain::checkpoint::{
    CheckpointError, CheckpointFlash, FlashError, FlashGeometry, Slot,
};

/// The first byte of the flash, bank 1, in the non-secure view (DS13543 §3.4).
pub const FLASH_BASE: u32 = 0x0800_0000;
/// The bytes of one bank: 256 pages of 8 KiB (DS13543 §3.4).
pub const BANK_BYTES: u32 = 2 * 1024 * 1024;
/// The first byte past the flash: two banks, 4 MiB (DS13543 §3.4).
pub const FLASH_END: u32 = FLASH_BASE + 2 * BANK_BYTES;
/// The bytes of one page, the erase unit (DS13543 §3.4; `FLASH_PAGE_SIZE`).
pub const PAGE_BYTES: u32 = 8 * 1024;
/// The pages of one bank (`FLASH_PAGE_NB`).
pub const PAGES_PER_BANK: u32 = BANK_BYTES / PAGE_BYTES;
/// The bytes of one quad-word, the program unit: a program writes 16 bytes at a 16-byte address
/// (RM0456; `FLASH_TYPEPROGRAM_QUADWORD`).
pub const QUAD_WORD_BYTES: u32 = 16;

/// The chip file's `checkpoint_a` slot: pages 508 and 509 of the flash, pages 252 and 253 of bank 2
/// (design §4.4).
pub const CHECKPOINT_A: u32 = 0x083F_8000;
/// The chip file's `checkpoint_b` slot: pages 510 and 511, the last pages of the flash (design §4.4).
pub const CHECKPOINT_B: u32 = 0x083F_C000;
/// The bytes of each checkpoint slot: two pages (design §4.4).
pub const CHECKPOINT_SLOT_BYTES: u32 = 16 * 1024;

/// How many polls a register wait makes before it fails: 2^20, about 65 ms at 160 MHz if a poll takes
/// ten cycles, against an erase of 3.4 ms at most (DS13543 Table 80).
pub const FLASH_POLLS: u32 = 1 << 20;

/// One of the two flash banks (RM0456: NSCR.BKER is clear for bank 1 and set for bank 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bank {
    /// Bank 1, from 0x0800_0000: the firmware.
    One,
    /// Bank 2, from 0x0820_0000: the application and the checkpoint slots.
    Two,
}

/// The bank and the page within it of a flash address, or `None` outside the flash (design §1.1).
pub fn page_of(address: u32) -> Option<(Bank, u32)> {
    if !(FLASH_BASE..FLASH_END).contains(&address) {
        return None;
    }
    let offset = address - FLASH_BASE;
    let (bank, within) = if offset < BANK_BYTES {
        (Bank::One, offset)
    } else {
        (Bank::Two, offset - BANK_BYTES)
    };
    Some((bank, within / PAGE_BYTES))
}

/// Whether `address` starts a quad-word the flash can program: 16-byte aligned, with all 16 bytes
/// inside the flash.
pub fn is_quad_word(address: u32) -> bool {
    address.is_multiple_of(QUAD_WORD_BYTES)
        && (FLASH_BASE..=FLASH_END - QUAD_WORD_BYTES).contains(&address)
}

/// Why a flash operation failed on the chip (RM0456, NSSR; ST's `FLASH_WaitForLastOperation`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fault {
    /// An NSSR error flag was set: `OPERR`, `PROGERR`, `WRPERR`, `PGAERR`, `SIZERR` or `PGSERR`.
    Flag(&'static str),
    /// The control register stayed locked after the two keys were written.
    Locked,
    /// The address is not a quad-word inside the flash, or the page is not a page of the bank.
    Address,
    /// The named wait did not end within [`FLASH_POLLS`] polls.
    Timeout(&'static str),
}

/// The flash at absolute addresses, as the chip's registers drive it (`Nvm`) and as a host
/// simulation does in the tests. [`SlotFlash`] keeps every call inside the slots before it makes one.
pub trait RawFlash {
    /// Reads `bytes.len()` bytes from `address`.
    fn read_at(&self, address: u32, bytes: &mut [u8]);

    /// Programs the quad-word at `address`: a bit that is 0 in `bytes` becomes 0, and a quad-word is
    /// programmed once between two erases of its page.
    ///
    /// # Errors
    ///
    /// [`Fault::Address`] when `address` is not a quad-word inside the flash; otherwise the fault of
    /// the operation.
    fn program_quad_word(&mut self, address: u32, bytes: &[u8; 16]) -> Result<(), Fault>;

    /// Erases page `page` of `bank`: every byte of it becomes 0xFF.
    ///
    /// # Errors
    ///
    /// [`Fault::Address`] when `page` is not a page of a bank; otherwise the fault of the operation.
    fn erase_page(&mut self, bank: Bank, page: u32) -> Result<(), Fault>;
}

/// The chip's two checkpoint slots over a [`RawFlash`] (specification 34 §10.1): the geometry of design
/// §4.4 (16-byte program units, 8 KiB erase units that are pages, 16 KiB slots) and the slots' addresses.
/// It refuses an access the flash cannot take before the flash sees it.
pub struct SlotFlash<R> {
    raw: R,
    geometry: FlashGeometry,
}

impl<R> SlotFlash<R> {
    /// The checkpoint flash over `raw` with the chip's geometry.
    ///
    /// # Errors
    ///
    /// [`CheckpointError::Geometry`] when the geometry cannot hold the log.
    pub fn new(raw: R) -> Result<Self, CheckpointError> {
        let geometry = FlashGeometry::new(QUAD_WORD_BYTES, PAGE_BYTES, CHECKPOINT_SLOT_BYTES)?;
        Ok(Self { raw, geometry })
    }
}

/// The flash address of the first byte of `slot`.
const fn slot_base(slot: Slot) -> u32 {
    match slot {
        Slot::A => CHECKPOINT_A,
        Slot::B => CHECKPOINT_B,
    }
}

/// The flash address of the `len` bytes at `offset` in `slot`, or `None` when they run past the slot.
fn slot_address(slot: Slot, offset: u32, len: usize) -> Option<u32> {
    let len = u32::try_from(len).ok()?;
    let end = offset.checked_add(len)?;
    (end <= CHECKPOINT_SLOT_BYTES).then(|| slot_base(slot) + offset)
}

impl<R: RawFlash> CheckpointFlash for SlotFlash<R> {
    fn geometry(&self) -> FlashGeometry {
        self.geometry
    }

    /// A read needs no alignment: the log reads the last bytes of a payload from their unit.
    fn read(&mut self, slot: Slot, offset: u32, bytes: &mut [u8]) -> Result<(), FlashError> {
        let address = slot_address(slot, offset, bytes.len()).ok_or(FlashError::Read)?;
        self.raw.read_at(address, bytes);
        Ok(())
    }

    /// A program writes whole quad-words: its offset and its length are multiples of 16 (RM0456).
    fn program(&mut self, slot: Slot, offset: u32, bytes: &[u8]) -> Result<(), FlashError> {
        if !offset.is_multiple_of(QUAD_WORD_BYTES) || !bytes.len().is_multiple_of(16) {
            return Err(FlashError::Program);
        }
        let mut address = slot_address(slot, offset, bytes.len()).ok_or(FlashError::Program)?;
        for quad_word in bytes.as_chunks::<16>().0 {
            self.raw
                .program_quad_word(address, quad_word)
                .map_err(|_| FlashError::Program)?;
            address += QUAD_WORD_BYTES;
        }
        Ok(())
    }

    /// An erase clears the whole page that holds `offset`; each slot is two pages, its two erase units.
    fn erase(&mut self, slot: Slot, offset: u32) -> Result<(), FlashError> {
        if offset >= CHECKPOINT_SLOT_BYTES {
            return Err(FlashError::Erase);
        }
        let (bank, page) = page_of(slot_base(slot) + offset).ok_or(FlashError::Erase)?;
        self.raw
            .erase_page(bank, page)
            .map_err(|_| FlashError::Erase)
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
pub use hardware::Nvm;

#[cfg(all(target_arch = "arm", target_os = "none"))]
mod hardware {
    use core::ptr::{read_volatile, write_volatile};

    use stm32u5::stm32u5a9 as pac;

    use super::{is_quad_word, Bank, Fault, RawFlash, FLASH_POLLS, PAGES_PER_BANK};

    /// The first key that unlocks NSCR, written to NSKEYR before [`KEY2`] (RM0456).
    const KEY1: u32 = 0x4567_0123;
    /// The second key that unlocks NSCR (RM0456).
    const KEY2: u32 = 0xCDEF_89AB;

    /// The flash through its non-secure registers (RM0456; ST's `stm32u5xx_hal_flash.c`), over the
    /// chip's `FLASH` block. Each erase or program unlocks NSCR, runs its sequence and locks it again.
    pub struct Nvm<'a> {
        flash: &'a pac::FLASH,
    }

    impl<'a> Nvm<'a> {
        /// The flash over the chip's `FLASH` block.
        pub fn new(flash: &'a pac::FLASH) -> Self {
            Self { flash }
        }

        /// Writes the two keys while NSCR is locked (ST's `HAL_FLASH_Unlock`) and checks it opened.
        fn unlock(&self) -> Result<(), Fault> {
            if self.flash.nscr().read().lock().bit() {
                self.flash.nskeyr().write(|w| unsafe { w.bits(KEY1) });
                self.flash.nskeyr().write(|w| unsafe { w.bits(KEY2) });
                if self.flash.nscr().read().lock().bit() {
                    return Err(Fault::Locked);
                }
            }
            Ok(())
        }

        /// Locks NSCR again after an operation (ST's `HAL_FLASH_Lock`).
        fn lock(&self) {
            self.flash.nscr().modify(|_, w| w.lock().set_bit());
        }

        /// Polls NSSR until neither BSY nor WDW is set; ST's `FLASH_WaitForLastOperation` waits on both.
        fn wait_idle(&self) -> Result<(), Fault> {
            for _ in 0..FLASH_POLLS {
                let status = self.flash.nssr().read();
                if !status.bsy().bit() && !status.wdw().bit() {
                    return Ok(());
                }
            }
            Err(Fault::Timeout("busy"))
        }

        /// Clears the error flags and EOP. NSSR clears a flag when its bit is written back as one (ST's
        /// `(*reg_sr) = error`).
        fn clear_status(&self) {
            self.flash.nssr().write(|w| {
                w.operr()
                    .set_bit()
                    .progerr()
                    .set_bit()
                    .wrperr()
                    .set_bit()
                    .pgaerr()
                    .set_bit()
                    .sizerr()
                    .set_bit()
                    .pgserr()
                    .set_bit()
                    .eop()
                    .set_bit()
            });
        }

        /// The first error flag of NSSR that is set, by its name.
        fn error_flag(&self) -> Option<&'static str> {
            let status = self.flash.nssr().read();
            [
                ("OPERR", status.operr().bit()),
                ("PROGERR", status.progerr().bit()),
                ("WRPERR", status.wrperr().bit()),
                ("PGAERR", status.pgaerr().bit()),
                ("SIZERR", status.sizerr().bit()),
                ("PGSERR", status.pgserr().bit()),
            ]
            .into_iter()
            .find_map(|(name, set)| set.then_some(name))
        }

        /// Waits for the operation, reads its error flags, clears them, and reports the first one set.
        fn finish(&self) -> Result<(), Fault> {
            self.wait_idle()?;
            let flag = self.error_flag();
            self.clear_status();
            match flag {
                Some(name) => Err(Fault::Flag(name)),
                None => Ok(()),
            }
        }

        /// The quad-word sequence: wait, PG, the four words with interrupts masked, the wait, PG cleared.
        fn program_unlocked(&self, address: u32, bytes: &[u8; 16]) -> Result<(), Fault> {
            self.wait_idle()?;
            self.clear_status();
            self.flash.nscr().modify(|_, w| w.pg().set_bit());
            cortex_m::interrupt::free(|_| {
                for (offset, word) in (0_u32..).step_by(4).zip(bytes.as_chunks::<4>().0) {
                    let value = u32::from_le_bytes(*word);
                    // SAFETY: `address` is a quad-word inside the flash, so each of its words is a
                    // programmable flash address. The write is the flash's program operation; the
                    // critical section keeps the four words of the quad-word in one run (ST's
                    // `FLASH_Program_QuadWord`).
                    unsafe { write_volatile((address + offset) as *mut u32, value) };
                }
            });
            let outcome = self.finish();
            self.flash.nscr().modify(|_, w| w.pg().clear_bit());
            outcome
        }

        /// The page sequence: wait, the bank, the page and PER, then STRT, the wait, PER cleared.
        fn erase_unlocked(&self, bank: Bank, page: u32) -> Result<(), Fault> {
            self.wait_idle()?;
            self.clear_status();
            let page = u8::try_from(page).map_err(|_| Fault::Address)?;
            let bker = bank == Bank::Two;
            self.flash.nscr().modify(|_, w| {
                // SAFETY: PNB is an eight-bit field (bits 3 to 10), and a bank has 256 pages.
                let w = unsafe { w.pnb().bits(page) };
                w.bker().bit(bker).per().set_bit()
            });
            self.flash.nscr().modify(|_, w| w.strt().set_bit());
            let outcome = self.finish();
            self.flash.nscr().modify(|_, w| w.per().clear_bit());
            outcome
        }
    }

    impl RawFlash for Nvm<'_> {
        fn read_at(&self, address: u32, bytes: &mut [u8]) {
            for (offset, byte) in (0_u32..).zip(bytes.iter_mut()) {
                // SAFETY: `SlotFlash` keeps every read inside the checkpoint slots, which are flash
                // memory; a volatile read of the flash has no effect on it.
                *byte = unsafe { read_volatile((address + offset) as *const u8) };
            }
        }

        fn program_quad_word(&mut self, address: u32, bytes: &[u8; 16]) -> Result<(), Fault> {
            if !is_quad_word(address) {
                return Err(Fault::Address);
            }
            self.unlock()?;
            let outcome = self.program_unlocked(address, bytes);
            self.lock();
            outcome
        }

        fn erase_page(&mut self, bank: Bank, page: u32) -> Result<(), Fault> {
            if page >= PAGES_PER_BANK {
                return Err(Fault::Address);
            }
            self.unlock()?;
            let outcome = self.erase_unlocked(bank, page);
            self.lock();
            outcome
        }
    }
}
