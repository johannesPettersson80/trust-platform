//! An upload written into the program store as its parts arrive (specification 34 §5.1; the owner,
//! 23 October 2026: stream uploads into the inactive slot, the header last).
//!
//! A chip whose resource is Stopped writes an upload into the slot that an install would write when
//! the upload begins ([`slot`]). [`StoreWrite::begin`] erases the slot's first erase unit, which holds
//! the magic, so the container the slot held is no longer committed before any of its bytes is
//! replaced, and [`StoreWrite::part`] writes one part at its offset: the erase units that the part
//! reaches into and no earlier part reached into, then its bytes, then their read-back. The
//! container's header is not written: the transfer session keeps it, and the install programs it with
//! the magic last ([`super::install::start_stored`]). A write runs one step per call, as an install
//! does, so the loop serves its link between steps (STATE-08). Nothing here reads a link or a clock.

use core::ops::Range;

use trust_runtime_core::ports::ProgramStore;
use trust_runtime_core::retain::checkpoint::Slot;

use super::install::{install_slot, padded, step_bytes, InstallError};
use super::read_padded;
use super::select::{self, ScanError};
use crate::transfer::HEADER_BYTES;

/// The slot that an upload beginning now is written into: the slot an install would write (§5.1),
/// for `held`, the install sequence of the container that the program the runtime holds came from.
///
/// # Errors
///
/// The [`ScanError`] of a store whose committed containers cannot be ordered.
pub fn slot(store: &mut ProgramStore, held: Option<u64>) -> Result<Slot, ScanError> {
    Ok(install_slot(&select::order(store)?, held))
}

/// A write into the program store that runs in steps: the erase of an upload's begin, or one part.
#[derive(Debug)]
pub struct StoreWrite {
    slot: Slot,
    erase_unit: u32,
    program_unit: u32,
    /// The bytes one program or read-back step covers: whole program units, at most one budget.
    chunk: u32,
    /// The erase units still to erase, numbered from the start of the slot.
    erase: Range<u32>,
    /// The bytes of the slot that the part's bytes are programmed into.
    range: Range<u32>,
    /// The slot offset of the part's first byte: the bytes a step is given start there.
    first: u32,
    phase: Phase,
}

/// Where a write is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    /// Erase the next erase unit.
    Erase,
    /// Program the bytes from this slot offset.
    Program(u32),
    /// Read back the bytes from this slot offset.
    Verify(u32),
}

impl StoreWrite {
    /// The erase of the first erase unit of `slot`, the unit that holds the magic, which an upload's
    /// `transfer.begin` runs (§5.1).
    #[must_use]
    pub fn begin(store: &ProgramStore, slot: Slot) -> Self {
        Self::new(store, slot, 0..1, 0..0, 0)
    }

    /// The write of the part of `len` bytes at `offset` of an upload into `slot`: the erase units that
    /// the part reaches into and no earlier part reached into, the first unit excepted, which the
    /// upload's begin erased; then the part's bytes after the container's header.
    #[must_use]
    pub fn part(store: &ProgramStore, slot: Slot, offset: u32, len: u32) -> Self {
        let unit = store.flash.geometry().erase_unit();
        let end = offset.saturating_add(len);
        let first_unit = offset.div_ceil(unit).max(1);
        let erase = first_unit..end.div_ceil(unit).max(first_unit);
        let header_end = u32::try_from(HEADER_BYTES).unwrap_or(u32::MAX);
        let start = offset.max(header_end).min(end);
        Self::new(store, slot, erase, start..end, offset)
    }

    fn new(
        store: &ProgramStore,
        slot: Slot,
        erase: Range<u32>,
        range: Range<u32>,
        first: u32,
    ) -> Self {
        let geometry = store.flash.geometry();
        let phase = if erase.is_empty() {
            Phase::Program(range.start)
        } else {
            Phase::Erase
        };
        Self {
            slot,
            erase_unit: geometry.erase_unit(),
            program_unit: geometry.program_unit(),
            chunk: step_bytes(store.program_budget, geometry.program_unit()),
            erase,
            range,
            first,
            phase,
        }
    }

    /// Advances the write by one step: one erase unit, or at most one budget of programmed or
    /// read-back bytes. `bytes` are the part's bytes, from its first byte. Returns true once the
    /// write is done.
    ///
    /// # Errors
    ///
    /// [`InstallError::Flash`] when the flash fails, and [`InstallError::Verify`] when a programmed
    /// byte does not read back as written or `bytes` do not cover the part.
    pub fn step(&mut self, store: &mut ProgramStore, bytes: &[u8]) -> Result<bool, InstallError> {
        match self.phase {
            Phase::Erase => {
                if let Some(unit) = self.erase.next() {
                    store
                        .flash
                        .erase(self.slot, unit * self.erase_unit)
                        .map_err(InstallError::Flash)?;
                }
                if self.erase.is_empty() {
                    self.phase = Phase::Program(self.range.start);
                }
                Ok(self.range.is_empty() && self.erase.is_empty())
            }
            Phase::Program(next) => {
                if next < self.range.end {
                    let end = self.range.end.min(next.saturating_add(self.chunk));
                    let chunk = self.bytes(bytes, next..end)?;
                    store
                        .flash
                        .program(self.slot, next, &padded(chunk, self.program_unit))
                        .map_err(InstallError::Flash)?;
                    self.phase = Phase::Program(end);
                    return Ok(false);
                }
                self.phase = Phase::Verify(self.range.start);
                self.verify(store, bytes, self.range.start)
            }
            Phase::Verify(next) => self.verify(store, bytes, next),
        }
    }

    /// Reads back the next chunk from `next` and checks it; true once the last byte is checked.
    fn verify(
        &mut self,
        store: &mut ProgramStore,
        bytes: &[u8],
        next: u32,
    ) -> Result<bool, InstallError> {
        if next >= self.range.end {
            return Ok(true);
        }
        let end = self.range.end.min(next.saturating_add(self.chunk));
        let expected = self.bytes(bytes, next..end)?;
        let read =
            read_padded(store, self.slot, next, expected.len()).map_err(InstallError::Flash)?;
        if read != expected {
            return Err(InstallError::Verify);
        }
        self.phase = Phase::Verify(end);
        Ok(end >= self.range.end)
    }

    /// The part's bytes that go to the slot bytes `range`.
    fn bytes<'b>(&self, bytes: &'b [u8], range: Range<u32>) -> Result<&'b [u8], InstallError> {
        let start = (range.start - self.first) as usize;
        let end = (range.end - self.first) as usize;
        bytes.get(start..end).ok_or(InstallError::Verify)
    }
}

#[cfg(test)]
mod tests;
