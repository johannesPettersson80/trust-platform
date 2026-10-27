//! A text that a device keeps in two flash sectors, one record in each, the newer by its sequence, so
//! that a write that power ends leaves the record before it whole (specification 34 TARGET-02): a
//! chip's device settings, and its pairings (specification 11 §6.9.6). A record is a magic of eight
//! bytes that names what it holds, the sequence, the text's length and CRC-32, and the text.

use alloc::boxed::Box;
use alloc::vec;
use alloc::vec::Vec;

use trust_runtime_core::retain::checkpoint::FlashError;

/// The bytes before a record's text: the magic, the sequence, the length and the CRC-32.
pub const HEADER_BYTES: usize = 20;

/// The record that holds `text` as sequence `sequence` under `magic`; `None` when it does not fit
/// `capacity` bytes, a sector.
#[must_use]
pub fn encode(magic: &[u8; 8], sequence: u32, text: &[u8], capacity: usize) -> Option<Vec<u8>> {
    let length = u32::try_from(text.len()).ok()?;
    if HEADER_BYTES + text.len() > capacity {
        return None;
    }
    let mut record = Vec::with_capacity(HEADER_BYTES + text.len());
    record.extend_from_slice(magic);
    record.extend_from_slice(&sequence.to_le_bytes());
    record.extend_from_slice(&length.to_le_bytes());
    record.extend_from_slice(&trust_runtime_core::crc32::checksum(text).to_le_bytes());
    record.extend_from_slice(text);
    Some(record)
}

/// The sequence and the text of the record under `magic` at the start of `sector`; `None` for an
/// erased sector, a record of another magic, and one that a write did not finish (its CRC-32 is
/// wrong).
#[must_use]
pub fn decode<'a>(magic: &[u8; 8], sector: &'a [u8]) -> Option<(u32, &'a [u8])> {
    let header = sector.get(..HEADER_BYTES)?;
    if header[..8] != *magic {
        return None;
    }
    let word = |at: usize| {
        u32::from_le_bytes([header[at], header[at + 1], header[at + 2], header[at + 3]])
    };
    let length = usize::try_from(word(12)).ok()?;
    let text = sector.get(HEADER_BYTES..HEADER_BYTES.checked_add(length)?)?;
    (trust_runtime_core::crc32::checksum(text) == word(16)).then_some((word(8), text))
}

/// The newer of the two sectors' records under `magic`, by its index (0 or 1), with its sequence and
/// its text; `None` when neither holds one.
#[must_use]
pub fn newest<'a>(magic: &[u8; 8], sectors: [&'a [u8]; 2]) -> Option<(usize, u32, &'a [u8])> {
    let read = |index: usize| {
        decode(magic, sectors[index]).map(|(sequence, text)| (index, sequence, text))
    };
    match (read(0), read(1)) {
        (Some(first), Some(second)) => Some(if second.1 > first.1 { second } else { first }),
        (first, second) => first.or(second),
    }
}

/// The flash of a device's two sectors of one record (`device_a` and `device_b`, or `pairing_a` and
/// `pairing_b`, of its chip file).
pub trait SectorFlash {
    /// The bytes of each sector, one erase unit.
    fn sector_bytes(&self) -> usize;

    /// Reads `bytes.len()` bytes from the start of `sector` (0 or 1).
    ///
    /// # Errors
    ///
    /// The flash's failure.
    fn read(&mut self, sector: usize, bytes: &mut [u8]) -> Result<(), FlashError>;

    /// Erases `sector`: every byte becomes 0xFF.
    ///
    /// # Errors
    ///
    /// The flash's failure.
    fn erase(&mut self, sector: usize) -> Result<(), FlashError>;

    /// Programs `bytes`, whole 4-byte units, from the start of `sector`.
    ///
    /// # Errors
    ///
    /// The flash's failure.
    fn program(&mut self, sector: usize, bytes: &[u8]) -> Result<(), FlashError>;
}

/// Why a record was not stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoreError {
    /// The text is longer than a sector holds ([`RecordStore::capacity`]).
    TooLong,
    /// The flash's erase failed.
    Erase,
    /// The flash's program failed.
    Program,
    /// The flash's read-back failed.
    Read,
    /// The programmed bytes do not read back as written.
    ReadBack,
}

/// A record kept in two sectors: where the newest is, and its sequence.
pub struct RecordStore {
    flash: Box<dyn SectorFlash>,
    magic: [u8; 8],
    /// The sector and the sequence of the newest record; `None` while neither sector holds one.
    newest: Option<(usize, u32)>,
}

impl RecordStore {
    /// The store of the records under `magic` in `flash`, and the newest record's sequence and text;
    /// a sector that cannot be read holds none.
    #[must_use]
    pub fn open(mut flash: Box<dyn SectorFlash>, magic: [u8; 8]) -> (Self, Option<(u32, Vec<u8>)>) {
        let size = flash.sector_bytes();
        let mut first = vec![0u8; size];
        let mut second = vec![0u8; size];
        if flash.read(0, &mut first).is_err() {
            first.fill(0xFF);
        }
        if flash.read(1, &mut second).is_err() {
            second.fill(0xFF);
        }
        let found = newest(&magic, [&first, &second]);
        let store = Self {
            flash,
            magic,
            newest: found.map(|(index, sequence, _)| (index, sequence)),
        };
        (
            store,
            found.map(|(_, sequence, text)| (sequence, text.to_vec())),
        )
    }

    /// The text a record of this store holds at most: a sector less the header.
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.flash.sector_bytes().saturating_sub(HEADER_BYTES)
    }

    /// Writes `text` into the sector that does not hold the newest record, with the next sequence,
    /// and reads it back; the record before it stays whole until this one is, so a write that power
    /// ends leaves the record before it. Returns the sequence.
    ///
    /// # Errors
    ///
    /// The [`StoreError`]: a text longer than [`Self::capacity`], a flash failure, or a read-back
    /// that differs.
    pub fn store(&mut self, text: &[u8]) -> Result<u32, StoreError> {
        let size = self.flash.sector_bytes();
        let (sector, sequence) = self.newest.map_or((0, 1), |(index, sequence)| {
            (1 - index, sequence.wrapping_add(1))
        });
        let mut record = encode(&self.magic, sequence, text, size).ok_or(StoreError::TooLong)?;
        record.resize(record.len().next_multiple_of(4).min(size), 0xFF);
        self.flash.erase(sector).map_err(|_| StoreError::Erase)?;
        self.flash
            .program(sector, &record)
            .map_err(|_| StoreError::Program)?;
        let mut read = vec![0u8; record.len()];
        self.flash
            .read(sector, &mut read)
            .map_err(|_| StoreError::Read)?;
        if read != record {
            return Err(StoreError::ReadBack);
        }
        self.newest = Some((sector, sequence));
        Ok(sequence)
    }
}

#[cfg(test)]
mod tests;
