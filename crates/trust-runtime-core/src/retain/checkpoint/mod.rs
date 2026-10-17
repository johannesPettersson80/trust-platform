//! The checkpoint log of a chip's retain store (specification 34 §10.1).
//!
//! A chip keeps its retain checkpoints in the two checkpoint slots of its chip file,
//! `checkpoint_a` and `checkpoint_b`. Each slot is a log of records behind a slot header. A record
//! is written header first, payload second and its commit marker last, after a read-back of
//! everything the write programmed, so a record whose write was interrupted never reads as
//! committed. The slot that holds the newest committed record is never erased, and the log
//! programs only flash that reads as erased, so a write or an erase interrupted at any byte leaves
//! the previous checkpoint readable (RETAIN-02).
//!
//! The platform adapter implements [`CheckpointFlash`] for its flash. [`CheckpointLog`] owns the
//! format, the order of a write and the recovery rule. The payload is opaque bytes to the log;
//! the retain codec defines them.
//!
//! Nothing here allocates. A write is a series of steps, each one erase of a unit or one program
//! ([`CheckpointLog::begin`] and [`CheckpointLog::step`]), so a caller can spread it over the time
//! between cycles; [`CheckpointLog::append`] runs all of them at once.

mod format;
mod log;
mod persist;
mod scan;
mod write;

use smol_str::SmolStr;
use thiserror::Error;

use crate::error::RuntimeError;

pub use self::format::{
    COMMIT_MARKER, FORMAT_VERSION, RECORD_HEADER_LEN, RECORD_MAGIC, SLOT_HEADER_LEN, SLOT_MAGIC,
};
pub use self::log::CheckpointLog;
pub use self::persist::{CheckpointPersister, Step};

/// One of the two checkpoint slots.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Slot {
    /// The chip file's `checkpoint_a`.
    A,
    /// The chip file's `checkpoint_b`.
    B,
}

impl Slot {
    /// The other slot.
    #[must_use]
    pub const fn other(self) -> Self {
        match self {
            Self::A => Self::B,
            Self::B => Self::A,
        }
    }

    const fn index(self) -> usize {
        match self {
            Self::A => 0,
            Self::B => 1,
        }
    }
}

/// What a checkpoint flash programs and erases at a time, and the size of each slot.
///
/// The record layout follows from it: the first record starts at the first multiple of the program
/// unit after the 16-byte slot header, and a record's commit field takes the larger of 4 bytes and
/// the program unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlashGeometry {
    program_unit: u32,
    erase_unit: u32,
    slot_len: u32,
}

impl FlashGeometry {
    /// The geometry of a flash that programs `program_unit` bytes and erases `erase_unit` bytes at
    /// a time, with slots of `slot_len` bytes.
    ///
    /// # Errors
    ///
    /// [`CheckpointError::Geometry`] unless the program unit is a power of two of at most 64 bytes,
    /// the erase unit a multiple of the program unit, the slot a multiple of the erase unit, and the
    /// slot holds its header and one record with an empty payload.
    pub const fn new(
        program_unit: u32,
        erase_unit: u32,
        slot_len: u32,
    ) -> Result<Self, CheckpointError> {
        if !program_unit.is_power_of_two()
            || program_unit > format::MAX_PROGRAM_UNIT
            || erase_unit == 0
            || !erase_unit.is_multiple_of(program_unit)
            || !slot_len.is_multiple_of(erase_unit)
        {
            return Err(CheckpointError::Geometry);
        }
        let geometry = Self {
            program_unit,
            erase_unit,
            slot_len,
        };
        let fixed = geometry.first_record_offset() + RECORD_HEADER_LEN + geometry.commit_len();
        if slot_len < fixed {
            return Err(CheckpointError::Geometry);
        }
        Ok(geometry)
    }

    /// The bytes one program writes at a time; every program is whole units at multiples of it.
    #[must_use]
    pub const fn program_unit(self) -> u32 {
        self.program_unit
    }

    /// The bytes one erase clears at a time.
    #[must_use]
    pub const fn erase_unit(self) -> u32 {
        self.erase_unit
    }

    /// The bytes of each slot.
    #[must_use]
    pub const fn slot_len(self) -> u32 {
        self.slot_len
    }

    /// The erase units of one slot.
    #[must_use]
    pub const fn erase_units(self) -> u32 {
        self.slot_len / self.erase_unit
    }

