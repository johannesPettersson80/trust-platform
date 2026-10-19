//! The C6's boot counter, kept in its flash (specification 34 PORT-05 and Appendix D.2).
//!
//! The boot number of a transfer id is a count that rises by one at every boot, power-on included, so
//! that an id never repeats across boots. Memory that a reset keeps is not enough: a loss of power
//! clears it, and the count would start again at 1. The count is therefore kept in flash, in two 4 KiB
//! sectors in the gap before the `factory` application that `partitions.csv` leaves free.
//!
//! A sector is a log of 512 records of 8 bytes: the count `n` and its complement `!n`, the integrity
//! pair that the reset cookie uses. A record is written once, into erased flash, so a write that power
//! ends at any byte leaves a record whose two words disagree, which reads as no record, or a whole
//! record that no boot counted, and the previous count stays readable. A boot reads both sectors,
//! takes the highest count of a whole record, writes `n + 1` after the last record of that sector and
//! reads it back before it counts the boot, so a count is never handed out twice, though one may be
//! skipped. When the sector is full the boot erases the other sector, which holds only older counts,
//! and writes the record at its start, so the newest count is in flash at every instant: a power loss
//! between the erase and the write leaves the full sector, with the highest count, as it was. One
//! erase is spent for 512 boots.

use core::fmt;

use trust_runtime_core::retain::checkpoint::FlashError;

use crate::checkpoint::RawFlash;

/// The flash offset of the first counter sector.
pub const SECTOR_A_OFFSET: u32 = 0x9000;
/// The flash offset of the second counter sector.
pub const SECTOR_B_OFFSET: u32 = 0xA000;
/// The bytes of each counter sector: one 4 KiB erase unit.
pub const SECTOR_BYTES: u32 = 4 * 1024;

/// The bytes of one record: the count and its complement, 4 bytes each, little-endian.
const RECORD_BYTES: usize = 8;
/// The bytes one read covers: 32 records.
const PAGE_BYTES: usize = 256;

/// Why a boot cannot be counted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BootCounterError {
    /// A read, a program or an erase of the counter's flash failed.
    Flash(FlashError),
    /// The record that was programmed does not read back as written.
    Verify,
    /// The count is the largest a `u32` holds, so counting on would repeat an id.
    Exhausted,
}

impl fmt::Display for BootCounterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Flash(error) => write!(formatter, "the boot counter's flash {error} failed"),
            Self::Verify => formatter.write_str("the boot counter does not read back as written"),
            Self::Exhausted => formatter.write_str("the boot counter cannot count higher"),
        }
    }
}

impl core::error::Error for BootCounterError {}

impl From<FlashError> for BootCounterError {
    fn from(error: FlashError) -> Self {
        Self::Flash(error)
    }
}

/// What one sector holds: its highest whole record, and the first erased record.
struct Scan {
    /// The highest count of a record whose two words agree.
    highest: Option<u32>,
    /// The index of the first record that is erased, or `None` when the sector is full.
    free: Option<usize>,
}

/// The boot counter over the chip's flash at absolute addresses.
pub struct BootCounter<R> {
    flash: R,
}

impl<R: RawFlash> BootCounter<R> {
    /// The boot counter over `flash`.
    pub fn new(flash: R) -> Self {
        Self { flash }
    }

    /// Counts this boot: reads the highest count in the two sectors, writes the next one after it and
    /// reads it back, and returns it. The first boot of a flash that never counted is 1.
    ///
    /// # Errors
    ///
    /// [`BootCounterError`] when the flash fails, when the record does not read back, or when the count
    /// cannot rise. The count that was in flash stays the highest.
    pub fn count_boot(&mut self) -> Result<u32, BootCounterError> {
        let first = self.scan(SECTOR_A_OFFSET)?;
        let second = self.scan(SECTOR_B_OFFSET)?;
        // The sector with the highest count is the current one; the first sector when they tie.
        let second_is_current = second
            .highest
            .is_some_and(|highest| first.highest.is_none_or(|other| highest > other));
        let (current, current_scan, other) = if second_is_current {
            (SECTOR_B_OFFSET, second, SECTOR_A_OFFSET)
        } else {
            (SECTOR_A_OFFSET, first, SECTOR_B_OFFSET)
        };
        let next = current_scan
            .highest
            .unwrap_or(0)
            .checked_add(1)
            .ok_or(BootCounterError::Exhausted)?;
        let (sector, index) = match current_scan.free {
            Some(index) => (current, index),
            None => {
                // The current sector is full: the other one holds only older counts.
                self.flash.erase_at(other)?;
                (other, 0)
            }
        };
        self.write(sector, index, next)?;
        Ok(next)
    }

    /// Reads `sector` and finds its highest whole record and its first erased record.
    fn scan(&mut self, sector: u32) -> Result<Scan, BootCounterError> {
        let mut scan = Scan {
            highest: None,
            free: None,
        };
        let mut page = [0_u8; PAGE_BYTES];
        let records_per_page = PAGE_BYTES / RECORD_BYTES;
        for page_index in 0..(SECTOR_BYTES as usize / PAGE_BYTES) {
            let offset = u32::try_from(page_index * PAGE_BYTES).map_err(|_| FlashError::Read)?;
            self.flash.read_at(sector + offset, &mut page)?;
            for (position, record) in page.as_chunks::<RECORD_BYTES>().0.iter().enumerate() {
                match Record::of(record) {
                    Record::Erased => {
                        if scan.free.is_none() {
                            scan.free = Some(page_index * records_per_page + position);
                        }
                    }
                    Record::Whole(count) => {
                        scan.highest =
                            Some(scan.highest.map_or(count, |highest| highest.max(count)));
                    }
                    Record::Torn => {}
                }
            }
        }
        Ok(scan)
    }

    /// Programs the record of `count` at record `index` of `sector` and reads it back.
    fn write(&mut self, sector: u32, index: usize, count: u32) -> Result<(), BootCounterError> {
        let record = Record::bytes(count);
        let offset = u32::try_from(index * RECORD_BYTES).map_err(|_| FlashError::Program)?;
        let address = sector + offset;
        self.flash.program_at(address, &record)?;
        let mut back = [0_u8; RECORD_BYTES];
        self.flash.read_at(address, &mut back)?;
        if back == record {
            Ok(())
        } else {
            Err(BootCounterError::Verify)
        }
    }
}

/// One record of a sector, as its 8 bytes read.
enum Record {
    /// All bytes erased: the record was never written.
    Erased,
    /// The count and its complement agree.
    Whole(u32),
    /// Written part-way or damaged: the two words disagree.
    Torn,
}

impl Record {
    /// The 8 bytes of the record of `count`: the count and its complement.
    fn bytes(count: u32) -> [u8; RECORD_BYTES] {
        let mut bytes = [0_u8; RECORD_BYTES];
        bytes[..4].copy_from_slice(&count.to_le_bytes());
        bytes[4..].copy_from_slice(&(!count).to_le_bytes());
        bytes
    }

    /// What the 8 bytes `record` read as.
    fn of(record: &[u8]) -> Self {
        let word = |at: usize| {
            u32::from_le_bytes([record[at], record[at + 1], record[at + 2], record[at + 3]])
        };
        let (count, complement) = (word(0), word(4));
        if count == u32::MAX && complement == u32::MAX {
            Self::Erased
        } else if complement == !count {
            Self::Whole(count)
        } else {
            Self::Torn
        }
    }
}
