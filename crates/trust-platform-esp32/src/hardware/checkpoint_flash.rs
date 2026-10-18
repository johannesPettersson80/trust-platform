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

use crate::checkpoint::RawFlash;
use crate::profile::{CHECKPOINT_STEP_BYTES, FLASH_ERASE_BYTES};

/// The bytes one transfer moves: one program page, which is also the most a checkpoint step programs.
const PAGE_BYTES: usize = CHECKPOINT_STEP_BYTES;

/// A transfer buffer aligned for whole words, so that `esp-storage` takes its direct path.
#[repr(C, align(4))]
struct Page([u8; PAGE_BYTES]);

impl RawFlash for FlashStorage<'_> {
    fn read_at(&mut self, address: u32, bytes: &mut [u8]) -> Result<(), FlashError> {
        let mut page = Page([0; PAGE_BYTES]);
        let mut at = address;
        for part in bytes.chunks_mut(PAGE_BYTES) {
            let buffer = &mut page.0[..part.len()];
            self.read_nor(at, buffer).map_err(|_| FlashError::Read)?;
            part.copy_from_slice(buffer);
            at = next_address(at, part.len(), FlashError::Read)?;
        }
        Ok(())
    }

    fn program_at(&mut self, address: u32, bytes: &[u8]) -> Result<(), FlashError> {
        let mut page = Page([0; PAGE_BYTES]);
        let mut at = address;
        for part in bytes.chunks(PAGE_BYTES) {
            let buffer = &mut page.0[..part.len()];
            buffer.copy_from_slice(part);
            self.write_nor(at, buffer)
                .map_err(|_| FlashError::Program)?;
            at = next_address(at, part.len(), FlashError::Program)?;
        }
        Ok(())
    }

    fn erase_at(&mut self, address: u32) -> Result<(), FlashError> {
        let end = address
            .checked_add(FLASH_ERASE_BYTES)
            .ok_or(FlashError::Erase)?;
        self.erase(address, end).map_err(|_| FlashError::Erase)
    }
}

/// The address `len` bytes after `at`, or `error` when it cannot be formed.
fn next_address(at: u32, len: usize, error: FlashError) -> Result<u32, FlashError> {
    u32::try_from(len)
        .ok()
        .and_then(|len| at.checked_add(len))
        .ok_or(error)
}