    /// Where a slot's first record starts.
    #[must_use]
    pub const fn first_record_offset(self) -> u32 {
        format::align_up(SLOT_HEADER_LEN, self.program_unit)
    }

    /// The bytes of a record's commit field.
    #[must_use]
    pub const fn commit_len(self) -> u32 {
        if self.program_unit > COMMIT_MARKER.len() as u32 {
            self.program_unit
        } else {
            COMMIT_MARKER.len() as u32
        }
    }

    /// The largest payload a record in an empty slot can take: 16,300 bytes in a 16 KiB slot
    /// with 4-byte program units.
    #[must_use]
    pub const fn max_payload_len(self) -> u32 {
        self.slot_len - self.first_record_offset() - RECORD_HEADER_LEN - self.commit_len()
    }

    /// The bytes a record with a payload of `payload_len` bytes takes, or `None` when the payload
    /// exceeds [`max_payload_len`](Self::max_payload_len).
    #[must_use]
    pub const fn record_len(self, payload_len: u32) -> Option<u32> {
        if payload_len > self.max_payload_len() {
            return None;
        }
        Some(
            RECORD_HEADER_LEN
                + self.commit_len()
                + format::align_up(payload_len, self.program_unit),
        )
    }

    /// Where the payload of the record at `record_offset` starts.
    const fn payload_offset(self, record_offset: u32) -> u32 {
        record_offset + RECORD_HEADER_LEN + self.commit_len()
    }
}

/// The flash of a chip's two checkpoint slots, as its platform adapter drives it
/// (specification 34 §10.1).
///
/// Offsets count from the first byte of a slot. The log reads and programs only whole program
/// units, at multiples of the unit and inside the slot, and erases one erase unit at a time by its
/// first byte. A power loss may stop a program or an erase at any byte; the log is built for that.
/// An operation the flash reports as failed returns an error.
pub trait CheckpointFlash {
    /// The flash's program and erase units and the slot size.
    fn geometry(&self) -> FlashGeometry;

    /// Read `bytes.len()` bytes from `offset` in `slot`.
    ///
    /// # Errors
    ///
    /// [`FlashError::Read`] when the flash cannot be read.
    fn read(&mut self, slot: Slot, offset: u32, bytes: &mut [u8]) -> Result<(), FlashError>;

    /// Program `bytes` at `offset` in `slot`: every bit that is 0 in `bytes` becomes 0. A program
    /// never sets a bit to 1.
    ///
    /// # Errors
    ///
    /// [`FlashError::Program`] when the flash reports the program as failed.
    fn program(&mut self, slot: Slot, offset: u32, bytes: &[u8]) -> Result<(), FlashError>;

    /// Erase the erase unit that starts at `offset` in `slot`: every byte becomes 0xFF.
    ///
    /// # Errors
    ///
    /// [`FlashError::Erase`] when the flash reports the erase as failed.
    fn erase(&mut self, slot: Slot, offset: u32) -> Result<(), FlashError>;
}

impl<T: CheckpointFlash + ?Sized> CheckpointFlash for &mut T {
    fn geometry(&self) -> FlashGeometry {
        (**self).geometry()
    }

    fn read(&mut self, slot: Slot, offset: u32, bytes: &mut [u8]) -> Result<(), FlashError> {
        (**self).read(slot, offset, bytes)
    }

    fn program(&mut self, slot: Slot, offset: u32, bytes: &[u8]) -> Result<(), FlashError> {
        (**self).program(slot, offset, bytes)
    }

    fn erase(&mut self, slot: Slot, offset: u32) -> Result<(), FlashError> {
        (**self).erase(slot, offset)
    }
}

/// A flash operation the platform reported as failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum FlashError {
    /// A read failed.
    #[error("read")]
    Read,
    /// A program failed.
    #[error("program")]
    Program,
    /// An erase failed.
    #[error("erase")]
    Erase,
}

/// What a checkpoint was written for (specification 34 §10.1). The log stores and returns both
/// identities; it does not compare them.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct CheckpointIdentity {
    /// The first 16 bytes of the bytecode hash in the deployment record.
    pub application: [u8; 16],
    /// The first 16 bytes of the retain-schema identity in the deployment record.
    pub schema: [u8; 16],
}

