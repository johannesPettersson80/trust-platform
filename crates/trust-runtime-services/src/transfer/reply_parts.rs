//! A reply held in parts (specification 34, PORT-05): the blocks of 4,096 bytes that a chip's heap can
//! give, in the order the reply was written, with the digest and the ranges that `transfer.get` reads
//! from them. A chip's heap is first-fit and its free space lies in many small blocks, so the reply
//! is never joined into one block, and each part is taken from the heap when the writing reaches it.

use alloc::borrow::Cow;
use alloc::collections::TryReserveError;
use alloc::vec::Vec;
use core::fmt;

use trust_runtime_core::deployment::sha256_parts;

use super::session::PART_BYTES;

/// How a part is taken from the heap: the part that follows `taken` parts, as a buffer with room for
/// [`PART_BYTES`] bytes, or the refusal of a heap that has no room for it. The session takes this
/// function, so that a test can make the heap refuse after some number of parts.
pub(crate) type PartReserve = fn(taken: usize) -> Result<Vec<u8>, TryReserveError>;

/// Takes a part of [`PART_BYTES`] bytes with `try_reserve_exact`: a heap that has no room for it is an
/// error that the caller refuses, never an abort (specification 34, PORT-05). It is the session's
/// [`PartReserve`] unless a test gives another.
pub(crate) fn reserve_part(_taken: usize) -> Result<Vec<u8>, TryReserveError> {
    let mut part = Vec::new();
    part.try_reserve_exact(PART_BYTES)?;
    Ok(part)
}

/// A heap that has room for `PARTS` parts of a reply and then none: the part after `PARTS` parts is
/// refused, as `try_reserve_exact` refuses a block the heap cannot hold. A test gives it to a writer
/// or a session as its [`PartReserve`], to see that a reply the heap has no room for is refused
/// rather than aborted.
#[cfg(test)]
pub(crate) fn room_for<const PARTS: usize>(taken: usize) -> Result<Vec<u8>, TryReserveError> {
    if taken < PARTS {
        return reserve_part(taken);
    }
    Err(Vec::<u8>::new()
        .try_reserve_exact(usize::MAX)
        .expect_err("usize::MAX bytes never fit"))
}

/// A reply in parts of [`PART_BYTES`] bytes: every part holds all of them but the last, which holds
/// one byte up to all of them. A part may end inside a UTF-8 character of a text reply, because the
/// parts are bytes; read in order they are the reply.
#[derive(Default)]
pub struct ReplyParts {
    parts: Vec<Vec<u8>>,
    len: usize,
}

impl fmt::Debug for ReplyParts {
    /// The length and the number of parts only, not the reply, which can be 114,688 bytes long.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ReplyParts")
            .field("len", &self.len)
            .field("parts", &self.parts.len())
            .finish()
    }
}

impl ReplyParts {
    /// A reply of no bytes, which holds no memory.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            parts: Vec::new(),
            len: 0,
        }
    }

    /// The length of the reply, in bytes.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Whether the reply has no byte yet.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Appends `bytes` to the reply. The current part is filled before the next part is taken from the
    /// heap, so the reply holds no memory ahead of its bytes but the rest of its last part.
    ///
    /// # Errors
    ///
    /// The refusal of a heap that has no room for the next part. The bytes written before it stay, and
    /// the length is then a whole number of parts.
    pub fn try_write(&mut self, bytes: &[u8]) -> Result<(), TryReserveError> {
        self.try_write_with(bytes, reserve_part)
    }

    /// As [`Self::try_write`], with each new part taken by `reserve`.
    pub(crate) fn try_write_with(
        &mut self,
        mut bytes: &[u8],
        reserve: PartReserve,
    ) -> Result<(), TryReserveError> {
        while !bytes.is_empty() {
            if self
                .parts
                .last()
                .is_none_or(|part| part.len() == PART_BYTES)
            {
                self.take_part(reserve)?;
            }
            let last = self.parts.len() - 1;
            let part = &mut self.parts[last];
            let (now, rest) = bytes.split_at((PART_BYTES - part.len()).min(bytes.len()));
            part.extend_from_slice(now);
            self.len += now.len();
            bytes = rest;
        }
        Ok(())
    }

    /// Takes the next part from the heap, empty, with room for [`PART_BYTES`] bytes.
    fn take_part(&mut self, reserve: PartReserve) -> Result<(), TryReserveError> {
        self.parts.try_reserve(1)?;
        let mut part = reserve(self.parts.len())?;
        part.clear();
        // A buffer with less room than a part would grow, and so allocate without a refusal, as the
        // part is written. This asks for the room it lacks, where a refusal is an error, and asks
        // for nothing of a buffer that has the room.
        part.try_reserve_exact(PART_BYTES)?;
        self.parts.push(part);
        Ok(())
    }

    /// The SHA-256 of the whole reply, worked out part by part.
    pub(crate) fn sha256(&self) -> [u8; 32] {
        sha256_parts(self.parts.iter().map(Vec::as_slice))
    }

    /// The bytes from `start` up to `end`, where `start <= end <= self.len()` and `end - start` is at
    /// most [`PART_BYTES`]. Borrowed from the part that holds them, or, for a range across two parts,
    /// copied into one block of the range's length, which is no longer than a part.
    pub(crate) fn range(&self, start: usize, end: usize) -> Cow<'_, [u8]> {
        let first = start / PART_BYTES;
        let base = first * PART_BYTES;
        // Where the first part's share of the range ends.
        let split = (base + PART_BYTES).min(end);
        let head = self
            .parts
            .get(first)
            .map_or(&[][..], |part| &part[start - base..split - base]);
        if split == end {
            return Cow::Borrowed(head);
        }
        let tail = self
            .parts
            .get(first + 1)
            .map_or(&[][..], |part| &part[..end - split]);
        let mut joined = Vec::with_capacity(end - start);
        joined.extend_from_slice(head);
        joined.extend_from_slice(tail);
        Cow::Owned(joined)
    }

    /// The whole reply as one block, for a reply that fits a frame. A reply of one part is that part
    /// itself, not copied. Each part is freed as it is copied.
    ///
    /// # Errors
    ///
    /// The refusal of a heap that has no room for the joined block.
    pub(crate) fn into_bytes(mut self) -> Result<Vec<u8>, TryReserveError> {
        if self.parts.len() <= 1 {
            return Ok(self.parts.pop().unwrap_or_default());
        }
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(self.len)?;
        for part in self.parts {
            bytes.extend_from_slice(&part);
        }
        Ok(bytes)
    }

    /// The length of each part, for a test that checks how a reply was cut.
    #[cfg(test)]
    pub(crate) fn part_lengths(&self) -> Vec<usize> {
        self.parts.iter().map(Vec::len).collect()
    }
}
