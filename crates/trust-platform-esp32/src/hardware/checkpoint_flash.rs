//! The C6's flash under the checkpoint slots: `esp-storage` at absolute addresses (specification 34
//! §10.1 and D.2).
//!
//! `esp-storage` runs each erase and each program in a RAM function, with the machine interrupts
//! masked by its critical section for the whole flash call, so no interrupt handler in flash runs
//! while the flash is busy. The transfers here still go through a word-aligned page of
//! `PAGE_BYTES`: `esp-storage` copies a buffer that is not word-aligned through a 4 KiB buffer on
//! the stack, and a checkpoint runs on the 24 KiB main stack.

use esp_storage::FlashStorage;
use trust_runtime_core::retain::checkpoint::FlashError;

use crate::checkpoint::{program_whole_units, read_whole_units, RawFlash};
use crate::profile::{CHECKPOINT_STEP_BYTES, FLASH_ERASE_BYTES};

/// The bytes one transfer moves: one program page, which is also the most a checkpoint step programs.
const PAGE_BYTES: usize = CHECKPOINT_STEP_BYTES;

/// A transfer buffer aligned for whole words, so that `esp-storage` takes its direct path.
#[repr(C, align(4))]
struct Page([u8; PAGE_BYTES]);

// `esp-storage` reads and programs whole 4-byte words only (`READ_SIZE` and `WRITE_SIZE`), so a
// transfer of any length goes through the whole-unit helpers: an installed container of 3,871 bytes
// could not be read before (bench, 18 October 2026).
impl RawFlash for FlashStorage<'_> {
    fn read_at(&mut self, address: u32, bytes: &mut [u8]) -> Result<(), FlashError> {
        let mut page = Page([0; PAGE_BYTES]);
        read_whole_units(address, bytes, &mut page.0, |at, buffer| {
            self.read_nor(at, buffer).map_err(|_| FlashError::Read)
        })
    }

    fn program_at(&mut self, address: u32, bytes: &[u8]) -> Result<(), FlashError> {
        let mut page = Page([0; PAGE_BYTES]);
        program_whole_units(address, bytes, &mut page.0, |at, buffer| {
            self.write_nor(at, buffer).map_err(|_| FlashError::Program)
        })
    }

    fn erase_at(&mut self, address: u32) -> Result<(), FlashError> {
        let end = address
            .checked_add(FLASH_ERASE_BYTES)
            .ok_or(FlashError::Erase)?;
        self.erase(address, end).map_err(|_| FlashError::Erase)
    }

    /// The bytes where the program image window maps them (specification 34 MEM-14).
    fn map_at(&mut self, address: u32, len: u32) -> Option<&'static [u8]> {
        super::image_window::map(address, len)
    }
}
