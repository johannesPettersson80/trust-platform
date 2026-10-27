//! The copy plan of an online change (specification 44 §12.2): the bytes the new program takes from
//! the running one, as runs copied from area to area and as the bytes of the values a change
//! converts or names anew.
//!
//! A change works the plan out while it reads the new program's image, which the new program's
//! memory lends, and applies it once it does not, so it works a batch of whole variables out at a
//! time and applies the batch before the next. Adjacent runs and adjacent bytes join, so a batch of
//! unchanged variables is a few runs.

use alloc::vec::Vec;

use super::reserve;
use crate::error::RuntimeError;
use crate::native::memory::ProgramMemory;
use crate::native::values::damaged;

/// The bytes a copy moves at a time.
const PIECE: u32 = 256;

/// Where a value lies: an area's id and the offset of its first byte in the area.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Spot {
    /// The area's id (specification 44 §7.1).
    pub(super) area: u8,
    /// The offset in the area.
    pub(super) offset: u32,
}

impl Spot {
    /// The spot `bytes` further on.
    pub(super) fn at(self, bytes: u32) -> Result<Self, RuntimeError> {
        let offset = self
            .offset
            .checked_add(bytes)
            .ok_or_else(|| damaged("symbol table"))?;
        Ok(Self { offset, ..self })
    }
}

/// One step of a plan.
#[derive(Clone, Copy)]
enum Step {
    /// Copies `length` bytes of the running program at `from` to the new program at `to`.
    Copy { from: Spot, to: Spot, length: u32 },
    /// Writes the plan's bytes from `start`, `length` of them, to the new program at `to`.
    Write {
        to: Spot,
        start: usize,
        length: usize,
    },
}

/// A batch of the copy plan.
pub(super) struct Plan {
    steps: Vec<Step>,
    bytes: Vec<u8>,
}

impl Plan {
    /// The steps of a full batch.
    const STEPS: usize = 64;
    /// The bytes of a full batch.
    const BYTES: usize = 1024;

    /// An empty plan with the room of a batch.
    pub(super) fn new() -> Result<Self, RuntimeError> {
        let mut plan = Self {
            steps: Vec::new(),
            bytes: Vec::new(),
        };
        reserve(&mut plan.steps, Self::STEPS)?;
        reserve(&mut plan.bytes, Self::BYTES)?;
        Ok(plan)
    }

    /// Whether the batch is full. A batch takes whole variables, so a large variable makes a larger
    /// batch.
    pub(super) fn full(&self) -> bool {
        self.steps.len() >= Self::STEPS || self.bytes.len() >= Self::BYTES
    }

    /// Copies `length` bytes of the running program at `from` to the new program at `to`.
    pub(super) fn copy(&mut self, from: Spot, to: Spot, length: u32) -> Result<(), RuntimeError> {
        if length == 0 {
            return Ok(());
        }
        if let Some(Step::Copy {
            from: last_from,
            to: last_to,
            length: last,
        }) = self.steps.last_mut()
            && last_from.area == from.area
            && last_to.area == to.area
            && last_from.offset.checked_add(*last) == Some(from.offset)
            && last_to.offset.checked_add(*last) == Some(to.offset)
            && let Some(joined) = last.checked_add(length)
        {
            *last = joined;
            return Ok(());
        }
        reserve(&mut self.steps, 1)?;
        self.steps.push(Step::Copy { from, to, length });
        Ok(())
    }

    /// Writes `data` to the new program at `to`.
    pub(super) fn write(&mut self, to: Spot, data: &[u8]) -> Result<(), RuntimeError> {
        self.write_with(to, data.len(), |out| {
            out.extend_from_slice(data);
            Ok(true)
        })
        .map(|_| ())
    }

    /// Writes to the new program at `to` the bytes `fill` appends, at most `room` of them, when it
    /// returns `true`; nothing when it returns `false`. Returns what `fill` returned.
    pub(super) fn write_with(
        &mut self,
        to: Spot,
        room: usize,
        fill: impl FnOnce(&mut Vec<u8>) -> Result<bool, RuntimeError>,
    ) -> Result<bool, RuntimeError> {
        reserve(&mut self.bytes, room)?;
        let start = self.bytes.len();
        if !fill(&mut self.bytes)? {
            self.bytes.truncate(start);
            return Ok(false);
        }
        let length = self.bytes.len() - start;
        if let Some(Step::Write {
            to: last_to,
            start: last_start,
            length: last,
        }) = self.steps.last_mut()
            && last_to.area == to.area
            && last_start.checked_add(*last) == Some(start)
            && u32::try_from(*last)
                .ok()
                .and_then(|bytes| last_to.offset.checked_add(bytes))
                == Some(to.offset)
        {
            *last += length;
            return Ok(true);
        }
        reserve(&mut self.steps, 1)?;
        self.steps.push(Step::Write { to, start, length });
        Ok(true)
    }

    /// Applies the batch: copies the running program's bytes from `old` and writes the plan's own
    /// into `new`, then empties the plan for the next batch.
    pub(super) fn apply(
        &mut self,
        old: &dyn ProgramMemory,
        new: &mut dyn ProgramMemory,
    ) -> Result<(), RuntimeError> {
        for step in &self.steps {
            match *step {
                Step::Copy { from, to, length } => {
                    copy(old, from, new, to, length, "symbol table")?
                }
                Step::Write { to, start, length } => {
                    let bytes = start
                        .checked_add(length)
                        .and_then(|end| self.bytes.get(start..end))
                        .ok_or(RuntimeError::InvalidExecutionState)?;
                    if !new.write_area(to.area, to.offset as usize, bytes) {
                        return Err(damaged("symbol table"));
                    }
                }
            }
        }
        self.steps.clear();
        self.bytes.clear();
        Ok(())
    }
}

/// Copies the running program's bytes at `at` into `out`.
pub(super) fn fetch(
    memory: &dyn ProgramMemory,
    at: Spot,
    out: &mut [u8],
) -> Result<(), RuntimeError> {
    if memory.read_area(at.area, at.offset as usize, out) {
        Ok(())
    } else {
        Err(damaged("symbol table"))
    }
}

/// Copies `length` bytes of `old` at `from` to `new` at `to`, a piece at a time; an access outside
/// an area is a damaged `what`.
pub(super) fn copy(
    old: &dyn ProgramMemory,
    from: Spot,
    new: &mut dyn ProgramMemory,
    to: Spot,
    length: u32,
    what: &str,
) -> Result<(), RuntimeError> {
    let mut piece = [0u8; PIECE as usize];
    let mut done = 0;
    while done < length {
        let size = (length - done).min(PIECE);
        let bytes = &mut piece[..size as usize];
        let (Some(source), Some(target)) =
            (from.offset.checked_add(done), to.offset.checked_add(done))
        else {
            return Err(damaged(what));
        };
        if !old.read_area(from.area, source as usize, bytes)
            || !new.write_area(to.area, target as usize, bytes)
        {
            return Err(damaged(what));
        }
        done += size;
    }
    Ok(())
}
