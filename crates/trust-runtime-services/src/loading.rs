//! The installed program of a device's program store (specification 34 §5.1 and §11.2; DEPLOY-03).
//!
//! The store holds two application slots. [`install`] checks a complete `TRSTA002` container and
//! commits it to the slot that does not hold the newest committed container, in steps the loop
//! advances one at a time. The commit point is the second half of the magic, programmed last.
//! [`select`] reads both slots after a reset and selects the newest committed container. A damaged container refuses the boot; the other slot
//! never runs in its place. Nothing here reads a link or a clock: the platform composes these with
//! its control loop.

pub mod install;
pub mod reload;
pub mod select;

#[cfg(test)]
mod tests;

use alloc::vec::Vec;

use trust_runtime_core::ports::ProgramStore;
use trust_runtime_core::retain::checkpoint::{FlashError, Slot};

/// Reads `len` bytes at `offset` in `slot`. The flash reads whole program units, so the read covers
/// the units around those bytes, and the padding is dropped from the result.
fn read_padded(
    store: &mut ProgramStore,
    slot: Slot,
    offset: u32,
    len: usize,
) -> Result<Vec<u8>, FlashError> {
    let unit = store.flash.geometry().program_unit() as usize;
    let mut bytes = alloc::vec![0xff_u8; len.next_multiple_of(unit)];
    store.flash.read(slot, offset, &mut bytes)?;
    bytes.truncate(len);
    Ok(bytes)
}