/// A committed record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecordInfo {
    /// The checkpoint sequence: it counts checkpoints and is not an execution generation.
    pub sequence: u64,
    /// The application and retain schema the checkpoint was written for.
    pub identity: CheckpointIdentity,
    /// The payload length in bytes.
    pub payload_len: u32,
}

/// What a write step did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Progress {
    /// The write needs more steps.
    Pending,
    /// The record is committed.
    Committed(RecordInfo),
}

/// How the log is damaged, when the recovery rule refuses it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Corruption {
    /// The newest committed record's header fails its check.
    RecordHeader,
    /// The newest committed record's payload fails its check.
    Payload,
    /// A slot whose header fails its check holds a valid committed record newer than the newest
    /// committed record.
    SlotHeader,
}

impl core::fmt::Display for Corruption {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            Self::RecordHeader => "its header fails its check",
            Self::Payload => "its payload fails its check",
            Self::SlotHeader => "a slot header fails its check",
        })
    }
}

/// A checkpoint log failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum CheckpointError {
    /// The flash geometry cannot hold the log.
    #[error("the flash geometry cannot hold the checkpoint log")]
    Geometry,
    /// The platform reported a flash operation as failed.
    #[error("checkpoint flash {0} failed")]
    Flash(FlashError),
    /// Flash the log must program does not read as erased.
    #[error("checkpoint flash is not erased where the log must program")]
    NotErased,
    /// Programmed flash did not read back as programmed; the record is not committed.
    #[error("checkpoint flash did not read back as programmed")]
    Verify,
    /// The newest committed record cannot be loaded: a warm start fails (specification 34 §10.1).
    #[error("the newest checkpoint is damaged: {0}")]
    Corrupt(Corruption),
    /// The payload does not fit in an empty slot.
    #[error("checkpoint payload of {len} bytes exceeds the {max} bytes a slot takes")]
    TooLarge {
        /// The payload length.
        len: usize,
        /// The largest payload a slot takes.
        max: u32,
    },
    /// The buffer is smaller than the newest record's payload.
    #[error("checkpoint payload needs a buffer of {needed} bytes")]
    BufferTooSmall {
        /// The payload length.
        needed: u32,
    },
    /// A write began and has not finished.
    #[error("a checkpoint write is already in progress")]
    WriteInProgress,
    /// No write is in progress.
    #[error("no checkpoint write is in progress")]
    NoWrite,
    /// The payload given to a step differs from the payload the write began with.
    #[error("the checkpoint payload changed during its write")]
    PayloadChanged,
    /// The checkpoint sequence or the slot epoch cannot count further.
    #[error("the checkpoint sequence or slot epoch is exhausted")]
    Exhausted,
}

impl CheckpointError {
    /// A fixed description, for records that hold no formatted text.
    const fn summary(self) -> &'static str {
        match self {
            Self::Geometry => "checkpoint flash geometry cannot hold the log",
            Self::Flash(FlashError::Read) => "checkpoint flash read failed",
            Self::Flash(FlashError::Program) => "checkpoint flash program failed",
            Self::Flash(FlashError::Erase) => "checkpoint flash erase failed",
            Self::NotErased => "checkpoint flash is not erased where the log must program",
            Self::Verify => "checkpoint flash did not read back as programmed",
            Self::Corrupt(Corruption::RecordHeader) => "newest checkpoint header fails its check",
            Self::Corrupt(Corruption::Payload) => "newest checkpoint payload fails its check",
            Self::Corrupt(Corruption::SlotHeader) => {
                "checkpoint slot header fails its check and the slot holds newer records"
            }
            Self::TooLarge { .. } => "checkpoint payload exceeds the slot",
            Self::BufferTooSmall { .. } => "checkpoint payload exceeds the buffer",
            Self::WriteInProgress => "a checkpoint write is already in progress",
            Self::NoWrite => "no checkpoint write is in progress",
            Self::PayloadChanged => "checkpoint payload changed during its write",
            Self::Exhausted => "checkpoint sequence or slot epoch is exhausted",
        }
    }
}

/// A damaged log is retain corruption, which fails a warm start as a damaged retain file does on a
/// computer (specification 11 §6.7); every other failure is a retain store error.
impl From<CheckpointError> for RuntimeError {
    fn from(error: CheckpointError) -> Self {
        let text = SmolStr::new_static(error.summary());
        match error {
            CheckpointError::Corrupt(_) => Self::RetainCorruption(text),
            _ => Self::RetainStore(text),
        }
    }
}
